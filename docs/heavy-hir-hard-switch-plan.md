# Heavy HIR Hard-Switch Plan

> **Track:** semantic architecture, HIR ownership, compiler/LSP fact cleanup
> before MIR and JIT foundation work
> **Document status:** Codex goal-mode execution plan
> **Compatibility policy:** breaking pre-release HIR, analysis,
> language-service, bytecode-compiler, and test APIs are allowed. Preserve
> product contracts: no script-language generics, no Rust `&mut` exposure,
> HostAccess safety, source-spanned diagnostics, execution budgets, GC roots,
> reflection permissioning, hot-reload ABI/schema checks, and analysis-only LSP.

Hard-switch policy: this plan is intended to be run by goal-mode loops using
large subsystem slices. At the start of each slice, move semantic ownership into
`vela_hir` first, then use compiler errors, service test failures, and audit
searches as the migration queue. It is acceptable for the working tree to be
temporarily uncompilable while a slice is in progress, but every committed
checkpoint must compile and pass the focused validation for the touched
subsystem.

Default commit granularity is a complete subsystem slice, not one fallback,
helper, or checklist item at a time. A completed checkpoint must not keep both
the old syntax-semantic path and the new HIR-semantic path alive as parallel
implementations; delete or rewrite the old path in the same checkpoint that
makes the replacement green.

Do not add compatibility HIR mirrors, syntax-to-HIR fallback adapters, duplicate
fact stores, temporary payload names, or helper APIs that exist only to keep the
old body-level syntax pipeline alive. The final state must make Heavy HIR the
semantic truth for body/expression/pattern facts consumed by analysis,
language-service, bytecode, and later MIR lowering.

---

## 0. Codex Goal

Use this prompt to execute the full refactor:

```text
/goal Execute the Heavy HIR hard switch from
docs/heavy-hir-hard-switch-plan.md. Treat docs/goal.md as the product roadmap,
docs/architecture.md and docs/architecture/*.md as the architecture contract,
and docs/progress.md as the current milestone state. Upgrade vela_hir in place
so body, expression, pattern, scope, binding, capture, call/member target, and
control-flow semantics are represented by stable HIR IDs and facts. Move
analysis, language-service, and bytecode compiler callers away from body-level
syntax reconstruction. Prefer deletion-first subsystem slices over compatibility
layers. Every checkpoint must pass the focused tests named in this document and
must leave no parallel old/new semantic implementation behind for the migrated
subsystem. Preserve VM behavior, diagnostics quality, HostAccess safety, hot
reload, and analysis-only LSP behavior.
```

---

## 1. Purpose

The current pipeline has useful HIR declaration metadata, binding maps,
`HirExprId`/`HirLocalId`, and analysis `TypeFact` maps, but body-level semantic
ownership remains spread across:

- `vela_hir` binding and module summaries,
- `vela_analysis` fact inference,
- `vela_language_service` query/context helpers,
- `vela_bytecode` compiler-owned syntax payload and runtime type facts.

That shape worked while the language surface was still settling, but it makes
MIR and later JIT work harder because lowering decisions still need to recover
semantic meaning from syntax wrappers and compiler-local helper trees.

Heavy HIR makes one layer responsible for semantic truth:

```text
Syntax/AST
  -> Heavy HIR: stable IDs, bodies, scopes, bindings, facts, semantic targets
  -> Analysis/Compiler/Language Service consume HIR facts
  -> MIR consumes Heavy HIR only
```

---

## 2. Boundary With MIR

Heavy HIR owns semantic meaning:

- declaration, body, statement, expression, and pattern identity;
- source origin and source-span mapping;
- scopes, locals, captures, pattern locals, and binding resolution;
- callable, member, field, variant, trait, type-hint, effect, and control-flow
  facts;
- diagnostics that depend on semantic source ownership rather than execution
  shape.

MIR owns execution shape and must not repair semantic gaps:

- basic blocks, terminators, and explicit control flow;
- places, operands, temporaries, liveness, and debug/root maps;
- typed operation selection, guards, and lowering decisions;
- bytecode backend input now and Cranelift backend input later.

If MIR lowering needs a fact that Heavy HIR does not provide, the implementer
must add that fact to Heavy HIR or analysis before continuing MIR work.

---

## 3. Target Model

Heavy HIR should introduce these concepts inside `vela_hir` rather than a new
semantic crate:

- `HirBody` for every function, method, trait default body, lambda, const
  initializer, global initializer, parameter default, and other executable body.
- Stable IDs for body-owned statements, expressions, patterns, blocks, arms,
  parameters, locals, and captures.
- Source origin records that map HIR IDs back to syntax nodes/tokens and spans
  without downstream callers holding body-level syntax wrappers.
- Scope and binding records that make lexical scope, pattern locals, lambda
  captures, `self`, imports, declarations, and shadowing explicit.
- Semantic target records for calls, methods, fields, variants, operators,
  host paths, stdlib methods, native functions, and dynamic-boundary fallbacks.
- Fact tables keyed by HIR IDs for type, effect, callable, member, pattern,
  control-flow, and diagnostic facts.

The existing `TypeFact` vocabulary may remain in `vela_analysis`, but its
anchor keys should become stable Heavy HIR IDs. Compiler-only
`RuntimeTypeFact` may remain as a bytecode/runtime contract type, but it should
be derived from Heavy HIR/analysis facts instead of being inferred from syntax
again.

---

## 4. Phase Status

Use this checklist as the durable execution tracker. Mark a task only after its
focused tests and validation command pass.

```text
[ ] not started
[~] in progress
[x] complete
```

---

## 5. Phase 1: Semantic Ownership Audit

Purpose: identify every body-level semantic decision currently made outside
Heavy HIR before deleting old caller-local logic.

- [x] Audit bytecode compiler syntax/payload/body lowering decisions.
- [x] Audit language-service query helpers that infer call/member/type facts.
- [x] Audit analysis facts keyed by unstable syntax ranges or duplicated IDs.
- [x] Audit parameter defaults, lambdas, pattern locals, host paths, calls,
  field/index access, operators, match arms, and control-flow values.
- [x] Record the migration order in this document before implementation begins.

Migration order recorded by the Phase 1 audit:

```text
1. Add `vela_hir` body ownership and stable body/statement/pattern/block/
   parameter/capture IDs while preserving existing binding and compiler
   behavior.
2. Move scope, binding, pattern-local, capture, and source-origin queries onto
   `HirBody` records before analysis or tooling callers switch.
3. Re-key analysis expression/member/call/control-flow facts to Heavy HIR IDs
   and expose shared display/format helpers.
4. Switch language-service query context and feature producers to HIR bodies
   plus analysis facts, leaving LSP projection unchanged.
5. Switch bytecode lowering from `CompilerBodyPayload` and body-level syntax
   reconstruction to HIR body IDs plus analysis/compiler facts, deleting old
   payload scaffolding by subsystem.
6. Run cleanup audits and keep MIR work blocked until the old syntax-semantic
   body path is gone for migrated subsystems.
```

Validation:

```bash
rg -n "Syntax.*Expr|Syntax.*Stmt|Compiler.*Payload|RuntimeTypeFact|TypeFact|HirExprId|HirLocalId" crates/vela_bytecode/src/compiler crates/vela_language_service/src crates/vela_analysis/src crates/vela_hir/src
cargo test -p vela_hir
cargo test -p vela_analysis
```

---

## 6. Phase 2: Body HIR Core

Purpose: add executable body ownership without changing behavior.

- [x] Add `HirBodyId`, `HirStmtId`, `HirPatternId`, and any missing body-local
  IDs required beside existing `HirExprId` and `HirLocalId`.
- [x] Add `HirBody` with owner metadata, source origin, statements,
  expressions, patterns, blocks, parameters, locals, and captures.
- [x] Lower function, method, trait default, lambda, const/global initializer,
  and parameter-default bodies into `HirBody`.
- [x] Preserve source spans and syntax origins for diagnostics, navigation, and
  bytecode frame metadata.
- [x] Keep existing public behavior and existing compiler entrypoints working
  until downstream consumers switch.

Validation:

```bash
cargo test -p vela_hir body
cargo test -p vela_hir module_graph
```

---

## 7. Phase 3: Scopes, Bindings, Patterns, And Captures

Purpose: make all lexical and binding facts body-HIR-owned.

- [~] Replace body-local binding scans with HIR body scopes and resolution
  tables.
- [x] Represent pattern locals for `let`, `match`, and `for` bindings with
  token spans and binding scope spans.
- [x] Represent lambda captures and `self` bindings explicitly.
- [ ] Represent imports, declaration references, shadowing, and unresolved
  references through HIR resolution records.
- [x] Preserve current diagnostics for unresolved names, duplicate bindings,
  and invalid pattern use.

Validation:

```bash
cargo test -p vela_hir bindings
cargo test -p vela_analysis
cargo test -p vela_language_service references rename definition
```

---

## 8. Phase 4: Analysis Facts On HIR IDs

Purpose: make analysis facts stable and reusable by LSP, bytecode, and MIR.

- [ ] Key `TypeFact`, callable facts, member facts, effect facts, and
  control-flow facts by Heavy HIR IDs.
- [ ] Represent call targets, method targets, field/member targets, variant
  targets, operator targets, stdlib/native targets, host-path targets, and
  dynamic-boundary fallback facts.
- [ ] Move fact formatting and display through shared analysis/HIR helpers.
- [ ] Keep analysis degradation explicit for unknown/dynamic/failed schema
  cases instead of rebuilding facts from syntax.

Validation:

```bash
cargo test -p vela_analysis
cargo test -p vela_language_service expression_facts signature hover completion
```

---

## 9. Phase 5: Language-Service Hard Switch

Purpose: make editor queries consume Heavy HIR facts instead of feature-local
semantic reconstruction.

- [ ] Update query context to expose HIR body, HIR IDs, analysis facts, and
  source-origin lookup as the default editor-neutral input.
- [ ] Move completion, signature help, hover, definition, references, rename,
  code actions, semantic tokens, inlay hints, and formatting inputs away from
  body-level syntax semantic inference.
- [ ] Keep LSP protocol projection unchanged.
- [ ] Preserve stale-generation, overlay, cancellation, and analysis-only LSP
  behavior.

Validation:

```bash
cargo test -p vela_language_service
cargo test -p vela_lsp_server
```

---

## 10. Phase 6: Bytecode Compiler Hard Switch

Purpose: make bytecode lowering consume Heavy HIR and analysis facts.

- [ ] Introduce compiler entrypoints that lower from `HirBody` plus analysis
  facts.
- [ ] Move statement, expression, pattern, call, assignment, host path, index,
  operator, container, lambda, default-parameter, and control-flow lowering to
  HIR body IDs.
- [ ] Derive runtime type contracts, guards, call targets, and frame/debug
  metadata from Heavy HIR facts.
- [ ] Delete body-level syntax payload scaffolding once each subsystem switches.
- [ ] Preserve bytecode output semantics, VM behavior, and diagnostics.

Validation:

```bash
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
```

---

## 11. Phase 7: Cleanup And Acceptance

Purpose: remove transition names and prove Heavy HIR is the semantic source.

- [ ] Delete migration-only payload/fact/helper names.
- [ ] Ensure downstream body-level semantic decisions do not read syntax
  directly except through source-origin/span lookup.
- [ ] Update docs/progress.md and docs/decisions.md only when implementation
  status changes.
- [ ] Keep MIR unimplemented until Heavy HIR acceptance passes.

Audit searches:

```bash
rg -n "Compiler.*Payload|body_payload|syntax_payload|Syntax.*Expr|Syntax.*Stmt" crates/vela_bytecode/src/compiler crates/vela_language_service/src crates/vela_analysis/src
rg -n "parse_source_with_id\\(|syntax_parse\\(" crates/vela_bytecode/src/compiler crates/vela_analysis/src
rg -n "TODO.*HIR|temporary.*HIR|compat.*HIR|fallback.*HIR" crates docs
```

Final validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
```

The plan is complete only when Heavy HIR owns body-level semantic facts,
language-service and bytecode compiler consume those facts, old body-level
syntax semantic reconstruction is removed, and full validation passes.
