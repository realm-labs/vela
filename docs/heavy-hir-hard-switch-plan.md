# Heavy HIR Hard-Switch Plan

> **Track:** semantic architecture, HIR ownership, compiler/LSP fact cleanup
> before MIR and JIT foundation work
> **Document status:** Codex goal-mode execution plan
> **Execution status:** D1 and D2 complete; D3 size close-out open
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
the full final validation passes. Start from the reopened D2 checkpoint in
Section 4: make production query context select the narrowest HIR body that
contains the cursor and prove nested-body record completion does not silently
fall back to syntax. Then complete D3 with an all-files size audit. Do not redo
the completed D1 identity work or the already deleted syntax payload compiler,
and do not introduce a replacement compatibility path.
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

Final acceptance remains open after a second review. D1 and D2 are complete:

- bytecode path, host-path, and record-shape helpers receive `HirExprId`;
- editor local features share HIR-backed `HirLocalId` source projection;
- production editor queries select nested lambda and parameter-default bodies;

D3 still has a concrete gap:

- the fixed-path size audit is green only for the four files named by the first
  close-out review. A directory-wide scan still finds over-1200-line active
  files in analysis semantic facts, HIR syntax binding, and HIR module graph,
  with no documented exceptions. Completion record tests
  moved to a focused module while closing D2.

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

Goal mode must use the following close-out status:

```text
[x] D1. Stable identity closure: pass HirExprId/HirLocalId through bytecode and
        editor semantic APIs; delete span-to-ID and feature-local local scans.
[x] D2. Completion boundary closure: select the active nested HIR body in the
        production query path, then use syntax only for proven incomplete-edit
        recovery that did not lower a usable HIR record.
[~] D3. Architecture and acceptance: run an all-files size audit over the
        affected crates, split every over-threshold active file or document a
        concrete exception, run final validation, update status docs, and only
        then unblock MIR and complete the goal.
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
- [x] Delete downstream local-by-name/local-by-span scans such as
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
  source-origin lookup as the default editor-neutral input. The active body is
  the narrowest cursor-containing body; enclosing call facts remain HIR-based.
- [x] Move completion, signature help, hover, definition, references, rename,
  code actions, semantic tokens, and inlay hints away from body-level syntax
  semantic inference. Stable local identity and nested-body record-field
  completion are complete.
- [x] Keep formatting on the canonical lossless syntax formatter; formatting
  must not build a second semantic tree or depend on feature-local semantic
  reconstruction.
- [x] Delete feature-local helpers that take a `SyntaxExpression` only to find
  `HirExprId` by span, or that find a syntax expression from a HIR ID before
  performing semantic work. Start semantic queries from HIR IDs and project
  results back through HIR source origins. Apply the same rule to duplicated
  local-binding lookup by name or source range.
- [x] Keep syntax/CST access only for lexical recovery under incomplete edits,
  lossless formatting, folding/selection structure, token trivia, and final
  source-range projection. Record-constructor identity must come from HIR when
  a recovered HIR record expression exists; syntax recovery must be isolated
  and must not own resolved semantic facts.
- [x] Make production `QueryContext` expose the narrowest `HirBody` whose
  source origin contains the cursor, including lambda and parameter-default
  bodies, instead of always exposing the root binding-map body.
- [x] Add public completion-path tests for root-body, nested-lambda, and
  parameter-default record constructors plus a malformed record that genuinely
  requires syntax recovery. Do not satisfy this checkpoint only with an
  internal helper test that manually selects a body.
- [x] Keep LSP protocol projection unchanged.
- [x] Preserve stale-generation, overlay, cancellation, and analysis-only LSP
  behavior.

Validation:

```bash
cargo test -p vela_language_service
cargo test -p vela_language_service record_field_completion
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
- [x] Move statement, expression, pattern, call, assignment, host path, index,
  operator, container, lambda, default-parameter, and control-flow lowering to
  HIR body IDs. Path, host-path, and record-shape helpers must receive IDs
  directly instead of reconstructing them from expression spans.
- [x] Derive runtime type contracts, guards, call targets, and frame/debug
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
- [x] Ensure downstream body-level semantic decisions do not read syntax or
  recover HIR identity directly from source spans. Source-origin/span lookup
  may project an existing HIR result back to source, but must not reconstruct
  semantic identity or operands.
- [~] Update docs/progress.md and docs/decisions.md only when implementation
  status changes.
- [~] Keep MIR unimplemented until the reopened Heavy HIR acceptance passes.
- [~] Split all active files in the affected HIR/analysis/bytecode/language-
  service trees that exceed 1200 lines unless a concrete exception is
  documented. The original four files are split; the directory-wide audit
  still reports `semantic_facts.rs`, `syntax_binding.rs`, and
  `module_graph.rs`.
- [x] Replace stale AST/migration descriptions and misleading test names,
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

Directory-wide file-size audit for D3:

```powershell
$roots = @(
  "crates/vela_hir/src",
  "crates/vela_analysis/src",
  "crates/vela_bytecode/src/compiler",
  "crates/vela_language_service/src"
)
$files = Get-ChildItem $roots -Recurse -Filter *.rs
$files | ForEach-Object {
  [pscustomobject]@{ Lines = (Get-Content $_.FullName).Count; File = $_.FullName }
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
boundaries from Phase 5. The directory-wide file-size audit must produce no
output unless every remaining row has a concrete exception and architectural
justification recorded in this document. Checking only a fixed historical file
list is not sufficient.

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
removed, D1 through D3 are complete, nested-body production-query tests prove
the HIR-first completion boundary, the all-files size audit is clean or every
exception is documented, and full validation passes. A green intermediate
tree, completion of one subsystem, or progress recorded in `docs/progress.md`
is not sufficient to mark the goal complete.
