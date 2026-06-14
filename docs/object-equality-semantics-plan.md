# Object Equality Semantics Implementation Plan

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
/goal Implement Vela's object equality semantics from
docs/object-equality-semantics-plan.md. Treat docs/goal.md as the product
roadmap, docs/architecture.md and docs/architecture/*.md as the architecture
contract, and docs/progress.md as the current milestone state. Replace
accidental structural equality caused by OwnedValue materialization with a
focused runtime equality module. Ordinary `==` and `!=` must be shallow:
immutable leaf values compare by value, mutable script heap objects and host
objects compare by identity, and transient proxy/internal values are rejected
or kept out of script-visible comparison. Deep structural comparison, if added,
must be an explicit budgeted helper, not the default equality operator. Keep
Map/Set ValueKey semantics aligned with these equality classes while remaining
a separate keyability and ordering layer. Validate with focused VM tests,
collection method tests, Map/Set key tests, docs, and full workspace checks.
Commit small Conventional Commit checkpoints.
```

---

## 1. Purpose

Vela needs one explicit equality model. It should not inherit equality behavior
from Rust derives, `OwnedValue` materialization, or whichever container happens
to call comparison.

Current runtime equality has a useful fast path for scalars, strings, and
bytes, but the fallback materializes values into `OwnedValue` and then compares
the detached representation. That makes arrays, maps, sets, records, and enums
structurally comparable by accident. It is expensive, can recurse through large
or cyclic object graphs, and conflicts with identity-keyed Map/Set semantics.

The target language rule is:

```text
ordinary equality is shallow equality
```

Shallow equality means immutable leaf values compare by value, while mutable
objects compare by identity.

---

## 2. Goals

- Define `==` and `!=` as language-level shallow equality.
- Keep scalar, string, bytes, and range equality cheap and value-based.
- Make script records/structs, user enums, arrays, maps, sets, closures, and
  iterators compare by identity.
- Make `HostRef` compare by host identity without reading host state.
- Reject or prevent comparison of internal/transient values such as `Missing`
  and `PathProxy`.
- Remove the generic materialize-then-compare fallback from ordinary runtime
  equality.
- Make array methods such as `contains`, `index_of`, and `distinct` use the
  same equality semantics as `==`.
- Make `ValueKey` for Map/Set keys follow the same equivalence classes for
  keyable values while staying stricter about keyability and ordering.
- Leave deep structural equality as a future explicit helper, not an operator.

---

## 3. Non-Goals

This pass must not:

- Add user-defined equality overloads or operator overloading.
- Add a Rust-like `Eq` trait to script types.
- Make `==` recursively compare arbitrary records, arrays, maps, sets, host
  state, or object graphs.
- Read host object fields to decide equality.
- Let reflection mutate type structure to affect equality.
- Add implicit numeric widening for equality.
- Treat `ValueKey` as the implementation of all equality; key lookup has
  additional constraints such as keyability, ordering, and NaN rejection.
- Add deep equality unless it is explicitly budgeted, cycle-safe, and separate
  from `==`.

---

## 4. Equality Categories

### 4.1 Value equality

These values compare by value:

```text
null
bool
char
i8 i16 i32 i64
u8 u16 u32 u64
f32 f64
String
Bytes
Range
```

Numeric equality is tag-exact. There is no hidden widening:

```vela
1i64 == 1i64   // true
1i64 == 1u64   // false
1i64 == 1.0    // false
```

Float equality follows ordinary runtime numeric equality:

```text
NaN is not equal to anything, including itself
-0.0 and 0.0 compare equal
```

`String` and `Bytes` compare by their contents, even though they are
heap-backed internally. They are immutable leaf data from the script point of
view.

`Range` compares by its range value. If a future range carries mutable cursor
state, cursor values must be treated as iterators and compare by identity.

### 4.2 Identity equality

These values compare by identity:

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

Identity equality is same-object equality, not same-content equality.

```vela
let a = Reward { code: "xp", amount: 10 };
let b = a;
let c = Reward { code: "xp", amount: 10 };

a == b  // true
a == c  // false
```

Mutating an object does not change its identity:

```vela
let reward = Reward { code: "xp", amount: 10 };
let alias = reward;
reward.amount += 5;

reward == alias // true
```

This is the same rule used by identity-keyed `Set<Reward>` and
`Map<Reward, V>`.

### 4.3 HostRef equality

`HostRef` equality compares the host reference identity. It must not read host
state or require host read capability.

The identity includes the host object's stable reference identity and
generation as represented by `HostRef`. A stale host ref can still be equal to
itself as a value; later reads, writes, or calls still fail through ordinary
HostAccess freshness checks.

### 4.4 Non-comparable values

`Missing` is not a script-visible value and must not compare successfully.

`PathProxy` is a mutation/read proxy, not a data value. Equality on `PathProxy`
should fail unless a future host path identity contract explicitly makes it
comparable.

---

## 5. Deep Equality

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

## 6. Map/Set Key Alignment

`ValueKey` should align with ordinary equality classes:

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

For keyable values, if `a == b` by shallow equality, their `ValueKey`s should
match. If a value is not keyable, Map/Set insertion and lookup fail before
mutation.

---

## 7. Implementation Phases

### Phase 1: Runtime equality module

- Add a focused VM module, for example `crates/vela_vm/src/equality.rs`.
- Move `values_equal` and simple equality helpers out of generic heap
  materialization code.
- Implement:
  - `values_equal(lhs, rhs, heap)`
  - `identity_equal(lhs, rhs, heap)`
  - helper functions for string/bytes/range/scalar equality
- Remove the materialize-then-compare fallback from ordinary equality.

Validation:

```bash
cargo test -p vela_vm equality
cargo test -p vela_vm execution_core
```

### Phase 2: Collection method alignment

- Update array, map, set, iterator, and callback helper paths that call
  equality so `contains`, `index_of`, `distinct`, `find`, and related helpers
  share the same `==` semantics.
- Add tests proving arrays and records compare by identity in collection
  lookup helpers.

Validation:

```bash
cargo test -p vela_vm array_methods
cargo test -p vela_vm standard_map_set_id_dispatch
```

### Phase 3: ValueKey integration

- Update `docs/value-keyed-map-set-plan.md` implementation to derive key
  equality from this plan's shallow equality categories.
- Ensure `Set<Player>` and `Map<Player, V>` use identity keys.
- Ensure string and bytes keys use value keys.
- Ensure NaN and PathProxy key attempts fail before mutation.

Validation:

```bash
cargo test -p vela_vm value_key
cargo test -p vela_vm set
cargo test -p vela_vm map
```

### Phase 4: Docs and diagnostics

- Update website operator docs after implementation.
- Add examples showing object identity equality and explicit field comparison.
- Add diagnostics for non-comparable transient values.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 8. Acceptance Criteria

- Two aliases of the same script record compare equal.
- Two separately constructed script records with identical fields compare not
  equal.
- Mutating a script record does not change equality with its aliases.
- Arrays, maps, sets, user enums, closures, and iterators compare by identity.
- Strings and bytes compare by contents.
- Numeric equality remains tag-exact and does not widen.
- `HostRef` equality does not read host state.
- `Missing` and `PathProxy` cannot silently compare as ordinary data.
- Ordinary `==` never recursively materializes and compares large object
  graphs.
- Map/Set `ValueKey` semantics agree with ordinary equality for keyable values.
