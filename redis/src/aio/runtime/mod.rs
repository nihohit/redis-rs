use std::{io, sync::Arc, time::Duration};

use super::RedisRuntime;
use futures_util::Future;

#[cfg(feature = "async-std-comp")]
pub(crate) mod async_std;
#[cfg(feature = "tokio-comp")]
pub(crate) mod tokio;
#[cfg(feature = "tokio-uring-comp")]
pub(crate) mod tokio_uring;
use crate::types::RedisError;

#[derive(Clone, Debug)]
pub(crate) enum Runtime {
    #[cfg(feature = "tokio-comp")]
    Tokio,
    #[cfg(feature = "tokio-uring-comp")]
    TokioUring,
    #[cfg(feature = "async-std-comp")]
    AsyncStd,
}

pub(crate) enum TaskHandle {
    #[cfg(feature = "tokio-comp")]
    Tokio(::tokio::task::JoinHandle<()>),
    #[cfg(feature = "tokio-uring-comp")]
    TokioUring(::tokio::task::JoinHandle<()>),
    #[cfg(feature = "async-std-comp")]
    AsyncStd(::async_std::task::JoinHandle<()>),
}

pub(crate) struct HandleContainer(Option<TaskHandle>);

impl HandleContainer {
    pub(crate) fn new(handle: TaskHandle) -> Self {
        Self(Some(handle))
    }
}

impl Drop for HandleContainer {
    fn drop(&mut self) {
        match self.0.take() {
            None => {}
            #[cfg(feature = "tokio-comp")]
            Some(TaskHandle::Tokio(handle)) => handle.abort(),
            #[cfg(feature = "tokio-uring-comp")]
            Some(TaskHandle::TokioUring(handle)) => handle.abort(),
            #[cfg(feature = "async-std-comp")]
            Some(TaskHandle::AsyncStd(handle)) => {
                // schedule for cancellation without waiting for result.
                Runtime::locate().spawn(async move { handle.cancel().await.unwrap_or_default() });
            }
        }
    }
}

#[derive(Clone)]
// we allow dead code here because the container isn't used directly, only in the derived drop.
#[allow(dead_code)]
pub(crate) struct SharedHandleContainer(Arc<HandleContainer>);

impl SharedHandleContainer {
    pub(crate) fn new(handle: TaskHandle) -> Self {
        Self(Arc::new(HandleContainer::new(handle)))
    }
}

impl Runtime {
    pub(crate) fn locate() -> Self {
        #[cfg(any(
            feature = "tokio-comp",
            feature = "tokio-uring-comp",
            feature = "async-std-comp"
        ))]
        {
            if ::tokio::runtime::Handle::try_current().is_ok() {
                Runtime::Tokio
            } else {
                Runtime::AsyncStd
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn spawn(&self, f: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        match self {
            #[cfg(feature = "tokio-comp")]
            Runtime::Tokio => tokio::Tokio::spawn(f),
            #[cfg(feature = "tokio-uring-comp")]
            Runtime::TokioUring => tokio::Tokio::spawn(f),
            #[cfg(feature = "async-std-comp")]
            Runtime::AsyncStd => async_std::AsyncStd::spawn(f),
        }
    }

    pub(crate) async fn timeout<F: Future>(
        &self,
        duration: Duration,
        future: F,
    ) -> Result<F::Output, Elapsed> {
        match self {
            #[cfg(feature = "tokio-comp")]
            Runtime::Tokio => ::tokio::time::timeout(duration, future)
                .await
                .map_err(|_| Elapsed(())),
            #[cfg(feature = "tokio-uring-comp")]
            Runtime::TokioUring => ::tokio::time::timeout(duration, future)
                .await
                .map_err(|_| Elapsed(())),
            #[cfg(feature = "async-std-comp")]
            Runtime::AsyncStd => ::async_std::future::timeout(duration, future)
                .await
                .map_err(|_| Elapsed(())),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Elapsed(());

impl From<Elapsed> for RedisError {
    fn from(_: Elapsed) -> Self {
        io::Error::from(io::ErrorKind::TimedOut).into()
    }
}
