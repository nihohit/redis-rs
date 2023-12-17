#[cfg(feature = "cluster")]
use crate::cluster_routing::{
    get_route, multi_shard, AggregateOp, LogicalAggregateOp, MultipleNodeRoutingInfo,
    ResponsePolicy, Routable, RoutingInfo, SingleNodeRoutingInfo,
};

#[derive(Clone, Copy)]
#[allow(missing_docs)]
pub enum KnownCommand {
    Acl,
    AclCat,
    AclDeluser,
    AclDryrun,
    AclGenpass,
    AclGetuser,
    AclHelp,
    AclList,
    AclLoad,
    AclLog,
    AclSave,
    AclSetuser,
    AclUsers,
    AclWhoami,
    Append,
    Asking,
    Auth,
    Bgrewriteaof,
    Bgsave,
    Bitcount,
    Bitfield,
    BitfieldRo,
    Bitop,
    Bitpos,
    Blmove,
    Blmpop,
    Blpop,
    Brpop,
    Brpoplpush,
    Bzmpop,
    Bzpopmax,
    Bzpopmin,
    Client,
    ClientCaching,
    ClientGetname,
    ClientGetredir,
    ClientHelp,
    ClientId,
    ClientInfo,
    ClientKill,
    ClientList,
    ClientNoEvict,
    ClientNoTouch,
    ClientPause,
    ClientReply,
    ClientSetinfo,
    ClientSetname,
    ClientTracking,
    ClientTrackinginfo,
    ClientUnblock,
    ClientUnpause,
    Cluster,
    ClusterAddslots,
    ClusterAddslotsrange,
    ClusterBumpepoch,
    ClusterCountFailureReports,
    ClusterCountkeysinslot,
    ClusterDelslots,
    ClusterDelslotsrange,
    ClusterFailover,
    ClusterFlushslots,
    ClusterForget,
    ClusterGetkeysinslot,
    ClusterHelp,
    ClusterInfo,
    ClusterKeyslot,
    ClusterLinks,
    ClusterMeet,
    ClusterMyid,
    ClusterMyshardid,
    ClusterNodes,
    ClusterReplicas,
    ClusterReplicate,
    ClusterReset,
    ClusterSaveconfig,
    ClusterSetConfigEpoch,
    ClusterSetslot,
    ClusterShards,
    ClusterSlaves,
    ClusterSlots,
    Command,
    CommandCount,
    CommandDocs,
    CommandGetkeys,
    CommandGetkeysandflags,
    CommandHelp,
    CommandInfo,
    CommandList,
    Config,
    ConfigGet,
    ConfigHelp,
    ConfigResetstat,
    ConfigRewrite,
    ConfigSet,
    Copy,
    Dbsize,
    Debug,
    Decr,
    Decrby,
    Del,
    Discard,
    Dump,
    Echo,
    Eval,
    Evalsha,
    EvalshaRo,
    EvalRo,
    Exec,
    Exists,
    Expire,
    Expireat,
    Expiretime,
    Failover,
    Fcall,
    FcallRo,
    Flushall,
    Flushdb,
    Function,
    FunctionDelete,
    FunctionDump,
    FunctionFlush,
    FunctionHelp,
    FunctionKill,
    FunctionList,
    FunctionLoad,
    FunctionRestore,
    FunctionStats,
    Geoadd,
    Geodist,
    Geohash,
    Geopos,
    Georadius,
    Georadiusbymember,
    GeoradiusbymemberRo,
    GeoradiusRo,
    Geosearch,
    Geosearchstore,
    Get,
    Getbit,
    Getdel,
    Getex,
    Getrange,
    Getset,
    Hdel,
    Hello,
    Hexists,
    Hget,
    Hgetall,
    Hincrby,
    Hincrbyfloat,
    Hkeys,
    Hlen,
    Hmget,
    Hmset,
    Hrandfield,
    Hscan,
    Hset,
    Hsetnx,
    Hstrlen,
    Hvals,
    Incr,
    Incrby,
    Incrbyfloat,
    Info,
    Keys,
    Lastsave,
    Latency,
    LatencyDoctor,
    LatencyGraph,
    LatencyHelp,
    LatencyHistogram,
    LatencyHistory,
    LatencyLatest,
    LatencyReset,
    Lcs,
    Lindex,
    Linsert,
    Llen,
    Lmove,
    Lmpop,
    Lolwut,
    Lpop,
    Lpos,
    Lpush,
    Lpushx,
    Lrange,
    Lrem,
    Lset,
    Ltrim,
    Memory,
    MemoryDoctor,
    MemoryHelp,
    MemoryMallocStats,
    MemoryPurge,
    MemoryStats,
    MemoryUsage,
    Mget,
    Migrate,
    Module,
    ModuleHelp,
    ModuleList,
    ModuleLoad,
    ModuleLoadex,
    ModuleUnload,
    Monitor,
    Move,
    Mset,
    Msetnx,
    Multi,
    Object,
    ObjectEncoding,
    ObjectFreq,
    ObjectHelp,
    ObjectIdletime,
    ObjectRefcount,
    Persist,
    Pexpire,
    Pexpireat,
    Pexpiretime,
    Pfadd,
    Pfcount,
    Pfdebug,
    Pfmerge,
    Pfselftest,
    Ping,
    Psetex,
    Psubscribe,
    Psync,
    Pttl,
    Publish,
    Pubsub,
    PubsubChannels,
    PubsubHelp,
    PubsubNumpat,
    PubsubNumsub,
    PubsubShardchannels,
    PubsubShardnumsub,
    Punsubscribe,
    Quit,
    Randomkey,
    Readonly,
    Readwrite,
    Rename,
    Renamenx,
    Replconf,
    Replicaof,
    Reset,
    Restore,
    RestoreAsking,
    Role,
    Rpop,
    Rpoplpush,
    Rpush,
    Rpushx,
    Sadd,
    Save,
    Scan,
    Scard,
    Script,
    ScriptDebug,
    ScriptExists,
    ScriptFlush,
    ScriptHelp,
    ScriptKill,
    ScriptLoad,
    Sdiff,
    Sdiffstore,
    Select,
    Set,
    Setbit,
    Setex,
    Setnx,
    Setrange,
    Shutdown,
    Sinter,
    Sintercard,
    Sinterstore,
    Sismember,
    Slaveof,
    Slowlog,
    SlowlogGet,
    SlowlogHelp,
    SlowlogLen,
    SlowlogReset,
    Smembers,
    Smismember,
    Smove,
    Sort,
    SortRo,
    Spop,
    Spublish,
    Srandmember,
    Srem,
    Sscan,
    Ssubscribe,
    Strlen,
    Subscribe,
    Substr,
    Sunion,
    Sunionstore,
    Sunsubscribe,
    Swapdb,
    Sync,
    Time,
    Touch,
    Ttl,
    Type,
    Unlink,
    Unsubscribe,
    Unwatch,
    Wait,
    Waitaof,
    Watch,
    Xack,
    Xadd,
    Xautoclaim,
    Xclaim,
    Xdel,
    Xgroup,
    XgroupCreate,
    XgroupCreateconsumer,
    XgroupDelconsumer,
    XgroupDestroy,
    XgroupHelp,
    XgroupSetid,
    Xinfo,
    XinfoConsumers,
    XinfoGroups,
    XinfoHelp,
    XinfoStream,
    Xlen,
    Xpending,
    Xrange,
    Xread,
    Xreadgroup,
    Xrevrange,
    Xsetid,
    Xtrim,
    Zadd,
    Zcard,
    Zcount,
    Zdiff,
    Zdiffstore,
    Zincrby,
    Zinter,
    Zintercard,
    Zinterstore,
    Zlexcount,
    Zmpop,
    Zmscore,
    Zpopmax,
    Zpopmin,
    Zrandmember,
    Zrange,
    Zrangebylex,
    Zrangebyscore,
    Zrangestore,
    Zrank,
    Zrem,
    Zremrangebylex,
    Zremrangebyrank,
    Zremrangebyscore,
    Zrevrange,
    Zrevrangebylex,
    Zrevrangebyscore,
    Zrevrank,
    Zscan,
    Zscore,
    Zunion,
    Zunionstore,
}

impl KnownCommand {
    #[cfg(feature = "cluster")]
    ///
    pub fn response_policy(&self) -> Option<ResponsePolicy> {
        match self {
            KnownCommand::ScriptExists => {
                Some(ResponsePolicy::AggregateLogical(LogicalAggregateOp::And))
            }

            KnownCommand::Dbsize
            | KnownCommand::Del
            | KnownCommand::Exists
            | KnownCommand::SlowlogLen
            | KnownCommand::Touch
            | KnownCommand::Unlink => Some(ResponsePolicy::Aggregate(AggregateOp::Sum)),

            KnownCommand::Wait => Some(ResponsePolicy::Aggregate(AggregateOp::Min)),

            KnownCommand::AclSetuser
            | KnownCommand::AclDeluser
            | KnownCommand::AclSave
            | KnownCommand::ClientSetname
            | KnownCommand::ClientSetinfo
            | KnownCommand::ConfigSet
            | KnownCommand::ConfigResetstat
            | KnownCommand::ConfigRewrite
            | KnownCommand::Flushall
            | KnownCommand::Flushdb
            | KnownCommand::FunctionDelete
            | KnownCommand::FunctionFlush
            | KnownCommand::FunctionLoad
            | KnownCommand::FunctionRestore
            | KnownCommand::MemoryPurge
            | KnownCommand::Mset
            | KnownCommand::Ping
            | KnownCommand::ScriptFlush
            | KnownCommand::ScriptLoad
            | KnownCommand::SlowlogReset => Some(ResponsePolicy::AllSucceeded),

            KnownCommand::Keys | KnownCommand::Mget | KnownCommand::SlowlogGet => {
                Some(ResponsePolicy::CombineArrays)
            }

            KnownCommand::FunctionKill | KnownCommand::ScriptKill => {
                Some(ResponsePolicy::OneSucceeded)
            }

            // This isn't based on response_tips, but on the discussion here - https://github.com/redis/redis/issues/12410
            KnownCommand::Randomkey => Some(ResponsePolicy::OneSucceededNonEmpty),

            KnownCommand::LatencyGraph
            | KnownCommand::LatencyHistogram
            | KnownCommand::LatencyHistory
            | KnownCommand::LatencyDoctor
            | KnownCommand::LatencyLatest => Some(ResponsePolicy::Special),

            KnownCommand::FunctionStats => Some(ResponsePolicy::Special),

            KnownCommand::MemoryMallocStats
            | KnownCommand::MemoryDoctor
            | KnownCommand::MemoryStats => Some(ResponsePolicy::Special),

            KnownCommand::Info => Some(ResponsePolicy::Special),

            _ => None,
        }
    }

    ///
    pub fn is_all_nodes(&self) -> bool {
        matches!(
            self,
            KnownCommand::AclSetuser
                | KnownCommand::AclDeluser
                | KnownCommand::AclSave
                | KnownCommand::ClientSetname
                | KnownCommand::ClientSetinfo
                | KnownCommand::SlowlogGet
                | KnownCommand::SlowlogLen
                | KnownCommand::SlowlogReset
                | KnownCommand::ConfigSet
                | KnownCommand::ConfigResetstat
                | KnownCommand::ConfigRewrite
                | KnownCommand::ScriptFlush
                | KnownCommand::ScriptLoad
                | KnownCommand::LatencyReset
                | KnownCommand::LatencyGraph
                | KnownCommand::LatencyHistogram
                | KnownCommand::LatencyHistory
                | KnownCommand::LatencyDoctor
                | KnownCommand::LatencyLatest
        )
    }

    #[cfg(feature = "cluster")]
    ///
    pub fn routing_info(&self, r: &(impl Routable + ?Sized)) -> Option<RoutingInfo> {
        if Self::is_all_nodes(self) {
            return Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllNodes,
                self.response_policy(),
            )));
        }
        match self {
            KnownCommand::Randomkey
            | KnownCommand::Keys
            | KnownCommand::ScriptExists
            | KnownCommand::Wait
            | KnownCommand::Dbsize
            | KnownCommand::Flushall
            | KnownCommand::FunctionRestore
            | KnownCommand::FunctionDelete
            | KnownCommand::FunctionFlush
            | KnownCommand::FunctionLoad
            | KnownCommand::Ping
            | KnownCommand::Flushdb
            | KnownCommand::MemoryPurge
            | KnownCommand::FunctionKill
            | KnownCommand::ScriptKill
            | KnownCommand::FunctionStats
            | KnownCommand::MemoryMallocStats
            | KnownCommand::MemoryDoctor
            | KnownCommand::MemoryStats
            | KnownCommand::Info => Some(RoutingInfo::MultiNode((
                MultipleNodeRoutingInfo::AllMasters,
                self.response_policy(),
            ))),

            KnownCommand::Mget
            | KnownCommand::Del
            | KnownCommand::Exists
            | KnownCommand::Unlink
            | KnownCommand::Touch => {
                multi_shard(r, self.is_readonly(), self.response_policy(), 1, false)
            }
            KnownCommand::Mset => {
                multi_shard(r, self.is_readonly(), self.response_policy(), 1, true)
            }
            // TODO - special handling - b"SCAN"
            KnownCommand::Scan
            | KnownCommand::Shutdown
            | KnownCommand::Slaveof
            | KnownCommand::Replicaof
            | KnownCommand::Move
            | KnownCommand::Bitop => None,
            KnownCommand::Evalsha | KnownCommand::Eval => {
                let key_count = r
                    .arg_idx(2)
                    .and_then(|x| std::str::from_utf8(x).ok())
                    .and_then(|x| x.parse::<u64>().ok())?;
                if key_count == 0 {
                    Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random))
                } else {
                    r.arg_idx(3).map(|key| self.routing_info_for_key(key))
                }
            }
            KnownCommand::XgroupCreate
            | KnownCommand::XgroupCreateconsumer
            | KnownCommand::XgroupDelconsumer
            | KnownCommand::XgroupDestroy
            | KnownCommand::XgroupSetid
            | KnownCommand::XinfoConsumers
            | KnownCommand::XinfoGroups
            | KnownCommand::XinfoStream => r.arg_idx(2).map(|key| self.routing_info_for_key(key)),
            KnownCommand::Xread | KnownCommand::Xreadgroup => {
                let streams_position = r.position(b"STREAMS")?;
                r.arg_idx(streams_position + 1)
                    .map(|key| self.routing_info_for_key(key))
            }
            _ => match r.arg_idx(1) {
                Some(key) => Some(self.routing_info_for_key(key)),
                None => Some(RoutingInfo::SingleNode(SingleNodeRoutingInfo::Random)),
            },
        }
    }

    #[cfg(feature = "cluster")]
    fn routing_info_for_key(&self, key: &[u8]) -> RoutingInfo {
        RoutingInfo::SingleNode(SingleNodeRoutingInfo::SpecificNode(get_route(
            self.is_readonly(),
            key,
        )))
    }

    ///
    pub fn is_readonly(&self) -> bool {
        matches!(
            self,
            KnownCommand::Bitcount
                | KnownCommand::BitfieldRo
                | KnownCommand::Bitpos
                | KnownCommand::Dbsize
                | KnownCommand::Dump
                | KnownCommand::EvalshaRo
                | KnownCommand::EvalRo
                | KnownCommand::Exists
                | KnownCommand::Expiretime
                | KnownCommand::FcallRo
                | KnownCommand::Geodist
                | KnownCommand::Geohash
                | KnownCommand::Geopos
                | KnownCommand::GeoradiusbymemberRo
                | KnownCommand::GeoradiusRo
                | KnownCommand::Geosearch
                | KnownCommand::Get
                | KnownCommand::Getbit
                | KnownCommand::Getrange
                | KnownCommand::Hexists
                | KnownCommand::Hget
                | KnownCommand::Hgetall
                | KnownCommand::Hkeys
                | KnownCommand::Hlen
                | KnownCommand::Hmget
                | KnownCommand::Hrandfield
                | KnownCommand::Hscan
                | KnownCommand::Hstrlen
                | KnownCommand::Hvals
                | KnownCommand::Keys
                | KnownCommand::Lcs
                | KnownCommand::Lindex
                | KnownCommand::Llen
                | KnownCommand::Lolwut
                | KnownCommand::Lpos
                | KnownCommand::Lrange
                | KnownCommand::MemoryUsage
                | KnownCommand::Mget
                | KnownCommand::ObjectEncoding
                | KnownCommand::ObjectFreq
                | KnownCommand::ObjectIdletime
                | KnownCommand::ObjectRefcount
                | KnownCommand::Pexpiretime
                | KnownCommand::Pfcount
                | KnownCommand::Pttl
                | KnownCommand::Randomkey
                | KnownCommand::Scan
                | KnownCommand::Scard
                | KnownCommand::Sdiff
                | KnownCommand::Sinter
                | KnownCommand::Sintercard
                | KnownCommand::Sismember
                | KnownCommand::Smembers
                | KnownCommand::Smismember
                | KnownCommand::SortRo
                | KnownCommand::Srandmember
                | KnownCommand::Sscan
                | KnownCommand::Strlen
                | KnownCommand::Substr
                | KnownCommand::Sunion
                | KnownCommand::Touch
                | KnownCommand::Ttl
                | KnownCommand::Type
                | KnownCommand::XinfoConsumers
                | KnownCommand::XinfoGroups
                | KnownCommand::XinfoStream
                | KnownCommand::Xlen
                | KnownCommand::Xpending
                | KnownCommand::Xrange
                | KnownCommand::Xread
                | KnownCommand::Xrevrange
                | KnownCommand::Zcard
                | KnownCommand::Zcount
                | KnownCommand::Zdiff
                | KnownCommand::Zinter
                | KnownCommand::Zintercard
                | KnownCommand::Zlexcount
                | KnownCommand::Zmscore
                | KnownCommand::Zrandmember
                | KnownCommand::Zrange
                | KnownCommand::Zrangebylex
                | KnownCommand::Zrangebyscore
                | KnownCommand::Zrank
                | KnownCommand::Zrevrange
                | KnownCommand::Zrevrangebylex
                | KnownCommand::Zrevrangebyscore
                | KnownCommand::Zrevrank
                | KnownCommand::Zscan
                | KnownCommand::Zscore
                | KnownCommand::Zunion
        )
    }

    ///
    pub(crate) fn as_args(&self) -> &'static [&'static [u8]] {
        match self {
            KnownCommand::Acl => &[b"ACL"],
            KnownCommand::AclCat => &[b"ACL", b"CAT"],
            KnownCommand::AclDeluser => &[b"ACL", b"DELUSER"],
            KnownCommand::AclDryrun => &[b"ACL", b"DRYRUN"],
            KnownCommand::AclGenpass => &[b"ACL", b"GENPASS"],
            KnownCommand::AclGetuser => &[b"ACL", b"GETUSER"],
            KnownCommand::AclHelp => &[b"ACL", b"HELP"],
            KnownCommand::AclList => &[b"ACL", b"LIST"],
            KnownCommand::AclLoad => &[b"ACL", b"LOAD"],
            KnownCommand::AclLog => &[b"ACL", b"LOG"],
            KnownCommand::AclSave => &[b"ACL", b"SAVE"],
            KnownCommand::AclSetuser => &[b"ACL", b"SETUSER"],
            KnownCommand::AclUsers => &[b"ACL", b"USERS"],
            KnownCommand::AclWhoami => &[b"ACL", b"WHOAMI"],
            KnownCommand::Append => &[b"APPEND"],
            KnownCommand::Asking => &[b"ASKING"],
            KnownCommand::Auth => &[b"AUTH"],
            KnownCommand::Bgrewriteaof => &[b"BGREWRITEAOF"],
            KnownCommand::Bgsave => &[b"BGSAVE"],
            KnownCommand::Bitcount => &[b"BITCOUNT"],
            KnownCommand::Bitfield => &[b"BITFIELD"],
            KnownCommand::BitfieldRo => &[b"BITFIELD_RO"],
            KnownCommand::Bitop => &[b"BITOP"],
            KnownCommand::Bitpos => &[b"BITPOS"],
            KnownCommand::Blmove => &[b"BLMOVE"],
            KnownCommand::Blmpop => &[b"BLMPOP"],
            KnownCommand::Blpop => &[b"BLPOP"],
            KnownCommand::Brpop => &[b"BRPOP"],
            KnownCommand::Brpoplpush => &[b"BRPOPLPUSH"],
            KnownCommand::Bzmpop => &[b"BZMPOP"],
            KnownCommand::Bzpopmax => &[b"BZPOPMAX"],
            KnownCommand::Bzpopmin => &[b"BZPOPMIN"],
            KnownCommand::Client => &[b"CLIENT"],
            KnownCommand::ClientCaching => &[b"CLIENT", b"CACHING"],
            KnownCommand::ClientGetname => &[b"CLIENT", b"GETNAME"],
            KnownCommand::ClientGetredir => &[b"CLIENT", b"GETREDIR"],
            KnownCommand::ClientHelp => &[b"CLIENT", b"HELP"],
            KnownCommand::ClientId => &[b"CLIENT", b"ID"],
            KnownCommand::ClientInfo => &[b"CLIENT", b"INFO"],
            KnownCommand::ClientKill => &[b"CLIENT", b"KILL"],
            KnownCommand::ClientList => &[b"CLIENT", b"LIST"],
            KnownCommand::ClientNoEvict => &[b"CLIENT", b"NO-EVICT"],
            KnownCommand::ClientNoTouch => &[b"CLIENT", b"NO-TOUCH"],
            KnownCommand::ClientPause => &[b"CLIENT", b"PAUSE"],
            KnownCommand::ClientReply => &[b"CLIENT", b"REPLY"],
            KnownCommand::ClientSetinfo => &[b"CLIENT", b"SETINFO"],
            KnownCommand::ClientSetname => &[b"CLIENT", b"SETNAME"],
            KnownCommand::ClientTracking => &[b"CLIENT", b"TRACKING"],
            KnownCommand::ClientTrackinginfo => &[b"CLIENT", b"TRACKINGINFO"],
            KnownCommand::ClientUnblock => &[b"CLIENT", b"UNBLOCK"],
            KnownCommand::ClientUnpause => &[b"CLIENT", b"UNPAUSE"],
            KnownCommand::Cluster => &[b"CLUSTER"],
            KnownCommand::ClusterAddslots => &[b"CLUSTER", b"ADDSLOTS"],
            KnownCommand::ClusterAddslotsrange => &[b"CLUSTER", b"ADDSLOTSRANGE"],
            KnownCommand::ClusterBumpepoch => &[b"CLUSTER", b"BUMPEPOCH"],
            KnownCommand::ClusterCountFailureReports => &[b"CLUSTER", b"COUNT-FAILURE-REPORTS"],
            KnownCommand::ClusterCountkeysinslot => &[b"CLUSTER", b"COUNTKEYSINSLOT"],
            KnownCommand::ClusterDelslots => &[b"CLUSTER", b"DELSLOTS"],
            KnownCommand::ClusterDelslotsrange => &[b"CLUSTER", b"DELSLOTSRANGE"],
            KnownCommand::ClusterFailover => &[b"CLUSTER", b"FAILOVER"],
            KnownCommand::ClusterFlushslots => &[b"CLUSTER", b"FLUSHSLOTS"],
            KnownCommand::ClusterForget => &[b"CLUSTER", b"FORGET"],
            KnownCommand::ClusterGetkeysinslot => &[b"CLUSTER", b"GETKEYSINSLOT"],
            KnownCommand::ClusterHelp => &[b"CLUSTER", b"HELP"],
            KnownCommand::ClusterInfo => &[b"CLUSTER", b"INFO"],
            KnownCommand::ClusterKeyslot => &[b"CLUSTER", b"KEYSLOT"],
            KnownCommand::ClusterLinks => &[b"CLUSTER", b"LINKS"],
            KnownCommand::ClusterMeet => &[b"CLUSTER", b"MEET"],
            KnownCommand::ClusterMyid => &[b"CLUSTER", b"MYID"],
            KnownCommand::ClusterMyshardid => &[b"CLUSTER", b"MYSHARDID"],
            KnownCommand::ClusterNodes => &[b"CLUSTER", b"NODES"],
            KnownCommand::ClusterReplicas => &[b"CLUSTER", b"REPLICAS"],
            KnownCommand::ClusterReplicate => &[b"CLUSTER", b"REPLICATE"],
            KnownCommand::ClusterReset => &[b"CLUSTER", b"RESET"],
            KnownCommand::ClusterSaveconfig => &[b"CLUSTER", b"SAVECONFIG"],
            KnownCommand::ClusterSetConfigEpoch => &[b"CLUSTER", b"SET-CONFIG-EPOCH"],
            KnownCommand::ClusterSetslot => &[b"CLUSTER", b"SETSLOT"],
            KnownCommand::ClusterShards => &[b"CLUSTER", b"SHARDS"],
            KnownCommand::ClusterSlaves => &[b"CLUSTER", b"SLAVES"],
            KnownCommand::ClusterSlots => &[b"CLUSTER", b"SLOTS"],
            KnownCommand::Command => &[b"COMMAND"],
            KnownCommand::CommandCount => &[b"COMMAND", b"COUNT"],
            KnownCommand::CommandDocs => &[b"COMMAND", b"DOCS"],
            KnownCommand::CommandGetkeys => &[b"COMMAND", b"GETKEYS"],
            KnownCommand::CommandGetkeysandflags => &[b"COMMAND", b"GETKEYSANDFLAGS"],
            KnownCommand::CommandHelp => &[b"COMMAND", b"HELP"],
            KnownCommand::CommandInfo => &[b"COMMAND", b"INFO"],
            KnownCommand::CommandList => &[b"COMMAND", b"LIST"],
            KnownCommand::Config => &[b"CONFIG"],
            KnownCommand::ConfigGet => &[b"CONFIG", b"GET"],
            KnownCommand::ConfigHelp => &[b"CONFIG", b"HELP"],
            KnownCommand::ConfigResetstat => &[b"CONFIG", b"RESETSTAT"],
            KnownCommand::ConfigRewrite => &[b"CONFIG", b"REWRITE"],
            KnownCommand::ConfigSet => &[b"CONFIG", b"SET"],
            KnownCommand::Copy => &[b"COPY"],
            KnownCommand::Dbsize => &[b"DBSIZE"],
            KnownCommand::Debug => &[b"DEBUG"],
            KnownCommand::Decr => &[b"DECR"],
            KnownCommand::Decrby => &[b"DECRBY"],
            KnownCommand::Del => &[b"DEL"],
            KnownCommand::Discard => &[b"DISCARD"],
            KnownCommand::Dump => &[b"DUMP"],
            KnownCommand::Echo => &[b"ECHO"],
            KnownCommand::Eval => &[b"EVAL"],
            KnownCommand::Evalsha => &[b"EVALSHA"],
            KnownCommand::EvalshaRo => &[b"EVALSHA_RO"],
            KnownCommand::EvalRo => &[b"EVAL_RO"],
            KnownCommand::Exec => &[b"EXEC"],
            KnownCommand::Exists => &[b"EXISTS"],
            KnownCommand::Expire => &[b"EXPIRE"],
            KnownCommand::Expireat => &[b"EXPIREAT"],
            KnownCommand::Expiretime => &[b"EXPIRETIME"],
            KnownCommand::Failover => &[b"FAILOVER"],
            KnownCommand::Fcall => &[b"FCALL"],
            KnownCommand::FcallRo => &[b"FCALL_RO"],
            KnownCommand::Flushall => &[b"FLUSHALL"],
            KnownCommand::Flushdb => &[b"FLUSHDB"],
            KnownCommand::Function => &[b"FUNCTION"],
            KnownCommand::FunctionDelete => &[b"FUNCTION", b"DELETE"],
            KnownCommand::FunctionDump => &[b"FUNCTION", b"DUMP"],
            KnownCommand::FunctionFlush => &[b"FUNCTION", b"FLUSH"],
            KnownCommand::FunctionHelp => &[b"FUNCTION", b"HELP"],
            KnownCommand::FunctionKill => &[b"FUNCTION", b"KILL"],
            KnownCommand::FunctionList => &[b"FUNCTION", b"LIST"],
            KnownCommand::FunctionLoad => &[b"FUNCTION", b"LOAD"],
            KnownCommand::FunctionRestore => &[b"FUNCTION", b"RESTORE"],
            KnownCommand::FunctionStats => &[b"FUNCTION", b"STATS"],
            KnownCommand::Geoadd => &[b"GEOADD"],
            KnownCommand::Geodist => &[b"GEODIST"],
            KnownCommand::Geohash => &[b"GEOHASH"],
            KnownCommand::Geopos => &[b"GEOPOS"],
            KnownCommand::Georadius => &[b"GEORADIUS"],
            KnownCommand::Georadiusbymember => &[b"GEORADIUSBYMEMBER"],
            KnownCommand::GeoradiusbymemberRo => &[b"GEORADIUSBYMEMBER_RO"],
            KnownCommand::GeoradiusRo => &[b"GEORADIUS_RO"],
            KnownCommand::Geosearch => &[b"GEOSEARCH"],
            KnownCommand::Geosearchstore => &[b"GEOSEARCHSTORE"],
            KnownCommand::Get => &[b"GET"],
            KnownCommand::Getbit => &[b"GETBIT"],
            KnownCommand::Getdel => &[b"GETDEL"],
            KnownCommand::Getex => &[b"GETEX"],
            KnownCommand::Getrange => &[b"GETRANGE"],
            KnownCommand::Getset => &[b"GETSET"],
            KnownCommand::Hdel => &[b"HDEL"],
            KnownCommand::Hello => &[b"HELLO"],
            KnownCommand::Hexists => &[b"HEXISTS"],
            KnownCommand::Hget => &[b"HGET"],
            KnownCommand::Hgetall => &[b"HGETALL"],
            KnownCommand::Hincrby => &[b"HINCRBY"],
            KnownCommand::Hincrbyfloat => &[b"HINCRBYFLOAT"],
            KnownCommand::Hkeys => &[b"HKEYS"],
            KnownCommand::Hlen => &[b"HLEN"],
            KnownCommand::Hmget => &[b"HMGET"],
            KnownCommand::Hmset => &[b"HMSET"],
            KnownCommand::Hrandfield => &[b"HRANDFIELD"],
            KnownCommand::Hscan => &[b"HSCAN"],
            KnownCommand::Hset => &[b"HSET"],
            KnownCommand::Hsetnx => &[b"HSETNX"],
            KnownCommand::Hstrlen => &[b"HSTRLEN"],
            KnownCommand::Hvals => &[b"HVALS"],
            KnownCommand::Incr => &[b"INCR"],
            KnownCommand::Incrby => &[b"INCRBY"],
            KnownCommand::Incrbyfloat => &[b"INCRBYFLOAT"],
            KnownCommand::Info => &[b"INFO"],
            KnownCommand::Keys => &[b"KEYS"],
            KnownCommand::Lastsave => &[b"LASTSAVE"],
            KnownCommand::Latency => &[b"LATENCY"],
            KnownCommand::LatencyDoctor => &[b"LATENCY", b"DOCTOR"],
            KnownCommand::LatencyGraph => &[b"LATENCY", b"GRAPH"],
            KnownCommand::LatencyHelp => &[b"LATENCY", b"HELP"],
            KnownCommand::LatencyHistogram => &[b"LATENCY", b"HISTOGRAM"],
            KnownCommand::LatencyHistory => &[b"LATENCY", b"HISTORY"],
            KnownCommand::LatencyLatest => &[b"LATENCY", b"LATEST"],
            KnownCommand::LatencyReset => &[b"LATENCY", b"RESET"],
            KnownCommand::Lcs => &[b"LCS"],
            KnownCommand::Lindex => &[b"LINDEX"],
            KnownCommand::Linsert => &[b"LINSERT"],
            KnownCommand::Llen => &[b"LLEN"],
            KnownCommand::Lmove => &[b"LMOVE"],
            KnownCommand::Lmpop => &[b"LMPOP"],
            KnownCommand::Lolwut => &[b"LOLWUT"],
            KnownCommand::Lpop => &[b"LPOP"],
            KnownCommand::Lpos => &[b"LPOS"],
            KnownCommand::Lpush => &[b"LPUSH"],
            KnownCommand::Lpushx => &[b"LPUSHX"],
            KnownCommand::Lrange => &[b"LRANGE"],
            KnownCommand::Lrem => &[b"LREM"],
            KnownCommand::Lset => &[b"LSET"],
            KnownCommand::Ltrim => &[b"LTRIM"],
            KnownCommand::Memory => &[b"MEMORY"],
            KnownCommand::MemoryDoctor => &[b"MEMORY", b"DOCTOR"],
            KnownCommand::MemoryHelp => &[b"MEMORY", b"HELP"],
            KnownCommand::MemoryMallocStats => &[b"MEMORY", b"MALLOC-STATS"],
            KnownCommand::MemoryPurge => &[b"MEMORY", b"PURGE"],
            KnownCommand::MemoryStats => &[b"MEMORY", b"STATS"],
            KnownCommand::MemoryUsage => &[b"MEMORY", b"USAGE"],
            KnownCommand::Mget => &[b"MGET"],
            KnownCommand::Migrate => &[b"MIGRATE"],
            KnownCommand::Module => &[b"MODULE"],
            KnownCommand::ModuleHelp => &[b"MODULE", b"HELP"],
            KnownCommand::ModuleList => &[b"MODULE", b"LIST"],
            KnownCommand::ModuleLoad => &[b"MODULE", b"LOAD"],
            KnownCommand::ModuleLoadex => &[b"MODULE", b"LOADEX"],
            KnownCommand::ModuleUnload => &[b"MODULE", b"UNLOAD"],
            KnownCommand::Monitor => &[b"MONITOR"],
            KnownCommand::Move => &[b"MOVE"],
            KnownCommand::Mset => &[b"MSET"],
            KnownCommand::Msetnx => &[b"MSETNX"],
            KnownCommand::Multi => &[b"MULTI"],
            KnownCommand::Object => &[b"OBJECT"],
            KnownCommand::ObjectEncoding => &[b"OBJECT", b"ENCODING"],
            KnownCommand::ObjectFreq => &[b"OBJECT", b"FREQ"],
            KnownCommand::ObjectHelp => &[b"OBJECT", b"HELP"],
            KnownCommand::ObjectIdletime => &[b"OBJECT", b"IDLETIME"],
            KnownCommand::ObjectRefcount => &[b"OBJECT", b"REFCOUNT"],
            KnownCommand::Persist => &[b"PERSIST"],
            KnownCommand::Pexpire => &[b"PEXPIRE"],
            KnownCommand::Pexpireat => &[b"PEXPIREAT"],
            KnownCommand::Pexpiretime => &[b"PEXPIRETIME"],
            KnownCommand::Pfadd => &[b"PFADD"],
            KnownCommand::Pfcount => &[b"PFCOUNT"],
            KnownCommand::Pfdebug => &[b"PFDEBUG"],
            KnownCommand::Pfmerge => &[b"PFMERGE"],
            KnownCommand::Pfselftest => &[b"PFSELFTEST"],
            KnownCommand::Ping => &[b"PING"],
            KnownCommand::Psetex => &[b"PSETEX"],
            KnownCommand::Psubscribe => &[b"PSUBSCRIBE"],
            KnownCommand::Psync => &[b"PSYNC"],
            KnownCommand::Pttl => &[b"PTTL"],
            KnownCommand::Publish => &[b"PUBLISH"],
            KnownCommand::Pubsub => &[b"PUBSUB"],
            KnownCommand::PubsubChannels => &[b"PUBSUB", b"CHANNELS"],
            KnownCommand::PubsubHelp => &[b"PUBSUB", b"HELP"],
            KnownCommand::PubsubNumpat => &[b"PUBSUB", b"NUMPAT"],
            KnownCommand::PubsubNumsub => &[b"PUBSUB", b"NUMSUB"],
            KnownCommand::PubsubShardchannels => &[b"PUBSUB", b"SHARDCHANNELS"],
            KnownCommand::PubsubShardnumsub => &[b"PUBSUB", b"SHARDNUMSUB"],
            KnownCommand::Punsubscribe => &[b"PUNSUBSCRIBE"],
            KnownCommand::Quit => &[b"QUIT"],
            KnownCommand::Randomkey => &[b"RANDOMKEY"],
            KnownCommand::Readonly => &[b"READONLY"],
            KnownCommand::Readwrite => &[b"READWRITE"],
            KnownCommand::Rename => &[b"RENAME"],
            KnownCommand::Renamenx => &[b"RENAMENX"],
            KnownCommand::Replconf => &[b"REPLCONF"],
            KnownCommand::Replicaof => &[b"REPLICAOF"],
            KnownCommand::Reset => &[b"RESET"],
            KnownCommand::Restore => &[b"RESTORE"],
            KnownCommand::RestoreAsking => &[b"RESTORE-ASKING"],
            KnownCommand::Role => &[b"ROLE"],
            KnownCommand::Rpop => &[b"RPOP"],
            KnownCommand::Rpoplpush => &[b"RPOPLPUSH"],
            KnownCommand::Rpush => &[b"RPUSH"],
            KnownCommand::Rpushx => &[b"RPUSHX"],
            KnownCommand::Sadd => &[b"SADD"],
            KnownCommand::Save => &[b"SAVE"],
            KnownCommand::Scan => &[b"SCAN"],
            KnownCommand::Scard => &[b"SCARD"],
            KnownCommand::Script => &[b"SCRIPT"],
            KnownCommand::ScriptDebug => &[b"SCRIPT", b"DEBUG"],
            KnownCommand::ScriptExists => &[b"SCRIPT", b"EXISTS"],
            KnownCommand::ScriptFlush => &[b"SCRIPT", b"FLUSH"],
            KnownCommand::ScriptHelp => &[b"SCRIPT", b"HELP"],
            KnownCommand::ScriptKill => &[b"SCRIPT", b"KILL"],
            KnownCommand::ScriptLoad => &[b"SCRIPT", b"LOAD"],
            KnownCommand::Sdiff => &[b"SDIFF"],
            KnownCommand::Sdiffstore => &[b"SDIFFSTORE"],
            KnownCommand::Select => &[b"SELECT"],
            KnownCommand::Set => &[b"SET"],
            KnownCommand::Setbit => &[b"SETBIT"],
            KnownCommand::Setex => &[b"SETEX"],
            KnownCommand::Setnx => &[b"SETNX"],
            KnownCommand::Setrange => &[b"SETRANGE"],
            KnownCommand::Shutdown => &[b"SHUTDOWN"],
            KnownCommand::Sinter => &[b"SINTER"],
            KnownCommand::Sintercard => &[b"SINTERCARD"],
            KnownCommand::Sinterstore => &[b"SINTERSTORE"],
            KnownCommand::Sismember => &[b"SISMEMBER"],
            KnownCommand::Slaveof => &[b"SLAVEOF"],
            KnownCommand::Slowlog => &[b"SLOWLOG"],
            KnownCommand::SlowlogGet => &[b"SLOWLOG", b"GET"],
            KnownCommand::SlowlogHelp => &[b"SLOWLOG", b"HELP"],
            KnownCommand::SlowlogLen => &[b"SLOWLOG", b"LEN"],
            KnownCommand::SlowlogReset => &[b"SLOWLOG", b"RESET"],
            KnownCommand::Smembers => &[b"SMEMBERS"],
            KnownCommand::Smismember => &[b"SMISMEMBER"],
            KnownCommand::Smove => &[b"SMOVE"],
            KnownCommand::Sort => &[b"SORT"],
            KnownCommand::SortRo => &[b"SORT_RO"],
            KnownCommand::Spop => &[b"SPOP"],
            KnownCommand::Spublish => &[b"SPUBLISH"],
            KnownCommand::Srandmember => &[b"SRANDMEMBER"],
            KnownCommand::Srem => &[b"SREM"],
            KnownCommand::Sscan => &[b"SSCAN"],
            KnownCommand::Ssubscribe => &[b"SSUBSCRIBE"],
            KnownCommand::Strlen => &[b"STRLEN"],
            KnownCommand::Subscribe => &[b"SUBSCRIBE"],
            KnownCommand::Substr => &[b"SUBSTR"],
            KnownCommand::Sunion => &[b"SUNION"],
            KnownCommand::Sunionstore => &[b"SUNIONSTORE"],
            KnownCommand::Sunsubscribe => &[b"SUNSUBSCRIBE"],
            KnownCommand::Swapdb => &[b"SWAPDB"],
            KnownCommand::Sync => &[b"SYNC"],
            KnownCommand::Time => &[b"TIME"],
            KnownCommand::Touch => &[b"TOUCH"],
            KnownCommand::Ttl => &[b"TTL"],
            KnownCommand::Type => &[b"TYPE"],
            KnownCommand::Unlink => &[b"UNLINK"],
            KnownCommand::Unsubscribe => &[b"UNSUBSCRIBE"],
            KnownCommand::Unwatch => &[b"UNWATCH"],
            KnownCommand::Wait => &[b"WAIT"],
            KnownCommand::Waitaof => &[b"WAITAOF"],
            KnownCommand::Watch => &[b"WATCH"],
            KnownCommand::Xack => &[b"XACK"],
            KnownCommand::Xadd => &[b"XADD"],
            KnownCommand::Xautoclaim => &[b"XAUTOCLAIM"],
            KnownCommand::Xclaim => &[b"XCLAIM"],
            KnownCommand::Xdel => &[b"XDEL"],
            KnownCommand::Xgroup => &[b"XGROUP"],
            KnownCommand::XgroupCreate => &[b"XGROUP", b"CREATE"],
            KnownCommand::XgroupCreateconsumer => &[b"XGROUP", b"CREATECONSUMER"],
            KnownCommand::XgroupDelconsumer => &[b"XGROUP", b"DELCONSUMER"],
            KnownCommand::XgroupDestroy => &[b"XGROUP", b"DESTROY"],
            KnownCommand::XgroupHelp => &[b"XGROUP", b"HELP"],
            KnownCommand::XgroupSetid => &[b"XGROUP", b"SETID"],
            KnownCommand::Xinfo => &[b"XINFO"],
            KnownCommand::XinfoConsumers => &[b"XINFO", b"CONSUMERS"],
            KnownCommand::XinfoGroups => &[b"XINFO", b"GROUPS"],
            KnownCommand::XinfoHelp => &[b"XINFO", b"HELP"],
            KnownCommand::XinfoStream => &[b"XINFO", b"STREAM"],
            KnownCommand::Xlen => &[b"XLEN"],
            KnownCommand::Xpending => &[b"XPENDING"],
            KnownCommand::Xrange => &[b"XRANGE"],
            KnownCommand::Xread => &[b"XREAD"],
            KnownCommand::Xreadgroup => &[b"XREADGROUP"],
            KnownCommand::Xrevrange => &[b"XREVRANGE"],
            KnownCommand::Xsetid => &[b"XSETID"],
            KnownCommand::Xtrim => &[b"XTRIM"],
            KnownCommand::Zadd => &[b"ZADD"],
            KnownCommand::Zcard => &[b"ZCARD"],
            KnownCommand::Zcount => &[b"ZCOUNT"],
            KnownCommand::Zdiff => &[b"ZDIFF"],
            KnownCommand::Zdiffstore => &[b"ZDIFFSTORE"],
            KnownCommand::Zincrby => &[b"ZINCRBY"],
            KnownCommand::Zinter => &[b"ZINTER"],
            KnownCommand::Zintercard => &[b"ZINTERCARD"],
            KnownCommand::Zinterstore => &[b"ZINTERSTORE"],
            KnownCommand::Zlexcount => &[b"ZLEXCOUNT"],
            KnownCommand::Zmpop => &[b"ZMPOP"],
            KnownCommand::Zmscore => &[b"ZMSCORE"],
            KnownCommand::Zpopmax => &[b"ZPOPMAX"],
            KnownCommand::Zpopmin => &[b"ZPOPMIN"],
            KnownCommand::Zrandmember => &[b"ZRANDMEMBER"],
            KnownCommand::Zrange => &[b"ZRANGE"],
            KnownCommand::Zrangebylex => &[b"ZRANGEBYLEX"],
            KnownCommand::Zrangebyscore => &[b"ZRANGEBYSCORE"],
            KnownCommand::Zrangestore => &[b"ZRANGESTORE"],
            KnownCommand::Zrank => &[b"ZRANK"],
            KnownCommand::Zrem => &[b"ZREM"],
            KnownCommand::Zremrangebylex => &[b"ZREMRANGEBYLEX"],
            KnownCommand::Zremrangebyrank => &[b"ZREMRANGEBYRANK"],
            KnownCommand::Zremrangebyscore => &[b"ZREMRANGEBYSCORE"],
            KnownCommand::Zrevrange => &[b"ZREVRANGE"],
            KnownCommand::Zrevrangebylex => &[b"ZREVRANGEBYLEX"],
            KnownCommand::Zrevrangebyscore => &[b"ZREVRANGEBYSCORE"],
            KnownCommand::Zrevrank => &[b"ZREVRANK"],
            KnownCommand::Zscan => &[b"ZSCAN"],
            KnownCommand::Zscore => &[b"ZSCORE"],
            KnownCommand::Zunion => &[b"ZUNION"],
            KnownCommand::Zunionstore => &[b"ZUNIONSTORE"],
        }
    }
}
