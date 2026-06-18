# Native LSP Full Capability Implementation Plan

> **Track:** pre-MVP native LSP capability track, parallel with M19/M20
> optimization work
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release tooling APIs are allowed. Do
> not preserve temporary single-file, WASM-first, or protocol-coupled internal
> shapes for compatibility. Preserve product contracts: no Rust `&mut`
> exposure, no runtime TypeRegistry mutation, no monkey patching, HostAccess
> safety, reflection permissioning, source-spanned diagnostics, hot-reload
> ABI/schema checks, and no editor feature that changes language or runtime
> semantics.

---

## 0. Codex Goal

```text
/goal Implement Vela's full native LSP capability track from
docs/lsp-implementation-plan.md and docs/architecture/lsp.md. Treat
docs/goal.md as the product roadmap, docs/architecture.md and
docs/architecture/*.md as the architecture contract, and docs/progress.md as
the current milestone state. Build a cleanly layered editor tooling system:
native `vela_lsp_server` owns LSP transport, file watching, cancellation,
progress, configuration, and editor lifecycle; reusable
`vela_language_service` owns virtual workspace state, source overlays, module
graph snapshots, diagnostics, completion, hover, definitions, symbols,
references, rename planning, code actions, semantic tokens, formatting inputs,
and incremental invalidation; existing `vela_syntax`, `vela_hir`,
`vela_analysis`, and `vela_reflect` remain the semantic source of truth.
Prefer `compile_dir` module-graph semantics with open-document overlays and a
host schema artifact loaded from exported TypeRegistry/RegistryFacts metadata.
For authoring UX work, especially Phase 19, follow a rust-analyzer-style
model where Vela syntax overlaps: build structured completion analysis from
syntax recovery and semantic facts, run feature producers over explicit
path/type/dot/declaration/call/pattern/statement contexts, keep member facts
unified across source/schema/stdlib/builtin surfaces, keep completion display
separate from insertion/projection fields, and keep formatting in a
syntax-owned CST/AST layout boundary. When a local rust-analyzer checkout is
available, including `~/CLionProjects/rust-analyzer` in this development
setup, inspect it before changing LSP authoring behavior; the most relevant
references are `crates/ide-completion/src/lib.rs`,
`crates/ide-completion/src/context.rs`,
`crates/ide-completion/src/context/analysis.rs`,
`crates/ide-completion/src/completions/dot.rs`, and
`crates/rust-analyzer/src/handlers/request.rs`. Borrow the editor model, not
Rust-only semantics: no macros, borrow checking, Rust trait solving, or
script-language generics.
Scale toward Vela workspaces whose total source size approaches one million
lines across many files and modules by avoiding per-keystroke full project
rebuilds, prioritizing open-file queries, using generation-based cancellation,
and adding explicit source/parse/HIR/analysis indexes. The LSP track may
progress in parallel with M19/M20 optimization work because it is analysis-only
and must not change VM semantics. WASM is optional for browser tooling and must
not constrain the native server architecture. Validate each checkpoint with
focused language-service tests, LSP JSON-RPC fixtures, scale-oriented tests,
docs, and relevant workspace checks. Commit small Conventional Commit
checkpoints.
```

---

## 1. Purpose

Vela already has the parser, HIR module graph, analysis facts, diagnostics,
completion facts, hover facts, source spans, and host schema metadata needed
for editor tooling. The missing layer is a clean language-service architecture
that can serve native LSP clients without coupling editor protocols to the
compiler pipeline.

The target design is native-first and scale-aware. VS Code, Zed, JetBrains,
and CLI tooling can use platform-specific binaries. Browser tooling may reuse
the language-service core through WASM later, but it should not shape the
primary LSP implementation.

This plan intentionally covers the full practical LSP feature set. Individual
features still land in small verified tasks, and each feature may be disabled
or unadvertised until its service-level behavior is correct.

---

## 2. Goals

- [ ] Add `vela_language_service` as a reusable, editor-neutral analysis
  service.
- [ ] Add `vela_lsp_server` as the native LSP protocol and platform boundary.
- [ ] Use `compile_dir`-style multi-file module graph semantics by default.
- [ ] Treat open editor buffers as source overlays that override disk
  snapshots.
- [ ] Load host type and reflection metadata from a static schema artifact.
- [ ] Keep absent or stale host schema as a degradable tooling condition.
- [ ] Support diagnostics, completion, signature help, hover, and go to
  definition.
- [ ] Support document symbols, workspace symbols, semantic tokens, and
  folding/selection ranges.
- [ ] Support find references and prepare a reference index suitable for
  rename.
- [ ] Support prepare rename and rename with conflict checks.
- [ ] Support code actions driven by structured diagnostics and safe import or
  typo repairs.
- [ ] Support formatting once syntax trivia/lossless token policy is stable.
- [ ] Support inlay hints from stable TypeFacts.
- [ ] Align user-facing authoring behavior with rust-analyzer where Vela
  syntax overlaps by adopting a structured authoring core:
  `CompletionAnalysis`, explicit dot/path/type/declaration contexts, unified
  member facts, readable completion item rendering, compact type formatting,
  and statement snippets.
- [ ] Build toward many-file workspaces whose total source size approaches one
  million lines with explicit source, parse, HIR, and analysis databases.
- [ ] Use generation IDs and cancellation so stale request results are
  discarded.
- [ ] Keep LSP, editor, filesystem, and watcher types out of
  `vela_language_service` public APIs.
- [ ] Keep runtime execution, DAP, and live host state separate from editor
  analysis.

---

## 3. Non-Goals

This plan must not:

- [ ] Build a custom full IDE product instead of a native LSP server plus thin
  editor integrations.
- [ ] Run Vela programs to answer editor requests.
- [ ] Run the Rust host application to discover schema metadata.
- [ ] Read or mutate live host state for hovers, completions, diagnostics,
  code actions, rename, or formatting.
- [ ] Mutate `TypeRegistry` or runtime type structure.
- [ ] Add script-language generics or new language semantics for the LSP.
- [ ] Make the LSP depend on WASM as its primary release format.
- [ ] Preserve old single-file-only analysis paths as compatibility shims.
- [ ] Let rename or code actions bypass hot-reload ABI/schema compatibility
  reporting.

---

## 4. Architecture Summary

The long-term contract lives in
[architecture/lsp.md](architecture/lsp.md). The implementation should follow
this ownership split:

```text
editor plugin
  starts the native server and passes editor configuration

vela_lsp_server
  owns LSP JSON-RPC, document sync, file watching, cancellation, progress

vela_language_service
  owns workspace state, source overlays, project model, analysis queries

vela_syntax / vela_hir / vela_analysis / vela_reflect
  own parsing, module graph, semantic facts, host schema facts
```

Protocol structs from `lsp-types` or any LSP server library must not appear in
`vela_language_service` public APIs. Filesystem paths and URLs should be
normalized into service-owned document IDs before analysis.

The completed cleanup model routes feature requests through shared
`QueryContext` and syntax-owned `CursorContext` construction before
feature-specific producers run. Service results carry editor-neutral
`SymbolRef`, `DisplayParts`, `EditPlan`, relevance metadata, and completion
resolve payloads; `vela_lsp_server` remains the only layer that projects those
models into LSP JSON-RPC shapes.

Protocol-level test planning lives in
[lsp-protocol-test-matrix.md](lsp-protocol-test-matrix.md). Future LSP coverage
work should use that document as the protocol-first checklist for pairing each
advertised method with service tests, JSON-RPC fixtures, syntax coverage, and
negative/degraded behavior.

Detailed test requirements belong in the matrix document so this execution
plan stays readable. Progress for implementing and auditing those tests is
tracked below. Existing LSP tests are useful inputs, but a tracker item must
remain unchecked until the relevant matrix row is explicitly audited or filled
with focused service tests and JSON-RPC fixtures.

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

## 4.1 Protocol Test Matrix Coverage Tracker

Purpose: turn the protocol matrix into executable test work without marking
coverage complete based only on the older capability phases.

- [x] Add a protocol-first LSP test matrix document and link it from this
  implementation plan.
- [x] Audit lifecycle capabilities against handlers and matrix rows.
  - Include `initialize`, `initialized`, `shutdown`, `exit`,
    `$/cancelRequest`, advertised provider options, unsupported provider
    behavior, and the `textDocument/didClose` versus `openClose` contract.
- [x] Audit rust-analyzer-aligned authoring behavior before treating the LSP
  as user-facing complete.
  - Cover the authoring-core model itself: structured completion analysis
    for path, type, dot-access, call-argument, declaration-body, pattern, and
    statement contexts before rendered completion items are asserted.
  - Cover a unified member completion index built from source fields,
    source impl methods, source trait methods, schema members, and
    stdlib/builtin value/container members.
  - Cover compact type argument formatting for `Array<i64>`,
    `Set<String>`, `Map<String, i64>`, and
    `Result<Map<String, i64>, String>`.
  - Cover empty-prefix `.` completion on typed source structs,
    source impl/trait methods, schema host receivers, and builtin
    `Array`/`Map`/`Set`/`Iterator`/`Option`/`Result`/`String`/`Bytes`
    methods.
  - Cover struct-field declaration contexts after `struct Player {` and
    type-hint contexts after a field `:`.
  - Cover readable completion label/detail separation so `Player` inserts as
    `Player` and owner paths are projected only as detail/labelDetails/docs.
  - Cover statement snippets for `for in` and `match` through service tests
    and LSP JSON-RPC fixtures.
- [x] Add or audit the canonical cross-file workspace fixture family.
  - Cover imported functions, const/global symbols, source types, enum
    variants, fields, methods, open overlays in defining and importing files,
    and file delete/rename invalidation.
  - Current fixture family includes service and LSP cross-file coverage for
    imported function/defaulted-parameter facts, const/global hover and
    references, imported source type navigation, source field and method
    references, enum variant and record-variant field references, call
    hierarchy across source and trait methods, open overlays in defining and
    importing files, and watched-file delete/rename invalidation.
- [x] Audit document sync, diagnostics, progress, and cancellation fixtures.
  - Cover `didOpen`, `didChange`, `didClose` or capability correction,
    publish diagnostics, stale generations, parser recovery, missing imports,
    config diagnostics, and schema degradation.
  - `textDocument/didClose` now has protocol coverage for scratch diagnostic
    clearing and disk snapshot restoration after closing an open overlay.
  - Disk snapshot restoration after `textDocument/didClose` now has
    cross-feature definition-query coverage in addition to diagnostics.
  - Disk snapshot restoration after `textDocument/didClose` now has
    cross-feature completion-query coverage in addition to diagnostics and
    definition queries.
  - Current fixtures cover full and incremental `didChange`, `didOpen`,
    `didClose`, structured publish diagnostics, syntax/HIR/analysis/schema
    diagnostics, parser-recovery isolation, missing import diagnostics,
    config diagnostics, schema missing/invalid/degraded states, open-file
    priority, stale and partial generations, workspace progress wrapping, and
    stale queued request cancellation.
- [x] Audit completion, completion resolve, signature help, and hover.
  - Cover all matrix syntax dimensions that apply, including cross-file
    imports, globals, functions, methods, type hints, defaulted parameters,
    schema facts, stdlib facts, dynamic `Any`, and malformed cursor contexts.
  - Current fixtures cover service and native LSP completion for open-overlay
    declarations, expression/item/statement/module/type/member/record/map-key/
    pattern/named-argument/lambda-parameter contexts, source and schema
    facts, source/schema function-return member receivers, stdlib and builtin
    facts, defaulted parameters, short labels with separate projection fields,
    repeated-query cache reuse, stale/cancelled completion rejection, and
    malformed/incomplete contexts without global fallback.
  - Current fixtures cover `completionItem/resolve` payload projection,
    lazy schema docs for schema types/functions/fields/methods/variants, item
    pass-through without payloads, and invalid payload rejection.
  - Current signature-help fixtures cover script, imported defaulted
    functions, script/source trait/schema/schema-trait/stdlib calls, callback
    methods, source/schema methods on function-return receivers, enum variant
    calls, active parameters, incomplete calls, and dynamic or unresolved null
    results.
  - Current hover fixtures cover source, cross-file, schema, stdlib, trait,
    method, field, enum variant, global, parameter, imported module path,
    docs/effects/permissions, parser-recovery, missing-schema `Any`
    degradation, and dynamic or unresolved null results.
- [x] Audit navigation protocols.
  - Cover `definition`, `declaration`, `typeDefinition`, and negative
    `implementation` behavior for locals, globals, imported functions, source
    types, fields, methods, enum variants, schema spans, builtin types,
    dynamic facts, and unresolved names.
  - W1 `typeDefinition` now has service and protocol coverage for imported
    local, parameter, trait, struct-field, enum-field, and return type hints
    plus const/global declaration type hints and local and parameter source
    type aliases in addition to existing imported field, function return,
    member, method return, struct constructor, enum variant, const, and global
    source type paths.
  - Current fixtures cover service and native LSP `definition`,
    `declaration`, and `typeDefinition` for local bindings, globals,
    imported functions, imported const/global declarations, source fields,
    source methods through return type queries, source enum variants,
    schema type/member/variant source spans, stdlib-call adjacency, builtin
    primitive and container types returning null, dynamic facts returning
    null, unresolved bare names returning null, and unsupported
    `textDocument/implementation` not being advertised or served.
- [x] Audit references, document highlights, and call hierarchy.
  - Cover same-document and cross-file references for functions,
    const/global symbols, fields, methods, variants, imports, schema-backed
    source spans, shadowed locals, and dynamic or unresolved calls.
  - W3 references now have service and protocol coverage for cross-file
    imported source field reads, source method calls, and enum variant
    constructors/patterns plus enum record-variant fields in addition to
    imported functions, function aliases, const/global symbols, and module
    path segments.
  - W3 references now have service and protocol coverage for imported source
    type aliases and type-hint uses.
  - W3/W0 references now have protocol coverage proving watched-file deletion
    of an imported defining source removes stale cross-file reference targets.
  - W3/W0 references now have protocol coverage proving watched-file rename
    events plus an importing overlay update refresh cross-file reference
    targets to the new module path.
  - W3/W0 references now have protocol coverage proving an open overlay in
    the imported defining file wins over a stale disk snapshot.
  - W3/W0 references now have protocol coverage proving an open overlay in
    the importing file wins over a stale disk snapshot.
  - Current fixtures cover service and native LSP `textDocument/references`
    and `textDocument/documentHighlight` for locals, imports, function aliases,
    const/global symbols, source type uses, module path segments, source and
    schema fields, methods including source/schema function-return receivers,
    trait impl uses, enum variants, record-variant fields, schema-backed source
    spans, active-document-only highlights, shadowed locals, and dynamic or
    unresolved empty results.
  - Current fixtures cover service and native LSP call hierarchy for source
    functions, imported function aliases, source methods, trait impl methods,
    trait default/interface methods, schema methods including
    schema-function-return and schema-method-return receivers, schema trait
    methods, cross-file method calls, and empty prepare results for
    unresolved, dynamic, and non-callable targets.
- [x] Audit rename and code actions.
  - Cover cross-file workspace edits, source-owned edit plans, stale versions,
    public ABI risk metadata, collisions, schema-only rejection, typo fixes,
    missing imports, unused imports, match arms, and record fields.
  - Current fixtures cover service and native LSP `prepareRename` and
    `rename` for locals, private value/type/function declarations, imports
    and aliased imports, struct fields, methods, enum variants, source-backed
    schema types/functions/fields/methods/variants, versioned
    `documentChanges`, source-owned edit-plan overlap rejection,
    hot-reload/schema ABI risk change annotations, schema-only host rejection,
    keyword/literal and non-source target rejection, module/import/scope/
    trait-method/schema-member collisions, and cross-file workspace edits.
  - Current fixtures cover service and native LSP `textDocument/codeAction`
    quick fixes for unknown-field typo candidates, missing imports, unused
    imports, non-exhaustive match arms, and missing record constructor fields,
    plus ambiguous import rejection, dynamic receiver rejection, local syntax
    pattern requirements, and open-overlay range stability.
- [x] Audit symbols, folding ranges, and selection ranges.
  - Cover document and workspace symbols, module-qualified source/schema
    facts, nested type/impl/trait members, imports, blocks, match arms,
    multiline literals, parser recovery, and workspace root changes.
  - Current fixtures cover service and native LSP document/workspace symbols
    for module-qualified source and schema facts, nested type/impl/trait
    members, missing-schema degradation, deleted files, and workspace-root
    reindexing; folding ranges for import groups, declarations, blocks, match
    arms, multiline literals, and parser-recovery degradation; and
    syntax-ancestry selection ranges under valid and recovered source.
- [x] Audit semantic tokens.
  - Cover full, delta, and range tokens across lexical and resolved
    classifications, cross-file imported symbols, source/schema/builtin
    provenance, unresolved references, parser recovery, and client fallback
    projection.
  - Current fixtures cover service and native LSP full/range/delta semantic
    tokens, lexical and trivia classes, resolved local/declaration/member
    classes, cross-file imports and imported module path segments,
    source/schema/stdlib/builtin provenance modifiers, source/schema method
    calls on function-return receivers, unresolved import and general
    unresolved-reference tokens, parser-recovery degradation with retained
    HIR-backed classifications, missing-schema degradation, and client legend
    fallback projection.
- [x] Audit formatting and inlay hints.
  - Cover full/range/on-type formatting, comments, blank lines, nested member
    selections, malformed source, parameter hints, local type hints, lambda
    facts, host-path hints, tuple-variant payload hints, range filtering, and
    `Any` suppression.
  - Current fixtures cover service and native LSP document/range/on-type
    formatting, comment and blank-line preservation, compact builtin container
    type arguments, nested member and field-group selections, malformed-source
    formatting degradation, idempotence, unsupported trigger fallback, and
    syntax-only formatting under HIR errors. Inlay fixtures cover service and
    native LSP parameter-name hints, local type hints, lambda parameter facts,
    host-path type facts, source/schema tuple-variant payload names, requested
    range filtering, missing-schema degradation, unknown-call suppression, and
    `Any` suppression for source/schema call parameters and variant payloads.
- [x] Audit workspace, configuration, file watching, schema reload, and launch
  behavior.
  - Cover `workspace/didChangeWatchedFiles`,
    `workspace/didChangeConfiguration`, `workspace/didChangeWorkspaceFolders`,
    `workspace/configuration` if used, `.vela` create/change/delete/rename,
    `vela.toml`, schema artifacts, CLI flags, stdio, and editor package thin
    launcher validation.
  - Current fixtures cover native watcher registration, `.vela`
    create/delete/rename module graph updates, `vela.toml` invalid/valid/delete
    diagnostics, schema invalid/valid/delete/reload diagnostics, schema-backed
    completion updates after reload, workspace-folder reindexing,
    `didChangeConfiguration` projection into workspace roots and host schema,
    initialization-option configuration, stdio transport, native `--stdio`,
    `--root`, `--schema`, and `--version` parsing, launch-configuration
    fallback behavior, and VS Code/Zed package validators that keep editor
    packages thin launchers around `vela_lsp_server` over stdio.
- [x] Complete matrix acceptance.
  - Every advertised row in
    [lsp-protocol-test-matrix.md](lsp-protocol-test-matrix.md) has service
    proof, protocol proof, applicable syntax coverage, cross-file coverage
    where required, negative/degraded coverage, and focused validation.
  - Matrix acceptance is validated by the protocol-first audit rows above,
    `cargo test -p vela_language_service`, `cargo test -p vela_lsp_server`,
    `cargo fmt --all -- --check`,
    `cargo clippy --workspace --all-targets -- -D warnings`, and
    `cargo test --workspace`.

Validation for tracker-only documentation changes:

```bash
git diff --check
```

Validation when marking any tracker item complete:

```bash
cargo test -p vela_language_service <focused-filter>
cargo test -p vela_lsp_server <focused-filter>
```

Run the full workspace validation before marking matrix acceptance complete:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 5. Phase 1: Language-Service Workspace Core

Purpose: establish reusable editor-neutral workspace state.

- [x] Add `crates/vela_language_service`.
- [x] Define `DocumentId`, `SourceVersion`, `WorkspaceGeneration`, `LineIndex`,
  and `TextRange` service types.
- [x] Implement `open_document`, `change_document`, `close_document`, and
  `document_text`.
- [x] Store open overlays separately from disk snapshots.
- [x] Implement UTF-8 offset to line/column conversion.
- [x] Add immutable `WorkspaceSnapshot` reads for query handlers.

Tests:

- [x] `open_document_creates_overlay`
- [x] `change_document_updates_version_and_generation`
- [x] `close_document_preserves_disk_snapshot`
- [x] `line_index_maps_offsets_and_positions`
- [x] `snapshot_reads_are_generation_stable`

Validation:

```bash
cargo test -p vela_language_service workspace
```

---

## 6. Phase 2: Project Model And Source Loading

Purpose: make Vela's multi-file module model the default editor model.

- [x] Define `WorkspaceConfig`, `WorkspaceRoot`, `ProjectMode`, and
  `SchemaConfig`.
- [x] Parse `vela.toml` with workspace roots and optional host schema path.
- [x] Add single-file scratch mode as fallback, not the default project model.
- [x] Add file snapshot ingestion from the platform layer.
- [x] Map root-relative `.vela` paths to module paths.
- [x] Build `ModuleSource` collections from disk snapshots plus overlays.
- [x] Track missing files/imports as diagnostics, not panics.

Tests:

- [x] `configured_roots_build_module_paths`
- [x] `vela_toml_parses_roots_and_schema`
- [x] `scratch_file_uses_single_file_mode`
- [x] `open_overlay_wins_over_disk_source`
- [x] `missing_import_reports_diagnostic`
- [x] `multi_root_config_keeps_module_paths_stable`
- [x] `project_config_invalidation_rebuilds_module_paths`

Validation:

```bash
cargo test -p vela_language_service project
cargo test -p vela_hir module_graph
```

---

## 7. Phase 3: Snapshot, Index, And Invalidation Model

Purpose: make the service scale before adding expensive features.

- [x] Split `SourceDb`, `ProjectDb`, `ParseDb`, `HirDb`, and `AnalysisDb`.
- [x] Store file content hashes and declaration/import fingerprints.
  - Module parse summaries now expose declaration/import fingerprints, and
    the project import/dependency index is preserved across body-only edits
    unless declaration, import, file-add/delete, or module-path fingerprints
    change.
- [x] Maintain module import and reverse-dependency indexes.
- [x] Reparse changed files without reparsing unrelated files.
- [x] Invalidate HIR and analysis by changed declaration/import fingerprints.
- [x] Prioritize open-file recomputation over workspace background work.
- [x] Add cancellation and stale-generation result handling.
  - Completion queries now expose a token-guarded path that checks generation
    and cancellation before query construction, before producer dispatch, and
    before returning results.

Tests:

- [x] `function_body_edit_does_not_invalidate_unrelated_modules`
- [x] `import_edit_invalidates_reverse_dependencies`
- [x] `declaration_edit_invalidates_dependent_modules`
- [x] `module_path_change_invalidates_hir_without_text_reparse`
- [x] `stale_background_diagnostics_are_not_published`
- [x] `cancelled_background_diagnostics_are_not_published`
- [x] `open_file_recomputation_is_scheduled_before_workspace_work`
- [x] `lsp_repeated_completion_queries_do_not_reparse_or_rebuild_hir`
- [x] `scale_fixture_avoids_full_rebuild_per_edit`
- [x] `larger_synthetic_workspace_reports_indexing_metrics`
- [x] `million_line_synthetic_workspace_checkpoint_avoids_full_rebuild_per_edit`
- [x] `completion_contexts_scale_in_million_line_workspace`

Validation:

```bash
cargo test -p vela_language_service incremental
cargo test -p vela_language_service million_line_synthetic_workspace_checkpoint -- --ignored
```

Scale checkpoint:

- [x] Synthetic workspace approaches one million total lines across many files
  and modules, not one unusually large source file.
- [x] Initial indexing reports timing and source-size metrics.
- [x] Single-file edit avoids full project parse.
- [x] Single-file edit avoids full HIR rebuild.
- [x] Open-file diagnostics remain responsive under background indexing.

---

## 8. Phase 4: Diagnostics

Purpose: publish actionable parser, HIR, analysis, and schema diagnostics.

- [x] Convert parser, HIR, and analysis `vela_common::Diagnostic` values into
  editor-neutral service diagnostics.
- [x] Preserve severity, code, primary span, labels, candidates, and repair
  hints.
- [x] Query diagnostics for one file.
- [x] Query diagnostics for all open files.
- [x] Add workspace diagnostics for background indexing.
- [x] Mark diagnostics as complete, partial, or stale.
- [x] Keep syntax errors from blocking diagnostics in unaffected modules.

Tests:

- [x] `syntax_diagnostics_map_to_document_ranges`
- [x] `hir_diagnostics_survive_multi_file_workspace`
- [x] `analysis_diagnostics_map_to_document_ranges`
- [x] `syntax_errors_do_not_block_unaffected_module_diagnostics`
- [x] `workspace_diagnostics_include_background_documents`
- [x] `schema_diagnostics_degrade_to_any`
- [x] `structured_diagnostics_preserve_candidates_and_repair_hints`
- [x] `open_file_diagnostics_are_prioritized`
- [x] `partial_diagnostics_report_stale_generation`

Validation:

```bash
cargo test -p vela_language_service diagnostics
cargo test -p vela_analysis diagnostic
```

---

## 9. Phase 5: Native LSP Server Foundation

Purpose: expose the service through a native LSP binary.

- [x] Add `crates/vela_lsp_server`.
- [x] Implement `initialize`, `initialized`, `shutdown`, and `exit`.
- [x] Advertise only implemented capabilities.
- [x] Implement full document sync first.
- [x] Add incremental document sync after text edit application is tested.
- [x] Implement `didClose` for the advertised `openClose` sync contract.
- [x] Implement workspace folder/config handling.
  - [x] Capture `initialize` workspace roots for open-document diagnostics.
  - [x] Load `vela.toml` and workspace files through the platform layer.
  - [x] Handle workspace folder changes.
- [x] Wire cancellation and work-done progress.
  - [x] Handle `$/cancelRequest` for stale queued requests.
  - [x] Publish work-done progress for long-running work.
- [x] Publish diagnostics for open files.
- [x] Add JSON-RPC fixture harness.

Tests:

- [x] `lsp_initialize_reports_capabilities`
- [x] `lsp_initialized_notification_has_no_response`
- [x] `lsp_did_open_publishes_diagnostics`
- [x] `lsp_did_change_replaces_document_text`
- [x] `lsp_did_change_applies_incremental_text_edit`
- [x] `lsp_did_close_clears_scratch_diagnostics`
- [x] `lsp_did_close_restores_disk_snapshot_diagnostics`
- [x] `lsp_did_close_restores_disk_snapshot_definition_queries`
- [x] `lsp_did_close_restores_disk_snapshot_completion_queries`
- [x] `lsp_initialize_uses_workspace_root_for_document_sync`
- [x] `file_create_adds_module`
- [x] `lsp_cancellation_discards_stale_request`
- [x] `lsp_progress_wraps_workspace_diagnostics`
- [x] `lsp_shutdown_exits_without_background_tasks`

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_language_service
```

---

## 10. Phase 6: Host Schema Artifact

Purpose: provide host-aware editor tooling without running host code.

- [x] Define a schema artifact format for `TypeRegistry`/`RegistryFacts`.
- [x] Export type, field, method, variant, trait, module, function, docs,
  effect, permission, type-hint, stable-ID, and source-span metadata.
  - Schema artifacts now accept optional `sourceSpan` metadata for exported
    type, trait, member, variant, method, trait-method, and function facts.
  - Schema artifacts now round-trip optional docs metadata for type, trait,
    field, variant, method, trait-method, and function facts.
  - Schema artifacts now export module facts with optional docs and source
    spans from `RegistryFacts`.
- [x] Load schema artifacts into language-service schema facts.
- [x] Validate schema version/hash compatibility.
- [x] Report missing, stale, or invalid schema diagnostics.
- [x] Watch schema artifact changes through the LSP server.

Tests:

- [x] `registry_facts_cover_types_fields_methods_functions_and_modules`
- [x] `schema_export_round_trips_registry_facts`
- [x] `schema_hash_compatibility_accepts_matching_facts`
- [x] `schema_hash_compatibility_rejects_stale_facts`
- [x] `schema_artifact_accepts_docs_metadata`
- [x] `invalid_schema_artifact_records_schema_diagnostic`
- [x] `invalid_schema_reports_diagnostic`
- [x] `invalid_schema_metadata_reports_diagnostic`
- [x] `schema_watch_publishes_invalid_schema_diagnostic`
- [x] `schema_watch_clears_diagnostic_after_valid_reload`
- [x] `missing_schema_keeps_syntax_diagnostics_available`
- [x] `schema_reload_updates_host_member_completion`
- [x] `schema_source_spans_enable_definition`

Validation:

```bash
cargo test -p vela_language_service schema
cargo test -p vela_reflect
```

---

## 11. Phase 7: Completion And Signature Help

Purpose: make common authoring flows fast and schema-aware.

- [x] Add cursor-context extraction in `vela_language_service`.
- [x] Complete locals, parameters, captures, declarations, modules, imports,
  stdlib APIs, fields, methods, variants, traits, and type hints.
  - Type-hint completion now suggests builtin type-hint names plus
    source/schema types and traits while suppressing value/function items.
  - Source declaration, module-path, expression module, and type-hint
    completion now query HIR declaration-name, per-module declaration, virtual
    module-child, and module-label indexes for source/module candidates.
  - Schema-backed completion docs are resolved lazily through
    `completionItem/resolve`; the initial list keeps symbol identity and
    lightweight details but does not eagerly attach schema documentation.
  - `completionItem/resolve` passes through ordinary items without lazy
    payloads and rejects unknown lazy payload kinds with an explicit invalid
    request instead of guessing or panicking.
- [x] Complete named arguments and defaulted parameters.
  - Initial service and LSP completion support source-backed script function
    parameters, unused named-argument filtering, defaulted-parameter detail,
    and `insertText` snippets such as `amount: `.
- [x] Complete record fields inside known constructors.
  - Initial service and LSP completion support source-owned struct
    constructors plus schema-backed host constructors and suppresses unknown
    constructor fallback.
- [x] Complete map literal keys only when appropriate.
  - Initial service and LSP completion detect map-key cursor contexts,
    suggest unused script/schema enum variants for typed `Map<Enum, V>` keys,
    and suppress global fallback for untyped map literals.
- [x] Complete host and trait receiver members from schema facts.
- [x] Add trigger-character behavior for `.`, `::`, `{`, `(`, `,`, and `|`.
  - [x] Advertise trigger characters for the implemented LSP completion request.
  - [x] Complete type hints at `:` trigger positions for typed declarations.
  - [x] Complete stdlib callback lambda parameter names at `|` trigger
    positions for typed receivers.
- [x] Add signature help for script functions, native functions, methods, and
  callbacks.
  - Initial service and LSP signature help support script function calls,
    schema-backed host/native function calls, stdlib function calls,
    source-owned inherent method calls, schema-backed host and trait method
    calls, source and schema methods on function-return receivers, stdlib
    callback method calls, and imported source function calls with defaulted
    parameters.
- [x] Close rust-analyzer-aligned completion gaps found in editor use.
  - Empty-prefix `.` on a known receiver must not return an empty list when
    the receiver has source, schema, trait, or builtin methods.
  - Source-owned struct fields, inherent impl methods, and trait methods must
    use the same typed receiver path as schema-backed members.
  - Builtin value and container method completions must be available for
    `Array<T>`, `Map<K, V>`, `Set<T>`, `Iterator<T>`, `Option<T>`,
    `Result<T, E>`, `String`, and `Bytes`.
  - Top-level item/type completions must keep labels and insert text short
    and put owner/module paths in structured detail fields.
  - `struct Player { | }` must be classified as a field-declaration context,
    not as expression/global fallback.
  - Statement completion must expose `for in` and `match` snippets with
    snippet insert text.

Tests:

- [x] `completion_uses_open_overlay_facts`
- [x] `global_completion_uses_schema_facts`
- [x] `lsp_completion_uses_open_overlay_declarations`
- [x] `lsp_completion_uses_loaded_schema_facts`
- [x] `lsp_completion_resolve_passes_through_items_without_payload`
- [x] `lsp_completion_resolve_rejects_unknown_payload_kind`
- [x] `lambda_parameter_completion_suggests_stdlib_callback_item`
- [x] `lambda_parameter_completion_filters_prefix_and_used_names`
- [x] `lambda_parameter_completion_suggests_map_key_and_value`
- [x] `lsp_lambda_parameter_completion_uses_pipe_trigger_context`
- [x] `type_hint_completion_suggests_only_type_items`
- [x] `type_hint_completion_suggests_builtin_container_arguments`
- [x] `lsp_type_hint_completion_uses_colon_trigger_context`
- [x] `member_context_is_detected_without_global_fallback`
- [x] `member_completion_uses_host_schema_facts`
- [x] `lsp_member_completion_uses_host_schema_facts`
- [x] `member_completion_uses_schema_trait_method_facts`
- [x] `lsp_member_completion_uses_schema_trait_method_facts`
- [x] `module_completion_follows_import_context`
- [x] `record_field_completion_requires_known_type`
- [x] `record_field_completion_uses_schema_facts`
- [x] `lsp_record_field_completion_uses_known_constructor`
- [x] `named_argument_completion_suggests_unused_script_parameters`
- [x] `named_argument_completion_uses_parameter_prefix`
- [x] `lsp_named_argument_completion_suggests_unused_script_parameters`
- [x] `map_key_completion_suggests_typed_enum_variants`
- [x] `map_key_completion_suppresses_untyped_global_fallback`
- [x] `lsp_map_key_completion_suggests_schema_enum_variants`
- [x] `signature_help_tracks_active_parameter`
- [x] `lsp_signature_help_tracks_active_parameter`
- [x] `signature_help_resolves_script_method_call`
- [x] `signature_help_resolves_source_trait_default_method_on_source_function_return`
- [x] `signature_help_resolves_source_method_on_source_method_return`
- [x] `signature_help_resolves_schema_method_call`
- [x] `signature_help_resolves_schema_method_on_schema_function_return`
- [x] `signature_help_resolves_schema_method_on_schema_method_return`
- [x] `signature_help_resolves_schema_trait_method_call`
- [x] `signature_help_resolves_schema_trait_method_on_schema_function_return`
- [x] `signature_help_resolves_schema_trait_method_on_schema_method_return`
- [x] `signature_help_resolves_stdlib_callback_method_call`
- [x] `signature_help_resolves_stdlib_function_call`
- [x] `signature_help_resolves_imported_function_with_defaulted_parameter`
- [x] `signature_help_returns_none_for_unknown_call`
- [x] `signature_help_returns_none_for_dynamic_receiver_call`
- [x] `lsp_signature_help_resolves_script_method_call`
- [x] `lsp_signature_help_resolves_source_trait_default_method_on_source_function_return`
- [x] `lsp_signature_help_resolves_source_method_on_source_method_return`
- [x] `lsp_signature_help_resolves_schema_method_call`
- [x] `lsp_signature_help_resolves_schema_method_on_schema_function_return`
- [x] `lsp_signature_help_resolves_schema_method_on_schema_method_return`
- [x] `lsp_signature_help_resolves_schema_trait_method_call`
- [x] `lsp_signature_help_resolves_schema_trait_method_on_schema_function_return`
- [x] `lsp_signature_help_resolves_schema_trait_method_on_schema_method_return`
- [x] `lsp_signature_help_resolves_stdlib_callback_method_call`
- [x] `lsp_signature_help_resolves_stdlib_function_call`
- [x] `lsp_signature_help_resolves_imported_function_with_defaulted_parameter`
- [x] `lsp_signature_help_returns_null_for_unknown_and_dynamic_calls`
- [x] `member_completion_triggers_after_dot_with_empty_prefix`
- [x] `member_completion_includes_builtin_container_methods`
- [x] `member_completion_includes_source_impl_and_trait_methods`
- [x] `query_context_resolves_member_callable_facts_from_expression_receivers`
- [x] `member_completion_uses_schema_function_return_receiver_facts`
- [x] `member_completion_uses_schema_method_return_receiver_facts`
- [x] `member_completion_uses_schema_trait_method_return_receiver_facts`
- [x] `lsp_member_completion_includes_source_and_builtin_methods`
- [x] `lsp_member_completion_uses_schema_function_return_receiver_facts`
- [x] `lsp_member_completion_uses_schema_method_return_receiver_facts`
- [x] `lsp_member_completion_uses_schema_trait_method_return_receiver_facts`
- [x] `completion_uses_short_type_labels_with_owner_details`
- [x] `lsp_completion_uses_short_type_labels_with_owner_details`
- [x] `struct_body_completion_enters_field_declaration_context`
- [x] `lsp_struct_body_completion_enters_field_declaration_context`
- [x] `statement_completion_offers_for_in_and_match_snippets`
- [x] `lsp_statement_completion_offers_for_in_and_match_snippets`

Validation:

```bash
cargo test -p vela_language_service completion signature
cargo test -p vela_analysis completion
```

---

## 12. Phase 8: Hover And Definitions

Purpose: expose semantic facts and navigation.

- [x] Hover locals, parameters, captures, declarations, modules, functions,
  methods, fields, variants, traits, and type hints.
  - Initial hover support now covers script parameters/declarations,
    source-owned globals, source-owned struct fields, source-owned method
    declarations plus typed record and trait receiver calls, source-owned
    traits and trait type hints, source-owned enum variants, schema-backed host
    members and trait receiver methods, schema-backed traits and enum variants,
    stdlib functions, stdlib receiver methods, imported module path segments,
    missing-schema type-hint degradation, and null results for unresolved names
    plus dynamic receiver members.
- [x] Include docs, type facts, effects, permissions, origins, and source spans
  where known.
  - Schema-backed hover now surfaces docs copied through the static schema
    artifact for types, fields, variants, methods, trait methods, and
    functions.
- [x] Implement go to definition for local bindings.
- [x] Implement go to definition for imported module declarations.
- [x] Implement go to definition for schema items with source spans.
  - Initial service and LSP definition support schema type, trait, and
    function source spans when the referenced schema `SourceId` exists in the
    current workspace snapshot.
  - Schema-backed field, method, and trait-method member uses now follow
    schema source spans when the referenced schema `SourceId` exists in the
    current workspace snapshot.
  - Schema-backed enum variant qualified paths now follow schema source spans
    when the referenced schema `SourceId` exists in the current workspace
    snapshot.
- [x] Implement go to declaration/type definition where LSP clients separate
  those requests.
  - Initial language-service and LSP support routes `textDocument/declaration`
    through the same source/schema-backed navigation spans as definition.
  - `textDocument/typeDefinition` now resolves through type facts for local
    values plus source/schema field member expressions, jumps to source/schema
    type declarations when source-backed, and returns null for primitive,
    method, variant, dynamic, or unknown targets instead of falling back to an
    enclosing declaration.
  - Cross-file type-definition coverage now includes imported source local
    type-hint aliases, parameter, trait, struct-field, enum-field, and return
    type-hint aliases, local annotations, parameters, struct constructors,
    function calls, and source method calls whose type or return type is
    source-owned.

Tests:

- [x] `hover_degrades_to_any_without_schema`
- [x] `hover_returns_none_for_unresolved_name`
- [x] `hover_returns_none_for_dynamic_receiver_member`
- [x] `hover_reports_script_parameter_fact`
- [x] `hover_recovers_parameter_fact_after_body_parse_error`
- [x] `hover_reports_effects_and_permissions`
- [x] `hover_reports_schema_trait_method_fact`
- [x] `hover_reports_schema_method_on_schema_method_return_receiver`
- [x] `hover_reports_schema_trait_method_on_schema_method_return_receiver`
- [x] `hover_reports_schema_trait_fact`
- [x] `hover_reports_schema_type_field_and_function_docs`
- [x] `hover_reports_schema_enum_variant_fact`
- [x] `hover_reports_source_global_fact`
- [x] `hover_reports_source_struct_field_fact`
- [x] `hover_reports_source_method_fact`
- [x] `hover_reports_source_trait_fact`
- [x] `hover_reports_source_trait_method_docs`
- [x] `hover_reports_source_trait_receiver_method_fact`
- [x] `hover_reports_source_trait_default_method_on_source_function_return_receiver`
- [x] `hover_reports_source_enum_variant_fact`
- [x] `hover_reports_stdlib_function_fact`
- [x] `hover_reports_stdlib_method_fact`
- [x] `hover_reports_imported_module_path_fact`
- [x] `lsp_hover_reports_open_overlay_parameter_fact`
- [x] `lsp_hover_recovers_parameter_fact_after_body_parse_error`
- [x] `lsp_hover_degrades_to_any_without_schema`
- [x] `lsp_hover_returns_null_for_unresolved_and_dynamic_members`
- [x] `lsp_hover_reports_effects_and_permissions`
- [x] `lsp_hover_reports_source_global_fact`
- [x] `lsp_hover_reports_imported_module_path_fact`
- [x] `lsp_hover_reports_schema_trait_fact`
- [x] `lsp_hover_reports_schema_trait_method_fact`
- [x] `lsp_hover_reports_schema_method_on_schema_method_return_receiver`
- [x] `lsp_hover_reports_schema_trait_method_on_schema_method_return_receiver`
- [x] `lsp_hover_reports_schema_enum_variant_fact`
- [x] `lsp_hover_reports_source_struct_field_fact`
- [x] `lsp_hover_reports_source_method_fact`
- [x] `lsp_hover_reports_source_trait_fact`
- [x] `lsp_hover_reports_source_trait_receiver_method_fact`
- [x] `lsp_hover_reports_source_trait_default_method_on_source_function_return_receiver`
- [x] `lsp_hover_reports_source_enum_variant_fact`
- [x] `lsp_hover_reports_stdlib_function_fact`
- [x] `lsp_hover_reports_stdlib_method_fact`
- [x] `hover_reports_imported_function_const_and_global_facts`
- [x] `lsp_hover_reports_imported_function_const_and_global_facts`
- [x] `definition_follows_local_binding`
- [x] `definition_follows_imported_module_declaration`
- [x] `definition_follows_imported_const_and_global_declarations`
- [x] `lsp_definition_follows_open_overlay_local_binding`
- [x] `lsp_definition_follows_imported_const_and_global_declarations`
- [x] `definition_follows_schema_source_span`
- [x] `definition_follows_schema_field_source_span`
- [x] `definition_follows_schema_method_source_span`
- [x] `definition_follows_schema_trait_method_source_span`
- [x] `definition_follows_schema_variant_source_span`
- [x] `definition_follows_source_trait_default_method_on_source_function_return_receiver`
- [x] `lsp_definition_follows_schema_source_span`
- [x] `lsp_definition_follows_schema_field_source_span`
- [x] `lsp_definition_follows_schema_method_source_span`
- [x] `lsp_definition_follows_schema_trait_method_source_span`
- [x] `lsp_definition_follows_schema_variant_source_span`
- [x] `lsp_definition_follows_source_trait_default_method_on_source_function_return_receiver`
- [x] `lsp_definition_returns_null_for_schema_type_without_source_span`
- [x] `declaration_follows_local_binding`
- [x] `declaration_follows_source_trait_default_method_on_source_function_return_receiver`
- [x] `declaration_does_not_fallback_to_enclosing_function_for_unknown_member`
- [x] `declaration_returns_none_for_dynamic_member`
- [x] `type_definition_follows_schema_source_span`
- [x] `type_definition_follows_local_source_type`
- [x] `type_definition_follows_source_field_type`
- [x] `type_definition_follows_imported_parameter_source_type_alias`
- [x] `type_definition_follows_imported_local_source_type_alias`
- [x] `type_definition_follows_imported_local_source_type_hint`
- [x] `type_definition_follows_imported_parameter_source_type_hint`
- [x] `type_definition_follows_imported_trait_source_type_hint`
- [x] `type_definition_follows_imported_field_source_type_hint`
- [x] `type_definition_follows_imported_enum_field_source_type_hint`
- [x] `type_definition_follows_imported_return_source_type_hint`
- [x] `type_definition_follows_imported_const_and_global_source_type_hints`
- [x] `type_definition_follows_imported_source_field_type_alias`
- [x] `type_definition_follows_imported_function_return_source_type`
- [x] `type_definition_follows_imported_source_member_type`
- [x] `type_definition_follows_imported_source_method_return_type`
- [x] `type_definition_follows_imported_enum_variant_constructor_type`
- [x] `type_definition_follows_imported_struct_constructor_type`
- [x] `type_definition_follows_imported_const_and_global_source_types`
- [x] `type_definition_returns_none_for_source_primitive_field`
- [x] `type_definition_returns_none_for_dynamic_local_value`
- [x] `lsp_declaration_follows_open_overlay_local_binding`
- [x] `lsp_declaration_follows_schema_source_span`
- [x] `lsp_declaration_follows_schema_field_source_span`
- [x] `lsp_declaration_follows_schema_method_source_span`
- [x] `lsp_declaration_follows_schema_trait_method_source_span`
- [x] `lsp_declaration_follows_schema_variant_source_span`
- [x] `lsp_declaration_follows_source_trait_default_method_on_source_function_return_receiver`
- [x] `lsp_declaration_returns_null_for_schema_type_without_source_span`
- [x] `lsp_declaration_returns_null_for_unknown_source_member`
- [x] `lsp_declaration_returns_null_for_dynamic_member`
- [x] `type_definition_follows_schema_field_type_source_span`
- [x] `type_definition_returns_none_for_schema_primitive_field`
- [x] `type_definition_returns_none_for_schema_method`
- [x] `type_definition_returns_none_for_schema_trait_method`
- [x] `type_definition_returns_none_for_schema_variant_without_owner_type_span`
- [x] `lsp_type_definition_follows_schema_source_span`
- [x] `lsp_type_definition_returns_null_for_schema_type_without_source_span`
- [x] `lsp_type_definition_follows_source_struct_field_type`
- [x] `lsp_type_definition_follows_imported_source_struct_field_type_alias`
- [x] `lsp_type_definition_follows_imported_parameter_source_type_alias`
- [x] `lsp_type_definition_follows_imported_local_source_type_alias`
- [x] `lsp_type_definition_follows_imported_local_source_type_hint`
- [x] `lsp_type_definition_follows_imported_parameter_source_type_hint`
- [x] `lsp_type_definition_follows_imported_trait_source_type_hint`
- [x] `lsp_type_definition_follows_imported_field_source_type_hint`
- [x] `lsp_type_definition_follows_imported_enum_field_source_type_hint`
- [x] `lsp_type_definition_follows_imported_return_source_type_hint`
- [x] `lsp_type_definition_follows_imported_const_and_global_source_type_hints`
- [x] `lsp_type_definition_follows_imported_function_return_source_type`
- [x] `lsp_type_definition_follows_imported_source_member_type`
- [x] `lsp_type_definition_follows_imported_source_method_return_type`
- [x] `type_definition_follows_imported_source_trait_method_return_type`
- [x] `lsp_type_definition_follows_imported_source_trait_method_return_type`
- [x] `lsp_type_definition_follows_imported_enum_variant_constructor_type`
- [x] `lsp_type_definition_follows_imported_struct_constructor_type`
- [x] `lsp_type_definition_follows_imported_const_and_global_source_types`
- [x] `lsp_type_definition_returns_null_for_source_primitive_field`
- [x] `lsp_type_definition_returns_null_for_dynamic_local_value`
- [x] `lsp_type_definition_follows_schema_field_type_source_span`
- [x] `lsp_type_definition_returns_null_for_schema_primitive_field`
- [x] `lsp_type_definition_returns_null_for_schema_method`
- [x] `lsp_type_definition_returns_null_for_schema_trait_method`
- [x] `lsp_type_definition_returns_null_for_schema_variant_without_owner_type_span`

Validation:

```bash
cargo test -p vela_language_service hover definition
cargo test -p vela_lsp_server definition lifecycle
cargo test -p vela_analysis hover
```

---

## 13. Phase 9: Document Symbols, Workspace Symbols, Folding, Selection

Purpose: support navigation and outline features.

- [x] Build document symbols from parsed declarations.
- [x] Build workspace symbols from module graph declarations and schema facts.
- [x] Add file/module/class/function/method/field/enum/variant symbol kinds.
  - [x] Add workspace file symbol kinds and source locations.
  - [x] Add script const/global/function/struct/enum/trait/impl/member kinds.
  - [x] Add workspace module symbol kinds and source locations.
  - [x] Add distinct schema host class, record struct, enum, trait,
    function, field, method, and variant symbol kinds.
- [x] Add folding ranges for imports, type declarations, impls, functions,
  blocks, match arms, and multiline literals.
- [x] Add selection ranges from token/expression/statement/item ancestry.

Tests:

- [x] `document_symbols_include_nested_type_members`
- [x] `lsp_document_symbols_include_nested_script_members`
- [x] `workspace_symbols_include_module_qualified_names`
- [x] `workspace_symbols_include_module_symbols`
- [x] `workspace_symbols_include_file_symbols`
- [x] `workspace_symbols_include_schema_items`
- [x] `workspace_symbols_degrade_to_source_only_when_schema_is_missing`
- [x] `lsp_workspace_symbols_include_script_and_schema_symbols`
- [x] `lsp_workspace_symbols_include_module_symbols`
- [x] `lsp_workspace_symbols_include_file_symbols`
- [x] `lsp_workspace_symbols_drop_deleted_files`
- [x] `lsp_workspace_symbols_degrade_to_source_only_when_schema_is_missing`
- [x] `folding_ranges_cover_items_and_blocks`
- [x] `lsp_folding_ranges_cover_items_and_blocks`
- [x] `selection_ranges_walk_syntax_ancestors`
- [x] `lsp_selection_ranges_walk_syntax_ancestors`

Validation:

```bash
cargo test -p vela_language_service symbols
cargo test -p vela_language_service folding
cargo test -p vela_language_service selection
cargo test -p vela_lsp_server selection
```

---

## 14. Phase 10: Semantic Tokens

Purpose: provide syntax and semantic highlighting without changing semantics.

- [x] Implement lexical semantic tokens from tokenizer output.
- [x] Add resolved token modifiers for declarations, definitions, readonly,
  deprecated, builtin, host, and unresolved symbols.
  - [x] Add declaration, definition, readonly, and unresolved modifiers for
    script declarations and binding-map resolutions.
  - [x] Add host modifiers for schema-backed member accesses and builtin
    modifiers for stdlib member method accesses.
  - [x] Add host modifiers for schema-backed function calls and builtin
    modifiers for stdlib qualified function calls.
  - [x] Add host modifiers for schema-backed type hints and builtin modifiers
    for builtin type hints.
- [x] Add token classes for modules, functions, methods, fields, variables,
  parameters, types, traits, enum variants, properties, keywords, numbers,
  strings, bytes, comments, operators, attributes, and macros.
  - [x] Add tokenizer-backed identifiers, keywords, numbers, strings, bytes,
    operators, and attribute marker tokens.
  - [x] Add resolved script function, type, parameter, and variable token
    classes from declarations and binding maps.
  - [x] Add trivia-backed shebang, line-comment, and block-comment token
    classes without changing parser tokenization.
  - [x] Add script-owned struct field, enum variant, enum payload field,
    trait method, and impl method declaration token classes.
  - [x] Add imported module path segment token classes while preserving the
    resolved declaration class for the imported item.
  - [x] Add member-use token classes for script fields/methods, schema-backed
    host fields/methods, and stdlib member methods.
  - [x] Add source-owned trait receiver method call classification through
    the same method token class.
  - [x] Add source-owned trait receiver method call classification when the
    receiver is produced by a source function return.
  - [x] Add source-owned method call classification when the receiver is
    produced by another source method return.
  - [x] Add host-modified schema trait receiver method call classification.
- [x] Implement full semantic tokens.
  - [x] Full tokens include lexical classes, comments, resolved script symbols,
    script member declarations, script/schema/stdlib member uses, and
    schema/stdlib function calls, plus schema/builtin type hints.
- [x] Implement semantic token delta only after generation-stable token caches
  exist.
  - [x] Full-token responses carry deterministic result IDs, and
    `semanticTokens/full/delta` returns no edits for unchanged streams or a
    full replacement edit for changed streams.

Tests:

- [x] `semantic_tokens_cover_lexical_classes`
- [x] `lsp_semantic_tokens_cover_lexical_classes`
- [x] `semantic_tokens_mark_resolved_symbols`
- [x] `lsp_semantic_tokens_mark_resolved_symbols`
- [x] `semantic_tokens_classify_import_module_path_segments`
- [x] `lsp_semantic_tokens_classify_import_module_path_segments`
- [x] `semantic_tokens_include_comments`
- [x] `lsp_semantic_tokens_include_comments`
- [x] `semantic_tokens_classify_script_members`
- [x] `lsp_semantic_tokens_classify_script_members`
- [x] `semantic_tokens_classify_script_member_uses`
- [x] `lsp_semantic_tokens_classify_script_member_uses`
- [x] `semantic_tokens_classify_script_trait_method_uses`
- [x] `lsp_semantic_tokens_classify_script_trait_method_uses`
- [x] `semantic_tokens_classify_source_method_on_source_function_return`
- [x] `lsp_semantic_tokens_classify_source_method_on_source_function_return`
- [x] `semantic_tokens_classify_source_method_on_source_method_return`
- [x] `lsp_semantic_tokens_classify_source_method_on_source_method_return`
- [x] `semantic_tokens_classify_source_trait_method_on_source_function_return`
- [x] `lsp_semantic_tokens_classify_source_trait_method_on_source_function_return`
- [x] `semantic_tokens_classify_source_trait_method_on_source_method_return`
- [x] `lsp_semantic_tokens_classify_source_trait_method_on_source_method_return`
- [x] `semantic_tokens_classify_schema_and_stdlib_member_uses`
- [x] `lsp_semantic_tokens_classify_host_and_builtin_member_uses`
- [x] `infers_schema_function_and_method_return_facts`
- [x] `semantic_tokens_classify_schema_method_on_schema_function_return`
- [x] `lsp_semantic_tokens_classify_schema_method_on_schema_function_return`
- [x] `semantic_tokens_classify_schema_method_on_schema_method_return`
- [x] `lsp_semantic_tokens_classify_schema_method_on_schema_method_return`
- [x] `semantic_tokens_classify_schema_trait_method_uses_as_host`
- [x] `lsp_semantic_tokens_classify_schema_trait_method_uses_as_host`
- [x] `semantic_tokens_classify_schema_trait_method_on_schema_function_return`
- [x] `lsp_semantic_tokens_classify_schema_trait_method_on_schema_function_return`
- [x] `semantic_tokens_classify_schema_trait_method_on_schema_method_return`
- [x] `lsp_semantic_tokens_classify_schema_trait_method_on_schema_method_return`
- [x] `lsp_semantic_tokens_classify_schema_enum_variant_uses`
- [x] `semantic_tokens_classify_schema_and_stdlib_function_calls`
- [x] `lsp_semantic_tokens_classify_host_and_builtin_function_calls`
- [x] `semantic_tokens_classify_host_and_builtin_type_hints`
- [x] `lsp_semantic_tokens_classify_host_and_builtin_type_hints`
- [x] `semantic_tokens_degrade_under_parse_errors`
- [x] `lsp_semantic_tokens_degrade_under_parse_errors`
- [x] `semantic_tokens_degrade_schema_type_hints_when_schema_is_missing`
- [x] `lsp_semantic_tokens_degrade_schema_type_hints_when_schema_is_missing`
- [x] `semantic_tokens_range_filters_tokens`
- [x] `semantic_tokens_range_returns_empty_for_empty_prefix_range`
- [x] `lsp_semantic_tokens_range_filters_tokens`
- [x] `lsp_semantic_tokens_range_returns_empty_for_empty_prefix_range`
- [x] `semantic_token_delta_matches_full_tokens`
- [x] `lsp_semantic_token_delta_matches_full_tokens`

Validation:

```bash
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server semantic_tokens
```

---

## 15. Phase 11: References And Call Hierarchy

Purpose: support workspace navigation and prepare rename.

- [x] Build a reference index from `BindingMap` and module graph use sites.
  - [x] Build initial local binding references from `BindingMap` declarations
    and resolved local expression spans.
  - [x] Build initial script declaration references from resolved
    `BindingMap` declaration uses and module import resolutions.
- [x] Index local, module, function, method, field, variant, trait, and schema
  references.
  - [x] Index local binding declaration and read references within the owning
    function.
  - [x] Index imported module path segments across workspace imports.
  - [x] Index imported script function declarations, imports, and resolved
    call/read sites across workspace sources.
  - [x] Index source-owned script struct field declarations plus typed
    receiver read/write member uses.
  - [x] Index explicit source-owned record-constructor field labels.
  - [x] Index source-owned record-constructor shorthand field labels.
  - [x] Index source-owned enum variant declarations, constructor uses, and
    match-pattern uses.
  - [x] Index source-owned enum record-variant field declarations plus
    constructor and match-pattern labels.
  - [x] Index source-owned inherent script method declarations and typed
    receiver call sites.
  - [x] Index source-owned trait declarations and `impl Trait for Type` uses.
  - [x] Index schema-backed field declarations with source spans plus typed
    host receiver read/write member uses.
  - [x] Index explicit schema-backed record-constructor field labels.
  - [x] Index schema-backed record-constructor shorthand field labels.
  - [x] Index schema-backed enum record-variant field declarations plus
    constructor and match-pattern labels.
  - [x] Index schema-backed method declarations with source spans plus typed
    host receiver call sites.
  - [x] Index schema-backed trait-method declarations with source spans plus
    typed trait receiver call sites.
  - [x] Index schema-backed variant declarations with source spans plus
    constructor and match-pattern uses.
- [x] Track reference kind: read, write, call, type use, import, pattern,
  declaration.
  - [x] Track local declaration and read reference kinds.
  - [x] Track script declaration, import, and read reference kinds.
  - [x] Track local write references and statically resolved script function
    call references.
  - [x] Track source-owned script struct field declaration/read/write
    reference kinds.
  - [x] Track explicit source-owned record-constructor field labels as reads.
  - [x] Track explicit schema-backed record-constructor field labels as reads.
  - [x] Track record-constructor shorthand field labels as reads when
    collecting field references while preserving local references from the
    shorthand token.
  - [x] Track source-owned enum variant declaration/read/pattern reference
    kinds.
  - [x] Track source-owned enum record-variant field declaration/read/pattern
    reference kinds.
  - [x] Track schema-backed enum record-variant field declaration/read/pattern
    reference kinds.
  - [x] Track schema-backed variant declaration/read/pattern reference kinds.
- [x] Implement `textDocument/references`.
  - [x] Serve local binding references through the native LSP request.
  - [x] Serve imported module path segment references through the native LSP
    request.
  - [x] Serve imported script function references through the native LSP
    request.
  - [x] Serve imported source type alias and type-hint references through the
    native LSP request.
  - [x] Serve source-owned script struct field references through the native
    LSP request.
  - [x] Serve explicit source-owned record-constructor field label references
    through the native LSP request.
  - [x] Serve source-owned record-constructor shorthand field label references
    through the native LSP request.
  - [x] Serve source-owned enum variant references through the native LSP
    request.
  - [x] Serve source-owned enum record-variant field references through the
    native LSP request.
  - [x] Serve source-owned script method references through the native LSP
    request.
  - [x] Serve source-owned trait impl references through the native LSP
    request.
  - [x] Serve schema-backed field references through the native LSP request.
  - [x] Serve explicit schema-backed record-constructor field label references
    through the native LSP request.
  - [x] Serve schema-backed enum record-variant field references through the
    native LSP request.
  - [x] Serve schema-backed method references through the native LSP request.
  - [x] Serve schema-backed trait-method references through the native LSP
    request.
  - [x] Serve schema-backed variant references through the native LSP request.
- [x] Implement `textDocument/documentHighlight`.
  - [x] Serve local declaration/read highlights through the native LSP request.
  - [x] Serve imported module path segment highlights in the active document.
  - [x] Serve imported script function import/read highlights in the active
    document.
  - [x] Keep imported script function document highlights local to the active
    document while workspace references include the defining file.
  - [x] Keep imported const/global document highlights local to the active
    document while workspace references include the defining file.
  - [x] Keep imported source type alias/type-hint document highlights local to
    the active document while workspace references include the defining file.
  - [x] Keep imported source field/method document highlights local to the
    active document while workspace references include the defining file.
  - [x] Serve source-owned script method declaration/call highlights in the
    active document.
  - [x] Serve source-owned trait declaration/impl highlights in the active
    document.
  - [x] Serve schema-backed field read/write highlights in the active
    document.
  - [x] Serve schema-backed method and trait-method call highlights in the
    active document.
  - [x] Serve schema-backed variant constructor and pattern highlights in the
    active document.
- [x] Implement incoming and outgoing call hierarchy for script functions and
  methods where calls are statically resolved.
  - [x] Serve initial source-backed script function prepare, incoming, and
    outgoing call hierarchy for statically resolved calls.
  - [x] Serve imported script function alias prepare from import statements,
    incoming, and outgoing call hierarchy for statically resolved calls.
  - [x] Serve source-owned inherent script method prepare, incoming, and
    outgoing call hierarchy for typed receiver calls.
  - [x] Serve cross-file source-owned inherent script method prepare,
    incoming, and outgoing call hierarchy for typed receiver calls.
  - [x] Serve source-owned trait impl method prepare, incoming, and outgoing
    call hierarchy for typed receiver calls.
  - [x] Serve cross-file source-owned trait impl method prepare, incoming,
    and outgoing call hierarchy for typed receiver calls, including method
    bodies that call imported helper functions.
  - [x] Serve source-owned trait default/interface method prepare, incoming,
    and default-body outgoing call hierarchy for typed trait receiver calls.
  - [x] Serve cross-file source-owned trait default/interface method prepare,
    incoming, and caller/default-body outgoing call hierarchy for typed trait
    receiver calls.
  - [x] Serve schema-backed method and trait-method prepare, incoming, and
    script-caller outgoing call hierarchy for typed receiver calls, including
    schema function-return and schema method-return receivers.

Tests:

- [x] `references_find_local_binding_uses`
- [x] `references_can_exclude_local_declaration`
- [x] `lsp_references_find_local_binding_uses`
- [x] `references_find_imported_module_segments`
- [x] `lsp_references_find_imported_module_segments`
- [x] `references_find_imported_function_uses`
- [x] `lsp_references_find_imported_function_uses`
- [x] `references_find_imported_function_alias_uses`
- [x] `lsp_references_find_imported_function_alias_uses`
- [x] `lsp_references_drop_deleted_imported_source_file`
- [x] `lsp_references_refresh_renamed_imported_source_file`
- [x] `lsp_references_use_open_overlay_for_imported_defining_file`
- [x] `lsp_references_use_open_overlay_for_importing_file`
- [x] `references_find_imported_const_and_global_uses`
- [x] `lsp_references_find_imported_const_and_global_uses`
- [x] `references_find_imported_source_type_uses`
- [x] `lsp_references_find_imported_source_type_uses`
- [x] `references_find_field_reads_and_writes`
- [x] `lsp_references_find_field_reads_and_writes`
- [x] `references_find_cross_file_imported_source_field_and_method_uses`
- [x] `lsp_references_find_cross_file_imported_source_field_and_method_uses`
- [x] `references_find_record_constructor_field_labels`
- [x] `lsp_references_find_record_constructor_field_labels`
- [x] `references_find_record_constructor_shorthand_field_labels`
- [x] `lsp_references_find_record_constructor_shorthand_field_labels`
- [x] `references_find_enum_variant_constructors_and_patterns`
- [x] `lsp_references_find_enum_variant_constructors_and_patterns`
- [x] `references_find_cross_file_imported_source_enum_variant_uses`
- [x] `lsp_references_find_cross_file_imported_source_enum_variant_uses`
- [x] `references_find_enum_record_variant_field_labels_and_patterns`
- [x] `lsp_references_find_enum_record_variant_field_labels_and_patterns`
- [x] `references_find_cross_file_imported_source_enum_record_variant_field_uses`
- [x] `lsp_references_find_cross_file_imported_source_enum_record_variant_field_uses`
- [x] `references_find_script_method_calls`
- [x] `lsp_references_find_script_method_calls`
- [x] `references_find_source_method_calls_on_source_function_return_receivers`
- [x] `lsp_references_find_source_method_calls_on_source_function_return_receivers`
- [x] `references_find_source_trait_default_method_calls_on_source_function_return_receivers`
- [x] `lsp_references_find_source_trait_default_method_calls_on_source_function_return_receivers`
- [x] `references_find_source_method_calls_on_source_method_return_receivers`
- [x] `lsp_references_find_source_method_calls_on_source_method_return_receivers`
- [x] `references_find_source_trait_default_method_calls_on_source_method_return_receivers`
- [x] `lsp_references_find_source_trait_default_method_calls_on_source_method_return_receivers`
- [x] `references_find_trait_impl_uses`
- [x] `lsp_references_find_trait_impl_uses`
- [x] `references_find_schema_field_reads_and_writes`
- [x] `lsp_references_find_schema_field_reads_and_writes`
- [x] `references_find_schema_record_constructor_field_labels`
- [x] `lsp_references_find_schema_record_constructor_field_labels`
- [x] `references_find_schema_record_constructor_shorthand_field_labels`
- [x] `lsp_references_find_schema_record_constructor_shorthand_field_labels`
- [x] `references_find_schema_record_variant_field_labels_and_patterns`
- [x] `lsp_references_find_schema_record_variant_field_labels_and_patterns`
- [x] `references_find_schema_method_calls`
- [x] `lsp_references_find_schema_method_calls`
- [x] `references_find_schema_method_calls_on_schema_function_return_receivers`
- [x] `lsp_references_find_schema_method_calls_on_schema_function_return_receivers`
- [x] `references_find_schema_method_calls_on_schema_method_return_receivers`
- [x] `lsp_references_find_schema_method_calls_on_schema_method_return_receivers`
- [x] `references_find_schema_trait_method_calls`
- [x] `lsp_references_find_schema_trait_method_calls`
- [x] `references_find_schema_trait_method_calls_on_schema_function_return_receivers`
- [x] `lsp_references_find_schema_trait_method_calls_on_schema_function_return_receivers`
- [x] `references_find_schema_trait_method_calls_on_schema_method_return_receivers`
- [x] `lsp_references_find_schema_trait_method_calls_on_schema_method_return_receivers`
- [x] `references_find_schema_variant_constructors_and_patterns`
- [x] `lsp_references_find_schema_variant_constructors_and_patterns`
- [x] `reference_query_reports_dynamic_any_resolution`
- [x] `reference_query_reports_unresolved_resolution`
- [x] `lsp_references_return_empty_for_dynamic_and_unresolved_targets`
- [x] `document_highlight_marks_local_declaration_and_reads`
- [x] `document_highlight_marks_import_and_calls_in_active_document`
- [x] `document_highlight_imported_symbol_stays_in_active_document`
- [x] `document_highlight_imported_const_and_global_stays_in_active_document`
- [x] `document_highlight_imported_source_type_stays_in_active_document`
- [x] `document_highlight_imported_source_field_and_method_stays_in_active_document`
- [x] `lsp_document_highlight_marks_local_declaration_and_reads`
- [x] `lsp_document_highlight_marks_import_and_calls_in_active_document`
- [x] `lsp_document_highlight_imported_symbol_stays_in_active_document`
- [x] `lsp_document_highlight_imported_const_and_global_stays_in_active_document`
- [x] `lsp_document_highlight_imported_source_type_stays_in_active_document`
- [x] `lsp_document_highlight_imported_source_field_and_method_stays_in_active_document`
- [x] `document_highlight_marks_imported_module_segments`
- [x] `lsp_document_highlight_marks_imported_module_segments`
- [x] `document_highlight_marks_read_write_call`
- [x] `lsp_document_highlight_marks_read_write_call`
- [x] `document_highlight_returns_empty_for_dynamic_and_unresolved_targets`
- [x] `lsp_document_highlight_returns_empty_for_dynamic_and_unresolved_targets`
- [x] `document_highlight_marks_script_method_calls`
- [x] `lsp_document_highlight_marks_script_method_calls`
- [x] `document_highlight_marks_source_method_calls_on_source_function_return_receivers`
- [x] `lsp_document_highlight_marks_source_method_calls_on_source_function_return_receivers`
- [x] `document_highlight_marks_source_trait_default_method_calls_on_source_function_return_receivers`
- [x] `lsp_document_highlight_marks_source_trait_default_method_calls_on_source_function_return_receivers`
- [x] `document_highlight_marks_source_method_calls_on_source_method_return_receivers`
- [x] `lsp_document_highlight_marks_source_method_calls_on_source_method_return_receivers`
- [x] `document_highlight_marks_source_trait_default_method_calls_on_source_method_return_receivers`
- [x] `lsp_document_highlight_marks_source_trait_default_method_calls_on_source_method_return_receivers`
- [x] `document_highlight_marks_trait_impl_uses`
- [x] `lsp_document_highlight_marks_trait_impl_uses`
- [x] `document_highlight_marks_schema_field_reads_and_writes`
- [x] `lsp_document_highlight_marks_schema_field_reads_and_writes`
- [x] `document_highlight_marks_schema_method_calls`
- [x] `lsp_document_highlight_marks_schema_method_calls`
- [x] `document_highlight_marks_schema_method_calls_on_schema_function_return_receivers`
- [x] `lsp_document_highlight_marks_schema_method_calls_on_schema_function_return_receivers`
- [x] `document_highlight_marks_schema_trait_method_calls_on_schema_function_return_receivers`
- [x] `lsp_document_highlight_marks_schema_trait_method_calls_on_schema_function_return_receivers`
- [x] `document_highlight_marks_schema_method_calls_on_schema_method_return_receivers`
- [x] `lsp_document_highlight_marks_schema_method_calls_on_schema_method_return_receivers`
- [x] `document_highlight_marks_schema_trait_method_calls_on_schema_method_return_receivers`
- [x] `lsp_document_highlight_marks_schema_trait_method_calls_on_schema_method_return_receivers`
- [x] `document_highlight_marks_schema_variant_uses`
- [x] `lsp_document_highlight_marks_schema_variant_uses`
- [x] `call_hierarchy_uses_resolved_call_graph`
- [x] `lsp_call_hierarchy_uses_resolved_call_graph`
- [x] `call_hierarchy_uses_imported_function_alias_calls`
- [x] `lsp_call_hierarchy_uses_imported_function_alias_calls`
- [x] `call_hierarchy_returns_empty_for_unresolved_dynamic_and_non_callable_targets`
- [x] `lsp_prepare_call_hierarchy_returns_empty_for_unresolved_dynamic_and_non_callable_targets`
- [x] `call_hierarchy_uses_resolved_script_method_calls`
- [x] `lsp_call_hierarchy_uses_resolved_script_method_calls`
- [x] `call_hierarchy_cross_file_source_method_calls`
- [x] `lsp_call_hierarchy_cross_file_source_method_calls`
- [x] `call_hierarchy_uses_resolved_trait_impl_method_calls`
- [x] `lsp_call_hierarchy_uses_resolved_trait_impl_method_calls`
- [x] `call_hierarchy_cross_file_trait_impl_method_calls`
- [x] `lsp_call_hierarchy_cross_file_trait_impl_method_calls`
- [x] `call_hierarchy_uses_trait_default_and_interface_methods`
- [x] `lsp_call_hierarchy_uses_trait_default_and_interface_methods`
- [x] `call_hierarchy_cross_file_trait_default_and_interface_methods`
- [x] `lsp_call_hierarchy_cross_file_trait_default_and_interface_methods`
- [x] `call_hierarchy_uses_schema_method_and_trait_method_calls`
- [x] `lsp_call_hierarchy_uses_schema_method_and_trait_method_calls`
- [x] `call_hierarchy_uses_schema_method_calls_on_schema_function_return_receivers`
- [x] `lsp_call_hierarchy_uses_schema_method_calls_on_schema_function_return_receivers`
- [x] `call_hierarchy_uses_schema_trait_method_calls_on_schema_function_return_receivers`
- [x] `lsp_call_hierarchy_uses_schema_trait_method_calls_on_schema_function_return_receivers`
- [x] `call_hierarchy_uses_schema_method_calls_on_schema_method_return_receivers`
- [x] `lsp_call_hierarchy_uses_schema_method_calls_on_schema_method_return_receivers`
- [x] `call_hierarchy_uses_schema_trait_method_calls_on_schema_method_return_receivers`
- [x] `lsp_call_hierarchy_uses_schema_trait_method_calls_on_schema_method_return_receivers`
- [x] `call_hierarchy_uses_source_method_calls_on_source_method_return_receivers`
- [x] `lsp_call_hierarchy_uses_source_method_calls_on_source_method_return_receivers`
- [x] `call_hierarchy_uses_source_trait_default_method_calls_on_source_method_return_receivers`
- [x] `lsp_call_hierarchy_uses_source_trait_default_method_calls_on_source_method_return_receivers`

Validation:

```bash
cargo test -p vela_language_service references
cargo test -p vela_lsp_server references
cargo test -p vela_language_service call_hierarchy
cargo test -p vela_lsp_server call_hierarchy
```

---

## 16. Phase 12: Rename

Purpose: provide safe refactoring without changing runtime contracts.

- [x] Implement `prepareRename` for local bindings.
  - [x] Prepare local binding rename ranges and placeholders.
  - [x] Reject keywords, literals, and non-local targets.
- [x] Implement local rename inside one function body.
  - [x] Return workspace edits for local declaration and resolved uses.
- [x] Implement private module declaration rename.
  - [x] Rename private value declarations (`const`/`global`) and resolved
    same-workspace uses.
  - [x] Rename private type declarations and type-hint uses once ownership
    spans are indexed.
- [x] Implement public module declaration rename with import rewrites.
  - [x] Rename script function declarations, resolved import path segments,
    and resolved unaliased call sites.
  - [x] Preserve import aliases while rewriting renamed source function path
    segments.
- [x] Implement field/method/variant rename only when ownership is known and
  source spans are script-owned.
  - [x] Rename source-owned private struct fields and typed receiver member
    uses.
  - [x] Rename source-owned private inherent methods and typed receiver member
    calls.
  - [x] Rename source-owned trait default methods and typed source-return
    receiver member calls.
  - [x] Rename source-owned private enum variants, constructor uses, and
    match-pattern uses.
- [x] Reject host schema rename unless the source is explicitly script-owned.
- [x] Rename source-backed schema items only when the schema declaration span
  maps to a workspace source.
  - [x] Rename source-backed schema types plus type-hint uses.
  - [x] Rename source-backed schema functions plus call sites.
  - [x] Rename source-backed schema fields and methods plus typed receiver
    member uses.
  - [x] Rename source-backed schema variants plus constructor and
    match-pattern uses.
- [x] Reject renames that would collide in scope, module exports, trait impls,
  or import aliases.
  - [x] Reject local binding renames that collide with an existing function
    binding.
  - [x] Reject same-module declaration collisions through native LSP rename.
  - [x] Reject trait-impl method renames that would collide with an existing
    method in the same impl block.
  - [x] Reject imported declaration renames that would collide with an
    existing import alias or import binding.
  - [x] Reject source-backed schema member renames that would collide with an
    existing member of the same kind on the same owner.
- [x] Report hot-reload ABI/schema risk for exported API rename.
  - [x] Public script function renames carry hot-reload ABI risk metadata in
    service workspace edits and LSP change annotations.
  - [x] Source-backed schema renames carry schema ABI risk metadata in service
    workspace edits and LSP change annotations.
- [x] Return workspace edits with stable text ranges and document versions.
  - Rename workspace edits now carry source versions through the editor-neutral
    service model, and native LSP rename responses include versioned
    `documentChanges` while retaining URI-keyed `changes`.

Tests:

- [x] `prepare_rename_rejects_keywords_and_literals`
- [x] `lsp_prepare_rename_rejects_keywords_and_literals`
- [x] `local_rename_updates_all_function_uses`
- [x] `lsp_local_rename_updates_all_function_uses`
- [x] `rename_workspace_edits_carry_document_versions`
- [x] `lsp_rename_returns_versioned_document_changes`
- [x] `private_function_rename_updates_imports`
- [x] `lsp_private_function_rename_updates_imports`
- [x] `private_function_rename_updates_aliased_import_path`
- [x] `lsp_private_function_rename_updates_aliased_import_path`
- [x] `private_value_declaration_rename_updates_uses`
- [x] `lsp_private_value_declaration_rename_updates_uses`
- [x] `lsp_private_type_declaration_rename_updates_type_hints`
- [x] `public_export_rename_reports_hot_reload_risk`
- [x] `lsp_public_export_rename_reports_hot_reload_risk`
- [x] `rename_rejects_scope_collision`
- [x] `rename_rejects_module_declaration_collision`
- [x] `lsp_rename_rejects_module_declaration_collision`
- [x] `private_method_rename_rejects_trait_impl_collision`
- [x] `lsp_private_method_rename_rejects_trait_impl_collision`
- [x] `function_rename_rejects_import_alias_collision`
- [x] `lsp_rename_rejects_import_alias_collision`
- [x] `source_backed_schema_member_rename_rejects_same_kind_collisions`
- [x] `lsp_source_backed_schema_member_rename_rejects_same_kind_collisions`
- [x] `private_struct_field_rename_updates_member_uses`
- [x] `lsp_private_struct_field_rename_updates_member_uses`
- [x] `private_method_rename_updates_typed_receiver_calls`
- [x] `lsp_private_method_rename_updates_typed_receiver_calls`
- [x] `source_trait_default_method_rename_updates_source_function_return_receiver_calls`
- [x] `lsp_source_trait_default_method_rename_updates_source_function_return_receiver_calls`
- [x] `private_enum_variant_rename_updates_constructors_and_patterns`
- [x] `private_enum_variant_rename_rejects_variant_collision`
- [x] `lsp_private_enum_variant_rename_updates_constructors_and_patterns`
- [x] `source_backed_schema_type_rename_updates_type_hints`
- [x] `lsp_source_backed_schema_type_rename_updates_type_hints`
- [x] `source_backed_schema_function_rename_updates_call_sites`
- [x] `lsp_source_backed_schema_function_rename_updates_call_sites`
- [x] `source_backed_schema_variant_rename_updates_constructors_and_patterns`
- [x] `lsp_source_backed_schema_variant_rename_updates_constructors_and_patterns`
- [x] `source_backed_schema_field_rename_updates_member_uses`
- [x] `source_backed_schema_method_rename_updates_member_calls`
- [x] `lsp_source_backed_schema_rename_updates_member_uses`
- [x] `host_schema_rename_is_not_editable`
- [x] `lsp_host_schema_rename_is_not_editable`

Validation:

```bash
cargo test -p vela_language_service rename
cargo test -p vela_lsp_server rename
```

---

## 17. Phase 13: Code Actions

Purpose: turn structured diagnostics into safe edits.

- [x] Add code action data model independent from LSP protocol types.
- [x] Add typo fixes from candidate diagnostics.
- [x] Add import insertion for unresolved qualified symbols.
- [x] Add remove-unused-import action after unused diagnostics exist.
- [x] Add fill missing match arms when enum facts are known.
- [x] Add missing record fields for known constructors.
- [x] Defer simple `if` null-check to Option/Result guard rewrites until
  syntax ownership is unambiguous.
  - No action is offered until a structured diagnostic or syntax pattern can
    prove the rewrite is local, source-owned, and semantics-preserving.
  - Current coverage keeps code actions diagnostic-backed and rejects
    ambiguous/dynamic fixes rather than offering speculative semantic rewrites.
- [x] Add quick-fix tests for range stability under open overlays.

Tests:

- [x] `code_action_fixes_unknown_field_typo`
- [x] `lsp_code_action_fixes_unknown_field_typo`
- [x] `code_action_inserts_missing_import`
- [x] `lsp_code_action_inserts_missing_import`
- [x] `code_action_ranges_follow_open_overlay_text`
- [x] `lsp_code_action_ranges_follow_open_overlay_text`
- [x] `unused_import_reports_warning`
- [x] `unused_import_ignores_type_hint_use`
- [x] `code_action_removes_unused_import`
- [x] `lsp_code_action_removes_unused_import`
- [x] `code_action_fills_enum_match_arms`
- [x] `lsp_code_action_fills_enum_match_arms`
- [x] `code_action_adds_missing_record_fields`
- [x] `lsp_code_action_adds_missing_record_fields`
- [x] `code_action_rejects_ambiguous_dynamic_fix`
- [x] `lsp_code_action_rejects_ambiguous_import_fix`
- [x] `lsp_code_action_rejects_dynamic_receiver_typo_fix`

Validation:

```bash
cargo test -p vela_language_service code_action
cargo test -p vela_lsp_server code_action
```

---

## 18. Phase 14: Formatting

Purpose: provide deterministic source formatting without losing comments.

- [x] Decide and document the lossless CST/trivia policy used by formatting.
  - Current policy: `vela_syntax::formatting` owns stable token/trivia
    extraction and token-driven full-document formatting, with parser-owned
    item/member spans used where range and on-type formatting claim support.
  - Semicolonless `use` item newline boundaries are preserved as syntax-owned
    trivia so imports do not collapse into following items.
- [x] Implement stable token/trivia extraction if current parser data is not
  sufficient.
- [x] Add formatting IR that preserves comments and blank-line groups.
  - Initial editor-neutral IR preserves token/trivia source text, comments,
    shebang trivia, spans, and blank-line whitespace groups.
- [x] Implement expression formatting.
  - Initial token-driven rules normalize operator and delimiter spacing.
  - Builtin container and nested `Option`/`Result` type arguments use compact
    spacing without formatter-created type-argument line breaks.
- [x] Implement statement and block formatting.
  - Initial token-driven rules indent brace blocks and comment lines.
- [x] Implement item/declaration formatting.
  - Initial token-driven rules indent struct fields, enum variants, trait
    method declarations, impl methods, nested enum record fields, and adjacent
    top-level declarations.
- [x] Implement range formatting.
  - Initial native LSP support limits trailing-whitespace cleanup edits to the
    requested range.
  - Whole top-level item selections now apply the token/trivia formatter to
    the selected item while preserving unselected text.
  - Whitespace-padded selections around one top-level item normalize to that
    item span before applying the token/trivia formatter.
  - Impl and trait method selections now use parser-owned method spans to
    format one selected nested member while preserving surrounding text.
  - Exact bodyless trait method selections avoid injecting a trailing newline
    before unselected same-line separator whitespace.
  - Indented nested method selections now preserve enclosing member indentation
    and avoid duplicating the following line break.
  - Struct fields, enum variants, and enum record fields now use parser-owned
    spans for selected nested member formatting.
  - Adjacent selected members within the same struct, enum variant, trait, or
    impl parent now format as a contiguous nested member group.
  - Range formatting uses parser-owned item/member spans after stable comment,
    blank-line, import-boundary, and compact type-argument behavior.
- [x] Implement full document formatting.
  - Native LSP full-document formatting now uses the token/trivia formatter
    for spacing, brace indentation, comment preservation, and final newline.
  - Full-document formatting must preserve compact type arguments in function
    parameters, return hints, local annotations, and nested container hints.
- [x] Implement on-type formatting only after full/range formatting is stable.
  - Initial native LSP support handles `}` and newline triggers by limiting
    trailing-whitespace cleanup to the current brace-delimited construct or
    current line fallback.
  - `}` triggers on completed top-level items now reflow that item through the
    token/trivia formatter while preserving surrounding text.
  - `}` triggers on completed impl/trait methods now reflow the innermost
    parser-owned nested member while preserving enclosing indentation.
  - `}` triggers on completed enum record variants now reflow the nested
    parser-owned record fields without formatting the enclosing enum.
  - On-type reflow is gated by parser-owned item/member spans and otherwise
    falls back to trivia-limited whitespace cleanup.
- [x] Add idempotence tests and malformed-source fallback behavior.

Tests:

- [x] `formatting_preserves_comments`
- [x] `formatting_is_idempotent`
- [x] `range_formatting_limits_edits_to_range`
- [x] `formatting_handles_malformed_source_without_panic`
- [x] `formatting_does_not_depend_on_successful_hir_analysis`
- [x] `formatting_extracts_comments_and_blank_line_groups`
- [x] `formatting_preserves_newline_after_use_item`
- [x] `formatting_ir_preserves_comments_and_blank_line_groups`
- [x] `formatting_formats_expressions_and_function_blocks`
- [x] `formatting_preserves_comments_while_formatting_blocks`
- [x] `formatting_formats_item_declarations`
- [x] `lsp_document_formatting_formats_declarations`
- [x] `on_type_formatting_only_edits_current_construct`
- [x] `lsp_on_type_formatting_only_edits_current_construct`
- [x] `on_type_formatting_reflows_completed_item`
- [x] `lsp_on_type_formatting_reflows_completed_item`
- [x] `on_type_formatting_reflows_completed_multiline_item`
- [x] `lsp_on_type_formatting_reflows_completed_multiline_item`
- [x] `on_type_formatting_reflows_completed_nested_method`
- [x] `lsp_on_type_formatting_reflows_completed_nested_method`
- [x] `on_type_formatting_reflows_completed_enum_record_variant`
- [x] `lsp_on_type_formatting_reflows_completed_enum_record_variant`
- [x] `on_type_formatting_ignores_unsupported_trigger`
- [x] `lsp_document_formatting_returns_full_document_edit`
- [x] `lsp_document_formatting_returns_empty_edits_when_idempotent`
- [x] `lsp_range_formatting_limits_edits_to_range`
- [x] `range_formatting_formats_selected_item`
- [x] `lsp_range_formatting_formats_selected_item`
- [x] `range_formatting_formats_item_with_leading_blank_selection`
- [x] `lsp_range_formatting_formats_item_with_leading_blank_selection`
- [x] `parses_inherent_impl_methods`
- [x] `range_formatting_formats_selected_impl_method`
- [x] `lsp_range_formatting_formats_selected_impl_method`
- [x] `range_formatting_formats_selected_trait_method`
- [x] `lsp_range_formatting_formats_selected_trait_method`
- [x] `range_formatting_preserves_nested_method_indent`
- [x] `lsp_range_formatting_preserves_nested_method_indent`
- [x] `range_formatting_preserves_struct_field_indent`
- [x] `lsp_range_formatting_preserves_struct_field_indent`
- [x] `range_formatting_formats_selected_struct_field_group`
- [x] `lsp_range_formatting_formats_selected_struct_field_group`
- [x] `range_formatting_formats_selected_enum_record_field_group`
- [x] `lsp_range_formatting_formats_selected_enum_record_field_group`
- [x] `formatting_compacts_builtin_container_type_arguments`
- [x] `formatting_compacts_nested_result_container_type_arguments`
- [x] `formatting_preserves_container_type_arguments_on_one_line`
- [x] `formatting_formats_container_type_hint_example`
- [x] `range_formatting_compacts_builtin_container_type_arguments`
- [x] `on_type_formatting_compacts_builtin_container_type_arguments`
- [x] `lsp_document_formatting_compacts_container_type_arguments`
- [x] `lsp_document_formatting_formats_container_type_hint_example`

Validation:

```bash
cargo test -p vela_language_service formatting
cargo test -p vela_lsp_server formatting
cargo test -p vela_syntax formatting
```

---

## 19. Phase 15: Inlay Hints And Type Hints

Purpose: expose gradual type facts without implying static typing.

- [x] Add parameter name hints for calls.
  - Initial native support exposes script/schema function and typed
    source/schema method parameter labels through `textDocument/inlayHint` and
    suppresses already named arguments.
  - Parameter hints suppress schema/script/variant parameters whose facts cross
    dynamic `Any` or unknown boundaries.
  - Inlay labels are stored as editor-neutral `DisplayParts` in the language
    service and rendered only at LSP projection boundaries.
- [x] Add inferred local type hints from stable TypeFacts.
  - Ordinary `let` bindings now expose stable inferred `TypeFact` labels and
    suppress explicit annotations plus unstable `unknown`/`Any` boundaries.
- [x] Add lambda parameter hints from collection/iterator facts.
  - Lambda callbacks on typed stdlib collection/iterator methods now expose
    stable inferred parameter labels, including map key/value arity variants.
- [x] Add enum variant payload hints.
  - Tuple-variant constructors now expose payload field-name hints through the
    shared signature/inlay path.
- [x] Add host path type hints from schema facts.
  - Host field paths with stable schema facts now expose type labels while
    suppressing method callees and dynamic `Any` fields.
- [x] Suppress hints at dynamic `Any` boundaries.
  - Local, lambda, and host-path type inlay hints suppress `Any`; tuple-variant
    payload-name hints suppress dynamic payload facts.
  - Parameter name hints suppress unstable `Any`/unknown parameter facts before
    emitting labels.

Tests:

- [x] `inlay_hints_show_parameter_names` checks rendered labels and structured
  label parts
- [x] `inlay_hints_skip_named_arguments_and_unknown_calls`
- [x] `inlay_hints_use_schema_function_names`
- [x] `inlay_hints_show_source_method_parameter_names`
- [x] `inlay_hints_show_stable_local_typefacts`
- [x] `inlay_hints_show_lambda_parameter_facts`
- [x] `inlay_hints_show_host_path_typefacts`
- [x] `inlay_hints_show_enum_variant_payload_names`
- [x] `inlay_hints_degrade_to_any_without_schema`
- [x] `inlay_hints_suppress_any_schema_function_parameters`
- [x] `inlay_hints_suppress_any_schema_method_parameters_on_schema_function_return_receiver`
- [x] `inlay_hints_suppress_any_schema_method_parameters_on_schema_method_return_receiver`
- [x] `inlay_hints_suppress_any_schema_trait_method_parameters_on_schema_function_return_receiver`
- [x] `inlay_hints_suppress_any_schema_trait_method_parameters_on_schema_method_return_receiver`
- [x] `inlay_hints_suppress_any_source_function_and_method_parameters`
- [x] `inlay_hints_suppress_any_source_method_parameters_on_source_function_return_receiver`
- [x] `inlay_hints_suppress_any_source_method_parameters_on_source_method_return_receiver`
- [x] `inlay_hints_suppress_any_source_trait_default_method_parameters_on_source_function_return_receiver`
- [x] `inlay_hints_suppress_any_source_trait_default_method_parameters_on_source_method_return_receiver`
- [x] `inlay_hints_suppress_any_enum_variant_payloads`
- [x] `inlay_hints_suppress_any_schema_enum_variant_payloads`
- [x] `inlay_hints_suppress_any_lambda_parameter_facts`
- [x] `lsp_inlay_hints_show_parameter_names`
- [x] `lsp_inlay_hints_show_source_method_parameter_names`
- [x] `lsp_inlay_hints_show_local_typefacts`
- [x] `lsp_inlay_hints_show_lambda_parameter_facts`
- [x] `lsp_inlay_hints_show_host_path_typefacts`
- [x] `lsp_inlay_hints_show_enum_variant_payload_names`
- [x] `lsp_inlay_hints_degrade_to_any_without_schema`
- [x] `lsp_inlay_hints_suppress_any_schema_function_parameters`
- [x] `lsp_inlay_hints_suppress_any_schema_method_parameters_on_schema_function_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_schema_method_parameters_on_schema_method_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_schema_trait_method_parameters_on_schema_function_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_schema_trait_method_parameters_on_schema_method_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_source_function_and_method_parameters`
- [x] `lsp_inlay_hints_suppress_any_source_method_parameters_on_source_function_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_source_method_parameters_on_source_method_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_source_trait_default_method_parameters_on_source_function_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_source_trait_default_method_parameters_on_source_method_return_receiver`
- [x] `lsp_inlay_hints_suppress_any_enum_variant_payloads`
- [x] `lsp_inlay_hints_suppress_any_schema_enum_variant_payloads`
- [x] `lsp_inlay_hints_suppress_any_lambda_parameter_facts`
- [x] `lsp_inlay_hints_respect_requested_range`

Validation:

```bash
cargo test -p vela_language_service inlay
cargo test -p vela_lsp_server inlay
```

---

## 20. Phase 16: File Watching, Configuration, And Schema Reload

Purpose: make the server robust in real projects.

- [x] Watch `.vela` sources under configured roots.
- [x] Watch `vela.toml`.
- [x] Watch host schema artifact.
- [x] Register watched files with clients that support dynamic watcher
  registration.
- [x] Debounce file events.
  - Watched-file notifications coalesce duplicate URI events within each
    batch, applying only the final event per URI while preserving final-event
    order for deterministic config/source/schema processing.
- [x] Handle created, changed, deleted, and renamed files.
  - [x] Created and changed `.vela` files update disk snapshots.
  - [x] Deleted `.vela` files remove disk snapshots and republish open diagnostics.
  - [x] Renamed `.vela` files update module paths.
  - [x] Deleted `vela.toml` files clear configuration diagnostics and fall
    back to workspace-root/editor configuration.
  - [x] Deleted host schema artifacts publish missing-schema diagnostics.
- [x] Rebuild module path index after file moves.
- [x] Surface configuration diagnostics.
- [x] Support workspace folder changes.

Tests:

- [x] `file_create_adds_module`
- [x] `file_delete_reports_removed_imports`
- [x] `file_rename_updates_module_path`
- [x] `invalid_vela_toml_publishes_config_diagnostic`
- [x] `deleting_vela_toml_clears_config_diagnostic`
- [x] `schema_watch_publishes_invalid_schema_diagnostic`
- [x] `schema_watch_clears_diagnostic_after_valid_reload`
- [x] `schema_delete_publishes_missing_schema_diagnostic`
- [x] `workspace_folder_change_reindexes_project`
- [x] `workspace_folder_removal_clears_disk_facts_but_keeps_open_overlay`
- [x] `watched_file_batch_coalesces_to_last_event_per_uri`
- [x] `lsp_initialized_registers_watched_files_when_supported`

Validation:

```bash
cargo test -p vela_lsp_server file_watching
cargo test -p vela_language_service project
```

---

## 21. Phase 17: Distribution And Editor Integrations

Purpose: package native LSP for real editors while keeping plugins thin.

- [x] Add stdio server command.
- [x] Add `--version`, `--stdio`, and config flags.
  - Native binary now runs stdio by default or with `--stdio`, and reports
    package version with `--version`.
  - Native launch flags `--root` and `--schema` seed the same
    `WorkspaceConfig` fallback used by editor initialization options.
  - Editor initialization options now map `workspace.roots` and `host.schema`
    into the server `WorkspaceConfig`.
  - `workspace/didChangeConfiguration` now remaps editor settings into
    `WorkspaceConfig`, reloads configured schema artifacts, and invalidates
    project-derived indexes.
- [x] Package VS Code extension as thin launcher/config UI.
  - `editors/vscode` contributes `.vela` language metadata, syntax metadata,
    `vela.*` settings, and a `vscode-languageclient` stdio launcher for the
    native server without duplicating language-service behavior.
- [x] Package Zed extension as thin launcher/config UI.
  - `editors/zed` contributes Vela language metadata and a native-server
    command hook that launches `vela_lsp_server --stdio` without duplicating
    language-service behavior.
- [x] Document manual setup for editors that can launch generic LSP servers.
  - `docs/lsp-editor-setup.md` documents stdio launch, `vela.toml`,
    `--root`/`--schema` fallback flags, initialization options, packaged
    binaries, VS Code and Zed package setup, and generic client wiring without
    moving behavior into editor plugins.
- [x] Add release matrix for Windows, macOS, and Linux binaries.
  - `.github/workflows/lsp-release.yml` builds native
    `vela_lsp_server` artifacts for Linux, macOS, and Windows, emits SHA-256
    checksum files, uploads workflow artifacts, and publishes tagged `v*`
    releases.
- [x] Keep feature behavior out of editor-specific plugins.
  - The VS Code and Zed packages only contribute launcher/configuration
    metadata; package validators assert that the editor integrations do not
    implement LSP request behavior.

Tests:

- [x] `lsp_server_stdio_smoke_test`
- [x] `editor_config_maps_to_workspace_config`
- [x] `lsp_workspace_configuration_request_updates_workspace_config`
- [x] `cli_config_flags_parse_roots_and_schema`
- [x] `cli_config_flags_seed_workspace_config`
- [x] `server_info_reports_version`

Validation:

```bash
cargo test -p vela_lsp_server
node editors/vscode/scripts/validate-package.js
node editors/zed/scripts/validate-package.js
```

---

## 22. Phase 18: Full-Capability Validation Gate

Purpose: prove the LSP track is complete enough to run alongside runtime work.

- [x] Run all language-service unit tests.
  - Verified with `cargo test -p vela_language_service`: 146 unit tests and
    doctests passed.
- [x] Run all LSP JSON-RPC fixture tests.
  - Verified with `cargo test -p vela_lsp_server`: 110 library tests, 3
    CLI/main tests, and doctests passed.
- [x] Run parser/HIR/analysis focused tests.
  - Verified with `cargo test -p vela_syntax`, `cargo test -p vela_hir`, and
    `cargo test -p vela_analysis`.
- [x] Run many-file synthetic scale checkpoint approaching one million total
  lines.
  - Verified with `cargo test -p vela_language_service
    million_line_synthetic_workspace_checkpoint -- --ignored`: 1 explicit
    million-line scale checkpoint passed.
- [x] Run full workspace validation.
  - Verified with `cargo fmt --all -- --check`,
    `cargo clippy --workspace --all-targets -- -D warnings`, and
    `cargo test --workspace`.
- [x] Update `docs/progress.md` with completed LSP capability coverage.
- [x] Archive long scale logs only if needed for later audit.
  - No archive needed; the validation summary is recorded in
    `docs/progress.md`.

Post-validation editor use has exposed user-facing authoring gaps that this
gate did not catch. The Phase 18 validation proves the protocol plumbing and
baseline capability coverage that existed at the time, but it is not enough to
call the LSP user-facing complete until the Phase 19 rust-analyzer-aligned
correction slice below passes.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## User-Facing LSP Exit Criteria

The native LSP must not be described as user-facing complete, and M20.5 must
not move from `Active follow-up` to `Complete enough`, until all of these
conditions are true:

- The protocol matrix is complete for every advertised capability in
  [lsp-protocol-test-matrix.md](lsp-protocol-test-matrix.md): lifecycle
  advertisement is pinned, each method has a JSON-RPC fixture, each service
  query has focused editor-neutral tests, applicable syntax dimensions have
  positive and negative coverage, dynamic/missing-schema/parser-recovery/stale
  generation behavior is explicit, unsupported methods are negatively pinned,
  and the relevant focused validation commands pass.
- Phase 19 authoring-core work is complete. The implementation has inspected
  the local rust-analyzer source layout when available, mapped the relevant
  completion and formatting model to Vela, added `CompletionAnalysis` or its
  equivalent service-owned structured context model, and routes completion
  producers through explicit path, type, dot-access, declaration-body,
  call-argument, pattern, statement, expected-type, and expected-name
  contexts.
- Member completion uses one unified source/schema/stdlib/builtin member
  surface for source struct fields, source inherent impl methods, source trait
  methods, schema-backed members, and builtin value/container methods.
  Typed receiver `.` requests must not fall back to global completions.
- Completion item rendering keeps symbol identity, filter text, insertion
  text, visible label, label details, owner/module details, docs, ranking,
  snippets, and resolve payloads separate until LSP projection.
- The editor issues that reopened M20.5 have service and LSP regression
  fixtures: compact builtin container type formatting for `Array<i64>`,
  `Set<String>`, `Map<String, i64>`, and
  `Result<Map<String, i64>, String>`; empty-prefix typed `.` completion for
  source/schema/builtin receivers; source impl and trait method completion;
  `struct Player { | }` declaration-body completion; readable short type
  labels with owner details separated from insert text; and `for in` plus
  `match` snippets.
- Negative authoring cases are covered: dynamic `Any` receivers suppress
  guessed members, unknown constructors suppress record fields, struct
  declaration bodies suppress global/value/constructor fallback, malformed
  cursor contexts recover without panics, stale generations are discarded, and
  missing or stale schema facts degrade without inventing host facts.
- Formatting is syntax-owned and idempotent for the Phase 19 type-hint
  examples through full-document, range, and on-type paths where those paths
  claim support. It must not introduce spaces around builtin type arguments or
  type-argument line breaks before an explicit line-width policy exists.
- Thin editor packages remain launch/configuration layers only; no VS Code,
  Zed, or other editor package implements LSP request behavior that belongs in
  `vela_language_service` or `vela_lsp_server`.
- The focused Phase 19 commands pass:

```bash
cargo test -p vela_syntax formatting
cargo test -p vela_language_service formatting
cargo test -p vela_language_service completion_analysis
cargo test -p vela_language_service completion
cargo test -p vela_lsp_server formatting
cargo test -p vela_lsp_server completion
```

- The full workspace gate passes after the focused checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Only after every item above is satisfied may `docs/progress.md` restore a
native LSP "user-facing complete" claim or mark the M20.5 follow-up complete
enough.

---

## 23. Phase 19: Rust-Analyzer-Style Authoring Core Refactor

Purpose: close the real-editor gaps found after the Phase 18 validation pass
by changing the authoring model, not by layering one-off completion and
formatting patches on top of the current coarse contexts. Vela should follow
rust-analyzer's high-level architecture where the syntax overlaps:
syntax-recovered context construction, semantic facts, feature producers,
editor-neutral items, then LSP projection. This does not import Rust-only
semantics such as macros, borrow checking, Rust trait solving, or
script-language generics.

Authoring-core tasks:

- [x] Use rust-analyzer's source layout as the reference model for the audit,
  without copying Rust-only semantics:
  `crates/ide-completion/src/lib.rs` for completion entry shape,
  `crates/ide-completion/src/context.rs` for structured context fields,
  `crates/ide-completion/src/context/analysis.rs` for expected type/name
  analysis, `crates/ide-completion/src/completions/dot.rs` for dot/member
  producers, and `crates/rust-analyzer/src/handlers/request.rs` for the LSP
  formatting boundary that delegates Rust formatting to rustfmt.
- [x] Add a short source audit note in this plan or a linked design note that
  maps rust-analyzer's completion shape to Vela's model:
  context construction, dot completion, expected type/name, item rendering,
  and rustfmt delegation versus Vela's own formatter.

Audit note:

- rust-analyzer's completion entry point builds a request-local
  `CompletionContext` plus `CompletionAnalysis`, then runs feature producers
  over structured contexts rather than letting each producer rediscover the
  syntax shape. Vela mirrors that with `CompletionAnalysis` built from
  `CursorContext`, HIR/module facts, TypeFacts, schema facts, and visible scope
  before completion dispatch.
- rust-analyzer's dot completion consumes a `DotAccess` carrying receiver type
  facts and keeps fields/methods in one producer path. Vela mirrors the shape
  with `DotAccess { receiver_range, receiver_fact }` and keeps dynamic `Any`
  receivers from falling back to globals.
- rust-analyzer's expected type/name analysis is stored beside the completion
  context. Vela records call-argument `expected_type` and `expected_name` from
  callable facts so later producers can rank and filter without ad hoc string
  scans.
- rust-analyzer separates completion item rendering and LSP projection.
  Vela keeps label, lookup/filter text, insertion text, label details,
  documentation resolve payloads, and relevance as service fields before
  native LSP conversion.
- rust-analyzer delegates formatting at the LSP boundary to rustfmt. Vela
  cannot delegate to rustfmt, so formatting remains inside the syntax-owned
  service boundary and should move toward CST/AST layout facts for type hints
  and declarations.
- [x] Introduce a service-owned `CompletionAnalysis` model that is built once
  per completion request from parser recovery, HIR/module facts, TypeFacts,
  schema facts, and visible scope.
- [x] Represent explicit authoring contexts instead of a single broad
  completion kind:
  `PathCompletionCtx`, `TypeLocation`, `DotAccess`, `RecordFieldContext`,
  `CallArgumentContext`, `PatternContext`, `StatementContext`,
  `expected_type`, and `expected_name`.
- [x] Route `textDocument/completion` through `CompletionAnalysis` before any
  feature producer runs. Feature producers may consume structured context and
  semantic facts, but must not reclassify broad request kind through ad hoc
  string scanning.
- [x] Add a unified `MemberCompletionIndex` for source-owned struct fields,
  source inherent impl methods, source trait methods, schema-backed fields and
  methods, and stdlib/builtin value/container methods.
- [x] Keep completion identity, filtering, labels, insertion text, details,
  docs, snippets, and ranking as separate service-item fields before protocol
  projection.
- [x] Move formatter follow-up from token-only whitespace decisions toward
  syntax-owned AST/CST layout facts for declarations and type hints. The first
  required slice is compact builtin container type-argument layout.

Behavior closure tasks:

- [x] Add golden formatter fixtures for the container type hint examples that
  currently format incorrectly:

```vela
fn load_rewards(rewards: Map<String, i64>) -> Result<Map<String, i64>, String> {
    return result::ok(rewards);
}

fn main() {
    let scores: Array<i64> = [1, 2, 3];
    let rewards: Map<String, i64> = {
        "xp": 5
    };
    let tags: Set<String> = set::from_array(["daily", "vip"]);
    return score(scores, rewards, tags).unwrap_or(0);
}
```

- [x] Fix `vela_syntax::formatting` so type arguments have no spaces around
  `<` or `>`, exactly one space after commas, no formatter-created type
  argument line breaks without an explicit line-width policy, and idempotent
  output through full-document, range, and on-type paths.
- [x] Extend completion context extraction so typed member access after `.`
  works with an empty prefix and never falls back to globals.
- [x] Unify member completion facts for source-owned struct fields, inherent
  impl methods, trait methods, schema-backed fields/methods, and builtin
  value/container methods.
- [x] Split completion display from insertion for source and schema types:
  `label` and inserted text stay as the short visible name, while module or
  owner path is projected through `labelDetails`, `detail`, or documentation.
- [x] Add a struct-field declaration completion context for
  `struct Player { | }`, with field snippets and type-hint completion after
  `:`, and suppress expression/global/constructor fallback in that context.
- [x] Add statement-position snippets for `for in` and `match`. Known-enum
  match-arm expansion remains a code action; completion should provide the
  skeleton only.
- [x] Add native LSP JSON-RPC fixtures mirroring the service tests for each
  correction so editor protocol projection cannot regress independently.

Tests:

- [x] `completion_analysis_classifies_empty_dot_access`
- [x] `completion_analysis_classifies_type_argument_location`
- [x] `completion_analysis_classifies_struct_field_declaration_body`
- [x] `completion_analysis_tracks_expected_type_and_name`
- [x] `member_completion_index_unifies_source_schema_trait_and_builtin_members`
- [x] `completion_item_keeps_rendering_projection_fields_separate`
- [x] `formatting_compacts_builtin_container_type_arguments`
- [x] `formatting_compacts_nested_result_container_type_arguments`
- [x] `formatting_preserves_container_type_arguments_on_one_line`
- [x] `formatting_formats_container_type_hint_example`
- [x] `lsp_document_formatting_compacts_container_type_arguments`
- [x] `lsp_document_formatting_formats_container_type_hint_example`
- [x] `member_completion_triggers_after_dot_with_empty_prefix`
- [x] `member_completion_includes_builtin_container_methods`
- [x] `member_completion_includes_source_impl_and_trait_methods`
- [x] `member_completion_uses_source_function_return_receiver_facts`
- [x] `member_completion_uses_source_method_return_receiver_facts`
- [x] `member_completion_uses_schema_method_return_receiver_facts`
- [x] `member_completion_uses_schema_trait_method_return_receiver_facts`
- [x] `lsp_member_completion_includes_source_and_builtin_methods`
- [x] `lsp_member_completion_uses_source_function_return_receiver_facts`
- [x] `lsp_member_completion_uses_source_method_return_receiver_facts`
- [x] `lsp_member_completion_uses_schema_method_return_receiver_facts`
- [x] `lsp_member_completion_uses_schema_trait_method_return_receiver_facts`
- [x] `completion_uses_short_type_labels_with_owner_details`
- [x] `lsp_completion_uses_short_type_labels_with_owner_details`
- [x] `struct_body_completion_enters_field_declaration_context`
- [x] `lsp_struct_body_completion_enters_field_declaration_context`
- [x] `statement_completion_offers_for_in_and_match_snippets`
- [x] `lsp_statement_completion_offers_for_in_and_match_snippets`

Validation:

```bash
cargo test -p vela_syntax formatting
cargo test -p vela_language_service formatting
cargo test -p vela_language_service completion_analysis
cargo test -p vela_language_service completion
cargo test -p vela_lsp_server formatting
cargo test -p vela_lsp_server completion
```

Run the full workspace validation before closing Phase 19 or restoring any
"user-facing complete" claim for the native LSP:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 24. First Executable Task

```text
Task: Implement the `vela_language_service` workspace skeleton.
Context: This starts the full native LSP capability track. The relevant crates
are `vela_common`, `vela_syntax`, `vela_hir`, and the new
`vela_language_service`.
Expected behavior:
  - open_document stores an overlay with a version and line index.
  - change_document replaces the overlay text and advances the workspace
    generation.
  - close_document removes the overlay while keeping any disk snapshot.
  - document_text and position conversion are queryable without LSP types.
Tests:
  - open_document_creates_overlay
  - change_document_updates_version_and_generation
  - close_document_preserves_disk_snapshot
  - line_index_maps_offsets_and_positions
Do not change:
  - Do not add LSP protocol types to the language-service API.
  - Do not change parser, HIR, compiler, VM, HostAccess, or runtime semantics.
  - Do not add WASM-specific constraints.
Validation:
  cargo test -p vela_language_service workspace
```

---

## 25. Checkpoint Rules

- Mark each task checkbox only after focused tests and validation pass.
- Commit small verified checkpoints with Conventional Commit messages.
- Update `docs/progress.md` when LSP capability coverage or milestone status
  changes.
- Update `docs/decisions.md` for architecture decisions that change the
  boundary, release model, schema artifact contract, feature scope, rename
  policy, or formatter policy.
- Keep ordinary source files under 1200 lines by splitting project, source,
  diagnostics, query, index, formatting, refactor, and protocol modules by
  responsibility.
- Prefer clean replacement over compatibility shims while the LSP architecture
  is pre-release.
