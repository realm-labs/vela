## Standard Library

### Array

```rust
arr.len()
arr.is_empty()
arr.push(value)
arr.pop()
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

Array methods should expose analysis-only signatures so LSP can infer lambda
parameter facts without adding script generics. For example, if `arr` has
`TypeFact::Array { element: E }`, then:

```text
arr.filter(|x| predicate) gives x: E and returns Array(element = E)
arr.map(|x| value) gives x: E and returns Array(element = TypeFact(value))
arr.find(|x| predicate) gives x: E and returns Option-like enum containing E
arr.sum(|x| value) gives x: E and returns int or float depending on value
```

### Map

```rust
map.len()
map.has(key)
map.get(key)
map.get_or(key, default)
map.set(key, value)
map.remove(key)
map.keys()
map.values()
map.entries()
map.map_values(|v| ...)
map.filter(|k, v| ...)
```

Map methods follow the same rule. If `map` has
`TypeFact::Map { key: K, value: V }`, `map.filter(|k, v| ...)` gives `k: K`,
`v: V`, and returns `Map(key = K, value = V)` as an internal fact only.

These analysis rules are not user-visible generic syntax. They are part of the
standard library metadata consumed by `vela_analysis` and future LSP tooling.

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
text.find(needle)
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
text.slice(start, end)
text.char_at(index)
text.split(separator)
text.split_once(separator)
text.split_lines()
text.split_whitespace()
text.parse_int()
text.parse_float()
text.parse_bool()
```

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

## Embedding API

### Engine

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .register_host_type::<Account>()
    .register_host_type::<Invoice>()
    .register_host_type::<Ledger>()
    .register_reflect_schema::<CustomerView>()
    .register_typed_native_fn::<(String,), _>(
        NativeFunctionDesc::new("audit::log", NativeFunctionId::new(10_001))
            .param("message", TypeHint::String)
            .returns(TypeHint::Null)
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

`Runtime::call` still accepts positional `OwnedValue` slices for static call
sites. Dynamic dispatch should prefer `CallArgs`: named entries are matched
against the target function's parameter names and reordered before execution,
while ordinary script values and host handles can be mixed in the same argument
list.

Direct `CallArgs::with_host_ref("name", &value)` and
`CallArgs::with_host_mut("name", &mut value)` are user-facing embedding
shortcuts. The script still receives a call-scope `HostRef`, not a real Rust
reference. Field reads and writes dispatch through the type's host object
adapter and `HostAccess`; `&T` is read-only, while `&mut T` allows write-through
mutation during the call. Hosts that already manage object identity through a
state adapter can pass an existing low-level handle with
`CallArgs::with_host_handle("name", host_ref)` and call
`runtime.call_with_adapter` with that adapter.

`call` returns `CallOutput`, which dereferences to the returned
`OwnedValue` for ordinary use. Most call sites do not need to construct or pass
a `HostAccess` explicitly.

With the `serde` feature enabled, hosts can pass ordinary Rust data as
script-owned values without registering it as host state:

```rust
#[derive(Serialize, Deserialize)]
struct DamageEvent {
    amount: i64,
}

let args = CallArgs::new().with_serde_value("event", &event)?;
let output = runtime.call("handle_damage", args, CallOptions::unbounded())?;
let result: DamageResult = from_owned_value(output.value())?;
```

Serde struct values become Vela records so scripts can use dot field access.
Serde enum values become Vela enum values. This path copies data into the VM;
it is intended for messages, configs, snapshots, and results. It does not
mutate the original Rust struct when scripts write to the script value.
Write-through Rust state should still be passed with `with_host_ref`,
`with_host_mut`, or adapter-backed host handles.

When the host wants to keep a returned script aggregate under VM management and
pass it back to another script call without materializing a detached copy, it
uses `Runtime::call_value`:

```rust
let reward = runtime.call_value("make_reward", CallArgs::new(), options)?;
let score = runtime.call_value(
    "score_reward",
    CallArgs::new().with_vela_value(reward.clone()),
    options,
)?;
let owned_score = runtime.value_to_owned(&score)?;
```

`VelaValue` belongs to the `Runtime` that returned it. It can be cloned and
passed back to calls on that same runtime; Rust calls `value_to_owned` only
when it needs an owned, heap-detached value.

### Hot Reload

```rust
runtime
    .stage_hot_reload_update_file("scripts/combat.vela")?
    ?;

if let Some(report) = runtime.check_reload()? {
    if !report.accepted {
        log::error!("hot reload failed: {:#?}", report.errors);
    }
}
```

Runtime update compilation uses the runtime's active `ProgramVersion`, so hosts
do not need to separately fetch the current version before compiling an update.
Source load and path errors are returned immediately, while accepted updates and
ABI or policy rejections are staged until the host calls `runtime.check_reload()`
at a safe point. Tick-loop hosts can call
`runtime.check_reload_at_tick_boundary()` when no event boundary is active. Host
mutations write through during the call, so reload checks are separate from host
state mutation.

For full module-root workflows, hosts can call
`runtime.stage_hot_reload_update_dir("scripts")` with the same safe-point
semantics. For file-watcher workflows, hosts may stage an update from a changed
`.vela` file inside a module root. The engine validates the changed path and
recompiles the full root so imports, module dependency impact, and ABI checks
are based on the same complete module graph as directory reloads.

Hot-reload ABI manifests copy optional declaration spans from reflected schema,
function, and method descriptors. When schema, function effect/access, or method
effect/access ABI checks reject an update, the rejected diagnostic points at the
new declaration span when it is known, and rendered report lines preserve that
span for editor/admin tooling.
