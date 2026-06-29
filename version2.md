# Announcing redis-rs & redis-test 2.0.0

With version 1.0.0 behind us, version 2.0.0 begins a new major series, with redis-test tracking it with the same version.

This document highlights the breaking changes in version 2.0.0. For a complete list of changes, see CHANGELOG.md. We appreciate feedback and bug reports — please open an issue for anything you encounter during migration. In order to get the newest version, please specify in your Cargo.toml file

```toml
redis = "2"
```

## Zero-copy response parsing (Breaking Change)

`Value` now stores its textual and binary payloads in cheaply-cloneable,
reference-counted buffers instead of owned `Vec<u8>`/`String`:

- `Value::BulkString(Vec<u8>)` → `Value::BulkString(bytes::Bytes)`
- `Value::SimpleString(String)` → `Value::SimpleString(Str)`
- `Value::VerbatimString { text: String, .. }` → `{ text: Str, .. }`
- `Value::BigNumber(Vec<u8>)` → `Value::BigNumber(bytes::Bytes)` (unchanged under the `num-bigint` feature)
- `PushKind::Other(String)` / `VerbatimFormat::Unknown(String)` → `Str`
- Server error code/detail are now `Str` as well

`Str` is a new UTF-8-guaranteed string backed by `bytes::Bytes`. It derefs to
`&str`, so most code keeps working unchanged, and it converts cheaply to/from
`String` and `Bytes` (`Into<String>`, `Into<Bytes>`).

The parser was rewritten to be **zero-copy**: instead of allocating a fresh
`Vec`/`String` for every element of a response, it parses into byte-range
offsets and then produces each leaf as a cheap reference-counted slice into the
response buffer. A response with many elements no longer performs a heap
allocation per element.

**Migration:** Most code that goes through `FromRedisValue`/`from_redis_value`
is unaffected. Code that matches on `Value` directly should:

```rust
// Before:
if let Value::BulkString(bytes) = v {
    let s = String::from_utf8(bytes)?;       // bytes: Vec<u8>
}
// After:
if let Value::BulkString(bytes) = v {
    let s = String::from_utf8(bytes.into())?; // bytes: Bytes  (or use &bytes as &[u8])
}
```

`Str` derefs to `&str`, so `Value::SimpleString` matches that previously used
the inner `String` as a `&str` continue to work; constructing one now takes a
`Str` (e.g. `Value::SimpleString("OK".into())`).

### Why it's faster

The new parser allocates a small, constant number of times per response rather
than once per element, and avoids copying bulk-string payloads out of the read
buffer entirely on the async codec path. Parsing benchmarks
(`cargo bench -p redis --bench bench_decode`) comparing the new parser against
the previous `Vec`/`String`-based one:

| Response                       | Allocations (before → after) | Time (before → after) |
| ------------------------------ | ---------------------------- | --------------------- |
| Single 1 MiB bulk string       | 154 → **2**   (77×)          | 50.8 µs → 12.3 µs (**4.1×**) |
| Array of 5000 small bulks      | 7509 → **16** (469×)         | 548 µs → 367 µs (1.5×) |
| Array of 500 × 1 KiB bulks     | 2022 → **11** (184×)         | 160.7 µs → 45.6 µs (**3.5×**) |
| Array of 5000 simple strings   | 7152 → **16** (447×)         | 411 µs → 263 µs (1.6×) |
| Map of 1000 key/value pairs    | 2933 → **13** (226×)         | 206 µs → 149 µs (1.4×) |

In short: **1.4×–4.1× faster parsing and 10×–470× fewer heap allocations**, with
the largest wins on responses that contain many elements. Cloning a `Value` (or
any `Str`/`BulkString` inside it) is now a reference-count bump rather than a
deep copy.

## `cmd_iter` yields `CmdRef` instead of `&Cmd` (Breaking Change)

**Most users can upgrade to 2.0.0 with no code changes.** The flattening is an internal representation change; the pipeline builder API (`cmd`, `arg`, `add_command`, `ignore`, `query`, `query_async`, `exec`, …) is unchanged. The only adjustments are needed if you iterate a pipeline's commands or call `with_capacity` directly.

Because a pipeline no longer owns a `Vec<Cmd>`, there is no `&Cmd` to hand out. [`Pipeline::cmd_iter`] and [`ClusterPipeline::cmd_iter`] now yield `CmdRef<'_>`, a lightweight, `Copy` view that borrows directly into the pipeline's shared buffers — iterating a pipeline's commands performs no per-command allocation.

`CmdRef` is intentionally opaque so that the underlying storage can keep evolving. It exposes the read-only accessors you previously reached for on `&Cmd`, including `args_iter()`, `arg_idx()`, `data()`, `cursor()`, `is_no_response()`, and `get_packed_command()`. If you genuinely need an owned `Cmd`, call `to_cmd()`.

**Migration:** Update code that iterates a pipeline's commands. Most call sites only need to drop a borrow or call an accessor:

```rust
// Before:
for cmd in pipe.cmd_iter() {
    let name = cmd.arg_idx(0);
    // cmd: &Cmd
}

// After:
for cmd in pipe.cmd_iter() {
    let name = cmd.arg_idx(0);
    // cmd: CmdRef<'_> — same read accessors, Copy
}
```

If you stored or passed the `&Cmd` onward and need an owned value:

```rust
// Before:
let owned: Vec<Cmd> = pipe.cmd_iter().cloned().collect();

// After:
let owned: Vec<Cmd> = pipe.cmd_iter().map(|cmd| cmd.to_cmd()).collect();
```

## `with_capacity` takes byte and argument estimates (Breaking Change)

[`Pipeline::with_capacity`] and [`ClusterPipeline::with_capacity`] previously took a single argument — the expected number of commands. With the flattened representation that unit no longer matches the underlying buffers, so the signature changed to describe the two buffers directly:

```rust
pub fn with_capacity(data_capacity: usize, args_capacity: Option<usize>) -> Self
```

`data_capacity` is an estimate of the total number of argument _bytes_ the pipeline will hold. `args_capacity` is an estimate of the total number of _arguments_ across all commands; when `None`, it is derived from `data_capacity` assuming an average argument size.

**Migration:** Replace the command-count argument with a byte estimate, and pass `None` to let the argument count be inferred:

```rust
// Before:
let mut pipe = redis::Pipeline::with_capacity(16); // 16 commands

// After:
let mut pipe = redis::Pipeline::with_capacity(1024, None); // ~1 KiB of argument data
```

`Pipeline::new()` and `pipe()` are unchanged.

## `Pipeline` and `ClusterPipeline` implement `RedisWrite` (New)

As part of the rework, both pipeline types now implement [`RedisWrite`], writing arguments directly into the shared buffers. This is additive and requires no migration; it does mean the `RedisWrite` methods (`write_arg`, `writer_for_next_arg`, …) are now available on pipelines should you want to write argument bytes yourself.

This is a new capability with no migration required.
