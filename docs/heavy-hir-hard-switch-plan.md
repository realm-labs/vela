# Heavy HIR Hard-Switch Plan

> **Track:** semantic architecture, HIR ownership, compiler/LSP fact cleanup
> before MIR and JIT foundation work
> **Document status:** Codex goal-mode execution plan
> **Execution status:** primary hard switch implemented; D1-D3 close-out reopened
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

For the remaining work, a "checkpoint" means one of the reopened close-out
checkpoints defined in Section 4. It does not mean one expression shape, one
operand edge, one fallback, one helper, or one compiler error. Do not make a
sequence of commits such as "read field receiver from HIR", "read index operand
from HIR", or "gate call lowering on HIR" while leaving the syntax-driven
dispatcher intact. Work through the temporary red tree and commit the complete
checkpoint after its old path has been deleted and its focused validation is
green.

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
reload, and analysis-only LSP behavior. Do not mark the goal complete after an
intermediate checkpoint or because focused tests are green. The goal is
complete only after Phase 7 audit searches satisfy their expected results and
the full final validation passes. Start from the reopened D1 checkpoint in
Section 4: remove bytecode span-to-HIR identity reconstruction and duplicated
language-service local-by-span lookup, then complete D2 record-completion
HIR/recovery separation and D3 architecture hygiene. Do not redo the already
deleted syntax payload compiler or introduce a replacement compatibility path.
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
- Payload-bearing HIR statement, expression, pattern, argument, arm, field, and
  container records. Operators, literals, operands, assignment targets,
  branches, guards, block roots, call arguments, and nested child relationships
  must be represented with HIR-owned values and IDs rather than kind-only tags
  that require callers to walk syntax again.
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

### Current Execution State

Checkpoint A is complete. Checkpoints B and C completed their primary hard
switches: Body HIR owns executable relationships, analysis owns HIR-keyed
semantic facts, the large language-service syntax semantic walkers are gone,
and bytecode's syntax payload/dispatcher path is deleted. Focused, workspace,
clippy, and runnable-example validation is green.

Final acceptance is reopened because review found identity and architecture
gaps that the earlier audits did not cover:

- bytecode path, host-path, and record-shape helpers still accept `Span` and
  call `Compiler::expression_at_span` to reconstruct `HirExprId`;
- definition, references, rename, hover, semantic tokens, and related editor
  paths still duplicate local-by-name/local-by-span lookup instead of consuming
  one shared HIR-backed symbol identity;
- record-field completion ignores its `HirBody`/`SourceId` inputs and uses a
  syntax-only recursive walk to discover constructor identity, while a test
  named as HIR coverage does not exercise HIR;
- one active analysis implementation file and three active test files exceed
  the ordinary 1200-line guideline, and the bytecode compiler module comment
  still describes the removed AST compiler.

Passing focused tests proves behavior preservation, not completion. The
remaining execution unit is no longer an individual call/field/index fact.
The original checkpoint history remains:

```text
A. Executable HIR closure: finish Phase 3 and Phase 4 together so a body can be
   understood semantically without walking body syntax.
B. Language-service hard switch: migrate every semantic feature and delete
   feature-local syntax-to-HIR span pairing.
C. Bytecode hard switch: switch the compiler entrypoint and all body lowering,
   then delete syntax payload/dispatcher scaffolding in the same checkpoint.
D. Cleanup and acceptance: reopened by the final architecture review below.
```

Goal mode must now complete these close-out checkpoints in order:

```text
D1. Stable identity closure: pass HirExprId/HirLocalId through bytecode and
    editor semantic APIs; delete span-to-ID and feature-local local-span scans.
D2. Completion boundary closure: make record-field completion HIR-first and
    isolate syntax traversal to explicitly named incomplete-edit recovery.
D3. Architecture and acceptance: split over-threshold files, remove stale
    AST/migration wording, run all zero-hit audits and full validation, update
    status docs, and only then unblock MIR and complete the goal.
```

Each D checkpoint is a complete architecture slice, not one caller or helper.
The tree may be temporarily red while signatures change. Do not preserve
compatibility constructors, span fallbacks, dual dispatch, aliases, or
test-only mirrors merely to keep every intermediate edit green.

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

- [x] Replace body-local binding ownership with HIR body scopes and resolution
  tables.
- [~] Delete downstream local-by-name/local-by-span scans such as
  `hir_let_local_name_for_span`; this remaining caller migration belongs to
  Checkpoints B and C, and consumers must start from a body-owned HIR ID.
- [x] Represent pattern locals for `let`, `match`, and `for` bindings with
  token spans and binding scope spans.
- [x] Represent lambda captures and `self` bindings explicitly.
- [x] Represent imports, declaration references, shadowing, and unresolved
  references through HIR resolution records.
- [x] Preserve current diagnostics for unresolved names, duplicate bindings,
  and invalid pattern use.

Validation:

```bash
cargo test -p vela_hir bindings
cargo test -p vela_analysis
cargo test -p vela_language_service references
cargo test -p vela_language_service rename
cargo test -p vela_language_service definition
```

---

## 8. Phase 4: Analysis Facts On HIR IDs

Purpose: close the executable HIR model, then make analysis facts stable and
reusable by LSP, bytecode, and MIR.

- [x] Replace kind-only expression/statement/pattern records with HIR-owned
  payload and operand relationships for literals, unary/binary operations,
  assignment targets, calls and arguments, fields, indexes, arrays, maps,
  records, tuples, lambdas, blocks, `if`, `match`, loops, `try`, returns, and
  control-flow statements.
- [x] Key `TypeFact`, callable facts, member facts, effect facts, and
  control-flow facts by Heavy HIR IDs.
- [x] Represent call targets, method targets, field/member targets, variant
  targets, operator targets, stdlib/native targets, host-path targets, and
  dynamic-boundary fallback facts.
- [x] Move fact formatting and display through shared analysis/HIR helpers.
- [x] Keep analysis degradation explicit for unknown/dynamic/failed schema
  cases instead of rebuilding facts from syntax.
- [x] Prove that analysis can evaluate a complete body from `HirBody` and
  source-independent registry inputs without receiving body-level syntax
  wrappers or mapping syntax nodes back to HIR by span.

Validation:

```bash
cargo test -p vela_analysis
cargo test -p vela_language_service expression_facts
cargo test -p vela_language_service signature
cargo test -p vela_language_service hover
cargo test -p vela_language_service completion
rg -n "SyntaxExpression|SyntaxStatement|expression_at_span|pattern_at_span" crates/vela_analysis/src
```

The Phase 4 search must have no body-semantic syntax or span-pairing hits.
Syntax is allowed only inside the `vela_hir` lowering boundary that constructs
HIR and source origins.

---

## 9. Phase 5: Language-Service Hard Switch

Purpose: make editor queries consume Heavy HIR facts instead of feature-local
semantic reconstruction.

- [x] Update query context to expose HIR body, HIR IDs, analysis facts, and
  source-origin lookup as the default editor-neutral input.
- [~] Move completion, signature help, hover, definition, references, rename,
  code actions, semantic tokens, and inlay hints away from body-level syntax
  semantic inference. Record-field completion and local identity lookup remain
  in the reopened close-out.
- [x] Keep formatting on the canonical lossless syntax formatter; formatting
  must not build a second semantic tree or depend on feature-local semantic
  reconstruction.
- [~] Delete feature-local helpers that take a `SyntaxExpression` only to find
  `HirExprId` by span, or that find a syntax expression from a HIR ID before
  performing semantic work. Start semantic queries from HIR IDs and project
  results back through HIR source origins. Apply the same rule to duplicated
  local-binding lookup by name or source range.
- [~] Keep syntax/CST access only for lexical recovery under incomplete edits,
  lossless formatting, folding/selection structure, token trivia, and final
  source-range projection. Record-constructor identity must come from HIR when
  a recovered HIR record expression exists; syntax recovery must be isolated
  and must not own resolved semantic facts.
- [x] Keep LSP protocol projection unchanged.
- [x] Preserve stale-generation, overlay, cancellation, and analysis-only LSP
  behavior.

Validation:

```bash
cargo test -p vela_language_service
cargo test -p vela_lsp_server
rg -n "hir_.*SyntaxExpression|syntax_expr_for_hir_expression|expression_at_span|pattern_at_span" crates/vela_language_service/src
rg -U -n "bindings\s*\.locals\(\)\s*\.(find|find_map)" crates/vela_language_service/src
```

Every Phase 5 search hit must either be deleted or documented at the lexical,
formatting, recovery, or source-projection boundary. A feature-local semantic
span join is not an accepted exception.

---

## 10. Phase 6: Bytecode Compiler Hard Switch

Purpose: make bytecode lowering consume Heavy HIR and analysis facts.

- [x] Introduce compiler entrypoints that lower from `HirBody` plus analysis
  facts, with source origins used only for diagnostics and debug metadata.
- [~] Move statement, expression, pattern, call, assignment, host path, index,
  operator, container, lambda, default-parameter, and control-flow lowering to
  HIR body IDs. Path, host-path, and record-shape helpers must receive IDs
  directly instead of reconstructing them from expression spans.
- [~] Derive runtime type contracts, guards, call targets, and frame/debug
  metadata from Heavy HIR facts without `expression_at_span` identity joins.
- [x] Replace the primary `compile_syntax_expression`/syntax-kind dispatcher
  with HIR expression/statement/pattern dispatch. Do this for the complete body
  compiler rather than adding more HIR gates around syntax-shaped helpers.
- [x] Delete `SyntaxBodyPayload`, `CompilerBodyPayload`,
  `CompilerStatementPayload`, body-level `function_body_payload` pairing,
  `hir_block_body_payload`, `expression_syntax_*` helpers, and migration-only
  `syntax_*` lowering module/function names in the same checkpoint that makes
  HIR lowering green.
- [x] Preserve bytecode output semantics, VM behavior, and diagnostics.

Validation:

```bash
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
rg -n "CompilerBodyPayload|SyntaxBodyPayload|CompilerStatementPayload|compile_syntax_expression|hir_block_body_payload|expression_syntax_|body_payload|syntax_payload" crates/vela_bytecode/src/compiler
rg -n "expression_at_span\(" crates/vela_bytecode/src/compiler
```

Both Phase 6 searches must have zero hits. Do not keep a test-only syntax compiler,
comparison backend, compatibility facade, alias, or dual-path equivalence test
after the HIR compiler is green. Preserve behavior with source-to-runtime tests
that use the production HIR path.

---

## 11. Phase 7: Cleanup And Acceptance

Purpose: remove transition names and prove Heavy HIR is the semantic source.

- [x] Delete migration-only payload/fact/helper names.
- [~] Ensure downstream body-level semantic decisions do not read syntax or
  recover HIR identity directly from source spans. Source-origin/span lookup
  may project an existing HIR result back
  to source, but must not reconstruct semantic identity or operands.
- [~] Update docs/progress.md and docs/decisions.md only when implementation
  status changes.
- [~] Keep MIR unimplemented until the reopened Heavy HIR acceptance passes.
- [ ] Split active files that exceed 1200 lines unless a concrete exception is
  documented: `vela_analysis/src/registry.rs`, HIR binding tests, and bytecode
  literal/call and expression tests are the current close-out set.
- [ ] Replace stale AST/migration descriptions and misleading test names,
  including the bytecode compiler module description and record completion's
  claimed HIR-operand test.

Audit searches:

```bash
rg -n "CompilerBodyPayload|SyntaxBodyPayload|CompilerStatementPayload|compile_syntax_expression|hir_block_body_payload|expression_syntax_|body_payload|syntax_payload" crates/vela_bytecode/src/compiler
rg -n "hir_let_local_name_for_span|hir_.*SyntaxExpression|syntax_expr_for_hir_expression" crates/vela_bytecode/src/compiler crates/vela_language_service/src crates/vela_analysis/src
rg -n "expression_at_span\(" crates/vela_bytecode/src/compiler
rg -U -n "bindings\s*\.locals\(\)\s*\.(find|find_map)" crates/vela_language_service/src
rg -n "parse_source_with_id\\(|syntax_parse\\(" crates/vela_bytecode/src/compiler crates/vela_analysis/src
rg -n "TODO.*HIR|temporary.*HIR|compat.*HIR|fallback.*HIR|Temporary 1200-line exception" crates examples editors
rg -n "SyntaxExpression|SyntaxStatement" crates/vela_bytecode/src/compiler crates/vela_language_service/src crates/vela_analysis/src
rg -n "Minimal AST-to-bytecode|uses_hir_index_operands" crates
```

File-size audit for the current close-out set:

```powershell
$files = @(
  "crates/vela_analysis/src/registry.rs",
  "crates/vela_hir/src/module_graph/tests/bindings.rs",
  "crates/vela_bytecode/src/compiler/tests/literals_and_calls.rs",
  "crates/vela_bytecode/src/compiler/tests/expressions.rs"
)
$files | ForEach-Object {
  [pscustomobject]@{ Lines = (Get-Content $_).Count; File = $_ }
} | Where-Object Lines -gt 1200
```

The first four and final stale-name searches must have zero hits. Review the
source-parsing search: parsing is allowed only at module/source ingestion and
in source-based behavior tests; analysis and body lowering must not reparse
source to recover semantic facts. Review every hit from the final syntax-type
search:
`vela_bytecode` and `vela_analysis` must have no body-level syntax dependency;
`vela_language_service` may retain syntax only for the explicit lexical,
formatting, recovery, folding/selection, trivia, and source projection
boundaries from Phase 5. The file-size audit must produce no output unless this
document records a concrete exception and its architectural justification.

Final validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
```

The plan is complete only when Heavy HIR owns body-level semantic facts,
language-service and bytecode compiler consume those facts, old body-level
syntax semantic reconstruction and span-based identity reconstruction are
removed, D1 through D3 are complete, over-threshold files are split or have a
documented exception, and full validation passes. A green intermediate tree,
completion of one subsystem, or progress recorded in `docs/progress.md` is not
sufficient to mark the goal complete.
