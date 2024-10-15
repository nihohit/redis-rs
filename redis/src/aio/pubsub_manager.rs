use super::{PubSub, SharedHandleContainer};
use crate::types::{closed_connection_error, RedisResult};
use crate::{Cmd, Msg, RedisConnectionInfo, ToRedisArgs};
use ::tokio::io::{AsyncRead, AsyncWrite};
use futures::channel::mpsc::UnboundedSender;
use futures::channel::oneshot;
use futures::SinkExt;
use futures_util::stream::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{self, Poll};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

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
pub struct PubSubManagerSink(UnboundedSender<SinkRequest>);

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
        stream: UnboundedReceiver<Msg>,
        // This handle ensures that once the stream will be dropped, the underlying task will stop.
        _task_handle: Option<SharedHandleContainer>,
    }
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

        let (sink_sender, sink_receiver) = unbounded_channel();
        let (stream_sender, stream_receiver) = unbounded_channel();
        let (setup_complete_sender, setup_complete_receiver) = oneshot::channel();
        let _task_handle = Runtime::locate().spawn(start_listening(
            sink_receiver,
            stream_sender,
            setup_complete_sender,
        ));
        setup_complete_receiver?;
        Ok(Self {
            stream: PubSubManagerStream {
                stream: stream_receiver,
                _task_handle,
            },
            sink: PubSubManagerSink(sink_sender),
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

async fn start_listening(
    sink_receiver: UnboundedReceiver<_>,
    stream_sender: tokio::sync::mpsc::UnboundedSender<Msg>,
    setup_completed_sender: oneshot::Sender<RedisResult<()>>,
) -> Option<SharedHandleContainer> {
    loop {}
}

enum SinkRequestType {
    Subscribe,
    Unsubscribe,
    PSubscribe,
    PUnsubscribe,
}

struct SinkRequest {
    request_type: SinkRequestType,
    arguments: Vec<Vec<u8>>,
    response: oneshot::Sender<RedisResult<()>>,
}

impl PubSubManagerSink {
    async fn send_request(
        &mut self,
        request_type: SinkRequestType,
        arguments: Vec<Vec<u8>>,
    ) -> RedisResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.0.send(SinkRequest {
            request_type,
            arguments,
            response: sender,
        });
        receiver
            .await
            .unwrap_or_else(|_| Err(closed_connection_error()))
    }

    /// Subscribes to a new channel.
    pub async fn subscribe(&mut self, channel_name: impl ToRedisArgs) -> RedisResult<()> {
        self.send_request(SinkRequestType::Subscribe, channel_name.to_redis_args())
            .await
    }

    /// Unsubscribes from channel.
    pub async fn unsubscribe(&mut self, channel_name: impl ToRedisArgs) -> RedisResult<()> {
        self.send_request(SinkRequestType::Unsubscribe, channel_name.to_redis_args())
            .await
    }

    /// Subscribes to a new channel with pattern.
    pub async fn psubscribe(&mut self, channel_pattern: impl ToRedisArgs) -> RedisResult<()> {
        self.send_request(SinkRequestType::PSubscribe, channel_pattern.to_redis_args())
            .await
    }

    /// Unsubscribes from channel pattern.
    pub async fn punsubscribe(&mut self, channel_pattern: impl ToRedisArgs) -> RedisResult<()> {
        self.send_request(
            SinkRequestType::PUnsubscribe,
            channel_pattern.to_redis_args(),
        )
        .await
    }
}

impl Stream for PubSubManagerStream {
    type Item = Msg;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().stream.poll_recv(cx)
    }
}
