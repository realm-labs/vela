# Iterator And Sequence Model Implementation Plan

> **Track:** language/stdlib/runtime architecture continuation, M20-adjacent but
> not an inline-cache-only task  
> **Document status:** Codex execution plan  
> **Compatibility policy:** breaking pre-release language, stdlib, bytecode,
> and internal runtime changes are allowed. Do not preserve old internal
> iterator or eager collection behavior only for compatibility. Preserve product
> contracts: no script-language generics, no Rust `&mut` exposure, HostAccess
> safety, source-spanned diagnostics, execution budgets, GC roots, reflection
> permissioning, and hot-reload ABI checks.

---

## 0. Codex Goal

```text
/goal Implement Vela's clean Iterator and Sequence model from
docs/iterator-sequence-plan.md. Treat docs/goal.md as the product roadmap,
docs/architecture.md and docs/architecture/*.md as the architecture contract,
and docs/progress.md as the current milestone state. Build the feature as a
vertical slice across syntax/HIR analysis/bytecode/VM/stdlib/docs/tests.
Preserve standing constraints: no script-language generics, no Rust &mut
references exposed to scripts, host mutation only through HostRef/HostPath/
PathProxy/HostAccess, reflection without runtime type-structure mutation, and
no MVP JIT/async/coroutines/moving GC/full LSP. Prefer clean replacement over
compatibility shims. Validate each checkpoint with the relevant tests and commit
small Conventional Commit checkpoints.
```

---

## 1. Purpose

Vela currently has working `for-in`, string `char` iteration, ranges, native
iterator values, and eager collection callback helpers. What is missing is a
single semantic model that explains:

```text
for-in sources
string character and byte traversal
range traversal
array/set/map views
lazy map/filter/take/skip/enumerate/zip-style operations
host-provided iterable data without eager script heap materialization
collecting lazy pipelines into arrays/maps/sets
```

The model should make Vela expressive without pretending that UTF-8 strings or
host-backed data are random-access script arrays.

---

## 2. Goals

- Define a domain-neutral `Iterable -> Iterator` protocol for VM-owned and
  host-provided values.
- Define `Sequence` as a repeatable iterable/view when the runtime can traverse
  the source more than once without consuming state.
- Make `for-in` lower through one focused iteration boundary instead of
  type-specific ad hoc paths.
- Keep string character traversal explicit and Rust-like:
  `text.len()` remains byte length, `text.slice(start, end)` remains byte range,
  and character traversal uses `text.chars()` or `for ch in text`.
- Add lazy iterator adapters without script-language generics.
- Keep TypeFacts analysis-only: infer callback parameter and return facts, but
  do not expose `Iterator<T>` or `Sequence<T>` syntax.
- Support host-provided iterables without placing Rust host state under script
  GC or exposing borrowed Rust references.
- Preserve budget and GC behavior for iterator state, lazy adapter chains, and
  final collection materialization.
- Keep hot paths open to inline caches and specialized range/string/array
  iterators.

---

## 3. Non-Goals

This pass must not:

- Add script-language generics such as `Iterator<T>` or `Sequence<T>`.
- Add user-defined iterator traits or monkey-patchable iteration hooks.
- Add async, coroutines, generators, or parallel iterators.
- Expose Rust references or host-owned iterator internals to scripts.
- Make strings random-access by character index or add `char_at`.
- Preserve old eager collection helpers if the new model intentionally replaces
  them.
- Require every iterator adapter in the first implementation.
- Change `for index, value in iterable`: it remains syntax-level indexed
  `for-in`, not an eager `enumerate()` allocation.

---

## 4. Design Model

### 4.1 Value Categories

Use three internal concepts:

```text
Iterable
  A value or view that can create an Iterator.

Iterator
  A one-shot cursor with internal state. Calling next advances it.

Sequence
  A repeatable iterable/view. It can create a fresh iterator each time and may
  expose cheap len/is_empty when the source supports it.
```

These are runtime and analysis concepts, not user-visible generic types.

### 4.2 Initial Iterable Sources

The first complete slice should cover:

```text
array.values() / array.iter()
set.values() / set.iter()
map.keys()
map.values()
map.entries()
map.iter()
string.chars()
string.bytes()
range.iter() or direct range iteration
existing native iterator values
host-provided iterable handles
```

`for ch in text` remains valid sugar for string character traversal. It should
lower to the same internal source as `text.chars()` rather than owning a
separate implementation path.

Map default iteration must be made explicit during implementation. Prefer
requiring `map.keys()`, `map.values()`, or `map.entries()` in user-facing docs
unless existing conformance already depends on direct map iteration. If direct
map iteration is kept, document the chosen item shape and make it a normal
`Iterable` source, not a special case.

### 4.3 Iterator Item Shapes

Keep item shapes simple and dynamic:

```text
array.iter()       -> value
set.iter()         -> value
map.keys()         -> key
map.values()       -> value
map.entries()      -> record-like entry with key and value fields, or the
                      existing current entry shape if already standardized
map.iter()         -> MapEntry { key, value }
string.chars()     -> char
string.bytes()     -> u8
range iteration    -> i64 for proven i64 ranges; otherwise current range item
host iterable      -> copied Value, HostRef, or PathProxy values only
```

Do not introduce tuple values just for map entries. If entry destructuring is
needed later, use existing record/pattern machinery or add it as a separate
language feature.

### 4.4 Lazy Adapters

Implement a small core before broadening:

```text
iterator.next()          -> Option
iterator.count()         -> i64
iterator.any(|x| ...)    -> bool
iterator.all(|x| ...)    -> bool
iterator.find(|x| ...)   -> Option
iterator.map(|x| ...)    -> Iterator
iterator.filter(|x| ...) -> Iterator
iterator.take(n)         -> Iterator
iterator.skip(n)         -> Iterator
iterator.collect_array() -> Array
```

Defer `zip`, `enumerate`, `flat_map`, `collect_map`, and `collect_set` until the
core execution and analysis model is stable. `for index, value in iterable`
already covers the common indexed loop case without creating an adapter.

### 4.5 Eager Collection Methods

Existing eager methods such as `array.map`, `array.filter`, `array.any`, and
`array.sum` should be reviewed after the iterator core exists.

Preferred final shape:

```vela
let values = items.iter().map(|item| item.score).collect_array();
let has = items.iter().any(|item| item.enabled);
```

If direct collection methods remain, implement them as thin stdlib wrappers
over the iterator engine so semantics, callback dispatch, budget checks, and
TypeFacts do not drift.

### 4.6 Host-Provided Iterables

Host-provided iterables must obey the host boundary:

```text
host iterator state is an opaque host handle or copied snapshot
scripts never hold Rust references
items crossing into script are Value, HostRef, or PathProxy
mutation still goes through HostAccess
iterator lifetime is call-scoped unless the host explicitly returns a safe
runtime-managed handle
```

The first implementation should prefer snapshot or call-scoped opaque iterator
handles. Persistent host iterator handles should be deferred until the host
lifetime, invalidation, and hot-reload diagnostics are explicit.

---

## 5. Repository Anchors

Start implementation from these areas:

- `docs/architecture/language.md`
  - `for-in`, indexed `for-in`, strings, value categories, and dynamic typing.
- `docs/architecture/runtime.md`
  - `HeapValue::Iterator`, GC roots, budgets, and threading constraints.
- `docs/architecture/stdlib-and-embedding.md`
  - array/map/string stdlib surface and analysis-only callback facts.
- `docs/decisions.md`
  - indexed `for-in`, first-class `char`, Rust-like string indexing, and
    no-compatibility clean architecture policy.
- `crates/vela_vm/src/iteration.rs`
  - current range, string, array/map/set, and native iterator stepping boundary.
- `crates/vela_vm/src/owned_value.rs`
  - `OwnedIteratorState` materialization boundary.
- `crates/vela_vm/src/callback_method_dispatch.rs`
  - current eager callback-style collection method execution.
- `crates/vela_analysis/src/stdlib/`
  - analysis-only method facts for collections, strings, and callbacks.
- `crates/vela_bytecode/src/compiler/`
  - `for-in` lowering, callback lowering, and range-loop specialization.
- `crates/vela_bytecode/src/linked.rs`
  - linked bytecode shape and verifier-owned iteration operands.
- `crates/vela_vm/src/tests/iteration.rs`
  - current executable iteration tests.

---

## 6. Phased Execution Plan

### Phase 0: Audit Current Iteration Semantics

Goal: document the current behavior before replacing internals.

Tasks:

- Inspect parser/HIR/compiler lowering for ordinary and indexed `for-in`.
- Inspect VM `iteration.rs` and `HeapValue::Iterator`.
- Inspect eager array/map/set/string callback methods.
- Inspect tests for array, map, set, string, range, and native iterator loops.
- Record the current direct map iteration item shape if it exists.

Validation:

```bash
cargo test -p vela_vm iteration -- --nocapture
cargo test -p vela_vm standard_callback_id_dispatch -- --nocapture
```

Termination:

- A short implementation note or commit body names the current behavior and
  the first behavior being replaced.

### Phase 1: Define Runtime Iterator Kinds

Goal: add a focused internal representation for iterator sources and adapter
state.

Tasks:

- Introduce focused modules instead of growing generic VM files:

```text
crates/vela_vm/src/iteration/
  mod.rs
  source.rs
  state.rs
  step.rs
  adapters.rs
  host.rs
```

- Define an internal `IteratorSource` or equivalent enum for array, set, map
  view, string chars, string bytes, range, native iterator, host iterator, and
  adapter sources.
- Define `IteratorState` as one-shot cursor state over those source kinds.
- Keep `Sequence` as source/view state that can create a fresh iterator.
- Ensure all iterator state stored in the script heap traces owned script
  values and never traces Rust host state as script-owned GC memory.
- Charge memory budget when iterator/adaptor objects are heap allocated and
  charge collection growth only when final collections grow.

Validation:

```bash
cargo test -p vela_vm execution_core
cargo test -p vela_vm iteration
```

Termination:

- Existing `for-in` tests pass through the new internal iterator boundary.
- No unrelated stdlib callback behavior changes yet.

### Phase 2: Lower For-In Through The Unified Boundary

Goal: remove type-specific `for-in` execution paths that duplicate iterator
logic.

Tasks:

- Make compiler lowering represent `for-in` as:

```text
evaluate iterable expression once
create iterator from iterable
loop next(iterator) until Option::None / end marker
bind item pattern
execute body with break/continue support
```

- Preserve indexed `for index, value in iterable` as syntax-level loop lowering.
- Keep proven i64 range loops on their existing specialized path when the
  compiler can still prove the same facts.
- Preserve source spans for unsupported iterable errors and callback errors.
- Ensure dynamic unknown values fail at runtime only when the actual value is
  not iterable.

Validation:

```bash
cargo test -p vela_bytecode compiler::tests::expressions
cargo test -p vela_vm iteration
cargo test --workspace conformance
```

Termination:

- Array, string char, range, native iterator, and indexed `for-in` tests pass
  through one runtime iteration boundary or a documented proven range fast path.

### Phase 3: Add Explicit Sequence Methods

Goal: expose repeatable traversal views without eager allocation.

Tasks:

- Add standard method definitions and IDs for:

```text
array.iter()
set.iter()
map.keys()
map.values()
map.entries()
string.chars()
string.bytes()
range.iter() if direct range method syntax is desired
```

- Implement methods as `Sequence` or `Iterator` source creation without copying
  the full collection.
- Update analysis facts so callbacks over these sources receive correct
  element facts without public generics.
- Add docs for string `chars()` and `bytes()` alongside existing Rust-like
  string indexing rules.

Validation:

```bash
cargo test -p vela_analysis stdlib
cargo test -p vela_vm standard_string_id_dispatch
cargo test -p vela_vm standard_id_dispatch
cargo test -p vela_vm linked_standard_method_cache
```

Termination:

- New methods dispatch by stable standard method IDs and are covered by cache
  hit/miss/fallback tests where they use standard method caches.

### Phase 4: Implement Core Lazy Adapters

Goal: make common pipelines possible without intermediate arrays.

Tasks:

- Add iterator methods:

```text
next
count
any
all
find
map
filter
take
skip
collect_array
```

- Reuse existing callback call machinery where correct, but move shared
  callback-loop logic into an iterator-owned boundary so eager array helpers do
  not remain the semantic owner.
- Ensure short-circuit methods stop pulling values once the result is known.
- Ensure `collect_array` is the only core adapter in this phase that eagerly
  allocates output.
- Preserve budget checks for callback invocation, iterator stepping, and output
  growth.

Validation:

```bash
cargo test -p vela_vm standard_callback_id_dispatch
cargo test -p vela_vm linked_standard_method_cache
cargo test -p vela_vm --test conformance
```

Termination:

- Pipelines such as `items.iter().filter(...).map(...).collect_array()` work
  without allocating an intermediate array between `filter` and `map`.

### Phase 5: Rebase Eager Collection Helpers

Goal: prevent duplicate semantics between eager helpers and iterators.

Tasks:

- Review `array.map/filter/find/any/all/count/sum`, map callbacks, and set
  callbacks.
- Either remove pre-release eager helpers in favor of iterator syntax or keep
  them as thin wrappers over the iterator engine.
- Update docs, examples, and playground snippets to prefer iterator pipelines
  where that expresses the operation better.
- Do not preserve old eager implementation internals as compatibility shims.

Validation:

```bash
cargo test -p vela_vm standard_callback_id_dispatch
cargo test -p vela_engine
cargo test -p vela_examples
```

Termination:

- There is one implementation owner for callback iteration semantics.

### Phase 6: Host Iterable Slice

Goal: support host-owned business data streams without eager materialization.

Tasks:

- Add host registration metadata for iterable-returning native/host methods
  without exposing Rust iterator types directly to scripts.
- Define call-scoped host iterator handles or snapshot-backed iterables.
- Ensure item reads return copied script values, `HostRef`, or `PathProxy`
  values; writes still route through HostAccess.
- Add stale-generation and permission diagnostics for host iterable items where
  applicable.
- Keep persistent host iterators deferred unless the lifetime model is explicit.

Validation:

```bash
cargo test -p vela_host
cargo test -p vela_engine host
cargo test -p vela_vm iteration
```

Termination:

- A standalone example can iterate host-provided data without building a full
  script array and without exposing Rust references.

### Phase 7: Performance And Cache Checkpoint

Goal: measure before adding specialized cache work.

Tasks:

- Add benchmark rows for:

```text
string chars iteration
string bytes iteration
array iter map/filter collect
array iter any/all/find short-circuit
map keys/values/entries iteration
range iteration retained fast path
host iterable iteration
```

- Compare lazy pipelines against old eager helpers where both still exist.
- Add inline cache entries only for measured hot families.
- Record durable threshold or exit conclusions in `docs/performance.md` only
  if this becomes a benchmark checkpoint.

Validation:

```bash
cargo bench -p vela_vm --bench baseline -- --quick collection
cargo bench -p vela_vm --bench baseline -- --quick string
cargo bench -p vela_vm --bench external_compare -- --quick array
```

Termination:

- The plan either closes with measured acceptable deltas or names deferred
  cache/JIT/value-layout work explicitly.

---

## 7. Required Tests

Add or update tests in these categories:

```text
parser/HIR:
  explicit iterator method calls parse and resolve
  callback parameter facts flow through iterator adapters

compiler:
  for-in evaluates iterable expression once
  indexed for-in preserves source positions
  dynamic unknown iterable values stay runtime-checked
  proven i64 ranges keep specialized lowering where applicable

VM:
  Array/set/map/string/range iteration
  string chars yield char, string bytes yield u8
  iterator next returns Option
  lazy map/filter/take/skip do not allocate intermediate arrays
  any/all/find short-circuit
  collect_array charges output growth
  iterator state survives GC when rooted and is reclaimed when unreachable
  budget traps keep source spans

host:
  host iterable items do not expose Rust references
  stale host item access reports source-spanned errors
  host item mutation routes through HostAccess

hot reload:
  Iterator bytecode and cache-site changes are ABI-safe
  accepted reload clears stale iterator-related inline caches

docs/examples:
  String chars/bytes examples
  iterator pipeline example
  host iterable example
```

---

## 8. Documentation Updates

Update these docs as implementation lands:

- `docs/architecture/language.md`
  - add the Iterator/Sequence semantic model and direct `for-in` rules.
- `docs/architecture/runtime.md`
  - document iterator heap state, GC tracing, and host boundary rules.
- `docs/architecture/stdlib-and-embedding.md`
  - replace eager-first collection descriptions with iterator-first APIs where
    appropriate.
- `docs/decisions.md`
  - add a concise decision once the core model lands.
- `docs/grammar.ebnf`
  - update only if new syntax is added. Method-only APIs do not require grammar
    changes.
- `site/docs/en` and `site/docs/zh`
  - update user-facing language and stdlib docs after the model is executable.

---

## 9. Completion Criteria

The Iterator/Sequence model is complete enough when:

- `for-in` has one semantic iteration boundary plus documented specialized
  fast paths.
- Strings expose explicit `chars()` and `bytes()` traversal, and direct string
  `for-in` yields `char`.
- Arrays, sets, maps, strings, ranges, and at least one host-provided iterable
  can be traversed through the same model.
- Core lazy adapters work and do not allocate intermediate arrays.
- Eager collection helpers are either removed or implemented as wrappers over
  the iterator engine.
- Analysis provides useful callback item facts without script-language
  generics.
- GC, budget, hot reload, source spans, and host safety tests cover the model.
- Benchmarks exist for the major iterator families, with any remaining
  performance gaps assigned to M20 cache work, M22 JIT, or value-layout work.

---

## 10. Suggested First Task

```text
Task: Audit and centralize current iteration semantics.
Context: The repository already has for-in, string char iteration, range
lowering, native iterator values, and eager callback collection helpers. This
belongs to the Iterator/Sequence model plan and should prepare a clean vertical
slice without changing public semantics yet.
Expected behavior:
  - Existing for-in behavior remains passing.
  - Current array/string/range/native iterator paths are identified.
  - Current direct map/set iteration behavior is named.
  - The next commit can introduce IteratorSource/SequenceSource without guessing.
Tests:
  - cargo test -p vela_vm iteration -- --nocapture
  - cargo test -p vela_vm standard_callback_id_dispatch -- --nocapture
Do not change:
  - Do not add script-language generics.
  - Do not expose Rust iterator references to scripts.
  - Do not add char_at or string character indexing.
Validation:
  cargo fmt --all -- --check
```
