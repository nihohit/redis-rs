//! A minimal framing layer over an async stream that decodes RESP [`Value`]s on the read side
//! and writes pre-encoded command buffers on the write side.
//!
//! It replaces `tokio_util::codec::Framed<C, ValueCodec>`. The read side keeps the same
//! zero-copy decoding (it reads directly into a [`BytesMut`] and hands frozen slices to the
//! parser, see [`crate::parser`]).
//!
//! The write side is *adaptive*. Coalescing many small buffers into one contiguous write is a win
//! (one cheap copy, one syscall) and writing many tiny buffers via vectored I/O is actually slower
//! than that. But copying a *large* already-encoded buffer just to write it is pure waste. So:
//!
//! - Small buffers (below [`COALESCE_THRESHOLD`]) are copied into a running coalesce buffer, exactly
//!   like the previous codec did — no regression for the common small-command case.
//! - Large buffers are queued by reference (a cheap [`Bytes`] clone) and never copied.
//!
//! At flush time the coalesced run and any large buffers are written in submission order with a
//! single vectored write ([`AsyncWrite::poll_write_vectored`]) when the transport supports it, so a
//! large value goes straight from its `Bytes` to the socket. Transports without vectored support
//! (e.g. TLS) write each segment in turn, which still avoids copying large payloads.

use std::collections::VecDeque;
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_util::{Sink, Stream, ready};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_util::codec::Decoder;

use crate::errors::RedisError;
use crate::parser::ValueCodec;
use crate::types::Value;

// How much spare capacity to ensure before each socket read.
const READ_CHUNK: usize = 16 * 1024;
// Reserve more capacity once the spare room drops below this.
const READ_LOW_WATER: usize = 1024;
// Maximum number of buffers handed to a single vectored write. Kept well under every platform's
// `IOV_MAX` (typically 1024) while still allowing plenty of batching.
const MAX_IOV: usize = 128;
// Buffers at least this large are written by reference instead of being copied into the coalesce
// buffer. Below it, coalescing (one copy + one syscall) beats issuing a separate write.
const COALESCE_THRESHOLD: usize = 8 * 1024;

/// Framing layer over an async stream `C`.
///
/// Implements [`Stream<Item = RedisResult<Value>>`](Stream) for reading decoded responses and
/// [`Sink<Bytes>`](Sink) for writing already-encoded command buffers.
pub(crate) struct RedisFramed<C> {
    io: C,
    codec: ValueCodec,
    // Bytes read from the socket but not yet decoded into a complete value.
    read_buf: BytesMut,
    // Whether `read_buf` may contain a decodable frame. Cleared when a decode comes up short so we
    // don't re-run the parser over the same incomplete buffer until more bytes arrive.
    is_readable: bool,
    // Set once the peer closes the read half.
    eof: bool,
    // Set once the read side has yielded a terminal event (clean end or error); the stream is
    // fused afterwards and only ever yields `None`.
    terminated: bool,
    // Large buffers queued by reference (never copied), plus any small run that had to be frozen
    // because a large buffer arrived after it. Written before `coalesce`. Partial writes advance
    // the front buffer in place via `Buf::advance`.
    write_queue: VecDeque<Bytes>,
    // The trailing run of small buffers, copied together into one reused buffer (exactly like the
    // previous codec). Logically sits *after* everything in `write_queue`. Kept un-frozen so the
    // allocation is reused batch-to-batch; only frozen into `write_queue` if a large buffer arrives
    // after it, to preserve submission order.
    coalesce: BytesMut,
    // Whether the underlying IO meaningfully supports vectored writes, probed once.
    vectored: bool,
}

impl<C> RedisFramed<C>
where
    C: AsyncWrite,
{
    pub(crate) fn new(io: C) -> Self {
        let vectored = io.is_write_vectored();
        RedisFramed {
            io,
            codec: ValueCodec,
            read_buf: BytesMut::new(),
            is_readable: false,
            eof: false,
            terminated: false,
            write_queue: VecDeque::new(),
            coalesce: BytesMut::new(),
            vectored,
        }
    }
}

impl<C> Stream for RedisFramed<C>
where
    C: AsyncRead + Unpin,
{
    type Item = Result<Value, RedisError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.terminated {
            return Poll::Ready(None);
        }
        loop {
            // Only attempt to decode when new bytes may have completed a frame. This drains
            // pipelined responses already in the buffer without touching the socket, but avoids
            // re-running the parser over the same incomplete buffer across polls.
            if this.is_readable {
                let decoded = if this.eof {
                    this.codec.decode_eof(&mut this.read_buf)
                } else {
                    this.codec.decode(&mut this.read_buf)
                };
                match decoded {
                    Ok(Some(value)) => return Poll::Ready(Some(Ok(value))),
                    Ok(None) => {
                        if this.eof {
                            // No more complete frames are coming.
                            if this.read_buf.is_empty() {
                                this.terminated = true;
                                return Poll::Ready(None);
                            }
                            // Trailing bytes that don't form a value: surface a framing error once,
                            // then report end-of-stream on subsequent polls.
                            this.terminated = true;
                            return Poll::Ready(Some(Err(io::Error::from(
                                io::ErrorKind::UnexpectedEof,
                            )
                            .into())));
                        }
                        // Need more bytes before another decode is worthwhile.
                        this.is_readable = false;
                    }
                    Err(err) => {
                        // A decode/parse error means the stream is no longer usable.
                        this.terminated = true;
                        return Poll::Ready(Some(Err(err)));
                    }
                }
            }

            // Ensure there's room to read into, then read directly into `read_buf` so the parser
            // can hand out zero-copy slices into it.
            if this.read_buf.capacity() - this.read_buf.len() < READ_LOW_WATER {
                this.read_buf.reserve(READ_CHUNK);
            }
            let n = {
                let dst = this.read_buf.chunk_mut();
                // SAFETY: `ReadBuf::uninit` only ever writes initialized bytes into the slice, and
                // we commit exactly the number it reports as filled via `advance_mut` below.
                let dst = unsafe { dst.as_uninit_slice_mut() };
                let mut buf = ReadBuf::uninit(dst);
                match Pin::new(&mut this.io).poll_read(cx, &mut buf) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(err)) => return Poll::Ready(Some(Err(err.into()))),
                    Poll::Ready(Ok(())) => buf.filled().len(),
                }
            };
            // SAFETY: `poll_read` initialized `n` bytes at the start of the spare capacity.
            unsafe { this.read_buf.advance_mut(n) };
            if n == 0 {
                this.eof = true;
            }
            // New bytes (or EOF) arrived, so a decode attempt is now worthwhile.
            this.is_readable = true;
        }
    }
}

impl<C> Sink<Bytes> for RedisFramed<C>
where
    C: AsyncWrite + Unpin,
{
    type Error = RedisError;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Queuing a buffer is allocation-free and never blocks; upstream backpressure is provided
        // by the bounded request channel that feeds this sink.
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Bytes) -> Result<(), Self::Error> {
        let this = self.get_mut();
        if item.is_empty() {
            return Ok(());
        }
        if item.len() >= COALESCE_THRESHOLD {
            // Large buffer: freeze the pending small run so order is preserved, then queue the
            // large buffer by reference (no copy).
            if !this.coalesce.is_empty() {
                this.write_queue.push_back(this.coalesce.split().freeze());
            }
            this.write_queue.push_back(item);
        } else {
            // Small buffer: copy into the reused coalesce buffer, exactly like the previous codec.
            this.coalesce.extend_from_slice(&item);
        }
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();

        // Phase 1: drain the queue of large / frozen buffers (written before the trailing small
        // run). A single vectored write covers many of them when the transport supports it.
        while !this.write_queue.is_empty() {
            let n = if this.vectored {
                let count = this.write_queue.len().min(MAX_IOV);
                let mut slices = [IoSlice::new(&[]); MAX_IOV];
                for (i, chunk) in this.write_queue.iter().take(count).enumerate() {
                    slices[i] = IoSlice::new(&chunk[..]);
                }
                match Pin::new(&mut this.io).poll_write_vectored(cx, &slices[..count]) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(n)) => n,
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                }
            } else {
                match Pin::new(&mut this.io).poll_write(cx, &this.write_queue.front().unwrap()[..])
                {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(n)) => n,
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
                }
            };
            if n == 0 {
                return Poll::Ready(Err(io::Error::from(io::ErrorKind::WriteZero).into()));
            }
            advance_queue(&mut this.write_queue, n);
        }

        // Phase 2: drain the coalesced small run from its reused buffer. `advance` keeps the
        // allocation around (it is never shared), so it is reclaimed for the next batch.
        while !this.coalesce.is_empty() {
            match Pin::new(&mut this.io).poll_write(cx, &this.coalesce[..]) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::from(io::ErrorKind::WriteZero).into()));
                }
                Poll::Ready(Ok(n)) => this.coalesce.advance(n),
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err.into())),
            }
        }

        Pin::new(&mut this.io).poll_flush(cx).map_err(Into::into)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.as_mut().poll_flush(cx))?;
        let this = self.get_mut();
        Pin::new(&mut this.io).poll_shutdown(cx).map_err(Into::into)
    }
}

/// Consumes `written` bytes from the front of the queue, advancing a partially-written front buffer
/// in place (a cheap pointer bump on `Bytes`) and popping buffers that are fully written.
fn advance_queue(queue: &mut VecDeque<Bytes>, written: usize) {
    let mut remaining = written;
    while remaining > 0 {
        let Some(front) = queue.front_mut() else {
            break;
        };
        if remaining < front.len() {
            front.advance(remaining);
            remaining = 0;
        } else {
            remaining -= front.len();
            queue.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use std::collections::VecDeque as StdVecDeque;

    /// A mock IO that records everything written, optionally caps how many bytes each write call
    /// accepts (to exercise partial writes), and can advertise vectored support. On the read side
    /// it hands back preset chunks and then signals EOF.
    struct MockIo {
        written: Vec<u8>,
        max_per_write: usize,
        vectored: bool,
        read_chunks: StdVecDeque<Vec<u8>>,
    }

    impl MockIo {
        fn writer(max_per_write: usize, vectored: bool) -> Self {
            MockIo {
                written: Vec::new(),
                max_per_write,
                vectored,
                read_chunks: StdVecDeque::new(),
            }
        }

        fn reader(chunks: Vec<Vec<u8>>) -> Self {
            MockIo {
                written: Vec::new(),
                max_per_write: usize::MAX,
                vectored: false,
                read_chunks: chunks.into(),
            }
        }
    }

    impl AsyncRead for MockIo {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            if let Some(chunk) = self.read_chunks.pop_front() {
                let n = chunk.len().min(buf.remaining());
                buf.put_slice(&chunk[..n]);
            }
            // No chunk left => zero bytes filled => the caller observes EOF.
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for MockIo {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let n = buf.len().min(self.max_per_write);
            self.written.extend_from_slice(&buf[..n]);
            Poll::Ready(Ok(n))
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            let mut budget = self.max_per_write;
            let mut total = 0;
            for buf in bufs {
                if budget == 0 {
                    break;
                }
                let n = buf.len().min(budget);
                self.written.extend_from_slice(&buf[..n]);
                budget -= n;
                total += n;
                if n < buf.len() {
                    break;
                }
            }
            Poll::Ready(Ok(total))
        }

        fn is_write_vectored(&self) -> bool {
            self.vectored
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    async fn send_chunks(io: MockIo, chunks: &[&[u8]]) -> Vec<u8> {
        let mut framed = RedisFramed::new(io);
        for chunk in chunks {
            framed.feed(Bytes::copy_from_slice(chunk)).await.unwrap();
        }
        framed.flush().await.unwrap();
        framed.io.written
    }

    #[tokio::test]
    async fn vectored_writes_preserve_order() {
        let out = send_chunks(
            MockIo::writer(usize::MAX, true),
            &[b"hello", b" ", b"world"],
        )
        .await;
        assert_eq!(out, b"hello world");
    }

    #[tokio::test]
    async fn vectored_partial_writes_span_chunk_boundaries() {
        // Only 3 bytes accepted per (vectored) write call, so the flush loop must re-issue writes
        // and correctly track partial progress across chunk boundaries.
        let out = send_chunks(MockIo::writer(3, true), &[b"abcd", b"ef", b"ghij"]).await;
        assert_eq!(out, b"abcdefghij");
    }

    #[tokio::test]
    async fn non_vectored_coalesces_and_preserves_order() {
        let out = send_chunks(MockIo::writer(usize::MAX, false), &[b"foo", b"bar", b"baz"]).await;
        assert_eq!(out, b"foobarbaz");
    }

    #[tokio::test]
    async fn non_vectored_partial_writes() {
        let out = send_chunks(MockIo::writer(2, false), &[b"abcde", b"fg"]).await;
        assert_eq!(out, b"abcdefg");
    }

    #[tokio::test]
    async fn large_buffers_and_small_runs_preserve_order() {
        // A small run, then a large buffer, then another small run. Large buffers bypass the
        // coalesce buffer but must still be written in submission order.
        let big = vec![b'L'; COALESCE_THRESHOLD]; // >= threshold => queued by reference
        let mut framed = RedisFramed::new(MockIo::writer(usize::MAX, true));
        framed.feed(Bytes::from_static(b"a")).await.unwrap();
        framed.feed(Bytes::from_static(b"b")).await.unwrap();
        framed.feed(Bytes::copy_from_slice(&big)).await.unwrap();
        framed.feed(Bytes::from_static(b"c")).await.unwrap();
        framed.flush().await.unwrap();

        let mut expected = b"ab".to_vec();
        expected.extend_from_slice(&big);
        expected.extend_from_slice(b"c");
        assert_eq!(framed.io.written, expected);
    }

    #[tokio::test]
    async fn large_buffers_partial_vectored_writes() {
        // Two large buffers written with a tight per-write cap, forcing partial vectored writes
        // that land in the middle of a queued (uncopied) buffer.
        let a = vec![b'A'; COALESCE_THRESHOLD];
        let b = vec![b'B'; COALESCE_THRESHOLD];
        let mut framed = RedisFramed::new(MockIo::writer(100, true));
        framed.feed(Bytes::copy_from_slice(&a)).await.unwrap();
        framed.feed(Bytes::copy_from_slice(&b)).await.unwrap();
        framed.flush().await.unwrap();

        let mut expected = a.clone();
        expected.extend_from_slice(&b);
        assert_eq!(framed.io.written, expected);
    }

    #[tokio::test]
    async fn reads_and_decodes_pipelined_values() {
        // Two simple-string replies arriving split across reads, then EOF.
        let io = MockIo::reader(vec![b"+OK\r\n+PO".to_vec(), b"NG\r\n".to_vec()]);
        let mut framed = RedisFramed::new(io);

        let first = framed.next().await.unwrap().unwrap();
        assert_eq!(first, Value::Okay);
        let second = framed.next().await.unwrap().unwrap();
        assert_eq!(second, Value::SimpleString("PONG".into()));
        // Clean EOF with an empty buffer yields end-of-stream.
        assert!(framed.next().await.is_none());
    }

    #[tokio::test]
    async fn trailing_bytes_at_eof_surface_an_error_then_end() {
        let io = MockIo::reader(vec![b"+OK\r\n$5\r\npar".to_vec()]);
        let mut framed = RedisFramed::new(io);

        assert_eq!(framed.next().await.unwrap().unwrap(), Value::Okay);
        // The incomplete bulk string left in the buffer at EOF is a framing error...
        assert!(framed.next().await.unwrap().is_err());
        // ...and the stream then reports end-of-stream.
        assert!(framed.next().await.is_none());
    }
}
