# Reduce allocations & copies on the async write path

## Summary

This change cuts the allocations and copies incurred when sending commands to the socket on the
**async multiplexed** connection and the **async cluster** connection (which delegates to it). It is
the write-side counterpart to the zero-copy response **parsing** already landed on this branch (see
`version2.md`).

Two wins:

1. **Encode once, carry as `bytes::Bytes`.** The encoded command travels through the request channel
   and codec as a cheaply-cloneable `Bytes` instead of a `Vec<u8>`.
2. **Zero-copy, vectored socket writes.** A new `RedisFramed` replaces `tokio_util::codec::Framed` on
   the write side: large already-encoded buffers go straight from their `Bytes` to the socket via
   `writev` instead of being copied into the codec's write buffer. Small commands are still coalesced
   (one copy, one syscall), so the common case is never slower.

## Motivation

Previously, sending one command did:

1. **Encode alloc** — `Cmd::get_packed_command()` / `Pipeline::get_packed_pipeline()` allocate a fresh
   `Vec<u8>` and RESP-encode into it.
2. **Channel hop** — the `Vec<u8>` is moved over an mpsc channel to the driver task.
3. **Copy into codec buffer** — `ValueCodec`'s `Encoder` did `dst.extend_from_slice(item)`, copying the
   already-encoded bytes into `Framed`'s internal `BytesMut` before flushing.

Step 3 is a redundant copy of bytes that are already fully encoded. For large values (e.g. a multi-MB
`SET`) it is pure waste. This change removes it for large payloads while preserving coalescing for
small ones.

## What changed

### Encode-to-`Bytes` threading
- The async path now moves `bytes::Bytes` through the request channel: `PipelineMessage.input`,
  `Pipeline::send_recv`, and every `Sink<Vec<u8>>` bound became `Sink<Bytes>`
  (`aio/multiplexed_connection.rs`, `aio/pubsub.rs`, `aio/monitor.rs`, `aio/mod.rs`).
- The public `Cmd::get_packed_command` / `Pipeline::get_packed_pipeline` **keep returning `Vec<u8>`**
  (no breaking change; the sync `ConnectionLike` path takes `&[u8]`). Call sites wrap with
  `Bytes::from(...)`, which is O(1) — it adopts the `Vec`'s allocation without copying.

### `RedisFramed<C>` — adaptive, vectored write sink (`redis/src/aio/framed.rs`, new)
Replaces `Framed<C, ValueCodec>` as both `Stream<Item = RedisResult<Value>>` and `Sink<Bytes>`.

- **Read side** is unchanged in behavior: it reads directly into a `BytesMut` and hands frozen slices
  to the existing parser (preserving zero-copy decoding). An `is_readable` flag (mirroring
  `tokio_util::FramedRead`) avoids re-running the parser over an incomplete buffer between polls.
- **Write side is adaptive**, governed by `COALESCE_THRESHOLD` (8 KiB):
  - Buffers **smaller** than the threshold are copied into a reused coalesce `BytesMut` — exactly like
    the old codec, so small/pipelined commands incur one copy and one `write`.
  - Buffers **at or above** the threshold are queued by reference (`Bytes`, no copy).
  - At flush, queued large buffers are written first via `poll_write_vectored` (up to `MAX_IOV = 128`
    `IoSlice`s), then the coalesced small run is written from its reused buffer. Submission order is
    preserved (a pending small run is frozen ahead of any large buffer that arrives after it).
- Transports that don't support vectored writes fall back to the coalescing path (see below), so there
  is **no regression** there.

### Vectored-write capability forwarding (`redis/src/aio/tokio.rs`)
The IO that reaches the codec is the `Tokio` enum (often boxed as `dyn AsyncStream`), whose default
`AsyncWrite` reports `is_write_vectored() == false` and whose default `poll_write_vectored` writes only
the first slice. Without forwarding, `RedisFramed` would never take the zero-copy path on real
connections. `Tokio` now forwards `is_write_vectored` / `poll_write_vectored` to the inner
`TcpStream`/`UnixStream`/TLS stream, so plaintext TCP and Unix sockets get true vectored writes; TLS
correctly reports `false` and uses the coalescing fallback.

### Deadlock safety preserved
The TCP-deadlock fix from issue #1955 lives in `PipelineSink`'s read-before-write poll ordering and is
unchanged. `RedisFramed` registers a waker on every `Pending`, so the contract holds; the regression
test `test_deadlock_when_writes_blocked_with_pending_response` passes.

## Benchmarks

Measured with `cargo bench -p redis --bench bench_basic` against a local server, comparing this branch
to its `2.0.x` base. Two large-payload write benches were added (`multiplexed_async_large_pipeline`,
`multiplexed_async_large_implicit_pipeline`; 64 KiB × 100 values).

**Method note on noise:** these benches run against a real server over loopback and the machine drifts
~2–3% slower across successive runs (a same-binary-vs-itself control showed a "+2.2%" delta with *zero*
code change). So sub-3% deltas are noise; only larger, reproducible deltas are real.

| Benchmark | Payload | Δ vs `2.0.x` | Verdict |
| --- | --- | --- | --- |
| `query/simple_getsetdel_async` | tiny, 1 cmd | ~0% | neutral |
| `query_pipeline/multiplexed_async_long_pipeline` | 1000 small | within ±noise | neutral |
| `query_pipeline/multiplexed_async_implicit_pipeline` | 1000 concurrent small | within ±noise | neutral |
| `query_pipeline_large/multiplexed_async_large_pipeline` | 100 × 64 KiB | **−12%** (−10.6%…−13.6%) | **improved** |
| `query_pipeline_large/multiplexed_async_large_implicit_pipeline` | 100 × 64 KiB | **−3.6%** (−1.8%…−5.4%) | **improved** |

**Conclusion:** clear, reproducible win for large payloads (the copy elimination); neutral within the
measurement-noise floor for small commands. Because the large-payload wins exceeded the ~2–3% upward
drift, the true improvement is, if anything, slightly understated.

> An earlier pure-vectored implementation regressed the small concurrent-command case by ~10% (writev
> of many tiny iovecs is slower than one coalesced write). The adaptive coalescing + `is_readable`
> read optimization above eliminated that regression while keeping the large-payload win.

## Testing

- `cargo build` + `cargo clippy` clean across feature combos: `tokio-comp`, `smol-comp`,
  `tokio-rustls-comp`, `tokio-native-tls-comp`, `cluster-async`, `connection-manager`, `cache-aio`.
- 8 new `RedisFramed` unit tests: vectored writes, partial writes across chunk boundaries, the
  non-vectored coalescing fallback, large + mixed-order writes, pipelined decode, and EOF/trailing-byte
  handling.
- Full lib test suite passes (incl. the #1955 deadlock regression). The one pre-existing failure,
  `test_lazy_connection_manager_with_config`, also fails on `2.0.x` (a single-thread-runtime test
  issue, unrelated).
- Integration: `test_basic` (101) and `test_async` (90, single-threaded) pass against a real server,
  exercising the real vectored write path over TCP.

## Scope decisions / deliberately not done

- **Public `get_packed_*` API unchanged.** Kept returning `Vec<u8>` to avoid a breaking change and to
  serve the sync `&[u8]` path; the `Bytes::from` wrap at async call sites is allocation-free.
- **No cluster command memoization.** Cluster already shares the command cheaply via `Arc<Cmd>`; the
  only "same bytes to N nodes" case is `AllNodes`/`AllMasters` fan-out (a cold administrative path —
  `MultiSlot` fan-out builds distinct per-slot commands). Memoizing packed bytes on `Cmd` would add
  overhead to the hot single-send path to speed up a cold one, so it was descoped.
- **Smol stays on the coalescing fallback.** futures-io's `AsyncWrite` exposes no vectored-capability
  probe, so smol cannot safely advertise vectored support; it coalesces (no regression).

## Possible follow-ups

- Collapse the ~9 `Bytes::from(x.get_packed_command())` call sites behind a small internal helper
  (cosmetic; the pattern is allocation-free as-is).
- If a future smol/futures-io version exposes a vectored-write capability, forward it the same way the
  `Tokio` enum does.

## Notes for maintainers

- The branch was rebased onto `2.0.x`; the duplicate flatten-pipeline commit was dropped in favor of
  `2.0.x`'s `#2157`, and `pipeline.rs` is now identical to `2.0.x`'s.
- The rebased commits are currently **unsigned** (signing was disabled to perform the rebase
  non-interactively). Re-sign before pushing if required:
  `git rebase --exec 'git commit --amend --no-edit -S' 2.0.x`.
