#![cfg(any(feature = "cluster", feature = "cluster_async"))]
#![allow(missing_docs)]
use crate::{
    aio, cluster, ErrorKind, FromRedisValue, IntoConnectionInfo, PushKind, RedisError, RedisResult,
    Value,
};

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::OnceLock,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

#[cfg(feature = "cluster-async")]
use crate::{cluster_async, RedisFuture};

#[cfg(feature = "cluster-async")]
use futures::future;

pub type Handler = Arc<dyn Fn(&[u8], u16) -> Result<(), RedisResult<Value>> + Send + Sync>;

pub fn contains_slice(xs: &[u8], ys: &[u8]) -> bool {
    for i in 0..xs.len() {
        if xs[i..].starts_with(ys) {
            return true;
        }
    }
    false
}

pub fn respond_startup(name: &str, cmd: &[u8]) -> Result<(), RedisResult<Value>> {
    if contains_slice(cmd, b"PING") || contains_slice(cmd, b"SETNAME") {
        Err(Ok(Value::SimpleString("OK".into())))
    } else if contains_slice(cmd, b"CLUSTER") && contains_slice(cmd, b"SLOTS") {
        Err(Ok(Value::Array(vec![Value::Array(vec![
            Value::Int(0),
            Value::Int(16383),
            Value::Array(vec![
                Value::BulkString(name.as_bytes().to_vec()),
                Value::Int(6379),
            ]),
        ])])))
    } else if contains_slice(cmd, b"READONLY") {
        Err(Ok(Value::SimpleString("OK".into())))
    } else {
        Ok(())
    }
}

pub struct RemoveHandler(String);

impl Drop for RemoveHandler {
    fn drop(&mut self) {
        MockConnectionBehavior::remove_mock(&self.0);
    }
}

pub struct MockConnectionBehavior {
    pub id: String,
    pub handler: Handler,
    pub connection_id_provider: AtomicUsize,
    pub returned_ip_type: ConnectionIPReturnType,
    pub return_connection_err: ShouldReturnConnectionError,
}

impl MockConnectionBehavior {
    fn new(id: &str, handler: Handler) -> Self {
        Self {
            id: id.to_string(),
            handler,
            connection_id_provider: AtomicUsize::new(0),
            returned_ip_type: ConnectionIPReturnType::default(),
            return_connection_err: ShouldReturnConnectionError::default(),
        }
    }

    #[must_use]
    pub fn register_new(id: &str, handler: Handler) -> RemoveHandler {
        get_behaviors().insert(id.to_string(), Self::new(id, handler));
        RemoveHandler(id.to_string())
    }

    pub fn remove_mock(id: &str) {
        get_behaviors().remove(id);
    }

    fn get_handler(&self) -> Handler {
        self.handler.clone()
    }
}

pub fn modify_mock_connection_behavior(name: &str, func: impl FnOnce(&mut MockConnectionBehavior)) {
    func(
        get_behaviors()
            .get_mut(name)
            .expect("Handler `{name}` was not installed"),
    );
}

pub fn get_mock_connection_handler(name: &str) -> Handler {
    get_behaviors()
        .get(name)
        .expect("Handler `{name}` was not installed")
        .get_handler()
}

static MOCK_CONN_BEHAVIORS: OnceLock<RwLock<HashMap<String, MockConnectionBehavior>>> =
    OnceLock::new();

fn get_behaviors() -> std::sync::RwLockWriteGuard<'static, HashMap<String, MockConnectionBehavior>>
{
    MOCK_CONN_BEHAVIORS
        .get_or_init(Default::default)
        .write()
        .unwrap()
}

#[derive(Default)]
pub enum ConnectionIPReturnType {
    /// New connections' IP will be returned as None
    #[default]
    None,
    /// Creates connections with the specified IP
    Specified(IpAddr),
    /// Each new connection will be created with a different IP based on the passed atomic integer
    Different(AtomicUsize),
}

#[derive(Default)]
pub enum ShouldReturnConnectionError {
    /// Don't return a connection error
    #[default]
    No,
    /// Always return a connection error
    Yes,
    /// Return connection error when the internal index is an odd number
    OnOddIdx(AtomicUsize),
}

#[derive(Clone)]
pub struct MockConnection {
    pub id: usize,
    pub handler: Handler,
    pub port: u16,
}

#[cfg(feature = "cluster-async")]
impl cluster_async::Connect for MockConnection {
    fn connect<'a, T>(
        info: T,
        _response_timeout: Duration,
        _connection_timeout: Duration,
        _socket_addr: Option<SocketAddr>,
    ) -> RedisFuture<'a, (Self, Option<IpAddr>)>
    where
        T: IntoConnectionInfo + Send + 'a,
    {
        let info = info.into_connection_info().unwrap();

        let (name, port) = match &info.addr {
            crate::ConnectionAddr::Tcp(addr, port) => (addr, *port),
            _ => unreachable!(),
        };
        let binding = get_behaviors();
        let conn_utils = binding
            .get(name)
            .unwrap_or_else(|| panic!("MockConnectionUtils for `{name}` were not installed"));
        let conn_err = Box::pin(future::err(RedisError::from(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "mock-io-error",
        ))));
        match &conn_utils.return_connection_err {
            ShouldReturnConnectionError::No => {}
            ShouldReturnConnectionError::Yes => return conn_err,
            ShouldReturnConnectionError::OnOddIdx(curr_idx) => {
                if curr_idx.fetch_add(1, Ordering::SeqCst) % 2 != 0 {
                    // raise an error on each odd number
                    return conn_err;
                }
            }
        }

        let ip = match &conn_utils.returned_ip_type {
            ConnectionIPReturnType::Specified(ip) => Some(*ip),
            ConnectionIPReturnType::Different(ip_getter) => {
                let first_ip_num = ip_getter.fetch_add(1, Ordering::SeqCst) as u8;
                Some(IpAddr::V4(Ipv4Addr::new(first_ip_num, 0, 0, 0)))
            }
            ConnectionIPReturnType::None => None,
        };

        Box::pin(future::ok((
            MockConnection {
                id: conn_utils
                    .connection_id_provider
                    .fetch_add(1, Ordering::SeqCst),
                handler: conn_utils.get_handler(),
                port,
            },
            ip,
        )))
    }
}

impl cluster::Connect for MockConnection {
    fn connect<'a, T>(info: T, _timeout: Option<Duration>) -> RedisResult<Self>
    where
        T: IntoConnectionInfo,
    {
        let info = info.into_connection_info().unwrap();

        let (name, port) = match &info.addr {
            crate::ConnectionAddr::Tcp(addr, port) => (addr, *port),
            _ => unreachable!(),
        };
        let binding = get_behaviors();
        let conn_utils = binding
            .get(name)
            .unwrap_or_else(|| panic!("MockConnectionUtils for `{name}` were not installed"));
        Ok(MockConnection {
            id: conn_utils
                .connection_id_provider
                .fetch_add(1, Ordering::SeqCst),
            handler: conn_utils.get_handler(),
            port,
        })
    }

    fn send_packed_command(&mut self, _cmd: &[u8]) -> RedisResult<()> {
        Ok(())
    }

    fn set_write_timeout(&self, _dur: Option<std::time::Duration>) -> RedisResult<()> {
        Ok(())
    }

    fn set_read_timeout(&self, _dur: Option<std::time::Duration>) -> RedisResult<()> {
        Ok(())
    }

    fn recv_response(&mut self) -> RedisResult<Value> {
        Ok(Value::Nil)
    }
}

#[cfg(feature = "cluster-async")]
impl aio::ConnectionLike for MockConnection {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a crate::Cmd) -> RedisFuture<'a, Value> {
        Box::pin(future::ready(
            (self.handler)(&cmd.get_packed_command(), self.port)
                .expect_err("Handler did not specify a response"),
        ))
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        _pipeline: &'a crate::Pipeline,
        _offset: usize,
        _count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        Box::pin(future::ok(vec![]))
    }

    fn get_db(&self) -> i64 {
        0
    }
}

impl crate::ConnectionLike for MockConnection {
    fn req_packed_command(&mut self, cmd: &[u8]) -> RedisResult<Value> {
        (self.handler)(cmd, self.port).expect_err("Handler did not specify a response")
    }

    fn req_packed_commands(
        &mut self,
        cmd: &[u8],
        offset: usize,
        _count: usize,
    ) -> RedisResult<Vec<Value>> {
        let res = (self.handler)(cmd, self.port).expect_err("Handler did not specify a response");
        match res {
            Err(err) => Err(err),
            Ok(res) => {
                if let Value::Array(results) = res {
                    match results.into_iter().nth(offset) {
                        Some(Value::Array(res)) => Ok(res),
                        _ => Err((ErrorKind::ResponseError, "non-array response").into()),
                    }
                } else {
                    Err((
                        ErrorKind::ResponseError,
                        "non-array response",
                        String::from_redis_value(&res).unwrap(),
                    )
                        .into())
                }
            }
        }
    }

    fn get_db(&self) -> i64 {
        0
    }

    fn check_connection(&mut self) -> bool {
        true
    }

    fn is_open(&self) -> bool {
        true
    }

    fn execute_push_message(&mut self, _kind: PushKind, _data: Vec<Value>) {
        // TODO - implement handling RESP3 push messages
    }
}
