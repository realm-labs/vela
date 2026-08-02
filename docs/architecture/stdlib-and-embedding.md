## Standard Library

### Array

```rust
arr.len()
arr.is_empty()
arr.push(value)
arr.pop()
arr.values()    // Iterator over values
arr.iter()      // Iterator over values
```

Iterator pipelines are the primary collection transformation model:

```rust
arr.iter().filter(|x| predicate).map(|x| value).collect_array()
arr.iter().filter(|x| predicate).collect_set()
arr.iter().find(|x| predicate)
arr.iter().any(|x| predicate)
arr.iter().all(|x| predicate)
arr.iter().count(|x| predicate)
arr.iter().fold(initial, |acc, x| next_acc)
```

The retained eager array helpers are convenience wrappers over the same
iterator and callback engine:

```rust
arr.map(|x| ...)
arr.filter(|x| ...)
arr.find(|x| ...)
arr.any(|x| ...)
arr.all(|x| ...)
arr.count(|x| ...)
arr.sum(|x| ...)
arr.group_by(|x| ...)
arr.sort_by(|x| ...)
```

Iterator adapters and retained eager helpers should expose analysis-only
signatures so LSP can infer lambda parameter facts without adding script
generics. For example, if `arr` has `TypeFact::Array { element: E }`, then:

```text
arr.iter().filter(|x| predicate) gives x: E and returns Iterator(item = E)
arr.iter().map(|x| value) gives x: E and returns Iterator(item = TypeFact(value))
arr.iter().find(|x| predicate) gives x: E and returns Option-like enum containing E
arr.sum(|x| value) gives x: E and returns the concrete scalar type produced by value
arr.group_by(|x| key) gives x: E and returns Map(key = TypeFact(key), value = Array<E>)
arr.iter().fold(initial, |acc, x| next_acc) returns the callback accumulator fact
```

`Iterator.fold` consumes the iterator in deterministic traversal order. The
callback receives the current accumulator and one item; an empty iterator
returns the initial value without invoking the callback. Host-backed iterators
use the same prepared live polling, call-scoped lifetime, and per-item budget
rules as their other terminal methods.

### Map

```rust
map.len()
map.has(key)
map.contains_key(key)
map.get(key)
map.get_or(key, default)
map.set(key, value)
map.remove(key)
map.iter()      // Iterator over MapEntry records
map.keys()      // Iterator over keys
map.values()    // Iterator over values
map.entries()   // Iterator over MapEntry records
```

Map traversal and transformation are explicit. Direct map iteration and
`map.iter()` produce `MapEntry { key, value }` records. Use views when the
pipeline only needs keys, values, or entries:

```rust
map.values().filter(|v| predicate).map(|v| value).collect_array()
map.entries().find(|entry| predicate)
map.entries().collect_map()
```

Retained eager map helpers are wrappers over the iterator callback engine when
the result should be materialized as a map or scalar immediately:

```rust
map.map_values(|v| ...)
map.filter(|k, v| ...)
map.group_by(|k, v| ...)
map.find(|k, v| ...)
map.any(|k, v| ...)
map.all(|k, v| ...)
map.count(|k, v| ...)
```

Map methods follow the same rule. If `map` has
`TypeFact::Map { key: K, value: V }`, `map.iter()` and `map.entries()` expose
`Iterator(item = MapEntry)` facts, `map.keys()` exposes `Iterator(item = K)`,
and `map.values()` exposes `Iterator(item = V)`. Eager helpers such as
`map.filter(|k, v| ...)` give `k: K`, `v: V`, and return
`Map(key = K, value = V)` as an internal fact only. `map.group_by` uses the
same callback parameter rules and returns
`Map(key = GroupKey, value = Map(key = K, value = V))`, preserving original
entries within each group.

These analysis rules are not user-visible generic syntax. They are part of the
standard library metadata consumed by `vela_analysis` and future LSP tooling.

### Set

```rust
set.len()
set.is_empty()
set.has(value)
set.contains(value)
set.add(value)
set.insert(value)
set.remove(value)
set.extend(other)
set.clear()
set.values()    // Iterator over values
set.iter()      // Iterator over values
```

Set iterator pipelines use the same callback boundary as arrays:

```rust
set.iter().filter(|value| predicate).map(|value| result).collect_array()
set.iter().filter(|value| predicate).collect_set()
```

Retained eager set callback helpers follow the same analysis-only item fact
rules as arrays and are wrappers over the iterator callback engine.

### Option And Result

```rust
enum Option {
    Some(value)
    None
}

enum Result {
    Ok(value)
    Err(error)
}
```

The `?` operator should support Option/Result-style propagation.

Use `Option` when absence is an ordinary script-visible branch, such as
collection lookup or search. Use `Result` when the caller needs a recoverable
failure reason. Runtime traps such as division by zero, type mismatch,
permission denial, budget exhaustion, and future explicit panic-style
operations should return VM diagnostics, not `Result::Err`.

### String

```rust
text.len()
text.is_empty()
text.contains(needle)
text.find(needle) // Option<i64>, byte index
text.starts_with(prefix)
text.ends_with(suffix)
text.strip_prefix(prefix)
text.strip_suffix(suffix)
text.to_upper()
text.to_lower()
text.trim()
text.trim_start()
text.trim_end()
text.replace(old, new)
text.repeat(count)
text.slice(start, end) // byte range, must be UTF-8 boundaries
text.split(separator)
text.split_once(separator) // Option<(String, String)>
text.split_lines()
text.split_whitespace()
text.parse_i8()
text.parse_i16()
text.parse_i32()
text.parse_i64()
text.parse_u8()
text.parse_u16()
text.parse_u32()
text.parse_u64()
text.parse_f32()
text.parse_f64()
text.parse_bool()
text.parse_char()
text.chars() // Iterator over char values
text.bytes() // Iterator over UTF-8 bytes as u8
```

Parsing uses exact primitive names and never performs implicit numeric
conversion. Integer parsers return `Option.None` for invalid text or
out-of-range values. Float parsers return `Option.None` for invalid, `NaN`, or
infinite values. `parse_bool()` accepts only `true` and `false`.
`parse_char()` accepts exactly one Unicode scalar value.

Vela strings follow Rust `str` indexing semantics: `len()` returns byte length,
`find()` returns byte indexes, and `slice(start, end)` uses byte ranges. Strings
remain valid UTF-8, so string slices must start and end on UTF-8 character
boundaries. Character-level traversal should use `for ch in text` or
`text.chars()`, which yield Rust-semantics `char` values. Byte traversal should
use `text.bytes()` and yields `u8` values.

### Char

```rust
ch.to_string()
ch.is_whitespace()
ch.is_ascii()
ch.is_ascii_digit()
```

`char` follows Rust `char` semantics: one Unicode scalar value. It is not a
byte and not a single-character string. Character-level string traversal
returns `char`, and conversion back to `string` is explicit through
`ch.to_string()`.

### Bytes

```rust
bytes.len()
bytes.is_empty()
bytes.slice(start, end)
bytes.get(index)
bytes.read_u32_le(index)
bytes.read_u32_be(index)
bytes.to_hex()
bytes.iter()   // Iterator over u8 values
bytes.values() // Iterator over u8 values
bytes::from_hex(text) -> Result<Bytes, String>
```

Bytes are repeatable byte sequences. Direct `for byte in bytes`,
`bytes.iter()`, and `bytes.values()` all yield `u8` values.

### Numeric Conversions

Numeric conversion is explicit standard-library surface. Type hints and
operators do not widen, narrow, or change float width implicitly.

```text
i64::from_i32(value: i32) -> i64
u64::from_u32(value: u32) -> u64
f64::from_f32(value: f32) -> f64
i16::try_from_i64(value: i64) -> Result<i16, String>
i8::try_from_i64(value: i64) -> Result<i8, String>
u16::try_from_u64(value: u64) -> Result<u16, String>
u8::try_from_u64(value: u64) -> Result<u8, String>
f32::try_from_f64(value: f64) -> Result<f32, String>
```

The widening helpers are infallible. Narrowing helpers return `Result::Ok`
with the narrowed scalar or `Result::Err(String)` when the value is out of
range. `f32::try_from_f64` accepts finite values representable in the finite
`f32` range and rounds to `f32`; non-finite or out-of-range values return
`Result::Err`.

### Numeric Wrapping And Bit Helpers

Wrapping arithmetic and bit manipulation are explicit standard-library
functions in the current checkpoint, not syntax operators and not implicit
overflow behavior.

```text
u8::wrapping_add(lhs: u8, rhs: u8) -> u8
u32::wrapping_mul(lhs: u32, rhs: u32) -> u32
i8::wrapping_add(lhs: i8, rhs: i8) -> i8
u8::bit_and(lhs: u8, rhs: u8) -> u8
u8::bit_or(lhs: u8, rhs: u8) -> u8
u8::bit_xor(lhs: u8, rhs: u8) -> u8
u8::shift_left(value: u8, bits: u32) -> u8
u8::shift_right(value: u8, bits: u32) -> u8
u8::rotate_left(value: u8, bits: u32) -> u8
u8::rotate_right(value: u8, bits: u32) -> u8
```

These helpers use exact primitive contracts from the stdlib manifest. Known
literal arguments are context-typed by those contracts, static mismatches are
compile errors, and dynamic mismatches are runtime guard errors. The `u8`
shift helpers return zero when the shift count is greater than or equal to the
bit width; rotate helpers use native modulo-width rotate semantics.

### Math And Time

```text
math::max
math::min
math::clamp
math::lerp
math::move_towards
math::distance2d
math::distance3d
math::pow
math::sqrt
math::sign
math::floor
math::ceil
math::round
math::abs
math::random  # only with the random capability
```

Time should come from host-provided deterministic time, not direct system time:

```rust
time::now()
time::tick()
time::elapsed_since(start)
```

### IO And Filesystem

I/O is not part of the always-on VM standard natives. Embedders opt in through
engine registration and capabilities:

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .capability(Capability::IoRead)
    .capability(Capability::IoWrite)
    .with_stdio()
    .with_fs_io("scripts/data")
    .build()?;
```

The minimal I/O surface is:

```text
io::print(value)                -> Result<(), IoError>
io::println(value)              -> Result<(), IoError>
fs::read_to_string(path)        -> Result<String, IoError>
fs::write_string(path, text)    -> Result<(), IoError>
```

`fs::*` paths are resolved relative to the configured sandbox root. Absolute
paths and parent-directory escapes are rejected. Runtime permission denial,
type mismatch, and budget exhaustion remain VM diagnostics; ordinary filesystem
failures are script-visible `Result::Err(IoError)` values.

## Embedding API

### Engine

```rust
let mut bindings = VelaBindings::new();
bindings.register_type(Account::vela_type());
bindings.register_type(Invoice::vela_type());
bindings.register_type(Ledger::vela_type());

let engine = Engine::builder()
    .with_standard_natives()
    .register_bindings(bindings)
    .register_reflect_schema::<CustomerView>()
    .register_typed_native_fn::<(String,), _>(
        NativeFunctionDesc::new("audit::log", NativeFunctionId::new(10_001))
            .param("message", TypeHint::String)
            .returns(TypeHint::unit())
            .effects(EffectSet::pure()),
        audit_log,
    )
    .build()?;
```

### Compile

```rust
let program = engine.compile_dir("scripts")?;
let mut runtime = Runtime::new(engine, program);
```

`compile_file(path)` is the single-script mode and ignores the source file name
for module identity; the usual entrypoint is `main`. `compile_dir(root)` is the
module-graph mode: every `.vela` file under `root` becomes a module whose path
is derived from its relative file path, such as `game/reward.vela` becoming
`game::reward`.

### Call

```rust
let args = CallArgs::new()
    .with_host_mut("account", &mut account)
    .with_host_ref("invoice", &invoice)
    .with_value("now", current_tick);

let output = runtime.call(
    "billing::events::on_invoice_paid",
    args,
    CallOptions::unbounded(),
)?;
```

`CallArgs::from_positional` accepts positional `OwnedValue` sequences for
static call sites. Dynamic dispatch should prefer named `CallArgs`: entries are
matched against the target function's parameter names and reordered before
execution, while ordinary script values and host handles can be mixed in the
same argument list.

Direct `CallArgs::with_host_ref("name", &value)` and
`CallArgs::with_host_mut("name", &mut value)` are user-facing embedding
shortcuts. The script still receives a call-scope `HostRef`, not a real Rust
reference. Field reads and writes dispatch through the type's host object
adapter and `HostAccess`; `&T` is read-only, while `&mut T` allows write-through
mutation during the call. A shared origin must be `Sync`; a mutable origin
requires only `Send` and may be non-`Sync` and non-`'static`. The mutable
binding has one exclusive root lease. Shared-receiver Host methods reborrow
`&T` through that guard, so they do not permit a second concurrent call on the
same origin. An async Host method may retain the scoped guard until its `Send`
future completes or is dropped. Hosts that
already manage object identity through a
state adapter can pass an existing low-level handle with
`CallArgs::with_host_handle("name", host_ref)` and attach the adapter to the
same argument owner with `CallArgs::with_fallback_adapter(adapter)`. Runtime
consumes the arguments and composes direct bindings, Runtime extern state, and the
fallback adapter behind one execution-owned `ExecutionHost`.

Erased Host methods receive detached `HostCallValue` arguments. Adapter code
normally calls `decode_host_call_arg::<T>` and
`encode_host_call_return(value)`, which supports the same derived Rust Value
records, enums, tuples, and collections as registered native method thunks.
The call boundary does not retain VM closures, iterators, ranges, or
PathProxies.

`call` returns a runtime-managed `VelaValue`. Hosts can pass it back to later
calls without materializing a detached copy, decode it with `from_value` when
the `serde` feature is enabled, or explicitly call `value_to_owned` when Rust
needs an owned boundary value. Most call sites do not need to construct or pass
a `HostAccess` explicitly.

High-frequency hosts can cache script entry lookup without switching APIs:

```rust
let handle_tick = runtime.entry("handle_tick")?;
let output = runtime.call(&handle_tick, args, CallOptions::unbounded())?;
```

The cached entry belongs to the runtime that created it. If hot reload advances
the active version, a later call through the cached entry re-resolves the
function by name against the current program version; removed or incompatible
functions report the normal runtime or reload errors.

The only asynchronous execution twin is `call_async`, and it accepts the same
function, bound-method, and provider-method targets as `call`. A sync call
rejects a declared async entry before executing its body. The scoped `Send`
future enters the shared frame driver, executes awaited sync targets inline,
and suspends through ordinary executor polling for registered Rust futures.

The CLI owns a minimal executor only when `--async` is selected; its default
path remains synchronous and reports that flag when `main` is async. The
synchronous C ABI has no poll/waker protocol and returns
`VelaStatus::AsyncEntry` plus a descriptive error string for async entries.

Stateful async Rust methods may hold direct host leases across ordinary Rust
await points and reenter Vela through their `NativeCallContext`. A mutable
receiver is reborrowed explicitly into the child scope:

```rust,ignore
ctx.call_async(
    "hooks::after_update",
    CallArgs::new().with_host_mut("state", &mut *state),
).await?;
```

The embedding must keep Runtime storage disjoint from host storage. Split the
fields before calling so borrowing Runtime does not also borrow the struct that
is passed as host state:

```rust,ignore
let Actor { runtime, state, services } = actor;
runtime.call_async(
    "handle",
    CallArgs::new()
        .with_host_mut("state", state)
        .with_host_ref("services", services),
    CallOptions::unbounded(),
).await?;
```

This is a Rust ownership requirement, not an actor-specific Runtime API.

With the `serde` feature enabled, hosts can pass ordinary Rust data as
script-owned values without registering it as host state:

```rust
#[derive(Serialize, Deserialize)]
struct DamageEvent {
    amount: i64,
}

let args = CallArgs::new().with_serde_value("event", &event)?;
let output = runtime.call("handle_damage", args, CallOptions::unbounded())?;
let result: DamageResult = runtime.from_value(&output)?;
```

Serde struct values become Vela records so scripts can use dot field access.
Serde enum values become Vela enum values. This path copies data into the VM;
it is intended for messages, configs, snapshots, and results. It does not
mutate the original Rust struct when scripts write to the script value.
Write-through Rust state should still be passed with `with_host_ref`,
`with_host_mut`, or adapter-backed host handles.

Detached `OwnedValue::Map` stores key-preserving entries rather than a
string-key object map. String-key serde maps may still become object-shaped
maps at host boundaries, but non-string keys serialize as owned key values and
must round-trip without stringification. Runtime insertion still applies the
normal `ValueKey` keyability checks before a script map is mutated or
allocated. Reflection map reads expose the same key-preserving entry shape
instead of converting maps through string-field records.

Native functions may return `OwnedValue::Iterator(...)` when a host wants to
provide copied iterable data without first materializing a script array. This is
a snapshot boundary: items are converted to VM-owned values, `HostRef`, or
`PathProxy` handles before script iteration begins. Persistent Rust iterator
handles are deferred until lifetime, invalidation, and hot-reload diagnostics
are explicit.

The same owned-value conversion model is used when Rust explicitly replaces a
VM state cell:

```rust
runtime.set_state("main::state", owned_record!("State", {
    "level" => 1,
}))?;
runtime.set_state("main::state", &serde_state)?;
runtime.set_state("main::state", runtime_value)?;
```

`OwnedValue` is inserted directly, serde values are passed by reference and
serialized into script-owned records/enums, and a `VelaValue` from the same
runtime is attached as a state root without first materializing an
`OwnedValue`.

Returned script aggregates stay under VM management by default and can be
passed back to another script call without materializing a detached copy:

```rust
let reward = runtime.call("make_reward", CallArgs::new(), options)?;
let score = runtime.call(
    "score_reward",
    CallArgs::new().with_vela_value(reward.clone()),
    options,
)?;
let owned_score = runtime.value_to_owned(&score)?;
let typed_score: Score = runtime.from_value(&score)?;
```

`VelaValue` belongs to the `Runtime` that returned it. It can be cloned and
passed back to calls on that same runtime; Rust calls `value_to_owned` only
when it needs an owned, heap-detached value. With the `serde` feature enabled,
`Runtime::from_value` can deserialize a `VelaValue` directly from the runtime
heap into a Rust struct, enum, or scalar without first materializing an
`OwnedValue`. VM-managed state has the same typed read surface through
`Runtime::state_as`.

Rust can also call script methods registered on the runtime value's script
type. Methods are still type-level script methods, not per-value monkey
patches:

```rust
let reward = runtime.call("make_reward", CallArgs::new(), options)?;
let score_target = runtime.bind_method(&reward, "score")?;
let score = runtime.call(
    score_target,
    CallArgs::new().with_value("bonus", 5),
    options,
)?;

let score_method = runtime.method(&reward, "score")?;
let fast_target = runtime.bind_method(&reward, &score_method)?;
let fast_score = runtime.call(fast_target, args, options)?;
```

`bind_method` produces the receiver-bound call target; `method` optionally
caches the owner script type and stable method ID before binding. Calls validate
that the receiver still belongs to the same runtime and has the expected script
type; hot reload re-resolves the method target by stable method ID on the
current program version. Provider methods use the same `call`/`call_async`
surface through their bound provider-method target.

### Hot Reload

```rust
let program = engine.compile_source(initial_source)?;
let mut runtime = Runtime::builder(engine, program)?
    .with_hot_reload()?
    .build()?;

runtime.stage_reload(ReloadSource::file("scripts/combat.vela"))?;

if let Some(report) = runtime.activate_reload()? {
    if !report.accepted {
        log::error!("hot reload failed: {:#?}", report.errors);
    }
}
```

Runtime update compilation uses the runtime's active `ProgramVersion`, so hosts
do not need to separately fetch the current version before compiling an update.
Source load and path errors are returned immediately, while accepted updates and
ABI or policy rejections are staged until the host calls
`runtime.activate_reload()` at a caller-selected safe point. Host mutations
write through during the call, so reload activation is separate from host state
mutation.

`Runtime::stage_reload` accepts text directly and uses `ReloadSource::file`,
`ReloadSource::directory`, or `ReloadSource::changed_file` for filesystem
workflows. A changed-file update still recompiles the full module root so
imports, dependency impact, and ABI checks use one complete graph.

Hot-reload ABI manifests copy optional declaration spans from reflected schema,
function, and method descriptors. When schema, function effect/access, or method
effect/access ABI checks reject an update, the rejected diagnostic points at the
new declaration span when it is known, and rendered report lines preserve that
span for editor/admin tooling.
