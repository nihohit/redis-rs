use crate::cluster_topology::get_slot;
use crate::cmd::{Arg, Cmd};
use crate::types::Value;
use crate::{ErrorKind, KnownCommand, RedisResult};
use std::cmp::min;
use std::collections::HashMap;
use std::iter::{Iterator, Once};

#[derive(Clone)]
pub(crate) enum Redirect {
    Moved(String),
    Ask(String),
}

/// Logical bitwise aggregating operators.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicalAggregateOp {
    /// Aggregate by bitwise &&
    And,
    // Or, omitted due to dead code warnings. ATM this value isn't constructed anywhere
}

/// Numerical aggreagting operators.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AggregateOp {
    /// Choose minimal value
    Min,
    /// Sum all values
    Sum,
    // Max, omitted due to dead code warnings. ATM this value isn't constructed anywhere
}

/// Policy defining how to combine multiple responses into one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponsePolicy {
    /// Wait for one request to succeed and return its results. Return error if all requests fail.
    OneSucceeded,
    /// Wait for one request to succeed with a non-empty value. Return error if all requests fail or return `Nil`.
    OneSucceededNonEmpty,
    /// Waits for all requests to succeed, and the returns one of the successes. Returns the error on the first received error.
    AllSucceeded,
    /// Aggregate success results according to a logical bitwise operator. Return error on any failed request or on a response that doesn't conform to 0 or 1.
    AggregateLogical(LogicalAggregateOp),
    /// Aggregate success results according to a numeric operator. Return error on any failed request or on a response that isn't an integer.
    Aggregate(AggregateOp),
    /// Aggregate array responses into a single array. Return error on any failed request or on a response that isn't an array.
    CombineArrays,
    /// Handling is not defined by the Redis standard. Will receive a special case
    Special,
}

/// Defines whether a request should be routed to a single node, or multiple ones.
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingInfo {
    /// Route to single node
    SingleNode(SingleNodeRoutingInfo),
    /// Route to multiple nodes
    MultiNode((MultipleNodeRoutingInfo, Option<ResponsePolicy>)),
}

/// Defines which single node should receive a request.
#[derive(Debug, Clone, PartialEq)]
pub enum SingleNodeRoutingInfo {
    /// Route to any node at random
    Random,
    /// Route to the node that matches the [route]
    SpecificNode(Route),
}

impl From<Option<Route>> for SingleNodeRoutingInfo {
    fn from(value: Option<Route>) -> Self {
        value
            .map(SingleNodeRoutingInfo::SpecificNode)
            .unwrap_or(SingleNodeRoutingInfo::Random)
    }
}

/// Defines which collection of nodes should receive a request
#[derive(Debug, Clone, PartialEq)]
pub enum MultipleNodeRoutingInfo {
    /// Route to all nodes in the clusters
    AllNodes,
    /// Route to all primaries in the cluster
    AllMasters,
    /// Instructions for how to split a multi-slot command (e.g. MGET, MSET) into sub-commands. Each tuple is the route for each subcommand, and the indices of the arguments from the original command that should be copied to the subcommand.
    MultiSlot(Vec<(Route, Vec<usize>)>),
}

/// Takes a routable and an iterator of indices, which is assued to be created from`MultipleNodeRoutingInfo::MultiSlot`,
/// and returns a command with the arguments matching the indices.
pub fn command_for_multi_slot_indices<'a>(
    original_cmd: &'a impl Routable,
    indices: impl Iterator<Item = &'a usize> + 'a,
) -> Cmd {
    let mut new_cmd = Cmd::new();
    let command_length = 1; // TODO - the +1 should change if we have multi-slot commands with 2 command words.
    new_cmd.arg(original_cmd.arg_idx(0));
    for index in indices {
        new_cmd.arg(original_cmd.arg_idx(index + command_length));
    }
    new_cmd
}

/// Aggreagte numeric responses.
pub fn aggregate(values: Vec<Value>, op: AggregateOp) -> RedisResult<Value> {
    let initial_value = match op {
        AggregateOp::Min => i64::MAX,
        AggregateOp::Sum => 0,
    };
    let result = values.into_iter().try_fold(initial_value, |acc, curr| {
        let int = match curr {
            Value::Int(int) => int,
            _ => {
                return RedisResult::Err(
                    (
                        ErrorKind::TypeError,
                        "expected array of integers as response",
                    )
                        .into(),
                );
            }
        };
        let acc = match op {
            AggregateOp::Min => min(acc, int),
            AggregateOp::Sum => acc + int,
        };
        Ok(acc)
    })?;
    Ok(Value::Int(result))
}

/// Aggreagte numeric responses by a boolean operator.
pub fn logical_aggregate(values: Vec<Value>, op: LogicalAggregateOp) -> RedisResult<Value> {
    let initial_value = match op {
        LogicalAggregateOp::And => true,
    };
    let results = values.into_iter().try_fold(Vec::new(), |acc, curr| {
        let values = match curr {
            Value::Array(values) => values,
            _ => {
                return RedisResult::Err(
                    (
                        ErrorKind::TypeError,
                        "expected array of integers as response",
                    )
                        .into(),
                );
            }
        };
        let mut acc = if acc.is_empty() {
            vec![initial_value; values.len()]
        } else {
            acc
        };
        for (index, value) in values.into_iter().enumerate() {
            let int = match value {
                Value::Int(int) => int,
                _ => {
                    return Err((
                        ErrorKind::TypeError,
                        "expected array of integers as response",
                    )
                        .into());
                }
            };
            acc[index] = match op {
                LogicalAggregateOp::And => acc[index] && (int > 0),
            };
        }
        Ok(acc)
    })?;
    Ok(Value::Array(
        results
            .into_iter()
            .map(|result| Value::Int(result as i64))
            .collect(),
    ))
}

/// Aggreagte arrau responses into a single array.
pub fn combine_array_results(values: Vec<Value>) -> RedisResult<Value> {
    let mut results = Vec::new();

    for value in values {
        match value {
            Value::Array(values) => results.extend(values),
            _ => {
                return Err((ErrorKind::TypeError, "expected array of values as response").into());
            }
        }
    }

    Ok(Value::Array(results))
}

/// Combines multiple call results in the `values` field, each assume to be an array of results,
/// into a single array. `sorting_order` defines the order of the results in the returned array -
/// for each array of results, `sorting_order` should contain a matching array with the indices of
/// the results in the final array.
pub(crate) fn combine_and_sort_array_results<'a>(
    values: Vec<Value>,
    sorting_order: impl Iterator<Item = &'a Vec<usize>> + ExactSizeIterator,
) -> RedisResult<Value> {
    let mut results = Vec::new();
    results.resize(
        values.iter().fold(0, |acc, value| match value {
            Value::Array(values) => values.len() + acc,
            _ => 0,
        }),
        Value::Nil,
    );
    assert_eq!(values.len(), sorting_order.len());

    for (key_indices, value) in sorting_order.into_iter().zip(values) {
        match value {
            Value::Array(values) => {
                assert_eq!(values.len(), key_indices.len());
                for (index, value) in key_indices.iter().zip(values) {
                    results[*index] = value;
                }
            }
            _ => {
                return Err((ErrorKind::TypeError, "expected array of values as response").into());
            }
        }
    }

    Ok(Value::Array(results))
}

pub(crate) fn get_route(is_readonly: bool, key: &[u8]) -> Route {
    let slot = get_slot(key);
    if is_readonly {
        Route::new(slot, SlotAddr::ReplicaOptional)
    } else {
        Route::new(slot, SlotAddr::Master)
    }
}

/// Takes the given `routable` and creates a multi-slot routing info.
/// This is used for commands like MSET & MGET, where if the command's keys
/// are hashed to multiple slots, the command should be split into sub-commands,
/// each targetting a single slot. The results of these sub-commands are then
/// usually reassembled using `combine_and_sort_array_results`. In order to do this,
/// `MultipleNodeRoutingInfo::MultiSlot` contains the routes for each sub-command, and
/// the indices in the final combined result for each result from the sub-command.
///
/// If all keys are routed to the same slot, there's no need to split the command,
/// so a single node routing info will be returned.
pub(crate) fn multi_shard<R>(
    routable: &R,
    is_readonly: bool,
    response_policy: Option<ResponsePolicy>,
    first_key_index: usize,
    has_values: bool,
) -> Option<RoutingInfo>
where
    R: Routable + ?Sized,
{
    let mut routes = HashMap::new();
    let mut key_index = 0;
    while let Some(key) = routable.arg_idx(first_key_index + key_index) {
        let route = get_route(is_readonly, key);
        let entry = routes.entry(route);
        let keys = entry.or_insert(Vec::new());
        keys.push(key_index);

        if has_values {
            key_index += 1;
            routable.arg_idx(first_key_index + key_index)?; // check that there's a value for the key
            keys.push(key_index);
        }
        key_index += 1;
    }

    let mut routes: Vec<(Route, Vec<usize>)> = routes.into_iter().collect();
    Some(if routes.len() == 1 {
        RoutingInfo::SingleNode(SingleNodeRoutingInfo::SpecificNode(routes.pop().unwrap().0))
    } else {
        RoutingInfo::MultiNode((MultipleNodeRoutingInfo::MultiSlot(routes), response_policy))
    })
}

impl ResponsePolicy {
    /// Parse the command for the matching response policy.
    pub fn for_command(cmd: &[u8]) -> Option<ResponsePolicy> {
        match cmd {
            b"SCRIPT EXISTS" => Some(ResponsePolicy::AggregateLogical(LogicalAggregateOp::And)),

            b"DBSIZE" | b"DEL" | b"EXISTS" | b"SLOWLOG LEN" | b"TOUCH" | b"UNLINK" => {
                Some(ResponsePolicy::Aggregate(AggregateOp::Sum))
            }

            b"WAIT" => Some(ResponsePolicy::Aggregate(AggregateOp::Min)),

            b"ACL SETUSER" | b"ACL DELUSER" | b"ACL SAVE" | b"CLIENT SETNAME"
            | b"CLIENT SETINFO" | b"CONFIG SET" | b"CONFIG RESETSTAT" | b"CONFIG REWRITE"
            | b"FLUSHALL" | b"FLUSHDB" | b"FUNCTION DELETE" | b"FUNCTION FLUSH"
            | b"FUNCTION LOAD" | b"FUNCTION RESTORE" | b"MEMORY PURGE" | b"MSET" | b"PING"
            | b"SCRIPT FLUSH" | b"SCRIPT LOAD" | b"SLOWLOG RESET" => {
                Some(ResponsePolicy::AllSucceeded)
            }

            b"KEYS" | b"MGET" | b"SLOWLOG GET" => Some(ResponsePolicy::CombineArrays),

            b"FUNCTION KILL" | b"SCRIPT KILL" => Some(ResponsePolicy::OneSucceeded),

            // This isn't based on response_tips, but on the discussion here - https://github.com/redis/redis/issues/12410
            b"RANDOMKEY" => Some(ResponsePolicy::OneSucceededNonEmpty),

            b"LATENCY GRAPH" | b"LATENCY HISTOGRAM" | b"LATENCY HISTORY" | b"LATENCY DOCTOR"
            | b"LATENCY LATEST" => Some(ResponsePolicy::Special),

            b"FUNCTION STATS" => Some(ResponsePolicy::Special),

            b"MEMORY MALLOC-STATS" | b"MEMORY DOCTOR" | b"MEMORY STATS" => {
                Some(ResponsePolicy::Special)
            }

            b"INFO" => Some(ResponsePolicy::Special),

            _ => None,
        }
    }
}

impl RoutingInfo {
    /// Returns true if the `cmd`` should be routed to all nodes.
    pub fn is_all_nodes(cmd: &[u8]) -> bool {
        matches!(
            cmd,
            b"ACL SETUSER"
                | b"ACL DELUSER"
                | b"ACL SAVE"
                | b"CLIENT SETNAME"
                | b"CLIENT SETINFO"
                | b"SLOWLOG GET"
                | b"SLOWLOG LEN"
                | b"SLOWLOG RESET"
                | b"CONFIG SET"
                | b"CONFIG RESETSTAT"
                | b"CONFIG REWRITE"
                | b"SCRIPT FLUSH"
                | b"SCRIPT LOAD"
                | b"LATENCY RESET"
                | b"LATENCY GRAPH"
                | b"LATENCY HISTOGRAM"
                | b"LATENCY HISTORY"
                | b"LATENCY DOCTOR"
                | b"LATENCY LATEST"
        )
    }

    /// Returns the routing info for `r`.
    pub fn for_routable<R>(r: &R) -> Option<RoutingInfo>
    where
        R: Routable + ?Sized,
    {
        if let Some(known_command) = r.known_command() {
            return known_command.routing_info(r);
        }

        let cmd = &r.command()?[..];
        if Self::is_all_nodes(cmd) {
            return Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllNodes,
                ResponsePolicy::for_command(cmd),
            )));
        }
        match cmd {
            b"RANDOMKEY"
            | b"KEYS"
            | b"SCRIPT EXISTS"
            | b"WAIT"
            | b"DBSIZE"
            | b"FLUSHALL"
            | b"FUNCTION RESTORE"
            | b"FUNCTION DELETE"
            | b"FUNCTION FLUSH"
            | b"FUNCTION LOAD"
            | b"PING"
            | b"FLUSHDB"
            | b"MEMORY PURGE"
            | b"FUNCTION KILL"
            | b"SCRIPT KILL"
            | b"FUNCTION STATS"
            | b"MEMORY MALLOC-STATS"
            | b"MEMORY DOCTOR"
            | b"MEMORY STATS"
            | b"INFO" => Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllMasters,
                ResponsePolicy::for_command(cmd),
            ))),

            b"MGET" | b"DEL" | b"EXISTS" | b"UNLINK" | b"TOUCH" => multi_shard(
                r,
                is_readonly_cmd(cmd),
                ResponsePolicy::for_command(cmd),
                1,
                false,
            ),
            b"MSET" => multi_shard(
                r,
                is_readonly_cmd(cmd),
                ResponsePolicy::for_command(cmd),
                1,
                true,
            ),
            // TODO - special handling - b"SCAN"
            b"SCAN" | b"SHUTDOWN" | b"SLAVEOF" | b"REPLICAOF" | b"MOVE" | b"BITOP" => None,
            b"EVALSHA" | b"EVAL" => {
                let key_count = r
                    .arg_idx(2)
                    .and_then(|x| std::str::from_utf8(x).ok())
                    .and_then(|x| x.parse::<u64>().ok())?;
                if key_count == 0 {
                    Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random))
                } else {
                    r.arg_idx(3).map(|key| RoutingInfo::for_key(cmd, key))
                }
            }
            b"XGROUP CREATE"
            | b"XGROUP CREATECONSUMER"
            | b"XGROUP DELCONSUMER"
            | b"XGROUP DESTROY"
            | b"XGROUP SETID"
            | b"XINFO CONSUMERS"
            | b"XINFO GROUPS"
            | b"XINFO STREAM" => r.arg_idx(2).map(|key| RoutingInfo::for_key(cmd, key)),
            b"XREAD" | b"XREADGROUP" => {
                let streams_position = r.position(b"STREAMS")?;
                r.arg_idx(streams_position + 1)
                    .map(|key| RoutingInfo::for_key(cmd, key))
            }
            _ => match r.arg_idx(1) {
                Some(key) => Some(RoutingInfo::for_key(cmd, key)),
                None => Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random)),
            },
        }
    }

    fn for_key(cmd: &[u8], key: &[u8]) -> RoutingInfo {
        RoutingInfo::SingleNode(SingleNodeRoutingInfo::SpecificNode(get_route(
            is_readonly_cmd(cmd),
            key,
        )))
    }
}

/// Returns true if the given `routable` represents a readonly command.
pub fn is_readonly(routable: &impl Routable) -> bool {
    routable
        .known_command()
        .map(|cmd| cmd.is_readonly())
        .or(routable
            .command()
            .map(|cmd| is_readonly_cmd(cmd.as_slice())))
        .unwrap_or_default()
}

/// Returns `true` if the given `cmd` is a readonly command.
pub fn is_readonly_cmd(cmd: &[u8]) -> bool {
    matches!(
        cmd,
        b"BITCOUNT"
            | b"BITFIELD_RO"
            | b"BITPOS"
            | b"DBSIZE"
            | b"DUMP"
            | b"EVALSHA_RO"
            | b"EVAL_RO"
            | b"EXISTS"
            | b"EXPIRETIME"
            | b"FCALL_RO"
            | b"GEODIST"
            | b"GEOHASH"
            | b"GEOPOS"
            | b"GEORADIUSBYMEMBER_RO"
            | b"GEORADIUS_RO"
            | b"GEOSEARCH"
            | b"GET"
            | b"GETBIT"
            | b"GETRANGE"
            | b"HEXISTS"
            | b"HGET"
            | b"HGETALL"
            | b"HKEYS"
            | b"HLEN"
            | b"HMGET"
            | b"HRANDFIELD"
            | b"HSCAN"
            | b"HSTRLEN"
            | b"HVALS"
            | b"KEYS"
            | b"LCS"
            | b"LINDEX"
            | b"LLEN"
            | b"LOLWUT"
            | b"LPOS"
            | b"LRANGE"
            | b"MEMORY USAGE"
            | b"MGET"
            | b"OBJECT ENCODING"
            | b"OBJECT FREQ"
            | b"OBJECT IDLETIME"
            | b"OBJECT REFCOUNT"
            | b"PEXPIRETIME"
            | b"PFCOUNT"
            | b"PTTL"
            | b"RANDOMKEY"
            | b"SCAN"
            | b"SCARD"
            | b"SDIFF"
            | b"SINTER"
            | b"SINTERCARD"
            | b"SISMEMBER"
            | b"SMEMBERS"
            | b"SMISMEMBER"
            | b"SORT_RO"
            | b"SRANDMEMBER"
            | b"SSCAN"
            | b"STRLEN"
            | b"SUBSTR"
            | b"SUNION"
            | b"TOUCH"
            | b"TTL"
            | b"TYPE"
            | b"XINFO CONSUMERS"
            | b"XINFO GROUPS"
            | b"XINFO STREAM"
            | b"XLEN"
            | b"XPENDING"
            | b"XRANGE"
            | b"XREAD"
            | b"XREVRANGE"
            | b"ZCARD"
            | b"ZCOUNT"
            | b"ZDIFF"
            | b"ZINTER"
            | b"ZINTERCARD"
            | b"ZLEXCOUNT"
            | b"ZMSCORE"
            | b"ZRANDMEMBER"
            | b"ZRANGE"
            | b"ZRANGEBYLEX"
            | b"ZRANGEBYSCORE"
            | b"ZRANK"
            | b"ZREVRANGE"
            | b"ZREVRANGEBYLEX"
            | b"ZREVRANGEBYSCORE"
            | b"ZREVRANK"
            | b"ZSCAN"
            | b"ZSCORE"
            | b"ZUNION"
    )
}

/// Objects that implement this trait define a request that can be routed by a cluster client to different nodes in the cluster.
pub trait Routable {
    /// Convenience function to return ascii uppercase version of the
    /// the first argument (i.e., the command).
    fn command(&self) -> Option<Vec<u8>> {
        let primary_command = self.arg_idx(0).map(|x| x.to_ascii_uppercase())?;
        let mut primary_command = match primary_command.as_slice() {
            b"XGROUP" | b"OBJECT" | b"SLOWLOG" | b"FUNCTION" | b"MODULE" | b"COMMAND"
            | b"PUBSUB" | b"CONFIG" | b"MEMORY" | b"XINFO" | b"CLIENT" | b"ACL" | b"SCRIPT"
            | b"CLUSTER" | b"LATENCY" => primary_command,
            _ => {
                return Some(primary_command);
            }
        };

        let secondary_command = self.arg_idx(1).map(|x| x.to_ascii_uppercase());
        Some(match secondary_command {
            Some(cmd) => {
                primary_command.reserve(cmd.len() + 1);
                primary_command.extend(b" ");
                primary_command.extend(cmd);
                primary_command
            }
            None => primary_command,
        })
    }

    /// Returns a reference to the data for the argument at `idx`.
    fn arg_idx(&self, idx: usize) -> Option<&[u8]>;

    /// Returns index of argument that matches `candidate`, if it exists
    fn position(&self, candidate: &[u8]) -> Option<usize>;

    /// Returns an enum representing a known command.
    /// If a routable has a known command, it's assumed that its arguments don't contain the command.
    fn known_command(&self) -> Option<KnownCommand> {
        None
    }
}

impl Routable for Cmd {
    fn arg_idx(&self, idx: usize) -> Option<&[u8]> {
        self.arg_idx(idx)
    }

    fn position(&self, candidate: &[u8]) -> Option<usize> {
        self.args_iter().position(|a| match a {
            Arg::Simple(d) => d.eq_ignore_ascii_case(candidate),
            _ => false,
        })
    }

    fn known_command(&self) -> Option<KnownCommand> {
        self.known_command
    }
}

impl Routable for Value {
    fn arg_idx(&self, idx: usize) -> Option<&[u8]> {
        match self {
            Value::Array(args) => match args.get(idx) {
                Some(Value::BulkString(ref data)) => Some(&data[..]),
                _ => None,
            },
            _ => None,
        }
    }

    fn position(&self, candidate: &[u8]) -> Option<usize> {
        match self {
            Value::Array(args) => args.iter().position(|a| match a {
                Value::BulkString(d) => d.eq_ignore_ascii_case(candidate),
                _ => false,
            }),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Slot {
    start: u16,
    end: u16,
    master: String,
    replicas: Vec<String>,
}

impl Slot {
    pub fn new(s: u16, e: u16, m: String, r: Vec<String>) -> Self {
        Self {
            start: s,
            end: e,
            master: m,
            replicas: r,
        }
    }

    pub fn start(&self) -> u16 {
        self.start
    }

    pub fn end(&self) -> u16 {
        self.end
    }
}

/// What type of node should a request be routed to, assuming read from replica is enabled.
#[derive(Eq, PartialEq, Clone, Copy, Debug, Hash)]
pub enum SlotAddr {
    /// The request must be routed to primary node
    Master,
    /// The request may be routed to a replica node.
    /// For example, a GET command can be routed either to replica or primary.
    ReplicaOptional,
    /// The request must be routed to replica node, if one exists.
    /// For example, by user requested routing.
    ReplicaRequired,
}

/// This is just a simplified version of [`Slot`],
/// which stores only the master and [optional] replica
/// to avoid the need to choose a replica each time
/// a command is executed
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SlotAddrs {
    pub(crate) primary: String,
    pub(crate) replicas: Vec<String>,
}

impl SlotAddrs {
    pub(crate) fn new(primary: String, replicas: Vec<String>) -> Self {
        Self { primary, replicas }
    }

    pub(crate) fn from_slot(slot: Slot) -> Self {
        SlotAddrs::new(slot.master, slot.replicas)
    }
}

impl<'a> IntoIterator for &'a SlotAddrs {
    type Item = &'a String;
    type IntoIter = std::iter::Chain<Once<&'a String>, std::slice::Iter<'a, String>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.primary).chain(self.replicas.iter())
    }
}

/// Defines the slot and the [`SlotAddr`] to which
/// a command should be sent
#[derive(Eq, PartialEq, Clone, Copy, Debug, Hash)]
pub struct Route(u16, SlotAddr);

impl Route {
    /// Returns a new Route.
    pub fn new(slot: u16, slot_addr: SlotAddr) -> Self {
        Self(slot, slot_addr)
    }

    /// Returns the slot number of the route.
    pub fn slot(&self) -> u16 {
        self.0
    }

    /// Returns the slot address of the route.
    pub fn slot_addr(&self) -> SlotAddr {
        self.1
    }
}

#[cfg(test)]
mod tests {
    use super::{
        command_for_multi_slot_indices, AggregateOp, MultipleNodeRoutingInfo, ResponsePolicy,
        Route, RoutingInfo, SingleNodeRoutingInfo, SlotAddr,
    };
    use crate::{cluster_topology::slot, cmd, parser::parse_redis_value, Value};
    use core::panic;

    #[test]
    fn test_routing_info_mixed_capatalization() {
        let mut upper = cmd("XREAD");
        upper.arg("STREAMS").arg("foo").arg(0);

        let mut lower = cmd("xread");
        lower.arg("streams").arg("foo").arg(0);

        assert_eq!(
            RoutingInfo::for_routable(&upper).unwrap(),
            RoutingInfo::for_routable(&lower).unwrap()
        );

        let mut mixed = cmd("xReAd");
        mixed.arg("StReAmS").arg("foo").arg(0);

        assert_eq!(
            RoutingInfo::for_routable(&lower).unwrap(),
            RoutingInfo::for_routable(&mixed).unwrap()
        );
    }

    #[test]
    fn test_routing_info() {
        let mut test_cmds = vec![];

        // RoutingInfo::AllMasters
        let mut test_cmd = cmd("FLUSHALL");
        test_cmd.arg("");
        test_cmds.push(test_cmd);

        // RoutingInfo::AllNodes
        test_cmd = cmd("ECHO");
        test_cmd.arg("");
        test_cmds.push(test_cmd);

        // Routing key is 2nd arg ("42")
        test_cmd = cmd("SET");
        test_cmd.arg("42");
        test_cmds.push(test_cmd);

        // Routing key is 3rd arg ("FOOBAR")
        test_cmd = cmd("XINFO");
        test_cmd.arg("GROUPS").arg("FOOBAR");
        test_cmds.push(test_cmd);

        // Routing key is 3rd or 4th arg (3rd = "0" == RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random))
        test_cmd = cmd("EVAL");
        test_cmd.arg("FOO").arg("0").arg("BAR");
        test_cmds.push(test_cmd);

        // Routing key is 3rd or 4th arg (3rd != "0" == RoutingInfo::Slot)
        test_cmd = cmd("EVAL");
        test_cmd.arg("FOO").arg("4").arg("BAR");
        test_cmds.push(test_cmd);

        // Routing key position is variable, 3rd arg
        test_cmd = cmd("XREAD");
        test_cmd.arg("STREAMS").arg("4");
        test_cmds.push(test_cmd);

        // Routing key position is variable, 4th arg
        test_cmd = cmd("XREAD");
        test_cmd.arg("FOO").arg("STREAMS").arg("4");
        test_cmds.push(test_cmd);

        for cmd in test_cmds {
            let value = parse_redis_value(&cmd.get_packed_command()).unwrap();
            assert_eq!(
                RoutingInfo::for_routable(&value).unwrap(),
                RoutingInfo::for_routable(&cmd).unwrap(),
            );
        }

        // Assert expected RoutingInfo explicitly:

        for cmd in [cmd("FLUSHALL"), cmd("FLUSHDB"), cmd("PING")] {
            assert_eq!(
                RoutingInfo::for_routable(&cmd),
                Some(RoutingInfo::MultiNode((
                    MultipleNodeRoutingInfo::AllMasters,
                    Some(ResponsePolicy::AllSucceeded)
                )))
            );
        }

        assert_eq!(
            RoutingInfo::for_routable(&cmd("DBSIZE")),
            Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllMasters,
                Some(ResponsePolicy::Aggregate(AggregateOp::Sum))
            )))
        );

        assert_eq!(
            RoutingInfo::for_routable(&cmd("SCRIPT KILL")),
            Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllMasters,
                Some(ResponsePolicy::OneSucceeded)
            )))
        );

        assert_eq!(
            RoutingInfo::for_routable(&cmd("INFO")),
            Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllMasters,
                Some(ResponsePolicy::Special)
            )))
        );

        assert_eq!(
            RoutingInfo::for_routable(&cmd("KEYS")),
            Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllMasters,
                Some(ResponsePolicy::CombineArrays)
            )))
        );

        for cmd in vec![
            cmd("SCAN"),
            cmd("SHUTDOWN"),
            cmd("SLAVEOF"),
            cmd("REPLICAOF"),
            cmd("MOVE"),
            cmd("BITOP"),
        ] {
            assert_eq!(
                RoutingInfo::for_routable(&cmd),
                None,
                "{}",
                std::str::from_utf8(cmd.arg_idx(0).unwrap()).unwrap()
            );
        }

        for cmd in [
            cmd("EVAL").arg(r#"redis.call("PING");"#).arg(0),
            cmd("EVALSHA").arg(r#"redis.call("PING");"#).arg(0),
        ] {
            assert_eq!(
                RoutingInfo::for_routable(cmd),
                Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random))
            );
        }

        for (cmd, expected) in [
            (
                cmd("EVAL")
                    .arg(r#"redis.call("GET, KEYS[1]");"#)
                    .arg(1)
                    .arg("foo"),
                Some(RoutingInfo::SingleNode(
                    SingleNodeRoutingInfo::SpecificNode(Route::new(slot(b"foo"), SlotAddr::Master)),
                )),
            ),
            (
                cmd("XGROUP")
                    .arg("CREATE")
                    .arg("mystream")
                    .arg("workers")
                    .arg("$")
                    .arg("MKSTREAM"),
                Some(RoutingInfo::SingleNode(
                    SingleNodeRoutingInfo::SpecificNode(Route::new(
                        slot(b"mystream"),
                        SlotAddr::Master,
                    )),
                )),
            ),
            (
                cmd("XINFO").arg("GROUPS").arg("foo"),
                Some(RoutingInfo::SingleNode(
                    SingleNodeRoutingInfo::SpecificNode(Route::new(
                        slot(b"foo"),
                        SlotAddr::ReplicaOptional,
                    )),
                )),
            ),
            (
                cmd("XREADGROUP")
                    .arg("GROUP")
                    .arg("wkrs")
                    .arg("consmrs")
                    .arg("STREAMS")
                    .arg("mystream"),
                Some(RoutingInfo::SingleNode(
                    SingleNodeRoutingInfo::SpecificNode(Route::new(
                        slot(b"mystream"),
                        SlotAddr::Master,
                    )),
                )),
            ),
            (
                cmd("XREAD")
                    .arg("COUNT")
                    .arg("2")
                    .arg("STREAMS")
                    .arg("mystream")
                    .arg("writers")
                    .arg("0-0")
                    .arg("0-0"),
                Some(RoutingInfo::SingleNode(
                    SingleNodeRoutingInfo::SpecificNode(Route::new(
                        slot(b"mystream"),
                        SlotAddr::ReplicaOptional,
                    )),
                )),
            ),
        ] {
            assert_eq!(
                RoutingInfo::for_routable(cmd),
                expected,
                "{}",
                std::str::from_utf8(cmd.arg_idx(0).unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn test_slot_for_packed_cmd() {
        assert!(matches!(RoutingInfo::for_routable(&parse_redis_value(&[
                42, 50, 13, 10, 36, 54, 13, 10, 69, 88, 73, 83, 84, 83, 13, 10, 36, 49, 54, 13, 10,
                244, 93, 23, 40, 126, 127, 253, 33, 89, 47, 185, 204, 171, 249, 96, 139, 13, 10
            ]).unwrap()), Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::SpecificNode(Route(slot, SlotAddr::ReplicaOptional)))) if slot == 964));

        assert!(matches!(RoutingInfo::for_routable(&parse_redis_value(&[
                42, 54, 13, 10, 36, 51, 13, 10, 83, 69, 84, 13, 10, 36, 49, 54, 13, 10, 36, 241,
                197, 111, 180, 254, 5, 175, 143, 146, 171, 39, 172, 23, 164, 145, 13, 10, 36, 52,
                13, 10, 116, 114, 117, 101, 13, 10, 36, 50, 13, 10, 78, 88, 13, 10, 36, 50, 13, 10,
                80, 88, 13, 10, 36, 55, 13, 10, 49, 56, 48, 48, 48, 48, 48, 13, 10
            ]).unwrap()), Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::SpecificNode(Route(slot, SlotAddr::Master)))) if slot == 8352));

        assert!(matches!(RoutingInfo::for_routable(&parse_redis_value(&[
                42, 54, 13, 10, 36, 51, 13, 10, 83, 69, 84, 13, 10, 36, 49, 54, 13, 10, 169, 233,
                247, 59, 50, 247, 100, 232, 123, 140, 2, 101, 125, 221, 66, 170, 13, 10, 36, 52,
                13, 10, 116, 114, 117, 101, 13, 10, 36, 50, 13, 10, 78, 88, 13, 10, 36, 50, 13, 10,
                80, 88, 13, 10, 36, 55, 13, 10, 49, 56, 48, 48, 48, 48, 48, 13, 10
            ]).unwrap()), Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::SpecificNode(Route(slot, SlotAddr::Master)))) if slot == 5210));
    }

    #[test]
    fn test_multi_shard() {
        let mut cmd = cmd("DEL");
        cmd.arg("foo").arg("bar").arg("baz").arg("{bar}vaz");
        let routing = RoutingInfo::for_routable(&cmd);
        let mut expected = std::collections::HashMap::new();
        expected.insert(Route(4813, SlotAddr::Master), vec![2]);
        expected.insert(Route(5061, SlotAddr::Master), vec![1, 3]);
        expected.insert(Route(12182, SlotAddr::Master), vec![0]);

        assert!(
            matches!(routing.clone(), Some(RoutingInfo::MultiNode((MultipleNodeRoutingInfo::MultiSlot(vec), Some(ResponsePolicy::Aggregate(AggregateOp::Sum))))) if {
                let routes = vec.clone().into_iter().collect();
                expected == routes
            }),
            "{routing:?}"
        );

        let mut cmd = crate::cmd("MGET");
        cmd.arg("foo").arg("bar").arg("baz").arg("{bar}vaz");
        let routing = RoutingInfo::for_routable(&cmd);
        let mut expected = std::collections::HashMap::new();
        expected.insert(Route(4813, SlotAddr::ReplicaOptional), vec![2]);
        expected.insert(Route(5061, SlotAddr::ReplicaOptional), vec![1, 3]);
        expected.insert(Route(12182, SlotAddr::ReplicaOptional), vec![0]);

        assert!(
            matches!(routing.clone(), Some(RoutingInfo::MultiNode((MultipleNodeRoutingInfo::MultiSlot(vec), Some(ResponsePolicy::CombineArrays)))) if {
                let routes = vec.clone().into_iter().collect();
                expected ==routes
            }),
            "{routing:?}"
        );
    }

    #[test]
    fn test_command_creation_for_multi_shard() {
        let mut original_cmd = cmd("DEL");
        original_cmd
            .arg("foo")
            .arg("bar")
            .arg("baz")
            .arg("{bar}vaz");
        let routing = RoutingInfo::for_routable(&original_cmd);
        let expected = vec![vec![0], vec![1, 3], vec![2]];

        let mut indices: Vec<_> = match routing {
            Some(RoutingInfo::MultiNode((MultipleNodeRoutingInfo::MultiSlot(vec), _))) => {
                vec.into_iter().map(|(_, indices)| indices).collect()
            }
            _ => panic!("unexpected routing: {routing:?}"),
        };
        indices.sort_by(|prev, next| prev.iter().next().unwrap().cmp(next.iter().next().unwrap())); // sorting because the `for_routable` doesn't return values in a consistent order between runs.

        for (index, indices) in indices.into_iter().enumerate() {
            let cmd = command_for_multi_slot_indices(&original_cmd, indices.iter());
            let expected_indices = &expected[index];
            assert_eq!(original_cmd.arg_idx(0), cmd.arg_idx(0));
            for (index, target_index) in expected_indices.iter().enumerate() {
                let target_index = target_index + 1;
                assert_eq!(original_cmd.arg_idx(target_index), cmd.arg_idx(index + 1));
            }
        }
    }

    #[test]
    fn test_combine_multi_shard_to_single_node_when_all_keys_are_in_same_slot() {
        let mut cmd = cmd("DEL");
        cmd.arg("foo").arg("{foo}bar").arg("{foo}baz");
        let routing = RoutingInfo::for_routable(&cmd);

        assert!(
            matches!(
                routing,
                Some(RoutingInfo::SingleNode(
                    SingleNodeRoutingInfo::SpecificNode(Route(12182, SlotAddr::Master))
                ))
            ),
            "{routing:?}"
        );
    }

    #[test]
    fn test_combining_results_into_single_array() {
        let res1 = Value::Array(vec![Value::Nil, Value::Okay]);
        let res2 = Value::Array(vec![
            Value::BulkString("1".as_bytes().to_vec()),
            Value::BulkString("4".as_bytes().to_vec()),
        ]);
        let res3 = Value::Array(vec![Value::SimpleString("2".to_string()), Value::Int(3)]);
        let results = super::combine_and_sort_array_results(
            vec![res1, res2, res3],
            [vec![0, 5], vec![1, 4], vec![2, 3]].iter(),
        );

        assert_eq!(
            results.unwrap(),
            Value::Array(vec![
                Value::Nil,
                Value::BulkString("1".as_bytes().to_vec()),
                Value::SimpleString("2".to_string()),
                Value::Int(3),
                Value::BulkString("4".as_bytes().to_vec()),
                Value::Okay,
            ])
        );
    }
}
