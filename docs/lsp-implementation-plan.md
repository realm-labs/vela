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
- [x] Maintain module import and reverse-dependency indexes.
- [x] Reparse changed files without reparsing unrelated files.
- [x] Invalidate HIR and analysis by changed declaration/import fingerprints.
- [x] Prioritize open-file recomputation over workspace background work.
- [x] Add cancellation and stale-generation result handling.

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
  - Schema artifacts now accept optional `sourceSpan` metadata for exported
    type, trait, member, variant, method, trait-method, and function facts.
  - Schema artifacts now round-trip optional docs metadata for type, trait,
    field, variant, method, trait-method, and function facts.
- [x] Load schema artifacts into language-service schema facts.
- [x] Validate schema version/hash compatibility.
- [~] Report missing, stale, or invalid schema diagnostics.
- [x] Watch schema artifact changes through the LSP server.

Tests:

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
    calls, and stdlib callback method calls.

Tests:

- [x] `completion_uses_open_overlay_facts`
- [x] `global_completion_uses_schema_facts`
- [x] `lsp_completion_uses_open_overlay_declarations`
- [x] `lsp_completion_uses_loaded_schema_facts`
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
- [x] `signature_help_resolves_schema_method_call`
- [x] `signature_help_resolves_schema_trait_method_call`
- [x] `signature_help_resolves_stdlib_callback_method_call`
- [x] `signature_help_resolves_stdlib_function_call`
- [x] `lsp_signature_help_resolves_script_method_call`
- [x] `lsp_signature_help_resolves_schema_method_call`
- [x] `lsp_signature_help_resolves_schema_trait_method_call`
- [x] `lsp_signature_help_resolves_stdlib_callback_method_call`
- [x] `lsp_signature_help_resolves_stdlib_function_call`

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
  - Initial hover support now covers script parameters/declarations,
    source-owned globals, source-owned struct fields, source-owned method
    declarations plus typed record and trait receiver calls, source-owned
    traits and trait type hints, source-owned enum variants, schema-backed host
    members and trait receiver methods, schema-backed traits and enum variants,
    stdlib functions, stdlib receiver methods, imported module path segments,
    and missing-schema type-hint degradation.
- [~] Include docs, type facts, effects, permissions, origins, and source spans
  where known.
  - Schema-backed hover now surfaces docs copied through the static schema
    artifact for types, fields, variants, methods, trait methods, and
    functions.
- [x] Implement go to definition for local bindings.
- [x] Implement go to definition for imported module declarations.
- [~] Implement go to definition for schema items with source spans.
  - Initial service and LSP definition support schema type, trait, and
    function source spans when the referenced schema `SourceId` exists in the
    current workspace snapshot.
  - Schema-backed field, method, and trait-method member uses now follow
    schema source spans when the referenced schema `SourceId` exists in the
    current workspace snapshot.
  - Schema-backed enum variant qualified paths now follow schema source spans
    when the referenced schema `SourceId` exists in the current workspace
    snapshot.
- [~] Implement go to declaration/type definition where LSP clients separate
  those requests.
  - Initial language-service and LSP support routes `textDocument/declaration`
    and `textDocument/typeDefinition` through the same source/schema-backed
    navigation spans as definition.

Tests:

- [x] `hover_degrades_to_any_without_schema`
- [x] `hover_reports_script_parameter_fact`
- [x] `hover_reports_effects_and_permissions`
- [x] `hover_reports_schema_trait_method_fact`
- [x] `hover_reports_schema_trait_fact`
- [x] `hover_reports_schema_type_field_and_function_docs`
- [x] `hover_reports_schema_enum_variant_fact`
- [x] `hover_reports_source_global_fact`
- [x] `hover_reports_source_struct_field_fact`
- [x] `hover_reports_source_method_fact`
- [x] `hover_reports_source_trait_fact`
- [x] `hover_reports_source_trait_method_docs`
- [x] `hover_reports_source_trait_receiver_method_fact`
- [x] `hover_reports_source_enum_variant_fact`
- [x] `hover_reports_stdlib_function_fact`
- [x] `hover_reports_stdlib_method_fact`
- [x] `hover_reports_imported_module_path_fact`
- [x] `lsp_hover_reports_open_overlay_parameter_fact`
- [x] `lsp_hover_degrades_to_any_without_schema`
- [x] `lsp_hover_reports_effects_and_permissions`
- [x] `lsp_hover_reports_source_global_fact`
- [x] `lsp_hover_reports_imported_module_path_fact`
- [x] `lsp_hover_reports_schema_trait_fact`
- [x] `lsp_hover_reports_schema_trait_method_fact`
- [x] `lsp_hover_reports_schema_enum_variant_fact`
- [x] `lsp_hover_reports_source_struct_field_fact`
- [x] `lsp_hover_reports_source_method_fact`
- [x] `lsp_hover_reports_source_trait_fact`
- [x] `lsp_hover_reports_source_trait_receiver_method_fact`
- [x] `lsp_hover_reports_source_enum_variant_fact`
- [x] `lsp_hover_reports_stdlib_function_fact`
- [x] `lsp_hover_reports_stdlib_method_fact`
- [x] `definition_follows_local_binding`
- [x] `definition_follows_imported_module_declaration`
- [x] `lsp_definition_follows_open_overlay_local_binding`
- [x] `definition_follows_schema_source_span`
- [x] `definition_follows_schema_field_source_span`
- [x] `definition_follows_schema_method_source_span`
- [x] `definition_follows_schema_trait_method_source_span`
- [x] `definition_follows_schema_variant_source_span`
- [x] `lsp_definition_follows_schema_source_span`
- [x] `lsp_definition_follows_schema_field_source_span`
- [x] `lsp_definition_follows_schema_method_source_span`
- [x] `lsp_definition_follows_schema_trait_method_source_span`
- [x] `lsp_definition_follows_schema_variant_source_span`
- [x] `declaration_follows_local_binding`
- [x] `type_definition_follows_schema_source_span`
- [x] `lsp_declaration_follows_open_overlay_local_binding`
- [x] `lsp_declaration_follows_schema_source_span`
- [x] `lsp_declaration_follows_schema_field_source_span`
- [x] `lsp_declaration_follows_schema_method_source_span`
- [x] `lsp_declaration_follows_schema_trait_method_source_span`
- [x] `lsp_declaration_follows_schema_variant_source_span`
- [x] `type_definition_follows_schema_field_source_span`
- [x] `type_definition_follows_schema_method_source_span`
- [x] `type_definition_follows_schema_trait_method_source_span`
- [x] `type_definition_follows_schema_variant_source_span`
- [x] `lsp_type_definition_follows_schema_source_span`
- [x] `lsp_type_definition_follows_schema_field_source_span`
- [x] `lsp_type_definition_follows_schema_method_source_span`
- [x] `lsp_type_definition_follows_schema_trait_method_source_span`
- [x] `lsp_type_definition_follows_schema_variant_source_span`

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
- [x] `lsp_workspace_symbols_include_script_and_schema_symbols`
- [x] `lsp_workspace_symbols_include_module_symbols`
- [x] `lsp_workspace_symbols_include_file_symbols`
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
- [~] Add resolved token modifiers for declarations, definitions, readonly,
  deprecated, builtin, host, and unresolved symbols.
  - [x] Add declaration, definition, readonly, and unresolved modifiers for
    script declarations and binding-map resolutions.
  - [x] Add host modifiers for schema-backed member accesses and builtin
    modifiers for stdlib member method accesses.
  - [x] Add host modifiers for schema-backed function calls and builtin
    modifiers for stdlib qualified function calls.
  - [x] Add host modifiers for schema-backed type hints and builtin modifiers
    for builtin type hints.
- [~] Add token classes for modules, functions, methods, fields, variables,
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
  - [x] Add host-modified schema trait receiver method call classification.
- [~] Implement full semantic tokens.
  - [x] Full tokens include lexical classes, comments, resolved script symbols,
    script member declarations, script/schema/stdlib member uses, and
    schema/stdlib function calls, plus schema/builtin type hints.
- [~] Implement semantic token delta only after generation-stable token caches
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
- [x] `semantic_tokens_classify_schema_and_stdlib_member_uses`
- [x] `lsp_semantic_tokens_classify_host_and_builtin_member_uses`
- [x] `semantic_tokens_classify_schema_trait_method_uses_as_host`
- [x] `lsp_semantic_tokens_classify_schema_trait_method_uses_as_host`
- [x] `semantic_tokens_classify_schema_and_stdlib_function_calls`
- [x] `lsp_semantic_tokens_classify_host_and_builtin_function_calls`
- [x] `semantic_tokens_classify_host_and_builtin_type_hints`
- [x] `lsp_semantic_tokens_classify_host_and_builtin_type_hints`
- [x] `semantic_tokens_degrade_under_parse_errors`
- [x] `lsp_semantic_tokens_degrade_under_parse_errors`
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

- [~] Build a reference index from `BindingMap` and module graph use sites.
  - [x] Build initial local binding references from `BindingMap` declarations
    and resolved local expression spans.
  - [x] Build initial script declaration references from resolved
    `BindingMap` declaration uses and module import resolutions.
- [~] Index local, module, function, method, field, variant, trait, and schema
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
- [~] Track reference kind: read, write, call, type use, import, pattern,
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
- [~] Implement `textDocument/references`.
  - [x] Serve local binding references through the native LSP request.
  - [x] Serve imported module path segment references through the native LSP
    request.
  - [x] Serve imported script function references through the native LSP
    request.
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
- [~] Implement `textDocument/documentHighlight`.
  - [x] Serve local declaration/read highlights through the native LSP request.
  - [x] Serve imported module path segment highlights in the active document.
  - [x] Serve imported script function import/read highlights in the active
    document.
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
- [~] Implement incoming and outgoing call hierarchy for script functions and
  methods where calls are statically resolved.
  - [x] Serve initial source-backed script function prepare, incoming, and
    outgoing call hierarchy for statically resolved calls.
  - [x] Serve source-owned inherent script method prepare, incoming, and
    outgoing call hierarchy for typed receiver calls.
  - [x] Serve source-owned trait impl method prepare, incoming, and outgoing
    call hierarchy for typed receiver calls.
  - [x] Serve source-owned trait default/interface method prepare, incoming,
    and default-body outgoing call hierarchy for typed trait receiver calls.
  - [x] Serve schema-backed method and trait-method prepare, incoming, and
    script-caller outgoing call hierarchy for typed receiver calls.

Tests:

- [x] `references_find_local_binding_uses`
- [x] `references_can_exclude_local_declaration`
- [x] `lsp_references_find_local_binding_uses`
- [x] `references_find_imported_module_segments`
- [x] `lsp_references_find_imported_module_segments`
- [x] `references_find_imported_function_uses`
- [x] `lsp_references_find_imported_function_uses`
- [x] `references_find_field_reads_and_writes`
- [x] `lsp_references_find_field_reads_and_writes`
- [x] `references_find_record_constructor_field_labels`
- [x] `lsp_references_find_record_constructor_field_labels`
- [x] `references_find_record_constructor_shorthand_field_labels`
- [x] `lsp_references_find_record_constructor_shorthand_field_labels`
- [x] `references_find_enum_variant_constructors_and_patterns`
- [x] `lsp_references_find_enum_variant_constructors_and_patterns`
- [x] `references_find_enum_record_variant_field_labels_and_patterns`
- [x] `lsp_references_find_enum_record_variant_field_labels_and_patterns`
- [x] `references_find_script_method_calls`
- [x] `lsp_references_find_script_method_calls`
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
- [x] `references_find_schema_trait_method_calls`
- [x] `lsp_references_find_schema_trait_method_calls`
- [x] `references_find_schema_variant_constructors_and_patterns`
- [x] `lsp_references_find_schema_variant_constructors_and_patterns`
- [x] `document_highlight_marks_local_declaration_and_reads`
- [x] `document_highlight_marks_import_and_calls_in_active_document`
- [x] `lsp_document_highlight_marks_local_declaration_and_reads`
- [x] `lsp_document_highlight_marks_import_and_calls_in_active_document`
- [x] `document_highlight_marks_imported_module_segments`
- [x] `lsp_document_highlight_marks_imported_module_segments`
- [x] `document_highlight_marks_read_write_call`
- [x] `lsp_document_highlight_marks_read_write_call`
- [x] `document_highlight_marks_script_method_calls`
- [x] `lsp_document_highlight_marks_script_method_calls`
- [x] `document_highlight_marks_trait_impl_uses`
- [x] `lsp_document_highlight_marks_trait_impl_uses`
- [x] `document_highlight_marks_schema_field_reads_and_writes`
- [x] `lsp_document_highlight_marks_schema_field_reads_and_writes`
- [x] `document_highlight_marks_schema_method_calls`
- [x] `lsp_document_highlight_marks_schema_method_calls`
- [x] `document_highlight_marks_schema_variant_uses`
- [x] `lsp_document_highlight_marks_schema_variant_uses`
- [x] `call_hierarchy_uses_resolved_call_graph`
- [x] `lsp_call_hierarchy_uses_resolved_call_graph`
- [x] `call_hierarchy_uses_resolved_script_method_calls`
- [x] `lsp_call_hierarchy_uses_resolved_script_method_calls`
- [x] `call_hierarchy_uses_resolved_trait_impl_method_calls`
- [x] `lsp_call_hierarchy_uses_resolved_trait_impl_method_calls`
- [x] `call_hierarchy_uses_trait_default_and_interface_methods`
- [x] `lsp_call_hierarchy_uses_trait_default_and_interface_methods`
- [x] `call_hierarchy_uses_schema_method_and_trait_method_calls`
- [x] `lsp_call_hierarchy_uses_schema_method_and_trait_method_calls`

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

- [~] Implement `prepareRename` for local bindings.
  - [x] Prepare local binding rename ranges and placeholders.
  - [x] Reject keywords, literals, and non-local targets.
- [~] Implement local rename inside one function body.
  - [x] Return workspace edits for local declaration and resolved uses.
- [~] Implement private module declaration rename.
  - [x] Rename private value declarations (`const`/`global`) and resolved
    same-workspace uses.
  - [x] Rename private type declarations and type-hint uses once ownership
    spans are indexed.
- [~] Implement public module declaration rename with import rewrites.
  - [x] Rename script function declarations, resolved import path segments,
    and resolved unaliased call sites.
- [x] Implement field/method/variant rename only when ownership is known and
  source spans are script-owned.
  - [x] Rename source-owned private struct fields and typed receiver member
    uses.
  - [x] Rename source-owned private inherent methods and typed receiver member
    calls.
  - [x] Rename source-owned private enum variants, constructor uses, and
    match-pattern uses.
- [x] Reject host schema rename unless the source is explicitly script-owned.
- [~] Rename source-backed schema items only when the schema declaration span
  maps to a workspace source.
  - [x] Rename source-backed schema types plus type-hint uses.
  - [x] Rename source-backed schema functions plus call sites.
  - [x] Rename source-backed schema fields and methods plus typed receiver
    member uses.
  - [x] Rename source-backed schema variants plus constructor and
    match-pattern uses.
- [~] Reject renames that would collide in scope, module exports, trait impls,
  or import aliases.
  - [x] Reject local binding renames that collide with an existing function
    binding.
  - [x] Reject same-module declaration collisions through native LSP rename.
  - [x] Reject imported declaration renames that would collide with an
    existing import alias or import binding.
- [~] Report hot-reload ABI/schema risk for exported API rename.
  - [x] Public script function renames carry hot-reload ABI risk metadata in
    service workspace edits and LSP change annotations.
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
- [x] `private_value_declaration_rename_updates_uses`
- [x] `lsp_private_value_declaration_rename_updates_uses`
- [x] `lsp_private_type_declaration_rename_updates_type_hints`
- [x] `public_export_rename_reports_hot_reload_risk`
- [x] `lsp_public_export_rename_reports_hot_reload_risk`
- [x] `rename_rejects_scope_collision`
- [x] `rename_rejects_module_declaration_collision`
- [x] `lsp_rename_rejects_module_declaration_collision`
- [x] `function_rename_rejects_import_alias_collision`
- [x] `lsp_rename_rejects_import_alias_collision`
- [x] `private_struct_field_rename_updates_member_uses`
- [x] `lsp_private_struct_field_rename_updates_member_uses`
- [x] `private_method_rename_updates_typed_receiver_calls`
- [x] `lsp_private_method_rename_updates_typed_receiver_calls`
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

- [~] Decide and document the lossless CST/trivia policy used by formatting.
  - Current policy: `vela_syntax::formatting` owns stable token/trivia
    extraction and token-driven full-document formatting; richer AST-aware
    formatting rules remain open.
  - Semicolonless `use` item newline boundaries are preserved as syntax-owned
    trivia so imports do not collapse into following items.
- [x] Implement stable token/trivia extraction if current parser data is not
  sufficient.
- [~] Add formatting IR that preserves comments and blank-line groups.
  - Initial editor-neutral IR preserves token/trivia source text, comments,
    shebang trivia, spans, and blank-line whitespace groups.
- [~] Implement expression formatting.
  - Initial token-driven rules normalize operator and delimiter spacing.
- [~] Implement statement and block formatting.
  - Initial token-driven rules indent brace blocks and comment lines.
- [~] Implement item/declaration formatting.
  - Initial token-driven rules indent struct fields, enum variants, trait
    method declarations, impl methods, nested enum record fields, and adjacent
    top-level declarations.
- [~] Implement range formatting.
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
  - Range formatting uses parser-owned item/member spans only after the
    token/trivia formatter has stable comment, blank-line, and import-boundary
    behavior.
- [~] Implement full document formatting.
  - Native LSP full-document formatting now uses the token/trivia formatter
    for spacing, brace indentation, comment preservation, and final newline.
- [~] Implement on-type formatting only after full/range formatting is stable.
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
- [x] `inlay_hints_suppress_any_source_function_and_method_parameters`
- [x] `inlay_hints_suppress_any_enum_variant_payloads`
- [x] `lsp_inlay_hints_show_parameter_names`
- [x] `lsp_inlay_hints_show_source_method_parameter_names`
- [x] `lsp_inlay_hints_show_local_typefacts`
- [x] `lsp_inlay_hints_show_lambda_parameter_facts`
- [x] `lsp_inlay_hints_show_host_path_typefacts`
- [x] `lsp_inlay_hints_show_enum_variant_payload_names`
- [x] `lsp_inlay_hints_degrade_to_any_without_schema`
- [x] `lsp_inlay_hints_suppress_any_schema_function_parameters`
- [x] `lsp_inlay_hints_suppress_any_source_function_and_method_parameters`
- [x] `lsp_inlay_hints_suppress_any_enum_variant_payloads`
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
- [~] Handle created, changed, deleted, and renamed files.
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
