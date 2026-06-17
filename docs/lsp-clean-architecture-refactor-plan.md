# Clean LSP Architecture Refactor Plan

> **Track:** native LSP architecture cleanup, before MVP editor tooling
> hardening
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release language-service and LSP
> internal APIs are allowed. Do not preserve the current coarse completion
> model, thin completion item shape, protocol projection shape, or feature
> specific cursor scanners as compatibility shims. Preserve product contracts:
> analysis-only LSP behavior, no runtime execution, no live host-state reads,
> no `TypeRegistry` mutation, no Rust `&mut` exposure, HostAccess safety,
> reflection permissioning, source-spanned diagnostics, and no editor feature
> that changes language or runtime semantics.

---

## 0. Codex Goal

```text
/goal Rebuild Vela's native LSP architecture around a clean, editor-neutral
query model inspired by rust-analyzer's separation of cursor context,
feature-specific producers, rich editor-neutral result items, and LSP protocol
projection. Treat completion as the first visible failure to fix, but design
the refactor for the full LSP surface: diagnostics, completion, signature
help, hover, definition, symbols, semantic tokens, references, rename, code
actions, formatting, inlay hints, workspace snapshots, cancellation, and
configuration. Replace obsolete language-service and LSP-server internals
instead of carrying compatibility shims. Keep the architecture analysis-only:
do not execute scripts, inspect live host state, mutate TypeRegistry, add new
language semantics, or build a custom IDE. Validate each checkpoint with
focused language-service model tests, LSP JSON-RPC or conversion fixtures,
existing feature tests, docs, and relevant workspace checks. Commit small
Conventional Commit checkpoints.
```

---

## 1. Purpose

The current native LSP track has enough feature coverage to be useful, but the
internal model is still too coarse. The most obvious symptom is completion:
typing `f` at an item boundary can rank stdlib function facts above the `fn`
keyword, and typing `st` does not reliably surface `struct` as the primary
choice. That is not only a sorting bug. It shows that the service does not yet
have a strong cursor-context model, typed completion producers, rich completion
items, or protocol projection boundaries.

This refactor should solve completion first because it is the most visible
editor failure, then carry the same architecture through the rest of the LSP.
The target is a language-service model that can answer each editor request
from shared source, syntax, HIR, analysis, schema, and workspace facts, while
keeping LSP transport and protocol details isolated in `vela_lsp_server`.

This is a cleanup track, not a compatibility track. Existing internal APIs can
be deleted when their replacements are verified.

---

## 2. Current Problems

- [ ] Completion uses broad contexts such as global, member, type hint, and
  module path instead of item, statement, expression, pattern, type, use,
  record, call, and keyword-sensitive contexts.
- [ ] Top-level and statement-position completion mixes declarations,
  modules, stdlib functions, schema facts, and keywords without a relevance
  model that understands what can syntactically appear there.
- [ ] Completion items are too thin for high-quality editors: source
  replacement ranges, lookup/filter text, label details, snippet shape,
  relevance, preselect, deprecation, documentation, and resolve payloads are
  not first-class service concepts.
- [ ] LSP conversion is forced to infer protocol behavior from a lossy service
  item instead of projecting a rich editor-neutral item into LSP types.
- [ ] Feature handlers still grow their own cursor and display decisions.
  Completion, hover, signature help, definition, semantic tokens, references,
  rename, and code actions need shared source identity, symbol identity, range,
  display, and edit-plan concepts.
- [ ] Tests prove individual happy paths but do not pin the model boundaries:
  cursor-context classification, item-vs-expression completion, replacement
  ranges, relevance ordering, LSP projection, and stale-result behavior.

---

## 3. rust-analyzer Ideas To Borrow

Borrow the architecture ideas, not Rust-specific implementation complexity.
Use this local checkout as the source reference root:

```text
~/CLionProjects/rust-analyzer
```

Useful reference areas in rust-analyzer, with paths relative to that root:

- `crates/ide-completion/src/lib.rs`: completion is split into context
  construction and completion production. The engine produces candidates and
  metadata; filtering by the already typed substring is left to the editor or
  projection layer.
- `crates/ide-completion/src/completions.rs`: completion dispatch is driven
  by syntax-aware name-reference and path contexts such as expression, type,
  item, pattern, visibility, use, dot access, and record fields.
- `crates/ide-completion/src/completions/item_list.rs`: item-list keyword
  completions are context-specific producers. `fn`, `struct`, and related
  snippets are not generic globals.
- `crates/ide-completion/src/item.rs`: completion items carry source ranges,
  text edits, lookup/filter identity, details, documentation, deprecation,
  relevance, imports, and resolve-time payload.
- `crates/rust-analyzer/src/lsp/to_proto.rs`: LSP conversion is a projection
  layer that maps editor-neutral item metadata into `CompletionItem`,
  `TextEdit`, `filterText`, `labelDetails`, `sortText`, `preselect`, docs,
  tags, and snippet formats.

Do not borrow rust-analyzer's macro expansion model, full Salsa setup,
Rust-specific type inference machinery, import insertion semantics, or trait
resolution complexity unless a later Vela-specific problem justifies it.

---

## 4. Target Architecture

```text
LSP request
  -> vela_lsp_server protocol handler
  -> WorkspaceSnapshot query
  -> QueryContext construction
  -> feature classifier
  -> feature producers
  -> editor-neutral feature result
  -> LSP projection
  -> JSON-RPC response
```

Shared service concepts:

- `QueryContext`: stable request inputs, document identity, snapshot
  generation, cursor offset/range, source text, syntax root, module facts,
  HIR/analysis facts, optional schema facts, cancellation generation, and
  client capability hints that do not leak LSP protocol types.
- `CursorContext`: syntax-aware cursor classification shared by completion,
  hover, signature help, definition, references, rename, and code actions.
- `SymbolRef`: editor-stable identity for source declarations, schema-owned
  host facts, builtin facts, local bindings, fields, variants, methods, and
  modules.
- `DisplayParts`: editor-neutral display text for signatures, hover labels,
  completion detail, symbol labels, inlay labels, and diagnostics.
- `EditPlan`: source-owned edit representation with ranges, replacement text,
  snippet intent, import insertion intent, and conflict metadata.
- `Relevance`: feature-owned scoring metadata that can be projected into LSP
  `sortText`, `preselect`, and secondary grouping without hard-coding protocol
  behavior in producers.

Feature ownership:

- `vela_language_service` owns all query construction, cursor classification,
  feature production, and editor-neutral result models.
- `vela_lsp_server` owns JSON-RPC, LSP request/response structs, position and
  range conversion, client capability mapping, request cancellation, progress,
  workspace folders, file watching, and configuration transport.
- `vela_syntax`, `vela_hir`, `vela_analysis`, and `vela_reflect` remain the
  source of truth for syntax, module graph, semantic facts, and schema facts.

---

## 5. Non-Goals

- [ ] Do not execute Vela scripts to answer editor queries.
- [ ] Do not run the Rust host application to discover schema metadata.
- [ ] Do not inspect or mutate live host state.
- [ ] Do not mutate `TypeRegistry` or runtime type structure.
- [ ] Do not add script-language generics, new overload semantics, monkey
  patching, JIT behavior, async/coroutine behavior, or runtime hot-reload
  semantics as part of LSP cleanup.
- [ ] Do not build a custom full IDE product.
- [ ] Do not keep old service APIs alive only to avoid updating tests or LSP
  callers.
- [ ] Do not let code actions, rename, or formatting invent type facts or
  choose ambiguous semantic rewrites.

---

## Phase Status

Use this checklist as the durable execution tracker. Mark a task only after
its focused tests and validation command pass.

```text
[ ] not started
[~] in progress
[x] complete
```

---

## 6. Phase 1: Pin The Broken Model With Tests

Purpose: write the failing tests before replacing the model.

- [ ] Add language-service completion fixtures for item-boundary keyword
  completion:
  - typing `f` at a top-level item boundary ranks `fn` before stdlib symbols.
  - typing `st` at a top-level item boundary ranks `struct` before unrelated
    globals.
  - item-boundary completion does not offer expression-only stdlib functions.
- [ ] Add statement, expression, type-hint, member, record-field, module-path,
  call-argument, and lambda-parameter cursor-context classification fixtures.
- [ ] Add LSP completion conversion fixtures for replacement ranges,
  `filterText`, `sortText`, snippets, label details, and preselect behavior.
- [ ] Add regression fixtures showing that existing member, type, module,
  record, and schema completions still work in their proper contexts.
- [ ] Add at least one stale-generation or cancellation fixture if existing
  request plumbing can expose it cheaply.

Tests:

```bash
cargo test -p vela_language_service completion
cargo test -p vela_lsp_server completion
```

---

## 7. Phase 2: Build Shared Query And Cursor Context

Purpose: classify the cursor once and reuse it across features.

- [ ] Add a focused query-context module in `vela_language_service` that
  builds request-local facts from a `WorkspaceSnapshot`.
- [ ] Add `CursorContextKind` variants for item, statement, expression, type,
  pattern, use/import, module path, member access, record expression field,
  record type field, call argument, lambda parameter, map key, rename target,
  and unknown/error recovery.
- [ ] Make cursor classification syntax-owned. Prefer parser/token structure
  and source spans over ad hoc substring checks.
- [ ] Include expected syntactic role, local scope, module scope, receiver
  expression facts, path qualifier facts, callable facts, and replacement
  range where available.
- [ ] Keep classification tolerant of incomplete source and parser recovery.
- [ ] Route completion, hover, signature help, definition, and rename prepare
  through the shared context where practical.

Tests:

```bash
cargo test -p vela_language_service cursor_context
cargo test -p vela_language_service completion
```

---

## 8. Phase 3: Rewrite Completion Around Producers

Purpose: replace coarse global completion with context-specific producers.

- [ ] Split completion into focused modules such as context, item, relevance,
  producers, render, and tests when file size or responsibility requires it.
- [ ] Replace `CompletionContextKind::Global` style dispatch with producers
  selected by `CursorContextKind`.
- [ ] Add keyword and snippet completions as first-class completion kinds.
- [ ] Add item-boundary producers for `fn`, `struct`, `enum`, `trait`, `impl`,
  `let`, `const`, imports, modules, and source declarations that are legal in
  that context.
- [ ] Add expression producers for locals, parameters, functions, methods,
  variants, builtin values, stdlib functions, and schema facts that are legal
  in expression position.
- [ ] Add type producers for builtin types, script types, schema types,
  modules, traits, and type aliases when those exist.
- [ ] Keep member and record producers receiver-aware. Do not leak unrelated
  globals into member contexts.
- [ ] Add a `CompletionAccumulator` that accepts candidates, de-duplicates by
  lookup identity and edit range, and applies relevance metadata without
  client-specific fuzzy filtering.
- [ ] Add a rich editor-neutral `CompletionItem` model with label, lookup,
  detail, documentation, kind, source range, text edit, snippet intent,
  filter text, label details, relevance, deprecation, symbol identity, and
  optional resolve payload.

Tests:

```bash
cargo test -p vela_language_service completion
```

---

## 9. Phase 4: Rewrite LSP Projection

Purpose: keep LSP protocol behavior out of producers while giving editors
high-quality metadata.

- [ ] Convert service completion items to LSP completion items using source
  ranges and text edits supplied by the service item.
- [ ] Set `filterText` from lookup identity when label text and inserted text
  differ.
- [ ] Set `labelDetails` for signatures, modules, receiver types, and return
  types where the client supports it.
- [ ] Set `insertTextFormat` from snippet intent rather than hard-coded
  function heuristics.
- [ ] Project relevance into stable `sortText` and `preselect`.
- [ ] Project deprecation into LSP tags.
- [ ] Add or preserve lazy resolve support for docs and expensive detail when
  it becomes useful.
- [ ] Add protocol fixtures for common editor clients, including Zed's
  completion behavior.

Tests:

```bash
cargo test -p vela_lsp_server completion
```

---

## 10. Phase 5: Unify Symbol And Display Models

Purpose: make hover, signature help, definition, symbols, references, rename,
and semantic tokens use the same identities and display primitives.

- [ ] Introduce `SymbolRef` or equivalent identity for local bindings, source
  declarations, modules, fields, methods, variants, builtin facts, stdlib
  facts, and schema-owned facts.
- [ ] Route hover labels, completion details, signature labels, symbol labels,
  inlay labels, and diagnostics through `DisplayParts` or equivalent
  structured display helpers.
- [ ] Make go-to-definition and hover resolve symbols through the shared
  cursor context before falling back to feature-local logic.
- [ ] Ensure schema-owned symbols can be displayed and completed without
  pretending they have source definitions.
- [ ] Preserve source-span accuracy for script-owned symbols.

Tests:

```bash
cargo test -p vela_language_service hover
cargo test -p vela_language_service definition
cargo test -p vela_language_service signature
cargo test -p vela_lsp_server hover
cargo test -p vela_lsp_server definition
```

---

## 11. Phase 6: References, Rename, And Semantic Tokens

Purpose: make cross-reference features use explicit symbol identity and
source-owned edit plans.

- [ ] Build references from symbol identity plus syntax/HIR binding facts, not
  plain text matching.
- [ ] Distinguish source-owned references, schema-owned facts, builtin facts,
  dynamic `Any` facts, and unresolved names.
- [ ] Make prepare-rename reject schema-owned, builtin, dynamic, unresolved,
  and ambiguous targets.
- [ ] Make rename produce an `EditPlan` with conflict checks and source-owned
  ranges only.
- [ ] Ensure semantic tokens are generated from syntax/HIR classification and
  stay stable under parser recovery.
- [ ] Add fixtures for shadowing, modules, methods, fields, and failed rename
  targets.

Tests:

```bash
cargo test -p vela_language_service references
cargo test -p vela_language_service rename
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server references
cargo test -p vela_lsp_server rename
```

---

## 12. Phase 7: Code Actions, Formatting, And Inlay Hints

Purpose: keep editor mutations structured, local, and source-owned.

- [ ] Route quick fixes through structured diagnostics and `EditPlan`.
- [ ] Reject ambiguous imports, dynamic receiver typo fixes, and semantic
  rewrites without a proven local pattern.
- [ ] Keep formatting syntax-owned and trivia-preserving. Do not rely on
  successful HIR or analysis.
- [ ] Add AST-aware range and on-type formatting only after token/trivia rules
  are stable.
- [ ] Generate inlay hints from stable type and signature facts, not ad hoc
  string parsing.

Tests:

```bash
cargo test -p vela_language_service code_action
cargo test -p vela_language_service formatting
cargo test -p vela_language_service inlay
cargo test -p vela_lsp_server code_action
cargo test -p vela_lsp_server formatting
```

---

## 13. Phase 8: Scale, Cancellation, And Configuration

Purpose: make the new model viable for large workspaces and real editors.

- [ ] Audit request paths for avoidable per-keystroke full-workspace rebuilds.
- [ ] Keep open-document queries prioritized over disk-only modules.
- [ ] Use workspace generation IDs to discard stale results.
- [ ] Keep cancellation checks at query construction and expensive producer
  boundaries.
- [ ] Ensure configuration is loaded through `vela.toml`, launch flags, and
  LSP settings without putting protocol types in the service.
- [ ] Add stress fixtures or benchmarks for many files when a cheap
  representative test can be maintained.

Tests:

```bash
cargo test -p vela_language_service workspace
cargo test -p vela_lsp_server workspace
```

---

## 14. Phase 9: Delete Old Paths And Update Docs

Purpose: complete the breaking cleanup.

- [ ] Delete obsolete completion context variants, compatibility conversion
  helpers, stale tests, and dead feature-local cursor scanners.
- [ ] Update `docs/architecture/lsp.md` with the final query/result/projection
  boundary.
- [ ] Update `docs/lsp-implementation-plan.md` if phase status or capability
  ownership changes.
- [ ] Update `docs/progress.md` only when milestone status changes.
- [ ] Update `docs/decisions.md` for any new durable architecture decision.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 15. Acceptance Criteria

- [ ] At an item boundary, typing `f` ranks the `fn` keyword/snippet before
  stdlib functions, and typing `st` ranks `struct` before unrelated globals.
- [ ] Expression-position completion still offers legal stdlib functions,
  locals, declarations, modules, builtins, and schema facts with sensible
  relevance.
- [ ] Type-position completion does not offer expression-only functions.
- [ ] Member completion is receiver-aware and does not leak top-level globals.
- [ ] LSP completion uses correct replacement ranges, filter text, snippets,
  label details, sorting, preselect, docs, and deprecation tags where
  supported.
- [ ] Hover, signature, definition, symbols, semantic tokens, references,
  rename, code actions, formatting, and inlay hints route through shared
  query, symbol, display, and edit models where relevant.
- [ ] Obsolete APIs are deleted rather than kept as compatibility shims.
- [ ] The architecture remains analysis-only and does not change runtime
  semantics.

---

## 16. First Execution Tasks

Use the repository task template when starting implementation.

```text
Task: Add completion context regression tests for item-boundary keywords.
Context: This belongs to the clean LSP architecture refactor. The visible bug
is that typing `f` at top level can rank stdlib facts above `fn`, and typing
`st` does not reliably surface `struct`.
Expected behavior:
  - top-level `f` completion includes and ranks `fn` first.
  - top-level `st` completion includes and ranks `struct` first.
  - item-boundary completion does not include expression-only stdlib functions.
Tests:
  - cargo test -p vela_language_service completion
Do not change:
  - Do not add compatibility shims for the old global completion model.
  - Do not change language syntax or runtime semantics.
Validation:
  cargo test -p vela_language_service completion
```

```text
Task: Introduce shared CursorContext classification for LSP queries.
Context: Completion, hover, signature help, definition, and rename currently
use feature-local cursor logic. The refactor needs one syntax-aware
classification boundary.
Expected behavior:
  - item, statement, expression, type, member, record, call, module, and
    lambda contexts are classified from parser/token spans.
  - incomplete source and parser recovery still produce useful contexts.
Tests:
  - cargo test -p vela_language_service cursor_context
  - cargo test -p vela_language_service completion
Do not change:
  - Do not expose LSP protocol types from vela_language_service.
  - Do not read or mutate live host state.
Validation:
  cargo test -p vela_language_service cursor_context completion
```

```text
Task: Replace completion items with a rich editor-neutral item model.
Context: LSP projection needs service-owned source ranges, text edits,
lookup/filter text, snippet intent, detail, docs, relevance, symbol identity,
and resolve payloads.
Expected behavior:
  - producers return editor-neutral items with replacement ranges and
    relevance metadata.
  - vela_lsp_server projects those items into LSP CompletionItem fields.
Tests:
  - cargo test -p vela_language_service completion
  - cargo test -p vela_lsp_server completion
Do not change:
  - Do not hard-code editor-specific ranking inside producers.
  - Do not preserve the old thin CompletionItem shape for compatibility.
Validation:
  cargo test -p vela_language_service completion
  cargo test -p vela_lsp_server completion
```
