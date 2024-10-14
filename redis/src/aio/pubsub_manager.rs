use super::{PubSub, PubSubSink, PubSubStream};
use crate::types::RedisResult;
use crate::{Msg, RedisConnectionInfo, ToRedisArgs};
use ::tokio::io::{AsyncRead, AsyncWrite};
use futures_util::stream::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{self, Poll};

/// The sink part of a split async managed Pubsub.
///
/// The sink is used to subscribe and unsubscribe from
/// channels.
/// The stream part is independent from the sink,
/// and dropping the sink doesn't cause the stream part to
/// stop working, nor will the managed stream stop reconnecting
/// and resubscribing.
/// The sink isn't independent from the stream - dropping
/// the stream will cause the sink to return errors on requests.
pub struct PubSubManagerSink(PubSubSink);

impl Clone for PubSubManagerSink {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pin_project! {
    /// The stream part of a split managed async Pubsub.
    ///
    /// The stream is used to receive messages from the server.
    /// The stream part is independent from the sink,
    /// and dropping the sink doesn't cause the stream part to
    /// stop working, nor will the managed stream stop reconnecting
    /// and resubscribing.
    /// The sink isn't independent from the stream - dropping
    /// the stream will cause the sink to return errors on requests.
    pub struct PubSubManagerStream{
        #[pin]
        stream:PubSubStream}
}

/// A managed connection dedicated to pubsub messages.
///
/// If the pubsub disconnects from the server, it will
/// automatically attempt to reconnect and resubscribe to
/// all channels.
pub struct PubSubManager {
    sink: PubSubManagerSink,
    stream: PubSubManagerStream,
}

impl PubSubManager {
    /// Constructs a new `MultiplexedConnection` out of a `AsyncRead + AsyncWrite` object
    /// and a `ConnectionInfo`
    pub async fn new<C>(connection_info: &RedisConnectionInfo, stream: C) -> RedisResult<Self>
    where
        C: Unpin + AsyncRead + AsyncWrite + Send + 'static,
    {
        #[cfg(all(not(feature = "tokio-comp"), not(feature = "async-std-comp")))]
        compile_error!("tokio-comp or async-std-comp features required for aio feature");

        let (sink, stream) = PubSub::new(connection_info, stream).await?.split();
        Ok(Self {
            stream: PubSubManagerStream { stream },
            sink: PubSubManagerSink(sink),
        })
    }

    /// Subscribes to a new channel.
    pub async fn subscribe(&mut self, channel_name: impl ToRedisArgs) -> RedisResult<()> {
        self.sink.subscribe(channel_name).await
    }

    /// Unsubscribes from channel.
    pub async fn unsubscribe(&mut self, channel_name: impl ToRedisArgs) -> RedisResult<()> {
        self.sink.unsubscribe(channel_name).await
    }

    /// Subscribes to a new channel with pattern.
    pub async fn psubscribe(&mut self, channel_pattern: impl ToRedisArgs) -> RedisResult<()> {
        self.sink.psubscribe(channel_pattern).await
    }

    /// Unsubscribes from channel pattern.
    pub async fn punsubscribe(&mut self, channel_pattern: impl ToRedisArgs) -> RedisResult<()> {
        self.sink.punsubscribe(channel_pattern).await
    }

    /// Returns [`Stream`] of [`Msg`]s from this [`PubSub`]s subscriptions.
    ///
    /// The message itself is still generic and can be converted into an appropriate type through
    /// the helper methods on it.
    pub fn on_message(&mut self) -> impl Stream<Item = Msg> + '_ {
        &mut self.stream
    }

    /// Returns [`Stream`] of [`Msg`]s from this [`PubSub`]s subscriptions consuming it.
    ///
    /// The message itself is still generic and can be converted into an appropriate type through
    /// the helper methods on it.
    /// This can be useful in cases where the stream needs to be returned or held by something other
    /// than the [`PubSub`].
    pub fn into_on_message(self) -> PubSubManagerStream {
        self.stream
    }

    /// Splits the PubSub into separate sink and stream components, so that subscriptions could be
    /// updated through the `Sink` while concurrently waiting for new messages on the `Stream`.
    pub fn split(self) -> (PubSubManagerSink, PubSubManagerStream) {
        (self.sink, self.stream)
    }
}

impl PubSubManagerSink {
    /// Subscribes to a new channel.
    pub async fn subscribe(&mut self, channel_name: impl ToRedisArgs) -> RedisResult<()> {
        self.0.subscribe(channel_name).await
    }

    /// Unsubscribes from channel.
    pub async fn unsubscribe(&mut self, channel_name: impl ToRedisArgs) -> RedisResult<()> {
        self.0.unsubscribe(channel_name).await
    }

    /// Subscribes to a new channel with pattern.
    pub async fn psubscribe(&mut self, channel_pattern: impl ToRedisArgs) -> RedisResult<()> {
        self.0.psubscribe(channel_pattern).await
    }

    /// Unsubscribes from channel pattern.
    pub async fn punsubscribe(&mut self, channel_pattern: impl ToRedisArgs) -> RedisResult<()> {
        self.0.punsubscribe(channel_pattern).await
    }
}

impl Stream for PubSubManagerStream {
    type Item = Msg;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_next(cx)
    }
}
