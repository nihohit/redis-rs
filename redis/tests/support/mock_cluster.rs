#![cfg(test)]
use redis::{
    cluster::{ClusterClient, ClusterClientBuilder},
    testing::mock_connection::{
        contains_slice, MockConnection, MockConnectionBehavior, RemoveHandler,
    },
    RedisResult, Value,
};

use std::sync::Arc;

#[cfg(feature = "cluster-async")]
use tokio::runtime::Runtime;

#[derive(Clone, Debug)]
pub struct MockSlotRange {
    pub primary_port: u16,
    pub replica_ports: Vec<u16>,
    pub slot_range: std::ops::Range<u16>,
}

pub fn respond_startup_with_replica(name: &str, cmd: &[u8]) -> Result<(), RedisResult<Value>> {
    respond_startup_with_replica_using_config(name, cmd, None)
}

pub fn respond_startup_two_nodes(name: &str, cmd: &[u8]) -> Result<(), RedisResult<Value>> {
    respond_startup_with_replica_using_config(
        name,
        cmd,
        Some(vec![
            MockSlotRange {
                primary_port: 6379,
                replica_ports: vec![],
                slot_range: (0..8191),
            },
            MockSlotRange {
                primary_port: 6380,
                replica_ports: vec![],
                slot_range: (8192..16383),
            },
        ]),
    )
}

pub fn create_topology_from_config(name: &str, slots_config: Vec<MockSlotRange>) -> Value {
    let slots_vec = slots_config
        .into_iter()
        .map(|slot_config| {
            let mut config = vec![
                Value::Int(slot_config.slot_range.start as i64),
                Value::Int(slot_config.slot_range.end as i64),
                Value::Array(vec![
                    Value::BulkString(name.as_bytes().to_vec()),
                    Value::Int(slot_config.primary_port as i64),
                ]),
            ];
            config.extend(slot_config.replica_ports.into_iter().map(|replica_port| {
                Value::Array(vec![
                    Value::BulkString(name.as_bytes().to_vec()),
                    Value::Int(replica_port as i64),
                ])
            }));
            Value::Array(config)
        })
        .collect();
    Value::Array(slots_vec)
}

pub fn respond_startup_with_replica_using_config(
    name: &str,
    cmd: &[u8],
    slots_config: Option<Vec<MockSlotRange>>,
) -> Result<(), RedisResult<Value>> {
    let slots_config = slots_config.unwrap_or(vec![
        MockSlotRange {
            primary_port: 6379,
            replica_ports: vec![6380],
            slot_range: (0..8191),
        },
        MockSlotRange {
            primary_port: 6381,
            replica_ports: vec![6382],
            slot_range: (8192..16383),
        },
    ]);
    if contains_slice(cmd, b"PING") || contains_slice(cmd, b"SETNAME") {
        Err(Ok(Value::SimpleString("OK".into())))
    } else if contains_slice(cmd, b"CLUSTER") && contains_slice(cmd, b"SLOTS") {
        let slots = create_topology_from_config(name, slots_config);
        Err(Ok(slots))
    } else if contains_slice(cmd, b"READONLY") {
        Err(Ok(Value::SimpleString("OK".into())))
    } else {
        Ok(())
    }
}

pub struct MockEnv {
    #[cfg(feature = "cluster-async")]
    pub runtime: Runtime,
    pub client: redis::cluster::ClusterClient,
    pub connection: redis::cluster::ClusterConnection<MockConnection>,
    #[cfg(feature = "cluster-async")]
    pub async_connection: redis::cluster_async::ClusterConnection<MockConnection>,
    #[allow(unused)]
    pub handler: RemoveHandler,
}

impl MockEnv {
    pub fn new(
        id: &str,
        handler: impl Fn(&[u8], u16) -> Result<(), RedisResult<Value>> + Send + Sync + 'static,
    ) -> Self {
        Self::with_client_builder(
            ClusterClient::builder(vec![&*format!("redis://{id}")]),
            id,
            handler,
        )
    }

    pub fn with_client_builder(
        client_builder: ClusterClientBuilder,
        id: &str,
        handler: impl Fn(&[u8], u16) -> Result<(), RedisResult<Value>> + Send + Sync + 'static,
    ) -> Self {
        #[cfg(feature = "cluster-async")]
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();

        let id = id.to_string();
        let remove_handler = MockConnectionBehavior::register_new(
            &id,
            Arc::new(move |cmd, port| handler(cmd, port)),
        );
        let client = client_builder.build().unwrap();
        let connection = client.get_generic_connection().unwrap();
        #[cfg(feature = "cluster-async")]
        let async_connection = runtime
            .block_on(client.get_async_generic_connection())
            .unwrap();
        MockEnv {
            #[cfg(feature = "cluster-async")]
            runtime,
            client,
            connection,
            #[cfg(feature = "cluster-async")]
            async_connection,
            handler: remove_handler,
        }
    }
}
