use std::{cell::RefCell, io, sync::Arc, time::Duration};

use futures_util::Future;

#[cfg(feature = "async-std-comp")]
use super::async_std as crate_async_std;
#[cfg(feature = "monoio-comp")]
use super::monoio as crate_smol;
#[cfg(feature = "tokio-comp")]
use super::tokio as crate_tokio;
use super::RedisRuntime;
use crate::types::RedisError;

#[derive(Clone, Debug, Copy)]
pub(crate) enum Runtime {
    #[cfg(feature = "tokio-comp")]
    Tokio,
    #[cfg(feature = "async-std-comp")]
    AsyncStd,
    #[cfg(feature = "monoio-comp")]
    MonoIo,
}

pub(crate) enum TaskHandle {
    #[cfg(feature = "tokio-comp")]
    Tokio(tokio::task::JoinHandle<()>),
    #[cfg(feature = "async-std-comp")]
    AsyncStd(async_std::task::JoinHandle<()>),
    #[cfg(feature = "monoio-comp")]
    MonoIo(monoio::task::JoinHandle<()>),
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
            #[cfg(feature = "async-std-comp")]
            Some(TaskHandle::AsyncStd(handle)) => {
                // schedule for cancellation without waiting for result.
                // TODO - can we cancel the task without awaiting its completion?
                Runtime::locate().spawn(async move { handle.cancel().await.unwrap_or_default() });
            }
            #[cfg(feature = "monoio-comp")]
            Some(TaskHandle::MonoIo(task)) => drop(task),
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

thread_local! {
    pub(crate) static CHOSEN_RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

/// Mark MonoIo as the preferred runtime.
///
/// Call this function if the application doesn't use multiple runtimes,
/// and the real-time check of which runtime is in use can be avoided.
#[cfg(feature = "monoio-comp")]
pub fn prefer_monoio() {
    CHOSEN_RUNTIME.set(Some(Runtime::MonoIo));
}

/// Mark async-std compliant runtimes, such as smol, as the preferred runtime.
///
/// Call this function if the application doesn't use multiple runtimes,
/// and the real-time check of which runtime is in use can be avoided.
#[cfg(feature = "async-std-comp")]
pub fn prefer_async_std() {
    CHOSEN_RUNTIME.set(Some(Runtime::AsyncStd));
}

/// Mark Tokio as the preferred runtime.
///
/// Call this function if the application doesn't use multiple runtimes,
/// and the real-time check of which runtime is in use can be avoided.
#[cfg(feature = "tokio-comp")]
pub fn prefer_tokio() {
    CHOSEN_RUNTIME.set(Some(Runtime::Tokio));
}

impl Runtime {
    pub(crate) fn locate() -> Self {
        #[cfg(all(
            feature = "tokio-comp",
            not(feature = "monoio-comp"),
            not(feature = "async-std-comp")
        ))]
        {
            Runtime::Tokio
        }

        #[cfg(all(
            not(feature = "tokio-comp"),
            not(feature = "monoio-comp"),
            feature = "async-std-comp"
        ))]
        {
            Runtime::AsyncStd
        }

        #[cfg(all(
            not(feature = "tokio-comp"),
            feature = "monoio-comp",
            not(feature = "async-std-comp")
        ))]
        {
            Runtime::MonoIo
        }

        // Is this a real-world scenario? Who, besides our test case, compile with multiple runtime support?
        #[cfg(all(
            feature = "tokio-comp",
            feature = "monoio-comp",
            feature = "async-std-comp"
        ))]
        {
            if let Some(runtime) = CHOSEN_RUNTIME.with_borrow(|r| *r) {
                return runtime;
            }

            if ::tokio::runtime::Handle::try_current().is_ok() {
                Runtime::Tokio
            } else {
                // TODO - is there a way to disambiguate whether there's an async-std runtime or a monoio runtime present?
                Runtime::AsyncStd
            }
        }

        #[cfg(all(
            not(feature = "tokio-comp"),
            not(feature = "monoio-comp"),
            not(feature = "async-std-comp")
        ))]
        {
            compile_error!(
                "tokio-comp, monoio-comp, or async-std-comp features required for aio feature"
            )
        }
    }

    #[allow(dead_code)]
    pub(crate) fn spawn(&self, f: impl Future<Output = ()> + Send + 'static) -> TaskHandle {
        match self {
            #[cfg(feature = "tokio-comp")]
            Runtime::Tokio => crate_tokio::Tokio::spawn(f),
            #[cfg(feature = "async-std-comp")]
            Runtime::AsyncStd => crate_async_std::AsyncStd::spawn(f),
            #[cfg(feature = "monoio-comp")]
            Runtime::MonoIo => crate_smol::MonoIo::spawn(f),
        }
    }

    pub(crate) async fn timeout<F: Future>(
        &self,
        duration: Duration,
        future: F,
    ) -> Result<F::Output, Elapsed> {
        match self {
            #[cfg(feature = "tokio-comp")]
            Runtime::Tokio => tokio::time::timeout(duration, future)
                .await
                .map_err(|_| Elapsed(())),
            #[cfg(feature = "async-std-comp")]
            Runtime::AsyncStd => async_std::future::timeout(duration, future)
                .await
                .map_err(|_| Elapsed(())),
            #[cfg(feature = "monoio-comp")]
            Runtime::MonoIo => futures_time::future::FutureExt::timeout(
                future,
                futures_time::time::Duration::from(duration),
            )
            .await
            .map_err(|_| Elapsed(())),
        }
    }

    #[cfg(any(feature = "connection-manager", feature = "cluster-async"))]
    pub(crate) async fn sleep(&self, duration: Duration) {
        match self {
            #[cfg(feature = "tokio-comp")]
            Runtime::Tokio => {
                tokio::time::sleep(duration).await;
            }
            #[cfg(feature = "async-std-comp")]
            Runtime::AsyncStd => {
                async_std::task::sleep(duration).await;
            }
            #[cfg(feature = "monoio-comp")]
            Runtime::MonoIo => {
                futures_time::task::sleep(duration.into()).await;
            }
        }
    }

    #[cfg(feature = "cluster-async")]
    pub(crate) async fn locate_and_sleep(duration: Duration) {
        Self::locate().sleep(duration).await
    }
}

#[derive(Debug)]
pub(crate) struct Elapsed(());

impl From<Elapsed> for RedisError {
    fn from(_: Elapsed) -> Self {
        io::Error::from(io::ErrorKind::TimedOut).into()
    }
}
