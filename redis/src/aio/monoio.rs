use super::{AsyncStream, RedisRuntime, TaskHandle};
#[cfg(all(feature = "tokio-native-tls-comp", not(feature = "tls-rustls")))]
use crate::connection::TlsConnParams;
#[cfg(feature = "tokio-rustls-comp")]
use crate::tls::TlsConnParams;
use crate::RedisResult;
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::path::Path;
use std::{future::Future, net::SocketAddr};

pub(crate) enum MonoIo {
    Tcp,
}

#[async_trait]
impl RedisRuntime for MonoIo {
    async fn connect_tcp(socket_addr: SocketAddr) -> RedisResult<Self> {
        todo!()
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

    fn spawn(f: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        TaskHandle::MonoIo(monoio::spawn(f))
    }
}

impl AsyncWrite for MonoIo {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        todo!()
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        todo!()
    }

    fn poll_close(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        todo!()
    }
}

impl AsyncRead for MonoIo {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        todo!()
    }
}

impl AsyncStream for MonoIo {}
