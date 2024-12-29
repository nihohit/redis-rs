use super::{AsyncStream, RedisRuntime, TaskHandle};
#[cfg(all(feature = "tokio-native-tls-comp", not(feature = "tls-rustls")))]
use crate::connection::TlsConnParams;
#[cfg(feature = "tokio-rustls-comp")]
use crate::tls::TlsConnParams;
use crate::RedisResult;
use monoio::net::TcpStream;
use std::path::Path;
use std::pin::Pin;
use std::{future::Future, net::SocketAddr};
use tokio::io::{AsyncRead, AsyncWrite};

pub(crate) enum MonoIo {
    Tcp(TcpStream),
}

async fn connect_tcp(socket_addr: SocketAddr) -> RedisResult<TcpStream> {
    let socket = TcpStream::connect(socket_addr).await?;
    #[cfg(feature = "tcp_nodelay")]
    socket.set_nodelay(true)?;

    #[cfg(feature = "keep-alive")]
    {
        //For now rely on system defaults
        const KEEP_ALIVE: socket2::TcpKeepalive = socket2::TcpKeepalive::new();
        socket.set_tcp_keepalive(None, None, None);
    }

    Ok(socket)
}

impl RedisRuntime for MonoIo {
    async fn connect_tcp(socket_addr: SocketAddr) -> RedisResult<Self> {
        Ok(MonoIo::Tcp(connect_tcp(socket_addr).await?))
    }

    #[cfg(all(feature = "tls-native-tls", not(feature = "tls-rustls")))]
    async fn connect_tcp_tls(
        hostname: &str,
        socket_addr: SocketAddr,
        insecure: bool,
        _: &Option<TlsConnParams>,
    ) -> RedisResult<Self> {
        todo!()
    }

    #[cfg(feature = "tls-rustls")]
    async fn connect_tcp_tls(
        hostname: &str,
        socket_addr: SocketAddr,
        insecure: bool,
        tls_params: &Option<TlsConnParams>,
    ) -> RedisResult<Self> {
        todo!()
    }

    #[cfg(unix)]
    async fn connect_unix(path: &Path) -> RedisResult<Self> {
        todo!()
    }

    type BoxedFuture = Box<dyn Future<Output = ()> + 'static>;
    type BoxedStream = Box<dyn AsyncStream + 'static>;

    fn spawn(f: Self::BoxedFuture) -> TaskHandle {
        TaskHandle::MonoIo(monoio::spawn(f))
    }

    fn boxed(self) -> Pin<Self::BoxedStream> {
        match self {
            MonoIo::Tcp(tcp_stream) => Box::pin(tcp_stream),
        }
    }
}

impl AsyncWrite for MonoIo {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        todo!()
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        todo!()
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        todo!()
    }
}

impl AsyncRead for MonoIo {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        todo!()
    }
}
