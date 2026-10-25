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
returns the initial value without invoking the callback. Script iterator steps
and callback work retain per-item budget accounting.

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

Build an Engine with the retained standard natives and explicitly registered
synchronous owned-value natives. Native host-read/write effects and permissions
remain; value-only arguments do not make an application-state operation pure.
See [native registration](host-and-registration.md) and
[Rust/Vela interop](../rust-vela-interop.md).

### Compile

`compile_file` is single-script mode; `compile_dir` assembles a module graph
from relative `.vela` paths. Ordinary packages/workspaces/path dependencies use
the shared manifest loader and package-aware compiler/linker. Runtime execution
requires a verified linked artifact, not an unlinked alternate interpreter.

### Call

`Runtime::call` executes synchronously using `CallArgs` and `CallOptions`.
Arguments use existing owned conversions or script values retained by the same
Runtime. Select finite execution/memory/depth/collection limits for production.
Cached entry handles validate Runtime ownership and resolve new entry calls
against the active accepted generation; they are not retained closures.

Serde inputs (`with_serde_value`, with the `serde` feature) become script-owned
records/enums/collections. A Rust reference used by serialization is not injected
into script. Script edits do not mutate the original Rust source. Rust explicitly
receives a result and decodes it with `from_value` or detaches it with
`value_to_owned`. Maps preserve non-string keys rather than stringifying them.

`call` returns a runtime-managed `VelaValue`. Cloned retained values can be
passed into later calls on the same Runtime without detaching a copy; another
Runtime cannot use them. `bind_method` binds a script receiver, and `method`
can cache its script type/stable method identity. These are script methods,
not Rust object methods or per-value monkey patches.

VM-owned `state` supports explicit reads/updates, including existing owned,
serde and same-Runtime retained value inputs. Compatible cells survive reload.
There are no extern host bindings, borrowed input/result/view APIs, host-object
factories, leases or transparent write-through paths. No independent opaque
runtime survived D; replacement objects/conversion/re-entry design is deferred.

Existing synchronous context-aware native re-entry shares its call session and
pinned executable owner, budgets, permissions and roots. This pass does not add
a new re-entry model. Owned iterator snapshots contain copied script data;
script collections and lazy script iterators remain supported, but borrowed Rust
iterators do not.

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
`runtime.activate_reload()` at a caller-selected safe point. Native effects
completed during a call are not rolled back by reload; script-owned snapshots
have no implicit Rust write-back.

`Runtime::stage_reload` accepts text directly and uses `ReloadSource::file`,
`ReloadSource::directory`, or `ReloadSource::changed_file` for filesystem
workflows. A changed-file update still recompiles the full module root so
imports, dependency impact, and ABI checks use one complete graph.

Hot-reload ABI manifests copy optional declaration spans from reflected schema,
function, and method descriptors. When schema, function effect/access, or method
effect/access ABI checks reject an update, the rejected diagnostic points at the
new declaration span when it is known, and rendered report lines preserve that
span for editor/admin tooling.
