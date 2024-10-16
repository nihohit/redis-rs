use super::SharedHandleContainer;
use crate::aio::Runtime;
use crate::subscription_tracker::{SubscriptionAction, SubscriptionTracker};
use crate::types::{closed_connection_error, RedisResult};
use crate::{Client, ConnectionInfo, Msg, RedisError, ToRedisArgs};
use futures::future::{join_all, select};
use futures_util::stream::{Stream, StreamExt};
use futures_util::FutureExt;
use pin_project_lite::pin_project;
use std::pin::{pin, Pin};
use std::task::{self, Poll};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio_retry2::strategy::{jitter, ExponentialBackoff};
use tokio_retry2::{MapErr, Retry};

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

/// The configuration for reconnect mechanism and request timing for the [PubsubManager]
#[derive(Clone, Debug)]
pub struct PubsubManagerConfig {
    /// The resulting duration is calculated by taking the base to the `n`-th power,
    /// where `n` denotes the number of past attempts.
    exponent_base: u64,
    /// A multiplicative factor that will be applied to the retry delay.
    ///
    /// For example, using a factor of `1000` will make each delay in units of seconds.
    factor: u64,
    /// number_of_retries the maximum number of retries before resetting the reconnection attempt
    number_of_retries: usize,
    /// Apply a maximum delay between connection attempts. The delay between attempts won't be longer than max_delay milliseconds.
    max_delay: Option<u64>,
    /// Each connection attempt to the server will time out after `connection_timeout`.
    connection_timeout: std::time::Duration,
}

impl PubsubManagerConfig {
    /// Creates a new instance of the options with nothing set
    pub fn new() -> Self {
        Self::default()
    }

    /// A multiplicative factor that will be applied to the retry delay.
    ///
    /// For example, using a factor of `1000` will make each delay in units of seconds.
    pub fn set_factor(mut self, factor: u64) -> Self {
        self.factor = factor;
        self
    }

    /// Apply a maximum delay between connection attempts. The delay between attempts won't be longer than max_delay milliseconds.
    pub fn set_max_delay(mut self, time: u64) -> Self {
        self.max_delay = Some(time);
        self
    }

    /// The resulting duration is calculated by taking the base to the `n`-th power,
    /// where `n` denotes the number of past attempts.
    pub fn set_exponent_base(mut self, base: u64) -> Self {
        self.exponent_base = base;
        self
    }

    /// number_of_retries times, with an exponentially increasing delay
    pub fn set_number_of_retries(mut self, amount: usize) -> Self {
        self.number_of_retries = amount;
        self
    }

    /// Each connection attempt to the server will time out after `connection_timeout`.
    pub fn set_connection_timeout(mut self, duration: std::time::Duration) -> Self {
        self.connection_timeout = duration;
        self
    }
}

impl Default for PubsubManagerConfig {
    fn default() -> Self {
        Self {
            exponent_base: crate::aio::connection_manager::ConnectionManagerConfig::DEFAULT_CONNECTION_RETRY_EXPONENT_BASE,
            factor: crate::aio::connection_manager::ConnectionManagerConfig::DEFAULT_CONNECTION_RETRY_FACTOR,
            number_of_retries: crate::aio::connection_manager::ConnectionManagerConfig::DEFAULT_NUMBER_OF_CONNECTION_RETRIES,
            max_delay: None,
            connection_timeout: crate::aio::connection_manager::ConnectionManagerConfig::DEFAULT_CONNECTION_TIMEOUT,
        }
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
    /// Constructs a new `PubSubManager`.
    pub async fn new(
        connection_info: &ConnectionInfo,
        config: PubsubManagerConfig,
    ) -> RedisResult<Self> {
        let (sink_sender, sink_receiver) = unbounded_channel();
        let (stream_sender, stream_receiver) = unbounded_channel();
        let (setup_complete_sender, setup_complete_receiver) = oneshot::channel();
        let client = Client::open(connection_info.clone())?;
        let task_handle = Runtime::locate().spawn(start_listening(
            sink_receiver,
            stream_sender,
            client,
            setup_complete_sender,
            config,
        ));
        setup_complete_receiver.await.map_err(|_| {
            RedisError::from((crate::ErrorKind::ClientError, "Failed to create pubsub"))
        })?;
        Ok(Self {
            stream: PubSubManagerStream {
                stream: stream_receiver,
                _task_handle: Some(SharedHandleContainer::new(task_handle)),
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
    mut sink_receiver: UnboundedReceiver<SinkRequest>,
    mut stream_sender: tokio::sync::mpsc::UnboundedSender<Msg>,
    client: Client,
    ready_sender: oneshot::Sender<()>,
    config: PubsubManagerConfig,
) {
    let mut ready_sender = Some(ready_sender);
    let mut subscription_tracker = SubscriptionTracker::default();

    let mut retry_strategy =
        ExponentialBackoff::from_millis(config.exponent_base).factor(config.factor);
    if let Some(max_delay) = config.max_delay {
        retry_strategy = retry_strategy.max_delay(std::time::Duration::from_millis(max_delay));
    }
    let retry_strategy = retry_strategy.map(jitter).take(config.number_of_retries);
    loop {
        let Ok((sink, stream)) = Retry::spawn(retry_strategy.clone(), || async {
            match Runtime::locate()
                .timeout(config.connection_timeout, client.get_async_pubsub())
                .await
            {
                Ok(Ok(pubsub)) => Ok(pubsub),
                Ok(Err(e)) => Err(e),
                Err(elapsed) => Err(elapsed.into()),
            }
            .map(|pubsub| pubsub.split())
            .map_transient_err()
        })
        .await
        else {
            continue;
        };
        if let Some(sender) = ready_sender.take() {
            let _ = sender.send(());
        } else {
            let pipeline = subscription_tracker.get_subscription_pipeline();
            let requests = pipeline.cmd_iter().map(|cmd| {
                let mut sink = sink.clone();
                let packed_cmd = cmd.get_packed_command();
                async move { sink.send_recv(packed_cmd).await }
            });
            // TODO - should we handle errors? how?
            join_all(requests).await;
        }

        // We don't want the sink future's completion to kill the connection,
        // because the stream might be used after the sink is dropped
        let sink_future = pin!(
            sink_handler(&mut sink_receiver, &mut subscription_tracker, sink)
                .then(|_| std::future::pending::<()>())
                .fuse()
        );
        let stream_future = pin!(stream_handler(&mut stream_sender, stream).fuse());
        // dropping the sink will cause the sink future to stop, but the
        // stream future should be independent and keep running.
        match select(sink_future, stream_future).await {
            futures::future::Either::Left((_, stream_future)) => stream_future.await,
            futures::future::Either::Right(_) => {}
        }
    }
}

async fn sink_handler(
    sink_receiver: &mut UnboundedReceiver<SinkRequest>,
    subscription_tracker: &mut SubscriptionTracker,
    mut sink: super::PubSubSink,
) {
    while let Some(request) = sink_receiver.recv().await {
        let response = match request.request_type {
            SinkRequestType::Subscribe => sink.subscribe(&request.arguments).await,
            SinkRequestType::Unsubscribe => sink.unsubscribe(&request.arguments).await,
            SinkRequestType::PSubscribe => sink.psubscribe(&request.arguments).await,
            SinkRequestType::PUnsubscribe => sink.punsubscribe(&request.arguments).await,
        };
        let mut should_close = false;
        if let Err(err) = &response {
            should_close = err.is_unrecoverable_error();
        } else {
            let action = match request.request_type {
                SinkRequestType::Subscribe => SubscriptionAction::Subscribe,
                SinkRequestType::Unsubscribe => SubscriptionAction::Unsubscribe,
                SinkRequestType::PSubscribe => SubscriptionAction::PSubscribe,
                SinkRequestType::PUnsubscribe => SubscriptionAction::PUnsubscribe,
            };
            subscription_tracker.update_with_request(action, request.arguments.into_iter());
        }
        let _ = request.response.send(response);
        if should_close {
            return;
        }
    }
}

async fn stream_handler(
    stream_sender: &mut tokio::sync::mpsc::UnboundedSender<Msg>,
    mut stream: super::PubSubStream,
) {
    while let Some(msg) = stream.next().await {
        if stream_sender.send(msg).is_err() {
            return;
        }
    }
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
        self.0
            .send(SinkRequest {
                request_type,
                arguments,
                response: sender,
            })
            .map_err(|_| closed_connection_error())?;
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
