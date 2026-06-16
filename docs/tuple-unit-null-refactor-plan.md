# Tuple, Unit, And Null Removal Implementation Plan

> **Track:** breaking language-value-model cleanup, M20/M23 adjacent
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release syntax, bytecode, VM value,
> stdlib, host conversion, reflection, serialization, tooling, diagnostics, and
> tests are allowed. Do not preserve `null` as a normal script no-value result
> for compatibility. Preserve product contracts: no general script-language
> generics, no Rust `&mut` exposure, HostAccess safety, GC roots,
> source-spanned diagnostics, execution budgets, reflection permissioning, and
> hot-reload ABI/schema checks.

---

## 0. Codex Goal

```text
/goal Implement Vela's breaking tuple, unit, and null cleanup from
docs/tuple-unit-null-refactor-plan.md. Treat docs/goal.md as the product
roadmap, docs/architecture.md and docs/architecture/*.md as the architecture
contract, and docs/progress.md as the current milestone state. Add Rust-like
tuple syntax and contracts using `(T1, T2)`, `(value1, value2)`, and
destructuring patterns, with `()` as the unit type and unit value. Replace
void-like and no-meaningful-value uses of `null` with `()`. Keep expected
absence in `Option<T>` and recoverable failure in `Result<T, E>`. Remove
`null` from ordinary script APIs; if external data formats need to preserve
raw null, represent that explicitly at the serde/JSON boundary rather than
overloading the VM no-value result. Prefer clean replacement over compatibility
aliases or migration shims. Validate each checkpoint with parser, HIR,
analysis, compiler, VM, stdlib, host conversion, reflection, hot-reload ABI,
LSP, formatter, diagnostics, and conformance tests, plus focused benchmark
captures for tuple/unit fast paths.
```

---

## 1. Purpose

Vela currently uses `null` for several unrelated concepts:

```text
no meaningful value / void-like result
expected absence
host nullable data
reflection metadata gaps
external serialized null
```

This is convenient during early implementation, but it weakens the language
contract. Vela already has `Option` and `Result` for ordinary script absence
and recoverable failure, and it needs a clear no-value result for statement-only
blocks, callbacks, and no-result native calls.

The target language model is:

```text
()             no meaningful value / unit
Option<T>      expected absence
Result<T, E>   recoverable failure
VM error       contract violation, permission denial, budget failure, script bug
external null  explicit serde/JSON boundary value, not ordinary script no-value
```

At the same time, Vela needs ergonomic multiple-return and destructuring syntax.
Tuple syntax should look like Rust source syntax rather than exposing a
dedicated `Tuple` generic:

```vela
fn split_name(full: String) -> Option<(String, String)> {
    let parts = full.split_once(" ")?;
    return Option::Some((parts.left, parts.right));
}

fn main() -> Result<(), Error> {
    let (first, last) = split_name("Ada Lovelace")?;
    return Result::Ok(());
}
```

The implementation may use a dedicated internal tuple value, heap object, type
fact, and bytecode representation, but source users should write tuple syntax,
not `Tuple<String, String>`.

---

## 2. Goals

- Add `()` as the unit type and unit literal.
- Make empty blocks, statement-only blocks, no-result native calls, callbacks
  with no meaningful result, and `return;` produce unit.
- Add tuple type syntax: `(A, B)`, `(A, B, C)`, and `(A,)` for a one-element
  tuple if one-element tuples are supported.
- Keep `(A)` as parenthesized type syntax, not a one-element tuple.
- Add tuple expression syntax: `(a, b)`, `(a, b, c)`, and `(a,)` if one-element
  tuples are supported.
- Keep `(a)` as parenthesized expression syntax, not a one-element tuple.
- Add tuple destructuring patterns at least for `let` bindings and `match`
  patterns.
- Add tuple TypeFacts and runtime contracts so typed tuple values can use
  fast paths and precise diagnostics.
- Make `?` work cleanly with tuple payloads inside `Option<(...)>` and
  `Result<(...), E>` without making tuples a general generic feature.
- Move ordinary script absence to `Option::None`.
- Move ordinary recoverable failure to `Result::Err`.
- Remove `null` from ordinary script no-value semantics.
- Decide and implement one explicit external-null boundary:
  either remove the script `null` literal entirely or keep raw external null
  only inside explicit data wrappers such as `Json::Null` or `SerdeValue::Null`.
- Update host conversion so Rust `Option<T>` maps to script `Option<T>` rather
  than `null`.
- Update reflection so missing optional metadata is represented as
  `Option::None` or omitted structured fields, not untyped `null`.
- Preserve hot-reload ABI/schema checking with tuple and unit contract changes.
- Update docs, website, grammar, LSP semantic tokens, formatting, hover,
  completions, diagnostics, and snippets to the new model.

---

## 3. Non-Goals

This pass must not:

- Add general user-defined generics.
- Add named tuple fields.
- Add variadic generics, tuple trait impl expansion, or arbitrary tuple arity
  metaprogramming.
- Use tuples as a replacement for records when fields need durable names.
- Add Python-style multiple assignment unrelated to tuple destructuring.
- Add implicit conversion between `()` and `Option::None`.
- Add implicit conversion between external raw null and `Option::None` at
  untyped script boundaries.
- Preserve `null` as a synonym for `()` or `Option::None`.
- Keep old APIs returning `null` behind compatibility wrappers.
- Weaken HostAccess, execution budgets, GC roots, reflection permissions, or
  hot-reload ABI/schema checks.

---

## 4. Target Semantics

### 4.1 Unit

`()` is a real value with one inhabitant. It means "there is no meaningful
result", not "missing data".

Unit-producing cases:

```text
empty block
statement-only block
expression-valued if branch whose body has no meaningful result
loop bodies without meaningful values
native functions registered as no-result callbacks
script functions with no return value
explicit `return;`
```

Examples:

```vela
fn log_level(level: i64) -> () {
    log::info("level changed");
}

fn update(player: Player) -> () {
    player.level += 1;
    return;
}

fn main() -> Result<(), Error> {
    update(ctx.player);
    return Result::Ok(());
}
```

Unit is not absence. APIs such as `map.get(key)`, `find(...)`, and
`split_once(...)` should return `Option<T>`, not `()`.

### 4.2 Option And Result

`Option<T>` and `Result<T, E>` remain restricted builtin parameterized
type-hint contracts. They are not user-defined generics.

Expected absence:

```vela
fn find_player(id: i64) -> Option<Player> {
    return players.get(id);
}
```

Recoverable failure:

```vela
fn charge(account: Account, amount: i64) -> Result<(), ChargeError> {
    if account.balance < amount {
        return Result::Err(ChargeError::InsufficientFunds);
    }

    account.balance -= amount;
    return Result::Ok(());
}
```

The `?` operator should propagate `Option::None` and `Result::Err` without
special tuple behavior. Tuple payloads are ordinary payload values:

```vela
fn split_name(full: String) -> Option<(String, String)> {
    let parts = full.split_once(" ")?;
    return Option::Some((parts.left, parts.right));
}
```

### 4.3 Tuples

Tuples are ordered, fixed-size product values. They are for temporary grouping,
multiple return values, and destructuring, not for durable business records.

Tuple type syntax:

```text
()                  unit
(String, String)    two-element tuple
(i64, bool, String) three-element tuple
(i64,)              optional one-element tuple spelling
(i64)               parenthesized i64 type
```

Tuple expression syntax:

```text
()                  unit literal
(first, last)       two-element tuple literal
(value,)            optional one-element tuple literal
(value)             parenthesized expression
```

Destructuring should reject arity mismatches:

```vela
let (first, last) = split_name("Ada Lovelace")?;
let (x, y, z) = point; // rejected if point is a 2-tuple
```

Tuple fields are positional. The first pass may expose positional accessors
only through destructuring. If direct access is added, prefer a syntax that does
not conflict with numeric field names or record fields.

### 4.4 External Null

Raw external null should not be the language's no-value result.

There are two acceptable final designs. The implementation plan should choose
one before code changes start:

```text
strict removal:
  remove script `null` literal and Value::Null from ordinary VM values
  map typed nullable host data to Option<T>
  preserve raw JSON/serde null only in explicit external data wrappers

explicit external null:
  keep a raw null value only inside explicit external data domains
  ordinary script APIs still cannot return null for absence or no-value
  type hints do not use `null` as a normal contract
```

The strict removal model is cleaner for the core language. The explicit
external-null model is easier for untyped JSON-like data. Either way, ordinary
script code should not use `null` for void, missing data, or recoverable
failure.

---

## 5. Architecture Impact

### 5.1 Syntax And Parser

Update grammar and parser support for:

```text
unit literal
unit type
tuple type
tuple expression
tuple pattern
return; as unit return
```

Parser ambiguity rules:

```text
()      unit expression or unit type depending on context
(x)     parenthesized expression
(T)     parenthesized type
(x,)    one-element tuple expression if supported
(T,)    one-element tuple type if supported
(x, y)  tuple expression
(A, B)  tuple type
```

Remove `null` from literal grammar if strict removal is selected. If explicit
external-null is selected, keep raw null only under the explicit data syntax or
constructor, not as a global literal used by normal expression typing.

### 5.2 HIR, Analysis, And TypeFacts

Add focused HIR forms:

```text
ExprKind::Unit
ExprKind::Tuple(Vec<ExprId>)
PatternKind::Unit
PatternKind::Tuple(Vec<PatternId>)
TypeHintKind::Unit
TypeHintKind::Tuple(Vec<TypeHint>)
```

Analysis should produce:

```text
TypeFact::Unit
TypeFact::Tuple(Vec<TypeFact>)
```

Tuple facts are trustworthy only when they come from verified contracts,
literal construction, or guarded dynamic boundaries. A tuple type hint alone
must not let the compiler skip guards for an unverified dynamic value.

### 5.3 Bytecode And VM Value Model

Add an internal representation for unit and tuples:

```rust
Value::Unit
Value::TupleInline(...) or HeapValue::Tuple(Vec<Value>)
```

The exact layout is an implementation decision:

- Small inline tuples are faster for common two-value returns.
- Heap tuples reduce enum size churn and are simpler for arbitrary arity.
- A hybrid layout may be added only if measurements justify the complexity.

Remove or isolate `Value::Null` according to the selected external-null policy.
Do not keep `Value::Null` as the ordinary no-result value.

The compiler should lower tuple construction and destructuring directly rather
than routing through generic array creation.

### 5.4 Stdlib And Builtins

Update stdlib APIs:

```text
functions with no meaningful result -> ()
lookup/search/split APIs -> Option<T>
fallible APIs -> Result<T, E>
callbacks with effect-only behavior -> () return
raw JSON/serde null -> explicit external value if needed
```

Potential examples:

```text
String.split_once(separator) -> Option<(String, String)>
Map.get(key) -> Option<V>
Array.find(predicate) -> Option<T>
Array.push(value) -> ()
Set.add(value) -> bool or () depending on final API semantics
```

### 5.5 Host Conversion And Embedding

Host conversion rules should become explicit:

```text
Rust ()                <-> Vela ()
Rust Option<T>::None   <-> Vela Option::None
Rust Option<T>::Some   <-> Vela Option::Some(value)
Rust Result<T, E>      <-> Vela Result<T, E> where registered
Rust tuples            <-> Vela tuples for supported arities
JSON/serde null        <-> explicit external null wrapper or typed Option::None
```

No host adapter should rely on `null` as a catch-all missing value. Untyped
host nullable data must choose between typed `Option<T>` and explicit raw data
wrappers.

### 5.6 Reflection And Metadata

Reflection metadata should model optional fields with `Option`, omitted fields,
or structured absence flags. It should not encode missing metadata as ordinary
script `null`.

Reflection must expose tuple and unit type descriptors:

```text
TypeDesc::Unit
TypeDesc::Tuple { elements: Vec<TypeDesc> }
```

Hot reload and schema compatibility must compare tuple arity and element
contracts structurally.

### 5.7 Serialization And OwnedValue

`OwnedValue` should distinguish:

```text
Unit
Tuple(Vec<OwnedValue>)
Option / Result enum-shaped values
ExternalNull only if explicit external-null is selected
```

If strict null removal is selected, raw JSON null cannot silently round-trip as
ordinary `OwnedValue::Null`; it must use an external data wrapper or typed
`Option::None`.

### 5.8 Equality, Ordering, Map, And Set

Unit is equality-keyable by a single stable key.

Tuple equality should follow the object equality plan:

- Builtin leaf tuple equality may be allowed only when all elements are
  comparable under the ordinary `PartialEq` rules.
- Tuple ordering should require all elements to satisfy the relevant ordering
  contract.
- Map/Set keyability for tuples should be a deliberate policy decision:
  either allow tuples only when all elements are `ValueKey` keyable, or reject
  tuple keys in the first slice to keep the key model small.

Do not make tuple Map/Set key behavior call user comparison traits.

### 5.9 Hot Reload ABI

Hot reload ABI checks must treat the following as incompatible:

```text
null no-result -> () when visible in exported function/native signatures
T -> (T, U)
tuple arity changes
tuple element contract changes
Option<T> payload tuple element changes
Result<T, E> payload tuple element changes
external raw null policy changes in host schema
```

Internal-only functions may change according to normal module recompilation
rules, but exported ABI and service-provider contracts must remain strict.

### 5.10 Tooling

Update:

```text
grammar docs
formatter spacing for tuple types, tuple expressions, and tuple patterns
semantic tokens for unit, tuple punctuation, Option/Result payloads
hover and completion display for tuple contracts
signature help for tuple return values
inlay hints around destructuring when useful
rename/reference behavior inside tuple patterns
diagnostics for arity mismatch, ambiguous one-element tuples, and null removal
website docs and playground examples
```

---

## 6. Implementation Phases

### Phase 1: Decide External Null Policy

- Choose strict null removal or explicit external-null wrapper.
- Update `docs/decisions.md`.
- Update architecture docs to distinguish current implementation from target
  semantics if implementation will be incremental.
- Add parser and VM tests that assert the old `null` behavior is no longer the
  target.

Exit criteria:

```text
durable decision recorded
tests name the selected null policy
no runtime changes yet required
```

### Phase 2: Add Unit

- Add `()` parsing in expression and type contexts.
- Add unit HIR/type facts.
- Add `Value::Unit` or equivalent runtime representation.
- Make empty/statement-only blocks produce unit.
- Make no-result functions and `return;` produce unit.
- Update host conversion for Rust `()`.
- Update diagnostics to print unit clearly.

Tests:

```text
parser accepts unit expression and type
compiler lowers empty block and return;
VM returns unit for effect-only functions
host native () returns unit
diagnostics render unit in type mismatch messages
```

### Phase 3: Move Absence To Option

- Update stdlib and native APIs that returned `null` for expected absence.
- Update Rust `Option<T>` conversion to use dynamic Option values.
- Update `?` tests for `Option<T>` paths.
- Update docs and examples.

Tests:

```text
lookup APIs return Option::None
split/search APIs return Option::None
typed Rust Option<T> round-trips through script Option
old null-as-not-found behavior is rejected or absent
```

### Phase 4: Add Tuple Syntax And Runtime Values

- Add tuple expressions, types, and patterns.
- Add tuple construction bytecode.
- Add tuple destructuring lowering.
- Add runtime tuple guards.
- Add tuple OwnedValue and reflection descriptors.
- Add host conversion for selected Rust tuple arities.

Tests:

```text
tuple literals evaluate to fixed-size tuples
tuple destructuring binds values in order
arity mismatch is a source-spanned diagnostic
tuple type hints guard dynamic values
host Rust tuple arities convert correctly
reflection reports tuple element descriptors
```

### Phase 5: Integrate Option/Result With Tuple Payloads

- Make `Option<(A, B)>` and `Result<(A, B), E>` precise in type facts.
- Ensure `?` propagation preserves tuple payloads.
- Update stdlib APIs such as `split_once`.
- Add benchmark rows for tuple-return hot paths if they become common in
  standard library code.

Tests:

```text
Option tuple payload unwraps through ?
Result tuple payload unwraps through ?
split_once-style APIs return Option tuple payloads
tuple payload type mismatches fail at guarded boundaries
```

### Phase 6: Remove Or Isolate Null

- Remove global `null` literal and `null` type hint if strict removal is
  selected.
- Or move raw null behind explicit external data wrappers if that policy is
  selected.
- Remove ordinary `Value::Null` usage from compiler, VM, stdlib, host bridge,
  reflection, and tests.
- Update documentation and diagnostics.

Tests:

```text
source-level null is rejected under strict removal
ordinary APIs do not return null
external raw null wrapper preserves JSON/serde null if selected
missing reflection metadata no longer appears as null
```

### Phase 7: Hot Reload, LSP, Formatter, And Website

- Update ABI comparison for unit and tuples.
- Update schema artifacts.
- Update LSP parsing, semantic tokens, completion, hover, signature help,
  formatting, references, rename, and diagnostics.
- Update website docs and playground examples.
- Update conformance fixtures.

Tests:

```text
hot reload rejects exported unit/tuple signature changes
schema artifact round-trips unit and tuple descriptors
formatter preserves tuple syntax
LSP hover/completion/signature help render tuple/unit contracts
website builds with updated examples
```

### Phase 8: Performance And Cleanup

- Add focused tuple/unit benchmark rows.
- Profile common tuple-return stdlib paths.
- Remove obsolete null compatibility helpers, tests, docs, and diagnostic
  wording.
- Validate full workspace.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo bench -p vela_vm --bench external_compare -- --quick tuple
```

The benchmark row may be added during implementation. If it does not exist yet,
record the focused VM/std-lib benchmark command that replaces it.

---

## 7. Test Plan

Parser and grammar:

```text
unit literal/type
tuple expression/type/pattern
parentheses vs one-element tuple ambiguity
null rejection or explicit external-null syntax
```

HIR and analysis:

```text
unit and tuple TypeFacts
tuple destructuring bindings
tuple arity mismatch diagnostics
Option/Result tuple payload propagation
```

Compiler and VM:

```text
unit-returning blocks/functions/native calls
tuple construction/destructuring
tuple guards at typed dynamic boundaries
? propagation with tuple payloads
old null no-result paths removed
```

Host, reflection, and serde:

```text
Rust () conversion
Rust Option<T> conversion
selected Rust tuple arity conversion
TypeDesc::Unit and TypeDesc::Tuple
OwnedValue unit/tuple/external-null policy
JSON/serde null behavior under selected policy
```

Hot reload:

```text
exported function unit/tuple ABI comparisons
tuple arity and element contract changes
Option/Result tuple payload changes
provider/service signature changes
```

Tooling:

```text
formatter tuple/unit syntax
semantic tokens
hover/completion/signature help
diagnostics
website build
playground examples
```

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm --prefix site run build
```

---

## 8. Design Rules For Implementation

- Keep unit, tuple, Option, Result, and external null as separate concepts.
- Do not implement `null` aliases for `()` or `Option::None`.
- Do not trust tuple type hints until values are proven by construction,
  verified contracts, or runtime guards.
- Keep tuple syntax structural; do not add a public `Tuple<T, U>` type.
- Keep tuples fixed-size and ordered; use records for named fields.
- Keep tuple arity limits explicit in host conversions and ABI metadata.
- Keep `Option<T>` and `Result<T, E>` as restricted builtin type-hint
  parameterization, not general script-language generics.
- Keep raw external null out of ordinary script control flow.
- Prefer source-spanned breaking diagnostics over compatibility coercions.

---

## 9. Open Decisions

These should be resolved before implementation starts:

1. Should source-level `null` be removed completely, or should raw null remain
   only through an explicit external data wrapper?
2. Should one-element tuples be supported in the first slice with `(T,)` and
   `(value,)`, or deferred until a concrete use case appears?
3. Should tuples be allowed as Map keys and Set elements in the first slice
   when all elements are `ValueKey` keyable?
4. Should tuple direct field access exist, or should first-slice tuple use be
   destructuring-only?
5. What maximum tuple arity should host conversion support without additional
   boilerplate?
