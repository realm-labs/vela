# Object Equality And Ordering Semantics Implementation Plan

> **Track:** M20 runtime semantics and collection-key continuation
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release equality behavior is allowed.
> Do not preserve the current materialize-then-compare fallback if it conflicts
> with the language semantics below. Preserve product contracts: source-spanned
> diagnostics, execution budgets, GC roots, HostAccess safety, reflection
> permissioning, hot-reload ABI/schema checks, and no Rust `&mut` exposure.

---

## 0. Codex Goal

```text
/goal Implement Vela's object equality and ordering semantics from
docs/object-equality-semantics-plan.md. Treat docs/goal.md as the product
roadmap, docs/architecture.md and docs/architecture/*.md as the architecture
contract, and docs/progress.md as the current milestone state. Replace
accidental structural equality caused by OwnedValue materialization with a
focused runtime equality module and closed builtin operator-trait hooks.
`PartialEq` drives semantic `==`/`!=`, `Eq` marks full equivalence,
`PartialOrd` drives source-level ordering operators, and `Ord` drives total
ordering and sorting. Missing static support is a compile-time error where the
compiler can prove it. Dynamic values perform runtime trait checks with
source-spanned failures. Deep structural comparison, if added, must be an
explicit budgeted helper, not the default equality operator. Keep Map/Set
ValueKey semantics separate from user `PartialEq`/`Eq`/`PartialOrd`/`Ord`:
containers use stable key equivalence, not business equality or ordering.
Validate with focused VM tests, collection method tests, Map/Set key tests,
docs, and full workspace checks. Commit small Conventional Commit checkpoints.
```

---

## 1. Purpose

Vela needs one explicit equality and ordering model. It should not inherit
equality behavior from Rust derives, `OwnedValue` materialization, or whichever
container happens to call comparison.

Current runtime equality has a useful fast path for scalars, strings, and
bytes, but the fallback materializes values into `OwnedValue` and then compares
the detached representation. That makes arrays, maps, sets, records, and enums
structurally comparable by accident. It is expensive, can recurse through large
or cyclic object graphs, and conflicts with identity-keyed Map/Set semantics.

The target language rule is:

```text
user object semantic equality and ordering are opt-in
```

Builtin leaf values keep cheap VM equality and ordering paths where they are
well-defined. User objects do not get structural equality or ordering by
default. They either implement the closed builtin `PartialEq` / `Eq` /
`PartialOrd` / `Ord` traits explicitly, derive them with strict field rules, or
fail at compile time or runtime when used with the corresponding operator.

---

## 2. Goals

- Add VM-recognized builtin `PartialEq`, `Eq`, `PartialOrd`, and `Ord` traits
  for semantic equality and ordering.
- Keep builtin scalar, string, bytes, and range equality cheap and value-based
  where the primitive semantics are total enough for the operation.
- Keep script records/structs, user enums, arrays, maps, sets, closures, and
  iterators from gaining implicit structural equality or ordering.
- Preserve separate identity operators `===` and `!==` for script heap objects
  and `HostRef` values without reading host state.
- Reject or prevent comparison of internal/transient values such as `Missing`
  and `PathProxy`.
- Remove the generic materialize-then-compare fallback from ordinary runtime
  equality.
- Make array methods such as `contains`, `index_of`, and `distinct` use the
  same semantic `PartialEq` dispatch as `==`.
- Make ordering operators require `PartialOrd`.
- Make array and collection sorting require `Ord`.
- Support explicit `#[derive(PartialEq)]`, `#[derive(PartialEq, Eq)]`,
  `#[derive(PartialEq, PartialOrd)]`, and
  `#[derive(PartialEq, Eq, PartialOrd, Ord)]` for records whose fields all
  satisfy the required builtin trait.
- Keep `ValueKey` for Map/Set keys separate from user `PartialEq`/`Eq`/
  `PartialOrd`/`Ord` while using stable leaf-value and object-identity key
  classes.
- Leave deep structural equality as a future explicit helper, not an operator.

---

## 3. Non-Goals

This pass must not:

- Add open-ended operator overloading.
- Add general Rust-like trait machinery or script-language generics.
- Add `Hash` as a script-visible builtin trait.
- Make `==` recursively compare arbitrary records, arrays, maps, sets, host
  state, or object graphs.
- Read host object fields to decide equality.
- Let reflection mutate type structure to affect equality.
- Add implicit numeric widening for equality.
- Treat `ValueKey` as the implementation of semantic equality or ordering; key
  lookup has independent constraints such as keyability, internal ordering, and
  NaN rejection.
- Make Map/Set lookup, uniqueness, or iteration order call user `PartialEq`,
  `Eq`, `PartialOrd`, `Ord`, or future `Hash` implementations.
- Make `Array<f32>.sort()` or `Array<f64>.sort()` use partial ordering or
  silently invent a total float order.
- Add deep equality unless it is explicitly budgeted, cycle-safe, and separate
  from `==`.

---

## 4. Builtin Operator Traits

### 4.1 `PartialEq` And `Eq`

`PartialEq` and `Eq` are closed builtin traits recognized by the compiler and
VM. They are not general script-generic traits and they should not imply
HashMap-style key semantics.

`==` and `!=` use `PartialEq` for user-defined semantic equality:

```text
static receiver type has PartialEq      -> compile and bind directly when possible
static receiver type lacks PartialEq    -> compile-time diagnostic
dynamic receiver type                   -> runtime trait check with source span
```

`Eq` is the stronger promise that equality is a full equivalence relation:
reflexive, symmetric, and transitive. In practical terms, a type with `Eq`
should never have a value where `value != value`. `Eq` requires `PartialEq`,
but `PartialEq` does not require `Eq`.

Builtin exact scalar tags, bool, char, `String`, `Bytes`, and range values keep
specialized VM equality paths. Numeric equality is tag-exact. There is no
hidden widening:

```vela
1i64 == 1i64   // true
1i64 == 1u64   // false
1i64 == 1.0    // false
```

`f32` and `f64` keep primitive comparison behavior where it already exists, but
they satisfy only `PartialEq`, not `Eq`, because `NaN != NaN`.

### 4.2 `PartialOrd` And `Ord`

`PartialOrd` and `Ord` are closed builtin traits for ordering. `PartialOrd`
drives source-level comparison operators:

```text
< <= > >= for user objects
derive(PartialOrd)
```

When `PartialOrd` reports two values as incomparable, comparison operators
return `false`. This matches ordinary floating-point comparison behavior around
`NaN`.

`Ord` is the stronger total-ordering promise. It requires `PartialOrd` and
`Eq`, and it drives:

```text
Array<T>.sort()
ordering-based helpers that require a total order
derive(Ord)
```

Builtin candidates for first-slice `Ord` are exact integers, `bool`, `char`,
`String`, and `Bytes`. `f32` and `f64` implement `PartialOrd` but not `Ord`, so
`Array<f64>.sort()` and `#[derive(Ord)]` on a record with float fields are
rejected until Vela adds a separate total-float-order API.

Sorting must not silently invent a float ordering:

```vela
let values: Array<f64> = [1.0, 0.5];
values.sort(); // rejected in the first slice
```

Future float ordering should be explicit, for example a dedicated total-order
float sort helper or a partial-order sort API that reports incomparable values
with clear diagnostics.

### 4.3 Explicit Implementations And Derive

User records and structs do not receive `PartialEq`, `Eq`, `PartialOrd`, or
`Ord` automatically.

Manual implementations are allowed through the normal trait implementation
surface once the builtin trait IDs exist:

```vela
impl PartialEq for PlayerId {
    fn eq(self, other: PlayerId) -> bool {
        return self.server == other.server && self.id == other.id;
    }
}
```

Vela may also synthesize implementations through explicit derive:

```vela
#[derive(PartialEq, Eq)]
struct PlayerId {
    server: i64,
    id: i64,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ScoreEntry {
    score: i64,
    player_id: i64,
}
```

Derive rules:

```text
derive(PartialEq)  -> every field must satisfy PartialEq
derive(Eq)         -> every field must satisfy Eq and the type must also satisfy PartialEq
derive(PartialOrd) -> every field must satisfy PartialOrd and the type must also satisfy PartialEq
derive(Ord)        -> every field must satisfy Ord and the type must also satisfy Eq and PartialOrd
field order -> declaration order
float field -> allowed for PartialEq/PartialOrd, rejected for Eq/Ord
container or object field -> rejected unless that field type has the trait
host type -> Rust registration metadata owns support; scripts cannot derive it
```

Generated implementations should use field slots and static dispatch where
possible. They must not materialize full `OwnedValue` graphs or recursively
compare arbitrary containers by accident.

### 4.4 Reference Identity

Reference identity comparison is still needed, but it is not semantic `Eq`.
Vela uses `===` and `!==` for this operation.

These values have stable identity:

```text
Array
Map
Set
script Record / struct
script Enum
Closure
Iterator
HostRef
```

Identity equality is same-object equality, not same-content equality. It should
be exposed through `===` and `!==`, not through default derived `Eq`.

```vela
let a = Reward { code: "xp", amount: 10 };
let b = a;
let c = Reward { code: "xp", amount: 10 };

a === b  // true
a === c  // false
a !== c  // true
```

Mutating an object does not change its identity:

```vela
let reward = Reward { code: "xp", amount: 10 };
let alias = reward;
reward.amount += 5;

reward === alias // true
```

This is the same stable identity class used by identity-keyed `Set<Reward>` and
`Map<Reward, V>`, but Map/Set lookup still goes through `ValueKey`, not user
`PartialEq`.

`===` and `!==` are not overloadable and do not call `PartialEq`, `Eq`,
`PartialOrd`, `Ord`, or `ValueKey`. They are valid only for identity-carrying
values. When both operand types are statically known to be leaf values such as
`i64` or `String`, the compiler should reject the operation and suggest `==` /
`!=` for semantic equality. Dynamic non-reference operands fail at runtime with
a source-spanned diagnostic.

### 4.5 HostRef Identity

`HostRef` identity comparison compares the host reference identity. It must not
read host state or require host read capability.

The identity includes the host object's stable reference identity and
generation as represented by `HostRef`. A stale host ref can still be identical
to itself as a value; later reads, writes, or calls still fail through ordinary
HostAccess freshness checks.

### 4.6 Non-comparable values

`Missing` is not a script-visible value and must not compare successfully.

`PathProxy` is a mutation/read proxy, not a data value. Equality on `PathProxy`
should fail unless a future host path identity contract explicitly makes it
comparable.

---

## 5. Builtin Value Categories

### 5.1 Value equality

These values have cheap builtin value equality:

```text
null
bool
char
i8 i16 i32 i64
u8 u16 u32 u64
String
Bytes
Range
```

`f32` and `f64` keep primitive comparison behavior through `PartialEq` and
`PartialOrd` but are excluded from `Eq`/`Ord` derivation and sorting.

Float primitive equality follows ordinary runtime numeric equality:

```text
NaN is not equal to anything, including itself
-0.0 and 0.0 compare equal
```

`String` and `Bytes` compare by their contents, even though they are
heap-backed internally. They are immutable leaf data from the script point of
view.

`Range` compares by its range value. If a future range carries mutable cursor
state, cursor values must be treated as iterators and compare by identity.

---

## 6. Deep Equality

Deep structural comparison is useful for tests, snapshots, and data
validation, but it must be explicit:

```vela
value::deep_equal(left, right)
```

This helper is a future feature, not part of the ordinary equality operator. If
implemented, it must be:

```text
budgeted
cycle-safe
depth-limited or otherwise bounded
deterministic
source-spanned on failure
permission-safe for HostRef values
```

Deep equality should still compare `HostRef` by identity unless a host-provided
capability explicitly exposes value snapshots.

---

## 7. Map/Set Key Alignment

`ValueKey` should align with stable key classes, not user-defined `PartialEq`,
`Eq`, `PartialOrd`, or `Ord`:

```text
leaf value equality -> value keys
object identity equality -> identity keys
```

It remains a separate layer because key lookup needs additional properties:

```text
stable ordering or hashing
keyability diagnostics
NaN rejection
owned string/bytes key payloads
no transient PathProxy keys
```

For keyable leaf values, if two values are equal by builtin leaf-value
equality, their `ValueKey`s should match. For objects, `ValueKey` uses identity
even when the object implements semantic `PartialEq`. If a value is not
keyable, Map/Set insertion and lookup fail before mutation.

---

## 8. Implementation Phases

### Phase 1: Runtime equality and identity modules

- Add a focused VM module, for example `crates/vela_vm/src/equality.rs`.
- Move `values_equal` and simple equality helpers out of generic heap
  materialization code.
- Implement:
  - `values_equal(lhs, rhs, heap)`
  - `identity_equal(lhs, rhs, heap)`
  - helper functions for string/bytes/range/scalar equality
- Add focused `===`/`!==` identity comparison for heap and host objects.
- Remove the materialize-then-compare fallback from ordinary equality.

Validation:

```bash
cargo test -p vela_vm equality
cargo test -p vela_vm execution_core
```

### Phase 2: Identity operator syntax and lowering

- Add lexer/parser tokens for `===` and `!==`.
- Add AST/HIR binary operators for reference identity equality and inequality.
- Lower statically known reference operands to direct identity bytecode or a
  focused VM helper.
- Emit compile diagnostics for statically known non-reference operands.
- Emit source-spanned runtime errors for dynamic non-reference operands.
- Ensure `===` and `!==` never call `PartialEq`, `Eq`, `PartialOrd`, `Ord`,
  `ValueKey`, or deep equality.

Validation:

```bash
cargo test -p vela_syntax lexer
cargo test -p vela_syntax parser
cargo test -p vela_bytecode identity
cargo test -p vela_vm equality
```

### Phase 3: Builtin comparison trait IDs and dispatch

- Add stable builtin trait IDs for `PartialEq`, `Eq`, `PartialOrd`, and `Ord`.
- Lower statically known comparison trait uses to direct targets where
  possible.
- Emit compile diagnostics when a statically known type lacks the required
  builtin trait.
- Emit source-spanned runtime errors when dynamic values lack the required
  builtin trait.
- Keep primitive leaf comparisons on specialized VM paths.
- Reject float sorting and float `Eq`/`Ord` derive until a total-float-order
  design exists.

Validation:

```bash
cargo test -p vela_bytecode trait
cargo test -p vela_vm equality
cargo test -p vela_vm sorting
```

### Phase 4: Derive

- Add explicit `#[derive(PartialEq)]`, `#[derive(PartialEq, Eq)]`,
  `#[derive(PartialEq, PartialOrd)]`, and
  `#[derive(PartialEq, Eq, PartialOrd, Ord)]` lowering for eligible records.
- Reject derive when any field lacks the required trait.
- Reject float fields for `Eq`/`Ord` derive while allowing them for
  `PartialEq`/`PartialOrd`.
- Ensure generated implementations use slots/static dispatch instead of
  materializing `OwnedValue`.

Validation:

```bash
cargo test -p vela_syntax attribute
cargo test -p vela_bytecode script_types
cargo test -p vela_vm equality
```

### Phase 5: Collection method alignment

- Update array, map, set, iterator, and callback helper paths that call
  equality so `contains`, `index_of`, `distinct`, `find`, and related helpers
  share semantic `PartialEq` dispatch where appropriate.
- Update ordering operators to require `PartialOrd`.
- Update sorting helpers to require `Ord`.
- Add tests proving `===`/`!==` and `ValueKey` remain separate from semantic
  comparison traits.

Validation:

```bash
cargo test -p vela_vm array_methods
cargo test -p vela_vm standard_map_set_id_dispatch
```

### Phase 6: ValueKey integration

- Update `docs/value-keyed-map-set-plan.md` implementation to derive key
  equivalence from stable `ValueKey` classes, not user
  `PartialEq`/`Eq`/`PartialOrd`/`Ord`.
- Ensure `Set<Player>` and `Map<Player, V>` use identity keys.
- Ensure string and bytes keys use value keys.
- Ensure NaN and PathProxy key attempts fail before mutation.

Validation:

```bash
cargo test -p vela_vm value_key
cargo test -p vela_vm set
cargo test -p vela_vm map
```

### Phase 7: Docs and diagnostics

- Update website operator docs after implementation.
- Add examples showing explicit comparison trait implementations, derive,
  `===`/`!==` identity comparison, and explicit field comparison.
- Add diagnostics for non-comparable transient values.
- Add diagnostics for missing `PartialEq`, missing `PartialOrd`, missing `Ord`,
  rejected float sort, and rejected derive.
- Add diagnostics for invalid `===`/`!==` operands.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 9. Acceptance Criteria

- User records without `PartialEq` cannot be used with semantic `==` when the
  compiler can prove the receiver type.
- Dynamic user values without `PartialEq` fail with a source-spanned runtime error
  when used with `==`.
- User records with explicit or derived `PartialEq` compare through that
  implementation.
- Ordering operators on user types require `PartialOrd`.
- Sorting requires `Ord`.
- `Array<f64>.sort()` is rejected until a total-float-order or explicit
  partial-sort API exists.
- Float fields allow `#[derive(PartialEq)]` and `#[derive(PartialOrd)]` but
  reject `#[derive(Eq)]` and `#[derive(Ord)]`.
- `===` and `!==` compare only reference identity for script heap objects and
  host refs.
- `===` and `!==` do not call user `PartialEq`, user `Eq`,
  user `PartialOrd`, user `Ord`, `ValueKey`, or deep equality.
- Statically known non-reference operands for `===`/`!==` are rejected.
- Strings and bytes compare by contents.
- Numeric equality remains tag-exact and does not widen.
- `HostRef` identity checks do not read host state.
- `Missing` and `PathProxy` cannot silently compare as ordinary data.
- Ordinary `==` never recursively materializes and compares large object
  graphs.
- Map/Set `ValueKey` semantics do not call user `PartialEq`, user `Eq`,
  user `PartialOrd`, user `Ord`, or future user `Hash`.
