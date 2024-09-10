#[cfg(unix)]
use tokio_uring::net::UnixStream;
use tokio_uring::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

async fn connect_tcp(addr: &SocketAddr) -> io::Result<TcpStream> {
    let socket = TcpStream::connect(addr).await?;
    #[cfg(feature = "tcp_nodelay")]
    socket.set_nodelay(true)?;
    #[cfg(feature = "keep-alive")]
    {
        //For now rely on system defaults
        const KEEP_ALIVE: socket2::TcpKeepalive = socket2::TcpKeepalive::new();
        //these are useless error that not going to happen
        let std_socket = socket.into_std()?;
        let socket2: socket2::Socket = std_socket.into();
        socket2.set_tcp_keepalive(&KEEP_ALIVE)?;
        TcpStream::from_std(socket2.into())
    }

    #[cfg(not(feature = "keep-alive"))]
    {
        Ok(socket)
    }
}

pub(crate) enum TokioUring {
    /// Represents a tokio-uring TCP connection.
    Tcp(TcpStream),
    /// Represents a tokio-uring TLS encrypted TCP connection
    #[cfg(any(
        feature = "tokio-uring-native-tls-comp",
        feature = "tokio-uring-rustls-comp"
    ))]
    TcpTls(Box<TlsStream<TcpStream>>),
    /// Represents a tokio-uring Unix connection.
    #[cfg(unix)]
    Unix(UnixStream),
}

impl AsyncWrite for TokioUring {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            TokioUring::Tcp(r) => Pin::new(r).poll_write(cx, buf),
            #[cfg(any(feature = "tokio-native-tls-comp", feature = "tokio-rustls-comp"))]
            TokioUring::TcpTls(r) => Pin::new(r).poll_write(cx, buf),
            #[cfg(unix)]
            TokioUring::Unix(r) => Pin::new(r).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        match &mut *self {
            TokioUring::Tcp(r) => Pin::new(r).poll_flush(cx),
            #[cfg(any(feature = "tokio-native-tls-comp", feature = "tokio-rustls-comp"))]
            TokioUring::TcpTls(r) => Pin::new(r).poll_flush(cx),
            #[cfg(unix)]
            TokioUring::Unix(r) => Pin::new(r).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<io::Result<()>> {
        match &mut *self {
            TokioUring::Tcp(r) => Pin::new(r).poll_shutdown(cx),
            #[cfg(any(feature = "tokio-native-tls-comp", feature = "tokio-rustls-comp"))]
            TokioUring::TcpTls(r) => Pin::new(r).poll_shutdown(cx),
            #[cfg(unix)]
            TokioUring::Unix(r) => Pin::new(r).poll_shutdown(cx),
        }
    }
}

impl AsyncRead for TokioUring {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            TokioUring::Tcp(r) => Pin::new(r).poll_read(cx, buf),
            #[cfg(any(feature = "tokio-native-tls-comp", feature = "tokio-rustls-comp"))]
            TokioUring::TcpTls(r) => Pin::new(r).poll_read(cx, buf),
            #[cfg(unix)]
            TokioUring::Unix(r) => Pin::new(r).poll_read(cx, buf),
        }
    }
}

#[async_trait]
impl RedisRuntime for TokioUring {
    async fn connect_tcp(socket_addr: SocketAddr) -> RedisResult<Self> {
        Ok(connect_tcp(&socket_addr).await.map(TokioUring::Tcp)?)
    }

    #[cfg(all(feature = "tls-native-tls", not(feature = "tls-rustls")))]
    async fn connect_tcp_tls(
        hostname: &str,
        socket_addr: SocketAddr,
        insecure: bool,
        _: &Option<TlsConnParams>,
    ) -> RedisResult<Self> {
        let tls_connector: tokio_native_tls::TlsConnector = if insecure {
            TlsConnector::builder()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .use_sni(false)
                .build()?
        } else {
            TlsConnector::new()?
        }
        .into();
        Ok(tls_connector
            .connect(hostname, connect_tcp(&socket_addr).await?)
            .await
            .map(|con| TokioUring::TcpTls(Box::new(con)))?)
    }

    #[cfg(feature = "tls-rustls")]
    async fn connect_tcp_tls(
        hostname: &str,
        socket_addr: SocketAddr,
        insecure: bool,
        tls_params: &Option<TlsConnParams>,
    ) -> RedisResult<Self> {
        let config = create_rustls_config(insecure, tls_params.clone())?;
        let tls_connector = TlsConnector::from(Arc::new(config));

        Ok(tls_connector
            .connect(
                rustls_pki_types::ServerName::try_from(hostname)?.to_owned(),
                connect_tcp(&socket_addr).await?,
            )
            .await
            .map(|con| TokioUring::TcpTls(Box::new(con)))?)
    }

    #[cfg(unix)]
    async fn connect_unix(path: &Path) -> RedisResult<Self> {
        Ok(UnixStreamTokio::connect(path).await.map(TokioUring::Unix)?)
    }

    fn spawn(f: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        TaskHandle::TokioUring(tokio::spawn(f))
    }

    fn boxed(self) -> Pin<Box<dyn AsyncStream + Send + Sync>> {
        match self {
            TokioUring::Tcp(x) => Box::pin(x),
            #[cfg(any(feature = "tokio-native-tls-comp", feature = "tokio-rustls-comp"))]
            TokioUring::TcpTls(x) => Box::pin(x),
            #[cfg(unix)]
            TokioUring::Unix(x) => Box::pin(x),
        }
    }
}
