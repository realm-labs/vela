# Value-Keyed Map And Set Implementation Plan

> **Track:** M20 type-contract and collection fast-path continuation
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release VM, runtime, stdlib,
> `OwnedValue`, serde, docs, and tests are allowed. Do not preserve old
> string-only map keys or vector-scanned set internals only for compatibility.
> Preserve product contracts: no user-defined script generics, no Rust `&mut`
> exposure, HostAccess safety, GC roots, source-spanned diagnostics, execution
> budgets, reflection permissioning, and hot-reload ABI/schema checks.

---

## 0. Codex Goal

```text
/goal Implement Vela's value-keyed Map and Set architecture from
docs/value-keyed-map-set-plan.md. Treat docs/goal.md as the product roadmap,
docs/architecture.md and docs/architecture/*.md as the architecture contract,
and docs/progress.md as the current milestone state. Replace the current
string-keyed map and vector-scanned set internals with focused ScriptMap,
ScriptSet, and ValueKey modules. Map keys and Set elements are runtime Values,
but key equality is defined only by ValueKey and follows stable key classes:
immutable leaf values compare by value, script heap objects and host refs
compare by identity, and unsupported transient values are rejected before
mutation. ValueKey does not call user `PartialEq`, `Eq`, `PartialOrd`, `Ord`,
or any future script-visible `Hash` implementation.
Propagate the new keyable contract through syntax, type hints, runtime guards,
container summaries/stamps, stdlib methods, reflection, OwnedValue, serde
bridges, benchmarks, docs, and tests. Prefer clean replacement over
compatibility shims. Validate each checkpoint with focused tests, benchmark
captures for map/set workloads, and the relevant workspace checks; commit small
Conventional Commit checkpoints.
```

---

## 1. Purpose

Current runtime maps and sets are asymmetric:

```rust
HeapValue::Map(BTreeMap<String, Value>)
HeapValue::Set(Vec<Value>)
```

Map values and Set elements already store runtime `Value`, but Map keys are
limited to `String`, and Set uniqueness is implemented by scanning a `Vec<Value>`
with duplicated `SetKey` helpers. This keeps common string-keyed maps working,
but it blocks clean `Map<Player, V>` / `Set<Player>` identity semantics and
makes Set lookup/removal O(n).

The goal is to make Map and Set use the same key model:

```text
Map<K, V> stores original key Values and value Values.
Set<T> stores original element Values.
Lookup, uniqueness, and removal are driven by ValueKey.
```

`ValueKey` follows the object equality model in
[object-equality-semantics-plan.md](object-equality-semantics-plan.md).
Mutable script structs/records, arrays, maps, sets, enums, closures, iterators,
and host objects use identity equality, not deep structural equality. A record
can be stored in a Set and looked up efficiently by the same object identity
even if fields later mutate.

This plan intentionally keeps container key semantics separate from semantic
object equality and ordering. User-defined or derived `PartialEq`/`Eq`/
`PartialOrd`/`Ord` may affect `==`, ordering operators, and sorting, but it
must not affect Map lookup, Set uniqueness, or deterministic container
iteration.

---

## 2. Goals

- Introduce a single `ValueKey` implementation for all script Map and Set key
  operations.
- Replace `HeapValue::Map(BTreeMap<String, Value>)` with a `ScriptMap` heap
  object that stores entries by `ValueKey`.
- Replace `HeapValue::Set(Vec<Value>)` with a `ScriptSet` heap object that
  stores elements by `ValueKey`.
- Preserve original key/element `Value`s for iteration, `keys()`, `entries()`,
  reflection, serialization, and GC tracing.
- Support value-key equality for immutable/key-stable leaf values:
  `null`, `bool`, `char`, scalar numeric tags, `String`, and `Bytes`.
- Support identity-key equality for script heap aggregate values and host refs:
  records, enums, arrays, maps, sets, closures, iterators, and `HostRef`.
- Reject transient or non-data values as keys: `Missing` and `PathProxy`.
- Keep record/struct keys efficient by using identity, not field-by-field
  comparison.
- Keep Map/Set lookup independent from user `PartialEq`, `Eq`, `PartialOrd`,
  `Ord`, and future script-visible `Hash`.
- Extend `Map<K, V>` and `Set<T>` type hints to use the same keyable policy as
  runtime `ValueKey`.
- Preserve fast typed-container mutation rules: statically proven key/value
  contracts skip guards; dynamic values are guarded before mutation.
- Keep string-key map workloads measurable and avoid accidental regressions in
  the existing benchmark suite.

---

## 3. Non-Goals

This pass must not:

- Add general user-defined generics.
- Add structural equality for mutable records, arrays, maps, sets, closures,
  iterators, or host objects.
- Make Map or Set keys depend on a value that can later mutate by content.
- Make Map or Set keys depend on user-defined or derived comparison traits.
- Add script-visible `Hash` or make container indexes call user hash code.
- Treat two independently constructed records with identical fields as the
  same Set element or Map key.
- Allow `PathProxy` to become a Map/Set key before there is an explicit host
  path identity policy.
- Silently coerce map keys, such as converting `1u64` to `1i64` or `"1"` to
  `1`.
- Preserve the old `BTreeMap<String, Value>` or `Vec<Value>` container shapes
  behind compatibility adapters.
- Weaken GC tracing, budget charging, hot-reload ABI checks, reflection
  permissions, or source-spanned runtime errors.

---

## 4. Key Semantics

### 4.1 ValueKey

Add a focused VM module, for example `crates/vela_vm/src/value_key.rs`:

```rust
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ValueKey {
    Null,
    Bool(bool),
    Char(char),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(u32),
    F64(u64),
    String(String),
    Bytes(Vec<u8>),
    HeapIdentity(GcRef),
    HostIdentity(HostRef),
}
```

`ValueKey::from_value(value, heap, operation)` should return a source-spanned
VM error when the value is not keyable. It should not allocate script heap
objects. It may clone immutable string/bytes payloads into the key.

For keyable leaf values, `ValueKey` should agree with builtin leaf-value
equality from
[object-equality-semantics-plan.md](object-equality-semantics-plan.md): values
that compare equal by builtin leaf equality should map to the same key. For
objects, `ValueKey` uses identity even when the object implements semantic
`PartialEq`. `ValueKey` remains a separate layer because it also defines
keyability, internal ordering, and NaN rejection.

### 4.2 Scalar equality

Scalar keys are tag-exact:

```text
i64(1) != u64(1)
f32(1.0) != f64(1.0)
```

Finite floating-point values are keyable by canonical bits. This is only a
Map/Set key policy; it does not define the script-level `PartialEq`/`Eq`/
`PartialOrd`/`Ord` contracts for floats. Reject `NaN` because it is not a
stable equality key. Normalize `-0.0` and `0.0` to the same key so numeric
equality does not surprise users.

### 4.3 String and bytes equality

`String` and `Bytes` use value equality. The key owns the string or byte
payload at insertion/lookup time. This is valid only because these are leaf
data values; if a future mutable bytes API is added, it must either preserve
key immutability or switch bytes keys to identity.

### 4.4 Identity equality

All mutable or potentially large script heap aggregates use identity equality:

```text
Array
Map
Set
Record / struct
Enum
Closure
Iterator
```

`HostRef` uses host identity. The identity key includes the host ref identity
already carried by `HostRef`, including generation. A stale host ref key remains
the key that was inserted; ordinary host access still rejects stale reads,
writes, and calls.

`PathProxy` is rejected as a key until Vela has a separate host path identity
contract. `Missing` is rejected as a key.

### 4.5 User-visible record example

```vela
struct Player {
    id: i64,
    level: i64,
}

fn main() -> bool {
    let a = Player { id: 1, level: 10 };
    let b = Player { id: 1, level: 10 };
    let seen: Set<Player> = Set::new();

    seen.add(a);
    a.level += 1;

    return seen.has(a) && !seen.has(b);
}
```

The lookup is efficient because it compares `HeapIdentity(GcRef)`. It does not
scan fields.

Even if `Player` later derives or implements semantic `PartialEq`/`Eq`, the
set above still uses identity. A business-keyed collection should store a
stable field such as `player.id` as the key instead of relying on object
equality.

---

## 5. Runtime Container Shape

### 5.1 ScriptMap

Replace the map heap payload with a focused container type:

```rust
pub(crate) struct ScriptMap {
    entries: BTreeMap<ValueKey, MapEntry>,
}

pub(crate) struct MapEntry {
    key: Value,
    value: Value,
}
```

`ScriptMap` owns all map operations:

```rust
len
is_empty
get(key, heap)
insert(key, value, heap, budget)
remove(key, heap, budget)
contains_key(key, heap)
keys()
values()
entries()
merge/extend
clear
```

The public VM code should not inspect the internal `BTreeMap` directly except
inside focused map modules and tests.

### 5.2 ScriptSet

Replace the set heap payload with a focused container type:

```rust
pub(crate) struct ScriptSet {
    entries: BTreeMap<ValueKey, Value>,
}
```

`ScriptSet` owns:

```rust
len
is_empty
has(value, heap)
add(value, heap, budget)
remove(value, heap, budget)
extend(values, heap, budget)
values()
union/intersection/difference
clear
```

This removes repeated O(n) scans for `has`, `add`, `remove`, and set
combination methods. Initial lookup complexity is O(log n) through `BTreeMap`.
If benchmark evidence later requires average O(1), switch the backend behind
`ScriptMap`/`ScriptSet` without exposing a new VM representation.

### 5.3 Deterministic iteration

Use `BTreeMap<ValueKey, _>` in the first implementation to preserve
deterministic iteration by key order. The ordering is an implementation detail,
not a source-level sorting guarantee, but deterministic output keeps tests,
diagnostics, and replay behavior stable.

This internal ordering is not user `PartialOrd` or user `Ord`. Changing or
adding a user ordering implementation must not reorder existing Map/Set
entries. If later benchmarks justify `HashMap` or `IndexMap` behind
`ScriptMap`/`ScriptSet`, script-visible key semantics should remain the same
unless the language explicitly documents a new iteration-order contract.

---

## 6. GC, Budget, And Memory Accounting

Map and Set must trace original stored `Value`s:

```text
ScriptMap traces entry.key and entry.value.
ScriptSet traces each stored value.
ValueKey itself does not need to trace when the original Value is stored.
```

This matters for identity keys: a `ValueKey::HeapIdentity(reference)` must not
be the only live reference to the heap object. The entry key or set element
`Value::HeapRef(reference)` keeps the object rooted through ordinary tracing.

Budget accounting must include:

```text
new map/set entry count
stored Value slots
owned string/bytes key payloads cloned into ValueKey
ScriptMap/ScriptSet shallow structure
```

Map replacement should invalidate value summaries but should not charge a new
entry count. Key replacement is not a separate operation: inserting an existing
key replaces the entry value and may replace the stored original key Value only
if that behavior is intentionally specified and tested. The cleaner default is
to preserve the first inserted key Value for iteration and replace only the
value.

---

## 7. Type Hint Contract Changes

### 7.1 Keyable type policy

Move keyability into one shared policy concept:

```text
runtime ValueKey::from_value
parser type-hint validation
engine metadata validation
macro inferred metadata
guard-plan construction
diagnostics
docs
```

Static keyable type hints include:

```text
null
bool
char
i8 i16 i32 i64
u8 u16 u32 u64
f32 f64
String
Bytes
script record/struct types
script enum types
host types
Array Map Set Option Result Iterator Closure when represented by runtime
identity and accepted by the corresponding runtime guard
Any as a dynamic key contract
```

Callable identity requires a separate explicit contract. Reject `Function` as a
keyable type hint until callable identity is implemented and tested.

### 7.2 Map hints

Allow:

```vela
Map<String, i64>
Map<i64, Player>
Map<Player, i64>
Map<Any, Any>
Map<Array, String>
```

`Map<K, V>` means:

```text
all current keys satisfy K
all future inserted keys must satisfy K
all current values satisfy V
all future inserted values must satisfy V
```

Unparameterized `Map` remains an erased map category contract.

### 7.3 Set hints

Allow:

```vela
Set<i64>
Set<String>
Set<Player>
Set<Any>
Set<Array>
```

`Set<T>` means every stored element satisfies `T` and is keyable under
`ValueKey`.

Unparameterized `Set` remains an erased set category contract.

### 7.4 Rejected hints

Reject non-keyable key contracts with source-spanned diagnostics:

```vela
Map<PathProxy, i64>
Set<PathProxy>
Set<Function>
```

---

## 8. Compiler And Guard Rules

### 8.1 Trusted facts

Keep the existing typed-container rule:

```text
static compatible fact    -> no runtime guard
static incompatible fact  -> compile error
dynamic or erased fact    -> runtime guard
```

For map/set mutation:

```text
proven key contract       -> skip key guard
dynamic key               -> guard key before mutation
proven value contract     -> skip value guard
dynamic value             -> guard value before mutation
non-keyable runtime key   -> runtime error before mutation
```

The keyability check is always required before mutation unless the compiler can
prove the key type is keyable and the value is already represented in a keyable
runtime category.

### 8.2 Guard plans

Map contracts carry both key and value plans:

```rust
TypeGuardPlan::Map {
    key: Option<Box<TypeGuardPlan>>,
    value: Option<Box<TypeGuardPlan>>,
}
```

The current value-only map guard path should become a true key/value guard.
`Map<Any, V>` skips key type refinement but still relies on `ScriptMap`
construction to guarantee keyability.

### 8.3 Summaries and stamps

Container summaries must track both key and value summaries for maps:

```text
ScriptMap key_summary
ScriptMap value_summary
ScriptSet value_summary
```

Identity keys should summarize to their shallow runtime type:

```text
Player record identity -> Shape(type_id, shape_id)
Array identity         -> Standard(Array)
HostRef identity       -> Host(type_id) when host type metadata is available
```

Repeated `Map<K, V>` / `Set<T>` boundary checks should use summary/stamp
metadata before falling back to budget-charged scans.

---

## 9. Stdlib And Syntax Surface

### 9.1 Indexing

`map[key]` and indexed assignment should use `ValueKey::from_value(key, heap)`.
String-literal indexing can stay a fast path but should lower into the same
logical key model.

```vela
scores[player] = 10; // insert or replace

let current = scores.get(player).unwrap_or(0);
scores[player] = current + 1;
```

If the key is not keyable, the operation fails before mutation. Compound
indexed assignment is read-modify-write:

```vela
scores[player] += 1;
```

It requires the key to already exist. Missing keys fail with a runtime
key-not-found error; they do not create implicit zero, empty string, empty
array, or other default values. Entry-style mutation such as
`entry(key).or_insert(value)` requires a separate writable entry-proxy design
and is not part of this plan.

### 9.2 Map methods

Update map methods to accept runtime key Values:

```text
get(key)
has(key)
set(key, value) / insert(key, value)
remove(key)
keys()
values()
entries()
merge(other)
extend(other)
clear()
```

`keys()` returns the stored original key Values in deterministic key order.
`entries()` returns records or arrays that preserve both original key and
value. Do not stringify keys unless the method name explicitly says it does.

### 9.3 Set methods

Update set methods to use `ScriptSet`:

```text
has(value)
add(value)
remove(value)
extend(other)
union(other)
intersection(other)
difference(other)
values()
clear()
```

`add` returns whether a new key was inserted. Adding the same record identity
twice returns false. Adding a different record with identical fields returns
true because identity differs.

### 9.4 Literals and constructors

Existing map literals may remain string-keyed if the syntax only supports
field-like or string keys. Arbitrary key maps can still be built through
indexing or methods. Computed-key map literal syntax is a separate language
surface decision and should not block the runtime refactor.

Set constructors and `set::from_array` should use `ValueKey` so duplicate
identity/value keys are removed without O(n) scans.

---

## 10. OwnedValue, Reflection, And Serde

### 10.1 OwnedValue

Current detached maps are string-keyed:

```rust
OwnedValue::Map(BTreeMap<String, OwnedValue>)
```

Value-keyed maps need to preserve arbitrary key Values. Replace this with a
detached entry representation or split object-style maps from script maps:

```rust
OwnedValue::Map(Vec<OwnedMapEntry>)

pub struct OwnedMapEntry {
    pub key: OwnedValue,
    pub value: OwnedValue,
}
```

If a string-key object representation remains useful for serde ergonomics, make
it explicit rather than overloading script maps:

```rust
OwnedValue::Object(BTreeMap<String, OwnedValue>)
OwnedValue::Map(Vec<OwnedMapEntry>)
```

This is a breaking pre-release boundary change and should be done cleanly.

### 10.2 Reflection

Reflection reads of maps and sets must return key-preserving values. Reflection
must not mutate map/set type structure. Controlled writes/calls still flow
through runtime methods and `ValueKey` checks.

### 10.3 Serde

Serde object maps are string-keyed by default. Non-string keyed script maps
cannot silently serialize as JSON objects without losing key type information.

Rules:

```text
string-key script maps may serialize as serde maps/objects
non-string-key script maps require an explicit entry-list representation or
return a serialization error
deserializing Rust maps with non-string keys should construct key Values when
the key serializer can represent them exactly
```

This must be tested so host boundary behavior is predictable.

---

## 11. Hot Reload And ABI

Hot-reload ABI comparisons already treat type-hint display as a contract. After
this change:

```text
Map<String, V> -> Map<Player, V> is an ABI change
Set<String>    -> Set<Player> is an ABI change
Map<K, V>      -> Map<K, Any> follows existing return/parameter ABI policy
```

The structured type-hint metadata should be compared structurally, not by
ad-hoc display strings. Display strings remain diagnostics/docs output.

---

## 12. Implementation Phases

### Phase 1: ValueKey core

- Add `value_key.rs`.
- Centralize current duplicated `SetKey` behavior behind `ValueKey`.
- Add tests for scalar, string, bytes, heap identity, host identity,
  non-keyable `Missing`, non-keyable `PathProxy`, float `NaN`, and `-0.0`.
- Add tests proving user comparison-trait implementations do not affect
  `ValueKey::from_value` output or Map/Set lookup.

Validation:

```bash
cargo test -p vela_vm value_key
```

### Phase 2: ScriptSet

- Add `script_set.rs`.
- Change `HeapValue::Set(Vec<Value>)` to `HeapValue::Set(ScriptSet)`.
- Move add/remove/has/extend/combination methods to `ScriptSet`.
- Update GC tracing, shallow size, budget growth, iteration, reflection,
  runtime value serde, and materialization.
- Remove duplicated `SetKey` modules.

Validation:

```bash
cargo test -p vela_vm set
cargo test -p vela_vm container_contracts
```

### Phase 3: ScriptMap

- Add `script_map.rs`.
- Change `HeapValue::Map(BTreeMap<String, Value>)` to
  `HeapValue::Map(ScriptMap)`.
- Update indexing, map methods, map mutation cache, map materialization,
  iteration, reflection, runtime value serde, and map construction helpers.
- Preserve or replace the string-literal key fast path through the `ScriptMap`
  API, not by keeping old map internals.

Validation:

```bash
cargo test -p vela_vm map
cargo test -p vela_vm indexing
cargo test -p vela_vm external_compare_contract
```

### Phase 4: Type hints and guards

- Expand parser/engine/macro keyable policy for `Map<K, V>` and `Set<T>`.
- Update HIR, analysis TypeFacts, compiler RuntimeTypeFacts, guard plans, and
  VM guard execution so map keys are checked as well as values.
- Ensure `Set<Player>` and `Map<Player, V>` use identity semantics at runtime.
- Ensure `Set<Player>` and `Map<Player, V>` continue to use identity semantics
  even when `Player` implements or derives semantic comparison traits.
- Ensure typed mutation checks guard dynamic keys and values before mutation.

Validation:

```bash
cargo test -p vela_syntax -p vela_hir -p vela_analysis -p vela_bytecode
cargo test -p vela_engine -p vela_macros
cargo test -p vela_vm type_guards
```

### Phase 5: OwnedValue, reflection, serde, docs

- Replace or split `OwnedValue::Map` so arbitrary key maps can materialize
  without losing key type.
- Update reflection value conversion.
- Update serde behavior for string-key and non-string-key script maps.
- Update website docs and architecture docs.

Validation:

```bash
cargo test -p vela_vm owned_boundary
cargo test -p vela_vm value_access
cargo test -p vela_reflect
cargo test -p vela_engine
```

### Phase 6: Benchmarks and profiling

- Add focused benchmark rows:
  - `map_string_key_lookup_update`
  - `map_i64_key_lookup_update`
  - `map_record_identity_lookup_update`
  - `set_i64_lookup_mutation`
  - `set_string_lookup_mutation`
  - `set_record_identity_lookup_mutation`
- Capture before/after results under `perf-results/` or `perf-baselines/`
  according to the existing performance workflow.
- Profile the largest regression or remaining gap before optimizing.

Validation:

```bash
cargo bench -p vela_vm --bench external_compare -- --quick map
cargo bench -p vela_vm --bench external_compare -- --quick set
```

### Phase 7: Full validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 13. Acceptance Criteria

- `Set<Player>` stores script records by identity.
- `set.has(player)` is O(log n) through the container index, not O(n) scan.
- Two different records with identical fields are different Set elements.
- Mutating a record after insertion does not break Set or Map lookup.
- `Map<Player, i64>` supports get/set/remove with record identity keys.
- User comparison-trait implementations do not affect Map lookup, Set
  uniqueness, or Map/Set iteration order.
- `f32`/`f64` finite values can be used as `ValueKey` keys according to the
  finite-float key policy, but this does not make float arrays sortable or make
  floats satisfy semantic `Ord`.
- Existing string-key map behavior still works through the new `ScriptMap`
  implementation.
- Dynamic non-keyable values fail before mutation.
- Map and Set type hints, guards, container summaries, mutation checks, hot
  reload ABI, reflection, and docs agree on keyability.
- `OwnedValue` and serde do not silently lose non-string map keys.
- Benchmark captures show the Set lookup/mutation path no longer scales by
  linear scan for keyable values.
