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
Scale toward one-million-line Vela workspaces by avoiding per-keystroke full
project rebuilds, prioritizing open-file queries, using generation-based
cancellation, and adding explicit source/parse/HIR/analysis indexes. The LSP
track may progress in parallel with M19/M20 optimization work because it is
analysis-only and must not change VM semantics. WASM is optional for browser
tooling and must not constrain the native server architecture. Validate each
checkpoint with focused language-service tests, LSP JSON-RPC fixtures,
scale-oriented tests, docs, and relevant workspace checks. Commit small
Conventional Commit checkpoints.
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
- [ ] Build toward one-million-line workspaces with explicit source, parse,
  HIR, and analysis databases.
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
- [x] `scratch_file_uses_single_file_mode`
- [x] `open_overlay_wins_over_disk_source`
- [x] `missing_import_reports_diagnostic`
- [x] `multi_root_config_keeps_module_paths_stable`

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
- [x] Maintain module import and reverse-dependency indexes.
- [x] Reparse changed files without reparsing unrelated files.
- [x] Invalidate HIR and analysis by changed declaration/import fingerprints.
- [x] Prioritize open-file recomputation over workspace background work.
- [x] Add cancellation and stale-generation result handling.

Tests:

- [x] `function_body_edit_does_not_invalidate_unrelated_modules`
- [x] `import_edit_invalidates_reverse_dependencies`
- [x] `declaration_edit_invalidates_dependent_modules`
- [x] `stale_background_diagnostics_are_not_published`
- [x] `cancelled_background_diagnostics_are_not_published`
- [x] `open_file_recomputation_is_scheduled_before_workspace_work`
- [x] `scale_fixture_avoids_full_rebuild_per_edit`
- [x] `larger_synthetic_workspace_reports_indexing_metrics`

Validation:

```bash
cargo test -p vela_language_service incremental
```

Scale checkpoint:

- [~] Synthetic workspace approaches one million lines.
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
- [x] `lsp_did_open_publishes_diagnostics`
- [x] `lsp_did_change_replaces_document_text`
- [x] `lsp_did_change_applies_incremental_text_edit`
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
- [~] Export type, field, method, variant, trait, module, function, docs,
  effect, permission, type-hint, stable-ID, and source-span metadata.
- [x] Load schema artifacts into language-service schema facts.
- [ ] Validate schema version/hash compatibility.
- [~] Report missing, stale, or invalid schema diagnostics.
- [x] Watch schema artifact changes through the LSP server.

Tests:

- [x] `schema_export_round_trips_registry_facts`
- [x] `invalid_schema_reports_diagnostic`
- [x] `schema_watch_publishes_invalid_schema_diagnostic`
- [x] `schema_watch_clears_diagnostic_after_valid_reload`
- [ ] `missing_schema_keeps_syntax_diagnostics_available`
- [ ] `schema_reload_updates_host_member_completion`
- [ ] `schema_source_spans_enable_definition`

Validation:

```bash
cargo test -p vela_language_service schema
cargo test -p vela_reflect
```

---

## 11. Phase 7: Completion And Signature Help

Purpose: make common authoring flows fast and schema-aware.

- [x] Add cursor-context extraction in `vela_language_service`.
- [~] Complete locals, parameters, captures, declarations, modules, imports,
  stdlib APIs, fields, methods, variants, traits, and type hints.
- [ ] Complete named arguments and defaulted parameters.
- [ ] Complete record fields inside known constructors.
- [ ] Complete map literal keys only when appropriate.
- [x] Complete host members from schema facts.
- [~] Add trigger-character behavior for `.`, `::`, `{`, `(`, `,`, and `|`.
  - [x] Advertise trigger characters for the implemented LSP completion request.
- [~] Add signature help for script functions, native functions, methods, and
  callbacks.

Tests:

- [x] `completion_uses_open_overlay_facts`
- [x] `global_completion_uses_schema_facts`
- [x] `lsp_completion_uses_open_overlay_declarations`
- [x] `lsp_completion_uses_loaded_schema_facts`
- [x] `member_completion_uses_host_schema_facts`
- [x] `module_completion_follows_import_context`
- [ ] `record_field_completion_requires_known_type`
- [x] `signature_help_tracks_active_parameter`
- [x] `lsp_signature_help_tracks_active_parameter`

Validation:

```bash
cargo test -p vela_language_service completion signature
cargo test -p vela_analysis completion
```

---

## 12. Phase 8: Hover And Definitions

Purpose: expose semantic facts and navigation.

- [~] Hover locals, parameters, captures, declarations, modules, functions,
  methods, fields, variants, traits, and type hints.
- [~] Include docs, type facts, effects, permissions, origins, and source spans
  where known.
- [ ] Implement go to definition for local bindings.
- [ ] Implement go to definition for imported module declarations.
- [ ] Implement go to definition for schema items with source spans.
- [ ] Implement go to declaration/type definition where LSP clients separate
  those requests.

Tests:

- [x] `hover_degrades_to_any_without_schema`
- [x] `hover_reports_effects_and_permissions`
- [x] `lsp_hover_reports_open_overlay_parameter_fact`
- [ ] `definition_follows_local_binding`
- [ ] `definition_follows_imported_module_declaration`
- [ ] `definition_follows_schema_source_span`

Validation:

```bash
cargo test -p vela_language_service hover definition
cargo test -p vela_analysis hover
```

---

## 13. Phase 9: Document Symbols, Workspace Symbols, Folding, Selection

Purpose: support navigation and outline features.

- [ ] Build document symbols from parsed declarations.
- [ ] Build workspace symbols from module graph declarations and schema facts.
- [ ] Add file/module/class/function/method/field/enum/variant symbol kinds.
- [ ] Add folding ranges for imports, type declarations, impls, functions,
  blocks, match arms, and multiline literals.
- [ ] Add selection ranges from token/expression/statement/item ancestry.

Tests:

- [ ] `document_symbols_include_nested_type_members`
- [ ] `workspace_symbols_include_module_qualified_names`
- [ ] `workspace_symbols_include_schema_items`
- [ ] `folding_ranges_cover_items_and_blocks`
- [ ] `selection_ranges_walk_syntax_ancestors`

Validation:

```bash
cargo test -p vela_language_service symbols folding selection
```

---

## 14. Phase 10: Semantic Tokens

Purpose: provide syntax and semantic highlighting without changing semantics.

- [ ] Implement lexical semantic tokens from tokenizer output.
- [ ] Add resolved token modifiers for declarations, definitions, readonly,
  deprecated, builtin, host, and unresolved symbols.
- [ ] Add token classes for modules, functions, methods, fields, variables,
  parameters, types, traits, enum variants, properties, keywords, numbers,
  strings, bytes, comments, operators, attributes, and macros.
- [ ] Implement full semantic tokens.
- [ ] Implement semantic token delta only after generation-stable token caches
  exist.

Tests:

- [ ] `semantic_tokens_cover_lexical_classes`
- [ ] `semantic_tokens_mark_resolved_symbols`
- [ ] `semantic_tokens_degrade_under_parse_errors`
- [ ] `semantic_token_delta_matches_full_tokens`

Validation:

```bash
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server semantic_tokens
```

---

## 15. Phase 11: References And Call Hierarchy

Purpose: support workspace navigation and prepare rename.

- [ ] Build a reference index from `BindingMap` and module graph use sites.
- [ ] Index local, module, function, method, field, variant, trait, and schema
  references.
- [ ] Track reference kind: read, write, call, type use, import, pattern,
  declaration.
- [ ] Implement `textDocument/references`.
- [ ] Implement `textDocument/documentHighlight`.
- [ ] Implement incoming and outgoing call hierarchy for script functions and
  methods where calls are statically resolved.

Tests:

- [ ] `references_find_local_binding_uses`
- [ ] `references_find_imported_function_uses`
- [ ] `references_find_field_reads_and_writes`
- [ ] `document_highlight_marks_read_write_call`
- [ ] `call_hierarchy_uses_resolved_call_graph`

Validation:

```bash
cargo test -p vela_language_service references
cargo test -p vela_lsp_server references
```

---

## 16. Phase 12: Rename

Purpose: provide safe refactoring without changing runtime contracts.

- [ ] Implement `prepareRename` for local bindings.
- [ ] Implement local rename inside one function body.
- [ ] Implement private module declaration rename.
- [ ] Implement public module declaration rename with import rewrites.
- [ ] Implement field/method/variant rename only when ownership is known and
  source spans are script-owned.
- [ ] Reject host schema rename unless the source is explicitly script-owned.
- [ ] Reject renames that would collide in scope, module exports, trait impls,
  or import aliases.
- [ ] Report hot-reload ABI/schema risk for exported API rename.
- [ ] Return workspace edits with stable text ranges and document versions.

Tests:

- [ ] `prepare_rename_rejects_keywords_and_literals`
- [ ] `local_rename_updates_all_function_uses`
- [ ] `private_function_rename_updates_imports`
- [ ] `public_export_rename_reports_hot_reload_risk`
- [ ] `rename_rejects_scope_collision`
- [ ] `host_schema_rename_is_not_editable`

Validation:

```bash
cargo test -p vela_language_service rename
cargo test -p vela_lsp_server rename
```

---

## 17. Phase 13: Code Actions

Purpose: turn structured diagnostics into safe edits.

- [ ] Add code action data model independent from LSP protocol types.
- [ ] Add typo fixes from candidate diagnostics.
- [ ] Add import insertion for unresolved qualified symbols.
- [ ] Add remove-unused-import action after unused diagnostics exist.
- [ ] Add fill missing match arms when enum facts are known.
- [ ] Add add missing record fields for known constructors.
- [ ] Add convert simple `if` null checks into Option/Result guard idioms only
  if syntax ownership is unambiguous.
- [ ] Add quick-fix tests for range stability under open overlays.

Tests:

- [ ] `code_action_fixes_unknown_field_typo`
- [ ] `code_action_inserts_missing_import`
- [ ] `code_action_fills_enum_match_arms`
- [ ] `code_action_adds_missing_record_fields`
- [ ] `code_action_rejects_ambiguous_dynamic_fix`

Validation:

```bash
cargo test -p vela_language_service code_action
cargo test -p vela_lsp_server code_action
```

---

## 18. Phase 14: Formatting

Purpose: provide deterministic source formatting without losing comments.

- [ ] Decide and document the lossless CST/trivia policy used by formatting.
- [ ] Implement stable token/trivia extraction if current parser data is not
  sufficient.
- [ ] Add formatting IR that preserves comments and blank-line groups.
- [ ] Implement expression formatting.
- [ ] Implement statement and block formatting.
- [ ] Implement item/declaration formatting.
- [ ] Implement range formatting.
- [ ] Implement full document formatting.
- [ ] Implement on-type formatting only after full/range formatting is stable.
- [ ] Add idempotence tests and malformed-source fallback behavior.

Tests:

- [ ] `formatting_preserves_comments`
- [ ] `formatting_is_idempotent`
- [ ] `range_formatting_limits_edits_to_range`
- [ ] `formatting_handles_malformed_source_without_panic`
- [ ] `on_type_formatting_only_edits_current_construct`

Validation:

```bash
cargo test -p vela_language_service formatting
cargo test -p vela_syntax formatting
```

---

## 19. Phase 15: Inlay Hints And Type Hints

Purpose: expose gradual type facts without implying static typing.

- [ ] Add parameter name hints for calls.
- [ ] Add inferred local type hints from stable TypeFacts.
- [ ] Add lambda parameter hints from collection/iterator facts.
- [ ] Add enum variant payload hints.
- [ ] Add host path type hints from schema facts.
- [ ] Suppress hints at dynamic `Any` boundaries.

Tests:

- [ ] `inlay_hints_show_parameter_names`
- [ ] `inlay_hints_show_stable_local_typefacts`
- [ ] `inlay_hints_show_lambda_parameter_facts`
- [ ] `inlay_hints_degrade_to_any_without_schema`

Validation:

```bash
cargo test -p vela_language_service inlay
cargo test -p vela_lsp_server inlay
```

---

## 20. Phase 16: File Watching, Configuration, And Schema Reload

Purpose: make the server robust in real projects.

- [ ] Watch `.vela` sources under configured roots.
- [ ] Watch `vela.toml`.
- [x] Watch host schema artifact.
- [ ] Debounce file events.
- [~] Handle created, changed, deleted, and renamed files.
  - [x] Created and changed `.vela` files update disk snapshots.
  - [x] Deleted `.vela` files remove disk snapshots and republish open diagnostics.
  - [x] Renamed `.vela` files update module paths.
- [x] Rebuild module path index after file moves.
- [x] Surface configuration diagnostics.
- [x] Support workspace folder changes.

Tests:

- [x] `file_create_adds_module`
- [x] `file_delete_reports_removed_imports`
- [x] `file_rename_updates_module_path`
- [x] `invalid_vela_toml_publishes_config_diagnostic`
- [x] `schema_watch_publishes_invalid_schema_diagnostic`
- [x] `schema_watch_clears_diagnostic_after_valid_reload`
- [x] `workspace_folder_change_reindexes_project`

Validation:

```bash
cargo test -p vela_lsp_server file_watching
cargo test -p vela_language_service project
```

---

## 21. Phase 17: Distribution And Editor Integrations

Purpose: package native LSP for real editors while keeping plugins thin.

- [ ] Add stdio server command.
- [ ] Add `--version`, `--stdio`, and config flags.
- [ ] Package VS Code extension as thin launcher/config UI.
- [ ] Package Zed extension as thin launcher/config UI.
- [ ] Document manual setup for editors that can launch generic LSP servers.
- [ ] Add release matrix for Windows, macOS, and Linux binaries.
- [ ] Keep feature behavior out of editor-specific plugins.

Tests:

- [ ] `lsp_server_stdio_smoke_test`
- [ ] `editor_config_maps_to_workspace_config`
- [ ] `server_info_reports_version`

Validation:

```bash
cargo test -p vela_lsp_server
```

---

## 22. Phase 18: Full-Capability Validation Gate

Purpose: prove the LSP track is complete enough to run alongside runtime work.

- [ ] Run all language-service unit tests.
- [ ] Run all LSP JSON-RPC fixture tests.
- [ ] Run parser/HIR/analysis focused tests.
- [ ] Run one-million-line synthetic scale checkpoint.
- [ ] Run full workspace validation.
- [ ] Update `docs/progress.md` with completed LSP capability coverage.
- [ ] Archive long scale logs only if needed for later audit.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 23. First Executable Task

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

## 24. Checkpoint Rules

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
