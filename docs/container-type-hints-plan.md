# Container Type Hint Contracts Implementation Plan

> **Track:** M20/M23 type-contract and fast-path architecture continuation  
> **Document status:** Codex execution plan  
> **Compatibility policy:** breaking pre-release syntax, metadata, bytecode,
> VM, docs, and tests are allowed. Do not preserve old rejected-container
> generic behavior only for compatibility. Preserve product contracts:
> no user-defined script generics, no Rust `&mut` exposure, HostAccess safety,
> source-spanned diagnostics, execution budgets, GC roots, reflection
> permissioning, and hot-reload ABI/schema checks.

---

## 0. Codex Goal

```text
/goal Implement Vela's builtin parameterized container type hints from
docs/container-type-hints-plan.md. Treat docs/goal.md as the product roadmap,
docs/architecture.md and docs/architecture/*.md as the architecture contract,
and docs/progress.md as the current milestone state. This is not a general
script-language generics feature: only builtin type-hint contracts may carry
type arguments. Build the feature as a vertical slice across syntax, HIR,
analysis TypeFacts, bytecode RuntimeTypeFacts, type guard plans, VM contract
execution, typed container mutation checks, embedding metadata, hot-reload ABI,
docs, examples, and tests. Prefer clean replacement over compatibility shims.
Validate each checkpoint with focused tests plus the relevant workspace checks,
and commit small Conventional Commit checkpoints.
```

---

## 1. Purpose

Vela now has consistent public type-hint spelling:

```text
lowercase scalar/literal primitives: null bool char i64 f64 ...
capitalized named contracts: Any String Bytes Array Map Set Iterator Option Result
```

Only `Option<T>` and `Result<T, E>` currently accept type arguments. The next
clean step is to open builtin container type arguments for type hints:

```vela
Array<i64>
Set<String>
Map<String, i64>
Iterator<Player>
Option<Array<i64>>
Result<Map<String, i64>, String>
```

The goal is not to add user-defined generics. The goal is to make type hints
precise enough for contracts, diagnostics, static TypeFacts, and fast-path
bytecode decisions.

---

## 2. Goals

- Allow type arguments only on selected builtin type-hint contracts:
  `Array<T>`, `Set<T>`, `Map<String, V>`, `Iterator<T>`, `Option<T>`, and
  `Result<T, E>`.
- Restrict `Set<T>` to the runtime's current set-keyable element contracts:
  `null`, `bool`, `i64`, `f64`, and `String`; use erased `Set` for other Rust
  set element types until arbitrary value-key support is designed.
- Keep scalar primitive type hints lowercase and named container contracts
  capitalized.
- Preserve the rule that type hints are contracts, not conversions.
- Propagate container item/key/value facts through analysis, compiler
  `RuntimeTypeFact`, and contract guard metadata.
- Make `Array<T>`, `Set<T>`, and `Map<String, V>` runtime contracts semantically
  real by validating existing contents at guarded boundaries.
- Add container content summaries and contract stamps so repeated downcasts to
  stable typed containers do not scan contents on every boundary crossing.
- Keep typed container mutation sound without adding unnecessary hot-path
  checks: writes, pushes, inserts, and map/set updates through a typed
  container contract should skip runtime guards when the inserted key/item/value
  is statically proven compatible, reject statically incompatible values at
  compile time, and guard only dynamic or erased values.
- Support `Iterator<T>` without consuming the iterator at the guard boundary.
- Make hot-reload ABI and reflection metadata compare/display structured
  parameterized contracts.
- Keep the design open to future container fast paths and JIT without adding
  a script generic type system.

---

## 3. Non-Goals

This pass must not:

- Add user-defined generic types, generic functions, or generic impls.
- Add `Player<T>`, `Foo<T>`, or other schema/host/user generic syntax.
- Add `Function<...>` or callable signature type syntax. Function types need a
  separate design.
- Add covariance/subtyping rules, wildcard types, type aliases, or trait
  bounds.
- Add implicit conversions for container contents.
- Trust container facts for fast paths after unchecked mutation.
- Recheck element types on every indexed read, `for` item, arithmetic op, or
  other hot read path after a container fact is trusted.
- Scan full container contents on every repeated contract check when summary
  or stamp metadata can answer the check safely.
- Consume an iterator merely to prove `Iterator<T>`.
- Expose Rust references or host-owned iterator internals to scripts.
- Weaken budget charging, GC rooting, hot reload ABI checks, or source-spanned
  diagnostics.

---

## 4. Syntax Contract

### 4.1 Allowed parameterized hints

The parser should accept only these arities:

| Type hint | Arity | Meaning |
|---|---:|---|
| `Array<T>` | 1 | script/runtime array whose current and future elements satisfy `T` |
| `Set<T>` | 1 | script/runtime set whose current and future values satisfy set-keyable `T` |
| `Map<String, V>` | 2 | script/runtime map whose current and future keys are strings and values satisfy `V` |
| `Iterator<T>` | 1 | one-shot iterator whose yielded items satisfy `T` |
| `Option<T>` | 1 | `Some` payload satisfies `T`; `None` carries no payload |
| `Result<T, E>` | 2 | `Ok` payload satisfies `T`; `Err` payload satisfies `E` |

Current runtime maps are string-keyed. This container type-hint slice therefore
accepts only `Map<String, V>` and rejects `Map<K, V>` when `K` is not `String`.
The follow-on value-keyed collection design is tracked in
[value-keyed-map-set-plan.md](value-keyed-map-set-plan.md); that plan replaces
the string-key restriction with a shared `ValueKey` policy and identity keys
for script records/structs. Current runtime sets are also limited to keyable
values, so `Set<T>` accepts only `null`, `bool`, `i64`, `f64`, and `String`
element contracts in this slice.

Unparameterized forms remain valid erased container contracts:

```vela
Array
Map
Set
Iterator
Option
Result
```

These validate only the outer runtime category and carry unknown item facts.

### 4.2 Rejected hints

Reject all other generic-looking hints with a source-spanned diagnostic:

```vela
Player<T>       // rejected: script schemas are not generic
String<T>       // rejected: String is not parameterized
Bytes<T>        // rejected
Range<T>        // rejected for now
Function<T>     // rejected; callable signatures need a separate design
Array<T, U>     // rejected: wrong arity
Map<K>          // rejected: wrong arity
Map<i64, V>     // rejected in this slice: runtime maps are string-keyed
Map<Any, V>     // rejected in this slice: runtime maps are string-keyed
Option<T, E>    // rejected: wrong arity
```

The diagnostic should say that only builtin container/Option/Result type hints
support type arguments.

### 4.3 Nested hints

Nested builtin hints are allowed:

```vela
Array<Option<i64>>
Map<String, Result<Player, String>>
Result<Array<i64>, String>
```

The nesting limit is the parser's normal recursion behavior. Do not add a new
unbounded execution path; if a practical nesting limit exists for diagnostics
or stack safety, document it and test it.

---

## 5. Semantic Model

### 5.1 Type hints remain contracts

The existing contract table still applies:

```text
exact compatible fact    -> accepted, no runtime guard
exact incompatible fact  -> compile error
dynamic or erased fact   -> accepted with runtime contract guard
```

Examples:

```vela
fn sum(values: Array<i64>) -> i64 {
    let total = 0;
    for value in values {
        total += value;
    }
    return total;
}

fn names_by_id(players: Map<String, String>) -> String {
    return players.get("player-1").unwrap_or("unknown");
}
```

`Array<i64>` is not a conversion from mixed arrays to integer arrays. A mixed
array fails when it crosses the contract boundary or when an invalid value is
inserted through a typed container path.

### 5.2 Any and unknown

`Any` erases the inner contract at that position:

```vela
Array<Any>          // array category only; elements are dynamic
Map<String, Any>    // string keys; dynamic values
Option<Any>         // option category only; Some payload dynamic
```

No type hint and `Any` are not identical metadata, but neither introduces a
specific runtime contract by itself.

### 5.3 Static facts

Analysis and compiler facts should preserve known item facts:

```text
Array<T>     index read and for item -> T
Set<T>       for item -> T
Map<String,V> key view item -> String; value view item -> V; entry fields -> String/V
Iterator<T>  next Some payload and for item -> T
Option<T>    ? and Some match payload -> T
Result<T,E>  ? and Ok/Err match payload -> T/E
```

When facts become unknown through dynamic calls, reflection, erased `Any`,
untyped native returns, or untracked mutation, the compiler must degrade to
dynamic checks instead of keeping stale typed facts.

### 5.4 Trusted container fact provenance

A container type hint is not itself proof. The compiler may use
`Array<T>`, `Set<T>`, `Map<String, V>`, or `Iterator<T>` facts for fast paths
only after the container fact is trusted.

Trusted container facts may come from:

```text
checked function entry guards
typed let/global/field boundaries after the guard succeeds
statically proven literals or constructors
typed mutation paths that guard or statically prove every inserted value
native/host/reflection returns after an explicit contract guard succeeds
```

Untrusted container facts include:

```text
annotation text before validating the right-hand side
untyped or Any values
dynamic native/host/reflection returns before guarding
aliases after an unknown mutation
containers passed through dynamic calls that may mutate them
```

The required lowering order is:

```text
evaluate RHS
prove or guard RHS against the annotated container contract
bind the local/global/field fact only after proof or guard success
use the trusted fact for later reads, loops, and statically proven mutations
```

Example:

```vela
fn source() {
    return ["bad"];
}

fn f() {
    let xs: Array<i64> = source(); // guard must run before xs is trusted
    xs.push(1i64);                 // no guard only if the previous guard passed
    return xs[0] + 1;
}
```

It is a bug to assign `xs` a trusted `Array<i64>` fact only because the
annotation text says `Array<i64>`. If the RHS is dynamic, the trusted fact
starts after the contract guard, not before it.

---

## 6. Runtime Contract Model

### 6.1 Container summaries and contract stamps

Parameterized container contracts need runtime metadata to keep type checks out
of hot read paths. The first implementation should add two distinct concepts.

`ContainerTypeSummary` is an observed shallow content summary:

```text
Empty             no values observed
Exact(type_key)   every observed key/item/value has the same shallow type key
Mixed             more than one shallow type key is present
Unknown           summary is unavailable or invalidated
```

Arrays and sets track an element summary. Maps should use a storage/metadata
shape that has both key and value sides, but in the first implementation the key
contract is fixed to `String` because runtime maps are string-keyed. The value
side still needs a summary. Keep the key side explicit in the data model so a
future arbitrary-key map design does not require replacing every guard/stamp
interface.

Summary updates must be cheap:

```text
Empty + i64       -> Exact(i64)
Exact(i64) + i64  -> Exact(i64)
Exact(i64) + str  -> Mixed
Mixed + anything  -> Mixed
Unknown + any     -> Unknown until an explicit rescan
```

The first implementation may make `Exact -> Mixed` a one-way downgrade. It
does not need to rescan after deletes to recover `Exact`; that can be added
later as an explicit optimization.

`ContainerContractStamp` records that a specific container has already passed a
specific parameterized contract and has not been invalidated by later mutation.
It is needed for nested contracts such as `Array<Array<i64>>`, where a shallow
summary can only prove that outer elements are arrays, not that inner arrays
satisfy `Array<i64>`.

Contract checks should use this order:

```text
matching valid contract stamp -> pass O(1)
summary proves contract       -> install/update stamp and pass O(1)
summary proves mismatch       -> fail O(1)
summary Unknown               -> scan once, then update summary/stamp or fail
```

Container mutation must update summaries and stamps:

```text
statically compatible typed mutation -> preserve or update matching stamps O(1)
dynamic value that passes guard      -> preserve or update matching stamps O(1)
unchecked or incompatible mutation   -> downgrade summary and invalidate stamps
```

Once a container fact is trusted, indexed reads, `for` iteration, typed
arithmetic over items, and other hot read paths must use the trusted fact
directly. They must not repeat per-item type guards unless the operation itself
crosses a new dynamic contract boundary.

### 6.2 Deep guards for materialized containers

`Array<T>`, `Set<T>`, and `Map<String, V>` contract guards must validate existing
materialized contents:

```text
Array<T>     validate each element against T
Set<T>       validate each element against T
Map<String,V> validate map key storage is string-keyed and each value against V
```

These guards are language semantics. They fail with runtime type contract
errors, not inline-cache misses.

Deep scanning is the fallback for `Unknown` summaries or first-time nested
contract checks, not the normal path for stable containers.

### 6.3 Budget charging

Deep container validation must charge execution budget. Large containers must
not bypass budget limits merely by crossing a typed boundary.

The implementation should add a focused guard execution context rather than
threading ad hoc optional budget parameters through unrelated call paths.
Suggested shape:

```rust
pub struct GuardExecutionContext<'a> {
    heap: Option<&'a HeapExecution<'a>>,
    budget: Option<&'a mut ExecutionBudget>,
}
```

Use the repository's actual lifetime and ownership style; this is a design
sketch, not a required exact API.

### 6.4 Iterator guards

`Iterator<T>` must not eagerly consume the iterator to validate items.

Preferred model:

```text
Iterator<T> contract guard validates outer iterator category
then wraps or marks the iterator with an item guard
each next() validates the yielded value against T
```

If the first implementation cannot wrap iterator state cleanly, defer
`Iterator<T>` execution support behind a compile/runtime rejection with tests.
Do not implement an eager-consuming guard.

### 6.5 Mutation soundness

Typed container facts are only safe if later mutations preserve the contract.
Use the same compatibility table as function parameter and return contracts:

```text
inserted key/item/value statically compatible    -> no runtime guard
inserted key/item/value statically incompatible  -> compile error
inserted key/item/value dynamic or erased        -> runtime guard before write
```

Required behavior:

```vela
fn fast(values: Array<i64>) {
    values.push(1i64); // statically compatible: no runtime guard
}

fn checked(values: Array<i64>, value) {
    values.push(value); // dynamic: guard value as i64 before push
}

fn bad(values: Array<i64>) {
    values.push("x"); // statically incompatible: compile error
}

fn bad_map(values: Map<String, i64>) {
    values["level"] = "high"; // statically incompatible: compile error
}
```

The clean implementation should attach container element contracts to the
container handle/path/fact used by mutation lowering, or invalidate the typed
fact before any path that cannot prove mutation safety. Do not keep a stale
`Array<i64>` fact after an unchecked dynamic mutation.

This rule is required for performance as well as correctness. Hot loops that
push proven `i64` values into `Array<i64>` should not pay a redundant runtime
guard on every insertion. Guarded mutation exists for dynamic boundaries, not
for values the compiler has already proven.

### 6.6 Host containers

Host-owned fields that expose `Array<T>`, `Map<String,V>`, or `Set<T>` through
snapshot `OwnedValue`/`HostValue` boundaries can use the same deep guard model.

Host mutation still goes through `HostRef`, `HostPath`, `PathProxy`, and
`HostAccess`. Do not place Rust host containers under script GC or expose Rust
references to make typed container contracts work.

---

## 7. Implementation Plan

### Phase 1: Syntax and HIR shape

- Replace the parser's `supports_type_arguments` boolean with a builtin
  arity table.
- Accept `Array<T>`, `Set<T>`, `Map<String,V>`, `Iterator<T>`, `Option<T>`, and
  `Result<T,E>`.
- Reject `Map<K,V>` when `K` is anything other than `String` in this slice.
- Keep rejecting all other generic-looking type hints.
- Update `docs/grammar.ebnf` to describe builtin parameterized contracts.
- Add parser tests for accepted nested hints and rejected wrong-arity/unknown
  generic hints.

Checkpoint:

```bash
cargo test -p vela_syntax
```

### Phase 2: Analysis TypeFacts

- Extend `builtin_type_fact_from_hir_hint` to lower parameterized containers.
- Preserve nested facts through `TypeFact::array`, `TypeFact::set`,
  `TypeFact::map`, and `TypeFact::iterator`.
- Update completion, hover, member diagnostics, `for-in`, index read, map view,
  set view, iterator `next`, `Option`, and `Result` fact tests.
- Keep erased forms (`Array`, `Map`, `Set`, `Iterator`) as unknown inner facts.

Checkpoint:

```bash
cargo test -p vela_hir -p vela_analysis
```

### Phase 3: Compiler RuntimeTypeFacts

- Extend `RuntimeTypeFact` with parameterized container variants:

```rust
Array(Box<RuntimeTypeFact>)
Set(Box<RuntimeTypeFact>)
Map {
    key: Box<RuntimeTypeFact>,
    value: Box<RuntimeTypeFact>,
}
Iterator(Box<RuntimeTypeFact>)
```

- Keep `StandardRuntimeType::Array/Map/Set/Iterator` for erased outer-category
  contracts if that remains the cleanest representation.
- Update `type_hint_value_type`, expected-type outcomes, typed let/param/return
  guard generation, index/for item facts, and write-site contract checks.
- Track trusted container fact provenance explicitly enough that a type
  annotation on a dynamic RHS does not create fast-path facts before the
  required guard succeeds.
- Ensure dynamic/erased facts degrade safely.

Checkpoint:

```bash
cargo test -p vela_bytecode
```

### Phase 4: Guard plans and verifier

- Extend unlinked and linked `TypeGuardPlan` with parameterized container
  plans, or introduce a uniform recursive guard-plan node that can represent
  `Array`, `Set`, `Map`, `Iterator`, `Option`, and `Result`.
- Add or reference a stable contract identity usable by container contract
  stamps without string lookup on the hot path.
- Keep linked guard plans hot-path friendly: no string lookup or registry
  lookup during execution.
- Update linker interning, verification, and profile/program-image ownership
  tests as needed.
- Ensure guard debug names still render source-facing type hints.

Checkpoint:

```bash
cargo test -p vela_bytecode -p vela_vm
```

### Phase 5: VM guard execution and mutation checks

- Replace plain heap array/map/set storage with focused container storage
  structs, or otherwise attach equivalent metadata, so containers carry content
  summaries and contract stamps without scattering side tables through the VM.
- Maintain summaries during construction, owned/host value materialization,
  array push/set, set insert/remove, map insert/update/remove, collection
  transforms, iterator collection, and reflection-controlled writes.
- Use contract stamps and summaries before falling back to full scans.
- Implement deep guards for array/set/map contents.
- Charge budget while scanning materialized containers.
- Implement typed mutation checks for array push/set, set insert, map insert
  and map value update paths that carry typed container contracts.
- Apply the mutation compatibility table during lowering:
  - proven compatible inserted values emit no guard;
  - proven incompatible inserted values become compile diagnostics;
  - dynamic or erased inserted values emit a guard before the mutation.
- Add or defer `Iterator<T>` through an explicit guarded-iterator model. Do not
  consume iterators for validation.
- Preserve source spans in type contract errors.
- Add negative tests proving invalid existing contents and invalid later
  mutations fail before typed fast paths rely on them.
- Add positive tests proving repeated contract checks on a stable container use
  summary/stamp metadata instead of rescanning contents.

Checkpoint:

```bash
cargo test -p vela_vm
```

### Phase 6: Embedding, macros, and metadata

- Update macro-inferred hints:
  - `Vec<T>` and `[T; N]` -> `Array<T>`
  - `HashSet<T>` / `BTreeSet<T>` -> `Set<T>` only for set-keyable `T`; otherwise `Set`
  - `HashMap<String,V>` / `BTreeMap<String,V>` -> `Map<String,V>`
  - non-string Rust map keys remain unsupported until arbitrary Vela map keys
    have a runtime design
  - `Option<T>` and `Result<T,E>` remain parameterized.
- Update engine/native/host validation to accept the new builtin container
  parameterization and reject unsupported generic hints.
- Ensure reflection metadata displays structured public type hints.
- Ensure hot reload ABI treats `Array<i64>` vs `Array<String>` and
  `Map<String,i64>` vs `Map<String,String>` as incompatible.

Checkpoint:

```bash
cargo test -p vela_macros -p vela_engine -p vela_reflect -p vela_hot_reload
```

### Phase 7: Docs, site, examples, and benchmark hooks

- Update architecture docs and `docs/decisions.md` with the builtin
  parameterized contract decision.
- Update Starlight docs and playground examples to show `Array<T>`,
  `Map<String,V>`, and `Set<T>` where useful.
- Add conformance examples that exercise typed container reads, writes, and
  `?` through nested `Option`/`Result` containers.
- Add at least one benchmark or opcode/profile check that demonstrates the
  compiler sees `Array<i64>`/`Map<String,i64>` facts and can select existing or
  future fast paths.
- Add mutation-focused benchmark rows that separate:
  - proven compatible typed container push/update with no runtime guard;
  - dynamic guarded push/update;
  - erased container push/update.
- Capture profiling for the mutation rows before and after the feature lands.
  The proven-compatible typed path must not regress materially versus the
  erased path because of redundant contract machinery.

Checkpoint:

```bash
cargo test --workspace
(cd site && npm run build)
cargo bench -p vela_vm --bench baseline -- --quick container
```

---

## 8. Test Plan

### Parser and HIR

- Accept:
  - `Array<i64>`
  - `Set<String>`
  - `Map<String, i64>`
  - `Iterator<Player>`
  - `Option<Array<i64>>`
  - `Result<Map<String, i64>, String>`
- Reject:
  - `Array`
    with no rejection, as erased form
  - `Array<i64, String>`
  - `Map<String>`
  - `Map<i64, String>`
  - `Map<Any, String>`
  - `Player<i64>`
  - `Function<i64>`
  - `Range<i64>`

### Analysis

- `for value in values` where `values: Array<i64>` binds `value` as `i64`.
- `values[0]` where `values: Array<Player>` has `Player` fact.
- `map.get("level")` where `map: Map<String, i64>` returns `Option<i64>` fact
  if the stdlib method supports that fact shape.
- `iterator.next()` where `iterator: Iterator<Player>` returns
  `Option<Player>` fact.
- Nested `Result<Array<i64>, String>?` unwraps to `Array<i64>`.

### Compiler and VM

- Passing a mixed array to `fn f(values: Array<i64>)` fails before function body
  code assumes `i64` items.
- `let xs: Array<i64> = dynamic_value()` emits or performs the contract guard
  before binding `xs` as a trusted `Array<i64>` fact.
- A dynamic RHS that fails `Array<i64>` validation does not execute later
  no-guard typed mutations or typed index fast paths.
- A statically proven `[1i64, 2i64]` literal can bind trusted `Array<i64>`
  without a runtime guard.
- Rechecking the same stable `Array<i64>` value after a successful contract
  check uses its contract stamp or summary instead of rescanning every element.
- Rechecking an `Array<i64>` value after an unchecked mixed mutation invalidates
  the stamp and fails or rescans according to the updated summary.
- Returning `Array<String>` from a function declared `-> Array<i64>` fails at
  the return guard.
- `Array<i64>` rejects pushing `"x"` and preserves the previous array state.
- `Array<i64>` push of a statically proven `i64` emits no runtime guard.
- `Array<i64>` push of a dynamic value emits a runtime guard before mutation.
- `Array<i64>` push of a statically proven `String` is rejected before codegen.
- `Map<String, i64>` rejects non-string keys and non-i64 values on guarded
  insertion/update paths.
- `Map<String, i64>` update with statically proven `String` key and `i64` value
  emits no runtime guard.
- `Map<String, i64>` update with dynamic key or value emits only the necessary
  key/value guard before mutation.
- `Set<String>` rejects inserting a non-`String` value.
- `Set<String>` insert of a statically proven `String` emits no runtime guard.
- `Set<Player>` is rejected until arbitrary set value-key support is designed.
- `Array<Any>` accepts mixed values while still rejecting a non-array outer
  value.
- Deep guard scans charge budget and can fail with budget exhaustion before
  finishing a very large container.
- `Iterator<T>` either yields guarded items lazily or is explicitly rejected
  until guarded iterator adapters exist.

### Benchmark and profiling

- Add or reuse quick benchmark rows for typed container mutation:
  - `array_i64_push_static`
  - `array_i64_push_dynamic_guarded`
  - `map_string_i64_update_static`
  - `map_string_i64_update_dynamic_guarded`
- Store benchmark output under the existing `perf-results/` convention when
  taking a checkpoint.
- Run the existing profiling helper for at least the static and dynamic array
  push rows.
- Acceptance rule: the static typed push/update path must not show guard
  execution as a material hotspot. If a measurable regression appears, fix the
  lowering/guard placement before calling the feature complete.
- Add a repeated-boundary benchmark for passing the same stable `Array<i64>` or
  `Map<String, i64>` through a typed function many times. The benchmark should
  demonstrate O(1) stamp/summary checks after the first successful validation,
  not repeated full-container scans.

### Embedding and hot reload

- Macro-inferred collection hints include parameterized public names.
- Native/host function validation accepts supported container hints and rejects
  unsupported generic hints.
- Reflection metadata displays `Array<i64>`, `Map<String, Player>`, etc.
- Hot reload rejects ABI changes that alter any inner type argument.

### Future arbitrary map keys

- This plan intentionally keeps `Map<K, V>` restricted to `Map<String, V>`.
- The follow-on plan, [value-keyed-map-set-plan.md](value-keyed-map-set-plan.md),
  defines the `ValueKey` semantics needed to accept arbitrary key contracts:
  keyable runtime values, identity keys for records/structs, ordering behavior,
  float/NaN policy, serde/reflection/key iterator behavior, and hot-reload ABI
  compatibility for key contract changes.
- The current VM container storage and guard/stamp APIs should still name
  key-side and value-side metadata explicitly, such as `key_contract`,
  `value_contract`, `key_summary`, and `value_summary`, so the follow-on plan
  can replace the string-key restriction cleanly.

### Full validation

Use focused checks during implementation and end with:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd site && npm run build)
```

---

## 9. Architecture Risks

### 9.1 Deep guard cost

Deep guards can be expensive on large containers. The first implementation
must avoid making repeated deep scans the normal path. Use content summaries
and contract stamps so stable containers pay O(1) for repeated checks after
initial proof. Full scans remain the budgeted fallback for unknown summaries,
first-time nested contract validation, or repair after explicit rescan.

### 9.2 Mutation invalidates facts

The biggest soundness risk is keeping a typed fact after an unchecked mutation.
Every collection mutation path must either validate against the active typed
contract or invalidate the fact before downstream fast paths can rely on it.

### 9.3 Iterator contracts

Iterators are one-shot. Eager validation consumes them and changes behavior.
`Iterator<T>` must use lazy item validation or remain deferred.

### 9.4 Host boundary ambiguity

Host containers may be snapshots, path proxies, or adapter-backed data. The
implementation must avoid assuming host-owned Rust collections are script heap
containers. All host mutation remains mediated by `HostAccess`.

### 9.5 Metadata string drift

Public type hint display strings, registry metadata, hot reload ABI manifests,
docs, site examples, macro output, and diagnostics must use the same spelling.
Add tests at the metadata boundary instead of relying on manual consistency.

---

## 10. Acceptance Criteria

This plan is complete when:

- Supported builtin container type hints parse and display correctly.
- Unsupported generic type hints are rejected with source-spanned diagnostics.
- Analysis facts preserve container item/key/value information through common
  reads, loops, iterator methods, `Option`, and `Result`.
- Compiler/runtime guard plans represent parameterized containers without
  string lookup on the hot path.
- Materialized array/set/map contracts validate existing contents and charge
  budget.
- Typed container mutations validate inserted keys/items/values or safely
  invalidate the typed fact.
- `Iterator<T>` has lazy item validation, or is deliberately deferred with
  tests and docs explaining the boundary.
- Macro, embedding, reflection, and hot reload ABI surfaces agree on public
  parameterized type-hint spelling.
- The full validation command set passes.
