# Tuple, Unit, And Null Removal Hard-Switch Plan

> **Track:** breaking language-value-model cleanup, M20/M23 adjacent
> **Document status:** Codex goal-mode execution plan
> **Compatibility policy:** breaking pre-release syntax, bytecode, VM value,
> stdlib, host conversion, reflection, serialization, tooling, diagnostics, and
> tests are allowed. Preserve product contracts: no general script-language
> generics, no Rust `&mut` exposure, HostAccess safety, GC roots,
> source-spanned diagnostics, execution budgets, reflection permissioning, and
> hot-reload ABI/schema checks.

Hard-switch policy: this plan is intended to be run by goal-mode loops using
large deletion-first slices, not one-`null`-site micro-commits. At the start of
each slice, delete the obsolete `null` surface for that subsystem first, then
use compiler errors, audit searches, and focused failing tests as the migration
queue. It is acceptable for the working tree to be temporarily uncompilable
while a hard-switch slice is in progress, but every committed checkpoint must
compile and pass the relevant focused validation. Do not add compatibility
aliases, fallback conversions, `null`-to-unit coercions, migration helper names,
dual `null`/unit result APIs, or temporary tests that only prove both models
still work.

Checkpoint policy: prefer large subsystem checkpoints. A valid checkpoint
removes a whole obsolete surface such as source `null`, VM/bytecode `Null`,
host/owned `Null`, stdlib `null` return contracts, or LSP/editor `null`
language behavior. Do not commit one call-site replacement at a time unless it
is the last blocker for one of those subsystem checkpoints.

Final-state policy: ordinary Vela code must not contain source-level `null`,
`Value::Null`, `OwnedValue::Null`, `HostValue::Null`, `Constant::Null`,
`PrimitiveTag::Null`, `Literal::Null`, `NullKw`, `TypeHint::null`, `null`
stdlib contracts, `null` completions, or tests that preserve old null behavior.
JSON-RPC protocol nulls, serde JSON fixture nulls, and editor protocol
`processId: null` values are outside the Vela language value model, but they
must be classified explicitly in final audits instead of hidden by broad ignore
patterns.

## 0. Codex Goal

Use this prompt to execute the full refactor:

```text
/goal Execute the breaking tuple, unit, and null hard switch from
docs/tuple-unit-null-refactor-plan.md. Treat docs/goal.md,
docs/architecture.md, docs/architecture/*.md, docs/progress.md,
docs/decisions.md, docs/grammar.ebnf, and this plan as required context.
This is a breaking pre-release language value-model refactor: the priority is
to delete ordinary `null` semantics and finish the `()` / tuple / Option /
Result architecture, not to preserve compatibility with old no-value behavior.

At the start of each execution turn, inspect the current git diff, then inspect
remaining references to source `null`, NullKw, Literal::Null, Value::Null,
OwnedValue::Null, HostValue::Null, Constant::Null, PrimitiveTag::Null,
TypeKind::Null, TypeHint::null, StdTypeSpec::primitive("Null"), stdlib/native
return type strings "null", null completions/snippets/semantic tokens,
reflection metadata nulls, hot-reload null ABI rows, and tests/examples that
expect null as void, absence, missing metadata, or host no-result. Separately
classify protocol JSON nulls in LSP/server/schema fixtures; they are not Vela
language nulls.

Use a hard-switch strategy with large subsystem slices. Delete the obsolete
surface at the start of a slice, then use compiler errors and focused failing
tests as the migration queue. It is acceptable for the working tree to be
temporarily uncompilable during a turn, but do not commit until the changed
slice has focused tests passing. Do not add `null` aliases for unit,
Option::None, or Result errors. Do not add compatibility wrappers, migration
helpers, dual return APIs, external-null fallbacks, or temporary tests that keep
old and new semantics alive.

Current execution order:
1. Lock hard decisions and baselines: strict source-level null removal, no
   one-element tuples in the first slice, destructuring-only tuple access,
   tuple Map/Set keys rejected initially, and host Rust tuple conversion
   limited to arities 2 through 4 until measured need expands it.
2. Hard-switch syntax and tooling grammar. Add unit expression/type syntax,
   tuple expression/type/destructuring syntax, and source-spanned diagnostics
   for removed `null`. Delete `NullKw`, Literal::Null extraction, global null
   literal parsing, grammar docs, editor grammar entries, null completions,
   null semantic-token classifications, and quick fixes that insert `null`.
3. Hard-switch the core value model. Add Unit and tuple runtime/owned/host
   representations, then delete Value::Null, OwnedValue::Null, HostValue::Null,
   Constant::Null, PrimitiveTag::Null, null type facts, null keying/equality,
   null guard plans, null verifier names, and null register initialization.
   Replace no-result defaults with unit and use compile errors as the queue.
4. Hard-switch compiler, VM, stdlib, host, reflection, engine, and hot reload.
   No-result blocks, `return;`, native callbacks, mutation helpers, IO helpers,
   reflection metadata gaps, and host methods return unit or structured
   Option/Result. Expected absence uses Option::None. Recoverable failures use
   Result::Err. Hot-reload/schema ABI compares unit, tuple arity, and tuple
   element contracts structurally.
5. Add tuple payload behavior after the core null deletion is underway:
   Option<(A, B)> and Result<(A, B), E> facts, guards, bytecode lowering,
   destructuring, host arity 2..=4 conversion, reflection descriptors,
   OwnedValue tuple conversion, and stdlib APIs such as split_once.
6. Audit LSP, language service, formatter, examples, docs, and website. Replace
   user-facing null placeholders with `()`, Option::None, or typed fixits.
   Keep protocol JSON nulls only where the protocol or serde fixture requires
   them and document that classification in close-out notes.
7. Before final acceptance, run zero-result audits for Vela-language null
   symbols and separately review all surviving `null` strings. Every survivor
   must be protocol JSON, historical archive, or an explicit external-data
   wrapper introduced by a documented future plan. No temporary helper,
   compatibility wrapper, migration-only name, or transitional null test may
   remain.
8. Update docs/progress.md, docs/decisions.md, architecture docs, grammar docs,
   and this checklist only when milestone state, final architecture, or
   remaining gaps materially change.

For each checkpoint, choose the largest deletion-first subsystem slice that can
be restored to focused green validation in one execution turn. Validate with
the narrowest relevant tests first, usually cargo test -p vela_syntax,
cargo test -p vela_bytecode --no-fail-fast, cargo test -p vela_vm,
cargo test -p vela_engine, cargo test -p vela_reflect, cargo test -p
vela_language_service, and cargo test -p vela_lsp_server as the touched surface
expands. Close out with cargo fmt --all -- --check,
cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace,
and the examples test suite when practical. Commit large coherent Conventional
Commit checkpoints.
```

## 1. Purpose

The current implementation uses `null` for unrelated concepts:

```text
no meaningful value / void-like result
expected absence
host nullable data
reflection metadata gaps
external serialized null
register/default initialization
```

This weakens the language contract and keeps APIs ambiguous. Vela already has
`Option` for expected absence and `Result` for recoverable failure. It should
use `()` for no meaningful result and tuples for temporary product values.

The target model is:

```text
()             no meaningful value / unit
Option<T>      expected absence
Result<T, E>   recoverable failure
VM error       contract violation, permission denial, budget failure, script bug
external null  future explicit serde/JSON data wrapper, not ordinary script value
```

Tuple syntax should look like Rust source syntax without adding general
script-language generics:

```vela
fn split_name(full: String) -> Option<(String, String)> {
    let parts = full.split_once(" ")?;
    return Option::Some((parts.left, parts.right));
}

fn main() -> Result<(), Error> {
    let (first, last) = split_name("Ada Lovelace")
        .ok_or(Error::InvalidName)?;
    return Result::Ok(());
}
```

## 2. Selected Hard Decisions

These decisions are part of the plan and should not remain open during
implementation:

- Source-level `null` is removed from ordinary Vela. The first implementation
  does not keep a global `null` literal or `null` type hint.
- Raw external JSON/serde null is not modeled by `Value::Null`. If it is needed
  later, it must live inside an explicit external-data wrapper such as a future
  `Json::Null` or `SerdeValue::Null`.
- `()` is the only ordinary no-meaningful-result value.
- Rust `Option<T>` maps to script `Option<T>`, not to `null`.
- One-element tuples are deferred. `(x)` and `(T)` stay parenthesized
  expression/type syntax; `(x,)` and `(T,)` should be rejected with a clear
  diagnostic until a concrete first-class use case exists.
- Tuple direct field access is deferred. First-slice tuple use is through
  construction, return values, destructuring, pattern matching, reflection, and
  host conversion only.
- Tuple Map/Set keys are rejected in the first slice even when all elements
  are individually keyable. This avoids expanding `ValueKey` before tuple
  equality and ordering semantics are fully measured.
- Host Rust tuple conversion supports arities 2 through 4 in the first slice.
  Expanding the limit later requires focused tests and no broad macro magic.
- `?` remains Rust-aligned: `Option` propagates through Option-returning
  functions, `Result` through Result-returning functions, and cross-family
  conversion requires explicit helpers such as `ok_or`.

## 3. Current Codebase Audit Notes

The current code has broad `null` surface area. Goal-mode turns should use
these as concrete deletion targets:

- Syntax and grammar: `NullKw`, `Literal::Null`, literal patterns, formatter
  keyword handling, parser grammar, docs grammar, and tree-sitter grammar.
- Compiler and bytecode: `Constant::Null`, null default returns, null if/block
  branch defaults, null type facts, null guards, null verifier names, and
  `null_values` lowering helpers.
- VM runtime: `Value::Null`, null register initialization, null equality,
  null keying, null truthiness, null runtime guards, null serde unit/none
  decoding, and null standard method mutation returns.
- Owned and host boundaries: `OwnedValue::Null`, `HostValue::Null`, Rust
  `Option<T>` conversion through null, host method no-result returns, and
  reflection conversion through null.
- Stdlib and embedding metadata: `PrimitiveTag::Null`, `"Null"` std type
  entries, `"null"` return contract strings, `TypeHint::null`, IO helpers
  returning `Result::Ok(null)`, and context/native method descriptors.
- Reflection and hot reload: missing metadata as null, reflected type/kind
  names for null, schema/ABI null primitive rows, and tests that compare
  null signature changes.
- Language service and editor packages: null completions, semantic-token
  null classification, quick fixes that insert `null`, snippets/placeholders,
  formatter examples, tree-sitter null literals, docs, examples, and website
  examples.
- Protocol fixtures: LSP JSON-RPC `null` values and schema JSON nulls must be
  separated from Vela-language null during audits; they are allowed only as
  protocol/serde data, not as script semantics.

## 4. Target Semantics

### 4.1 Unit

`()` is a real value with one inhabitant. It means "there is no meaningful
result", not "missing data".

Unit-producing cases:

```text
empty block
statement-only block
expression-valued branch with no meaningful result
loop bodies without meaningful values
native functions registered as no-result callbacks
script functions with no return value
explicit return;
mutation helpers whose contract is effect-only
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
special tuple behavior. Tuple payloads are ordinary payload values.

### 4.3 Tuples

Tuples are ordered, fixed-size product values for temporary grouping, multiple
return values, and destructuring. They are not durable business records.

Tuple type syntax:

```text
()                  unit
(String, String)    two-element tuple
(i64, bool, String) three-element tuple
(i64)               parenthesized i64 type
(i64,)              rejected in the first slice
```

Tuple expression syntax:

```text
()                  unit literal
(first, last)       two-element tuple literal
(value)             parenthesized expression
(value,)            rejected in the first slice
```

Destructuring should reject arity mismatches:

```vela
let (first, last) = split_name("Ada Lovelace")?;
let (x, y, z) = point; // rejected if point is a 2-tuple
```

### 4.4 External Null

External null is not part of the ordinary script value model in this hard
switch. JSON-RPC and serde fixtures may still contain JSON null, but those
values must remain at protocol/data boundaries. Do not route them through
`Value::Null`, `OwnedValue::Null`, or `HostValue::Null`.

## 5. Architecture Impact

### 5.1 Syntax And Parser

Add syntax support for:

```text
unit literal
unit type
tuple type
tuple expression
tuple destructuring pattern
return; as unit return
source-spanned null-removal diagnostics
```

Remove global `null` literal and `null` type-hint parsing. If a future explicit
external-data wrapper is added, it must be ordinary named syntax such as
`Json::Null`, not a resurrected global literal.

### 5.2 HIR, Analysis, And TypeFacts

Add focused forms:

```text
ExprKind::Unit
ExprKind::Tuple(Vec<ExprId>)
PatternKind::Unit
PatternKind::Tuple(Vec<PatternId>)
TypeHintKind::Unit
TypeHintKind::Tuple(Vec<TypeHint>)
TypeFact::Unit
TypeFact::Tuple(Vec<TypeFact>)
```

Tuple facts are trustworthy only when they come from construction, verified
contracts, or runtime guards. A tuple type hint alone must not let the compiler
skip guards for an unverified dynamic value.

### 5.3 Bytecode And VM Value Model

Add:

```text
Value::Unit
OwnedValue::Unit
HostValue::Unit or an equivalent no-result host contract
tuple runtime representation
tuple owned representation
tuple reflection/runtime type facts
```

Then delete:

```text
Value::Null
OwnedValue::Null
HostValue::Null
Constant::Null
PrimitiveTag::Null
null register defaults
null equality/keying/truthiness/guard behavior
```

The compiler should lower tuple construction and destructuring directly rather
than routing through generic array creation.

### 5.4 Stdlib And Builtins

Update stdlib APIs:

```text
functions with no meaningful result -> ()
lookup/search/split APIs -> Option<T>
fallible APIs -> Result<T, E>
callbacks with effect-only behavior -> () return
raw JSON/serde null -> future explicit external value if needed
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

Host conversion rules:

```text
Rust ()                <-> Vela ()
Rust Option<T>::None   <-> Vela Option::None
Rust Option<T>::Some   <-> Vela Option::Some(value)
Rust Result<T, E>      <-> Vela Result<T, E> where registered
Rust (A, B)..(A, B, C, D) <-> Vela tuples
JSON/serde null        <-> explicit external data wrapper or typed Option::None
```

No host adapter should rely on `null` as a catch-all missing value. Untyped
host nullable data must choose between typed `Option<T>` and an explicit raw
data wrapper.

### 5.6 Reflection, Metadata, And Hot Reload

Reflection metadata should model optional fields with `Option`, omitted fields,
or structured absence flags. It should not encode missing metadata as ordinary
script `null`.

Reflection and schema contracts must expose:

```text
TypeDesc::Unit
TypeDesc::Tuple { elements: Vec<TypeDesc> }
```

Hot reload and schema compatibility must treat these as incompatible:

```text
null no-result -> () when visible in exported/native signatures
T -> (T, U)
tuple arity changes
tuple element contract changes
Option<T> payload tuple element changes
Result<T, E> payload tuple element changes
external raw null policy changes in host schema
```

## 6. Implementation Phases

Tracking rules:

```text
[ ] task not started
[~] task in progress or partially implemented
[x] task implemented and covered by named tests/validation
```

Do not mark a task `[x]` only because code compiles. A task is complete when
implementation, focused tests, docs/diagnostic impact, and validation notes are
updated. If a task is intentionally deferred, leave it unchecked and name the
follow-up.

### Phase 0: Lock Hard-Switch Baseline

- [x] Update `docs/decisions.md` with the selected strict source-null removal
  policy and first-slice tuple limits.
- [x] Add baseline tests or audit notes that name current `null` behavior to be
  deleted.
- [x] Confirm protocol JSON nulls are out of language-value scope and will be
  audited separately.

Exit criteria:

- [x] No open design decision blocks implementation.
- [x] Goal-mode can start deleting without asking whether `null` survives.

### Phase 1: Syntax And Grammar Hard Switch

- [x] Add `()` expression/type syntax.
- [x] Add tuple expression/type/destructuring syntax for arities 2+.
- [x] Reject `(x,)` and `(T,)` with source-spanned diagnostics.
- [x] Remove global `null` literal and type-hint syntax.
- [x] Remove `NullKw`, `Literal::Null`, null literal patterns, and null syntax
  extraction.
- [x] Update `docs/grammar.ebnf`, tree-sitter grammar, syntax tests,
  formatter lexical handling, semantic-token lexical categories, snippets, and
  code actions that insert `null`.

Focused validation:

- [x] `cargo test -p vela_syntax`
- [x] `cargo test -p vela_language_service semantic_tokens code_action formatting`
- [x] editor grammar validation for checked-in `.vela` fixtures

### Phase 2: Core Value Model Hard Switch

- [~] Add unit and tuple value representations across runtime, owned, host,
  bytecode constants, type facts, guards, equality, display, verifier, and
  heap conversion.
- [x] Delete `Value::Null`, `OwnedValue::Null`, `HostValue::Null`,
  `Constant::Null`, `PrimitiveTag::Null`, `TypeKind::Null`, `TypeHint::null`,
  null keying, null truthiness, null equality, null register defaults, and null
  type guards.
- [x] Replace default registers/no-result constants with unit.
- [x] Ensure Missing remains an internal sentinel only if still required; it
  must not become public null by another name.

Focused validation:

- [x] `cargo test -p vela_bytecode --no-fail-fast`
- [x] `cargo test -p vela_vm`
- [x] `cargo test -p vela_host`
- [x] `cargo test -p vela_reflect`

### Phase 3: Compiler, Control Flow, And Runtime Semantics

- [x] Make empty blocks, statement-only blocks, expression branches with no
  meaningful value, loop bodies, no-return functions, and `return;` produce
  unit.
- [~] Lower tuple construction and destructuring directly. Ordinary tuple
  expressions now lower through a first-class `MakeTuple` bytecode instruction,
  and tuple destructuring now lowers for `let`, `match`, and `for` patterns.
- [~] Add tuple arity/type mismatch diagnostics for destructuring and dynamic
  boundary guards. Runtime arity guards now exist for tuple destructuring;
  typed dynamic-boundary tuple guards now cover Option and Result tuple
  payloads. Broader diagnostic polish remains open.
- [x] Keep `?` Rust-aligned and reject cross-family `Option`/`Result`
  propagation without explicit helpers. `TryPropagate` bytecode now carries the
  enclosing typed return family when known, so both continue and short-circuit
  paths reject `Option`/`Result` family mismatches.
- [x] Remove tests that assert null as void, null equality, null control-flow
  defaults, or null literal matching; replace with behavior tests for unit,
  Option, Result, and tuple payloads. Active null test coverage now only
  proves source `null` rejection, absent `null` completions, and `"null"` not
  mapping to a primitive tag.

Focused validation:

- [x] `cargo test -p vela_bytecode`
- [x] `cargo test -p vela_vm try_propagation --no-fail-fast`
- [x] `cargo test -p vela_engine`

### Phase 4: Stdlib, Host, Reflection, And Embedding

- [x] Change stdlib no-result signatures from `"null"` to `"()"`.
- [x] Change mutation helpers and no-result native calls to return unit.
- [~] Change lookup/search/split APIs to `Option<T>` and tuple payloads where
  useful. `String.split_once` now returns `Option<(String, String)>` through
  analysis facts, stdlib metadata, VM execution, cached materialization,
  reflection metadata, examples, and docs. Statically known scalar/string
  lookup and parse returns such as `String.find -> Option<i64>`,
  `String.strip_prefix`/`strip_suffix -> Option<String>`, string primitive
  parsers, and `Array.index_of -> Option<i64>` now expose precise stdlib
  manifest and reflection metadata. Receiver-dependent collection payloads
  such as `Array.pop`, `Map.get`, and `Iterator.next` remain erased in the
  descriptor surface until descriptor metadata can carry receiver type facts.
- [x] Change Rust `Option<T>` conversion to script `Option<T>`. Embedding and
  serde owned/runtime conversions now use `Option::Some` and `Option::None`
  enum values; `()` and raw payload values are rejected for Rust
  `Option<T>`.
- [x] Add Rust `()` conversion and Rust tuple arity 2..=4 conversion.
- [~] Remove reflection missing-metadata nulls in favor of `Option`, omitted
  fields, or explicit structured absence. Field, parameter, and return hints
  now expose optional copied `ReflectTypeHint` descriptors alongside their raw
  strings; invalid or missing descriptors are `Option::None`, not unit.
  `reflect::type_of` now returns `Option<ReflectType>`, and analysis facts for
  optional reflection metadata use `Option<T>` instead of unit placeholders.
- [x] Remove engine/native/context schema descriptors that advertise `"null"`.

Focused validation:

- [x] `cargo test -p vela_stdlib`
- [x] `cargo test -p vela_engine`
- [x] `cargo test -p vela_reflect`
- [x] `cargo test --manifest-path examples/Cargo.toml`

### Phase 5: Tuple Payloads, ABI, And Contracts

- [~] Make `Option<(A, B)>` and `Result<(A, B), E>` precise in type facts,
  guard plans, reflection descriptors, schema artifacts, and hot-reload ABI.
  Tuple TypeFacts, compiler runtime facts, value shapes, runtime guard plans,
  descriptor validation, and `split_once` reflection metadata are implemented;
  Result tuple payload fixtures now cover VM `?`/destructuring propagation and
  linked parameter guards. Reflected descriptor type-hint strings now parse into
  structured tuple facts for analysis and schema artifact export, and reflected
  metadata records expose nested `ReflectTypeHint` descriptors for unit, tuple,
  Option, and Result hint strings.
- [x] Add tuple `OwnedValue` conversion and tuple serde behavior that does not
  use raw null.
- [x] Reject tuple Map/Set keys in the first slice with precise diagnostics.
- [~] Add hot-reload rejection for exported unit/tuple signature changes,
  tuple arity changes, and tuple element contract changes. Descriptor and
  schema ABI comparisons now parse unit and tuple type hints structurally, and
  source-reload tuple signature fixtures cover equivalent formatting plus
  tuple arity rejection. Typed dynamic-boundary tuple guard plans remain open.
- [x] Add focused benchmark rows for common tuple-return stdlib paths if they
  are introduced. The `string_splitting` baseline workload exercises
  `String.split_once -> Option<(String, String)>` across interpreter,
  profile-only, and cache-enabled rows.

Focused validation:

- [x] `cargo test -p vela_hot_reload`
- [x] `cargo test -p vela_bytecode type_contract`
- [x] `cargo test -p vela_analysis`
- [x] `cargo test -p vela_language_service schema`
- [x] `cargo test -p vela_vm option_result`
- [x] `cargo test -p vela_vm type_guards`
- [x] `cargo bench -p vela_vm --bench baseline -- --quick string_splitting`

### Phase 6: Tooling, Docs, Examples, And Website

- [~] Update architecture docs, grammar docs, examples, conformance fixtures,
  playground/site examples, and user-facing diagnostics. The split_once tuple
  payload is documented in architecture and website stdlib docs, and the
  gameplay helper example uses tuple destructuring.
- [~] Update LSP hover, completion, signature help, semantic tokens, rename,
  references, code actions, formatting, inlay hints, and diagnostics for unit
  and tuples. Unit type-hint completion and tuple/unit hover and signature
  display now preserve structural type facts through the language-service
  query layer and LSP-focused validation. LSP hover and signature-help tests
  now also cover precise stdlib `Option<T>` returns for string parse/split and
  array lookup methods.
- [x] Replace user-facing null placeholders with `()`, `Option::None`, or
  typed fixits. Active docs, examples, editor grammar, and website audits no
  longer show Vela-language null placeholders. The surviving active
  user-facing source strings are the intentional removed-`null` diagnostic and
  no-completion assertions.
- [x] Classify surviving JSON nulls as protocol/serde fixture data rather than
  Vela language values. LSP `processId`, `params`, unsupported-result, and
  no-result `JsonValue::Null` cases remain external JSON-RPC protocol data.

Focused validation:

- [x] `cargo test -p vela_language_service`
- [x] `cargo test -p vela_lsp_server`
- [x] `npm --prefix site run build`

### Phase 7: Final Audit And Cleanup

- [x] Remove obsolete null compatibility helpers, tests, docs, diagnostics, and
  migration-only names. Active survivors are rejection/no-completion coverage,
  protocol/data nulls, external C ABI terminology, and historical/planning
  text rather than compatibility behavior.
- [x] Confirm no temporary external-null wrapper was added only to keep old
  behavior alive. Audits for legacy or compatibility null wrapper names have no
  active code hits.
- [x] Confirm touched active source/test files stay under the ordinary
  1200-line guideline or have a documented exception. The edited source
  grammar is under the guideline; generated tree-sitter artifacts are exempt.
- [x] Run zero-result language-null audits and classify protocol/data nulls.
  Symbolic Vela-language null forms are gone. Intentional source-string
  survivors remain for the removed-`null` diagnostic/test and no-completion
  tests; protocol/data survivors are classified below and were rechecked in the
  final acceptance audit.
- [x] Run full workspace validation.

Final validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path examples/Cargo.toml
npm --prefix site run build
```

All listed commands passed in the final acceptance checkpoint. The runnable
example suite also passed with `cargo test --manifest-path examples/Cargo.toml`.
Editor grammar fixture validation passed with `tree-sitter parse` over the
checked-in site, LSP highlighting, conformance, and example `.vela` fixtures.
The `string_splitting` benchmark was run after the tuple-return stdlib update
and recorded cache, uncached, and profile-only rows.

## Current Surviving Null Classification

As of the 2026-07-09 audit checkpoint, active Vela-language null symbols are
absent for `NullKw`, `Literal::Null`, `Value::Null`, `OwnedValue::Null`,
`HostValue::Null`, `Constant::Null`, `PrimitiveTag::Null`, `TypeKind::Null`,
`TypeHint::null`, and `StdTypeSpec::primitive("Null")` or
`StdTypeSpec::primitive("null")`.

Reviewed survivors are classified as follows:

- Intentional source rejection coverage: the syntax diagnostic that reports
  removed `null` source and parser tests proving `return null;` is rejected.
- Intentional completion coverage: language-service tests asserting `null` is
  not suggested as a type hint.
- External JSON protocol data: LSP JSON-RPC `processId: null`,
  request `params: null`, unsupported or empty response results, and
  `serde_json::Value::Null` or `JsonValue::Null` transport/test fixtures.
- External C ABI terminology: null pointer and null-terminated string
  validation in `vela_c_api` docs and website reference pages.
- Historical or planning text: `docs/archive`, this plan, progress notes, and
  decision records describing the completed removal.

## 7. Audit Commands

Primary Vela-language null audit. At close-out, symbolic language null forms
must produce zero hits; source-string survivors must be limited to intentional
removed-source diagnostics, no-completion assertions, or documented
historical/protocol/external-data text:

```bash
rg -n "\bNullKw\b|Literal::Null|Value::Null|OwnedValue::Null|HostValue::Null|Constant::Null|PrimitiveTag::Null|TypeKind::Null|TypeHint::null|StdTypeSpec::primitive\(\"Null\"|\"null\"|return null|=> null" crates/vela_syntax crates/vela_hir crates/vela_analysis crates/vela_bytecode crates/vela_vm crates/vela_engine crates/vela_host crates/vela_reflect crates/vela_stdlib crates/vela_hot_reload crates/vela_language_service examples docs/architecture docs/grammar.ebnf editors/tree-sitter-vela
```

Protocol/data null classification audit, expected reviewed survivors only:

```bash
rg -n "\bnull\b|Value::Null|JsonValue::Null|serde_json::Value::Null" crates/vela_lsp_server crates/vela_language_service crates/vela_vm/src/serde.rs crates/vela_vm/src/serde docs examples
```

Tuple/unit implementation audit:

```bash
rg -n "Tuple|Unit|\(\)" crates/vela_syntax crates/vela_bytecode crates/vela_vm crates/vela_engine crates/vela_host crates/vela_reflect crates/vela_stdlib crates/vela_hot_reload crates/vela_language_service docs/architecture docs/grammar.ebnf
```

Architecture hygiene audit:

```bash
rg -n "legacy_null|null_compat|null_or_unit|null_to_unit|unit_or_null|temporary null|TODO.*null|FIXME.*null" crates examples docs
```

## 8. Test Plan

Parser and grammar:

```text
unit literal/type
tuple expression/type/pattern
parentheses vs rejected one-element tuple diagnostics
source-level null rejection
```

HIR and analysis:

```text
unit and tuple TypeFacts
tuple destructuring bindings
tuple arity mismatch diagnostics
Option/Result tuple payload propagation
Rust-aligned ? diagnostics for Option/Result return-kind mismatch
```

Compiler and VM:

```text
unit-returning blocks/functions/native calls
tuple construction/destructuring
tuple guards at typed dynamic boundaries
? propagation with tuple payloads
no implicit Option-to-Result or Result-to-Option ? conversion
old null no-result paths removed
```

Host, reflection, and serde:

```text
Rust () conversion
Rust Option<T> conversion
Rust tuple arities 2..=4 conversion
TypeDesc::Unit and TypeDesc::Tuple
OwnedValue unit/tuple conversion
JSON/serde null kept outside ordinary Vela values
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

## 9. Design Rules For Implementation

- Keep unit, tuple, Option, Result, internal Missing sentinels, and
  external-data null as separate concepts.
- Do not implement `null` aliases for `()`, `Option::None`, or `Result::Err`.
- Keep `?` propagation Rust-aligned. Cross-family `Option`/`Result`
  propagation must use explicit conversion helpers.
- Do not trust tuple type hints until values are proven by construction,
  verified contracts, or runtime guards.
- Keep tuple syntax structural; do not add a public `Tuple<T, U>` type.
- Keep tuples fixed-size and ordered; use records for named fields.
- Keep tuple arity limits explicit in host conversions and ABI metadata.
- Keep `Option<T>` and `Result<T, E>` as restricted builtin type-hint
  parameterization, not general script-language generics.
- Keep raw external null out of ordinary script control flow and VM values.
- Prefer source-spanned breaking diagnostics over compatibility coercions.
- Delete temporary helpers and migration names before final acceptance.
