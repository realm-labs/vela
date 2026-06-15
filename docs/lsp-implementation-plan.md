# Native LSP Implementation Plan

> **Track:** post-MVP editor tooling architecture
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release tooling APIs are allowed. Do
> not preserve temporary single-file, WASM-first, or protocol-coupled internal
> shapes for compatibility. Preserve product contracts: no Rust `&mut`
> exposure, no runtime TypeRegistry mutation, no monkey patching, HostAccess
> safety, reflection permissioning, source-spanned diagnostics, hot-reload
> ABI/schema checks, and no full LSP in the MVP.

---

## 0. Codex Goal

```text
/goal Implement Vela's native-first LSP architecture from
docs/lsp-implementation-plan.md and docs/architecture/lsp.md. Treat
docs/goal.md as the product roadmap, docs/architecture.md and
docs/architecture/*.md as the architecture contract, and docs/progress.md as
the current milestone state. Build a cleanly layered editor tooling system:
native `vela_lsp_server` owns LSP transport, file watching, cancellation, and
editor lifecycle; reusable `vela_language_service` owns virtual workspace
state, module graph snapshots, diagnostics, completion, hover, definitions,
and incremental invalidation; existing `vela_syntax`, `vela_hir`,
`vela_analysis`, and `vela_reflect` remain the semantic source of truth.
Prefer `compile_dir` module-graph semantics with open-document overlays and a
host schema artifact loaded from exported TypeRegistry/RegistryFacts metadata.
Scale toward one-million-line Vela workspaces by avoiding per-keystroke full
project rebuilds, prioritizing open-file queries, using generation-based
cancellation, and adding explicit invalidation indexes. WASM is optional for
browser tooling and must not constrain the native server architecture. Do not
keep compatibility shims for obsolete pre-LSP shapes. Validate each checkpoint
with focused language-service tests, LSP JSON-RPC fixtures, scale-oriented
tests, docs, and the relevant workspace checks. Commit small Conventional
Commit checkpoints.
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

---

## 2. Goals

- Add `vela_language_service` as a reusable, editor-neutral analysis service.
- Add `vela_lsp_server` as the native LSP protocol and platform boundary.
- Use `compile_dir`-style multi-file module graph semantics by default.
- Treat open editor buffers as source overlays that override disk snapshots.
- Load host type and reflection metadata from a static schema artifact.
- Keep absent or stale host schema as a degradable tooling condition, not a
  runtime dependency.
- Support diagnostics, completion, hover, and go-to definition before broader
  IDE features.
- Build toward one-million-line workspaces with explicit source, parse, HIR,
  and analysis databases.
- Use generation IDs and cancellation so stale request results are discarded.
- Keep LSP, editor, filesystem, and watcher types out of the language-service
  API.
- Keep runtime execution, DAP, and live host state separate from editor
  analysis.

---

## 3. Non-Goals

This plan must not:

- Make full LSP support part of the MVP.
- Run Vela programs to answer editor requests.
- Run the Rust host application to discover schema metadata.
- Read or mutate live host state for hovers, completions, or diagnostics.
- Mutate `TypeRegistry` or runtime type structure.
- Add script-language generics or new language semantics for the LSP.
- Make the LSP depend on WASM as its primary release format.
- Preserve old single-file-only analysis paths as compatibility shims.
- Implement rename, code actions, formatting, or full workspace references
  before symbol ownership and reference indexing are explicit.

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

## 5. Phase 1: Language-Service Skeleton

Task: add the reusable language-service crate and in-memory document model.

Expected behavior:

```text
open a document
change a document
close a document
query current document text/version/line index
convert byte offsets to line/column positions
track workspace generation after every mutation
```

Implementation notes:

- Add `crates/vela_language_service`.
- Keep it independent from LSP protocol and filesystem watchers.
- Use service-owned `DocumentId`, `SourceVersion`, and
  `WorkspaceGeneration` newtypes.
- Store open document overlays separately from disk-backed snapshots.
- Start with UTF-8 byte offsets internally and a line index for editor
  position mapping.

Tests:

```text
open_document_creates_overlay
change_document_updates_version_and_generation
close_document_removes_overlay_without_forgetting_disk_snapshot
line_index_maps_offsets_and_positions
```

Validation:

```bash
cargo test -p vela_language_service
```

---

## 6. Phase 2: Project Model And Source Loading

Task: add project configuration, file discovery inputs, and module source
assembly.

Expected behavior:

```text
vela.toml config identifies workspace roots and optional host schema path
compile_dir-style source collection maps files to module paths
single opened file can run as scratch compile_file-style workspace
open-document overlays override disk snapshots
missing imports and missing files produce diagnostics
```

Implementation notes:

- Introduce `WorkspaceConfig`, `WorkspaceRoot`, and `ProjectMode`.
- Keep filesystem discovery in `vela_lsp_server` or a small platform adapter;
  pass discovered file snapshots into `vela_language_service`.
- Use the existing `vela_hir::module_graph::ModuleSource` as the bridge into
  HIR until a more incremental HIR API exists.
- Add declaration/import fingerprints even if the first implementation still
  rebuilds the graph.

Tests:

```text
configured_roots_build_module_paths
open_overlay_wins_over_disk_source
scratch_file_uses_single_file_mode
changed_import_updates_reverse_dependency_index
```

Validation:

```bash
cargo test -p vela_language_service
cargo test -p vela_hir module_graph
```

---

## 7. Phase 3: Diagnostics Query

Task: expose editor-neutral diagnostics for parser, HIR, and analysis errors.

Expected behavior:

```text
diagnostics can be queried for one file
workspace diagnostics can be queried for all known files
diagnostics include source spans, related labels, codes, and severity
syntax errors do not prevent stale or partial semantic information elsewhere
```

Implementation notes:

- Build a `WorkspaceSnapshot` that owns module graph and analysis facts for a
  generation.
- Convert `vela_common::Diagnostic` into a service-owned diagnostic shape.
- Prioritize diagnostics for open files.
- Record whether diagnostics are complete, partial, or stale.

Tests:

```text
syntax_diagnostics_map_to_document_ranges
hir_diagnostics_survive_multi_file_workspace
open_file_diagnostics_are_prioritized
stale_generation_results_are_discardable
```

Validation:

```bash
cargo test -p vela_language_service
cargo test -p vela_analysis diagnostic
```

---

## 8. Phase 4: Native LSP Server

Task: add the native server binary and document-sync loop.

Expected behavior:

```text
initialize returns server capabilities
shutdown and exit complete cleanly
didOpen/didChange/didClose update the language service
publishDiagnostics sends diagnostics for open Vela files
unsupported methods return structured LSP errors
```

Implementation notes:

- Add `crates/vela_lsp_server`.
- Keep the server as a protocol adapter over `vela_language_service`.
- Use LSP incremental sync only after text application is tested; full sync is
  acceptable for the first slice if the service boundary stays clean.
- Add JSON-RPC fixture tests so protocol behavior is not tied to one editor.

Tests:

```text
lsp_initialize_reports_text_document_sync
lsp_did_open_publishes_diagnostics
lsp_did_change_replaces_document_text
lsp_shutdown_exits_without_background_tasks
```

Validation:

```bash
cargo test -p vela_lsp_server
cargo test -p vela_language_service
```

---

## 9. Phase 5: Completion, Hover, And Definition

Task: expose the first interactive editor queries.

Expected behavior:

```text
completion suggests locals, declarations, modules, stdlib APIs, fields, methods
hover reports type facts, docs, effects, origins, and schema metadata
go to definition resolves local and module declarations with source spans
unknown dynamic values degrade to Any instead of failing requests
```

Implementation notes:

- Reuse existing `vela_analysis::completion` and `vela_analysis::hover`
  facts.
- Add cursor-context extraction in the service layer, not the LSP server.
- Keep host schema completions driven by `RegistryFacts`.
- Defer rename and full references until the reference index exists.

Tests:

```text
completion_uses_open_overlay_facts
member_completion_uses_host_schema_facts
hover_degrades_to_any_without_schema
definition_follows_imported_module_declaration
```

Validation:

```bash
cargo test -p vela_language_service completion hover definition
cargo test -p vela_analysis completion hover
```

---

## 10. Phase 6: Host Schema Artifact

Task: define and load a static schema artifact for editor tooling.

Expected behavior:

```text
host project can export TypeRegistry/RegistryFacts metadata to schema JSON
language service can load schema JSON into RegistryFacts
schema diagnostics report missing, stale, or invalid artifacts
completion and hover use loaded host schema metadata
missing schema degrades host facts to Any
```

Implementation notes:

- Prefer a focused schema serialization module over ad hoc JSON in the LSP
  server.
- Store stable IDs, docs, effects, permissions, type hints, and source spans.
- Do not run host code inside the LSP server.
- Do not make schema loading required for syntax or module diagnostics.

Tests:

```text
schema_export_round_trips_registry_facts
invalid_schema_reports_diagnostic
missing_schema_keeps_syntax_diagnostics_available
host_member_completion_uses_schema_artifact
```

Validation:

```bash
cargo test -p vela_language_service schema
cargo test -p vela_reflect
```

---

## 11. Phase 7: Incremental Invalidation And Scale

Task: replace full-workspace recomputation on every edit with explicit
invalidation.

Expected behavior:

```text
editing a function body reparses one file and recomputes impacted open files
editing imports updates reverse dependencies
editing declarations invalidates dependent modules
workspace diagnostics continue in background
completion can use the latest valid snapshot under active edits
```

Implementation notes:

- Add declaration fingerprints, import fingerprints, and file content hashes.
- Maintain reverse dependency indexes by module.
- Split parse, HIR, and analysis caches.
- Use a coordinator for state mutation and worker tasks for analysis.
- Add cancellation/generation checks to long-running work.

Tests:

```text
function_body_edit_does_not_invalidate_unrelated_modules
import_edit_invalidates_reverse_dependencies
stale_background_diagnostics_are_not_published
scale_fixture_avoids_full_rebuild_per_edit
```

Validation:

```bash
cargo test -p vela_language_service incremental
cargo test -p vela_lsp_server cancellation
```

Scale checkpoint:

```text
synthetic workspace approaches one million lines
initial indexing reports timing and memory
single-file edit avoids full project parse and full HIR rebuild
open-file diagnostics remain responsive under background indexing
```

---

## 12. Phase 8: Editor Distribution

Task: package native LSP integration for supported editors.

Expected behavior:

```text
VS Code extension can locate or download native server binary
Zed extension can launch native server binary
manual CLI invocation works over stdio
server version is visible through initialize/serverInfo
configuration supports workspace roots and schema path
```

Implementation notes:

- Keep editor packages thin.
- Use platform-specific binaries for primary desktop editors.
- Keep the language-service crate reusable for future WASM/browser tooling.
- Do not duplicate feature behavior in editor plugins.

Tests:

```text
lsp_server_stdio_smoke_test
editor_config_maps_to_workspace_config
server_info_reports_version
```

Validation:

```bash
cargo test -p vela_lsp_server
```

---

## 13. Phase 9: Advanced Editor Features

Implement only after the base service, schema, and invalidation model are
stable.

Candidate features:

```text
semantic tokens
workspace symbols
find references
rename with conflict checks
code actions from structured diagnostics
inlay hints from stable TypeFacts
formatting, only after syntax trivia/lossless CST policy is stable
```

Each feature needs its own focused task with service-level tests and LSP
fixture coverage. Rename and code actions must not mutate runtime schemas or
invent language semantics.

---

## 14. First Executable Task

```text
Task: Implement the `vela_language_service` workspace skeleton.
Context: This belongs to the post-MVP native LSP plan. The relevant crates are
`vela_common`, `vela_syntax`, `vela_hir`, and the new
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
  cargo test -p vela_language_service
```

---

## 15. Checkpoint Rules

- Commit small verified checkpoints with Conventional Commit messages.
- Update `docs/progress.md` only when LSP work becomes the active milestone or
  milestone status changes.
- Update `docs/decisions.md` for architecture decisions that change the
  boundary, release model, schema artifact contract, or feature scope.
- Keep ordinary source files under 1200 lines by splitting project, source,
  diagnostics, query, and LSP protocol modules by responsibility.
- Prefer clean replacement over compatibility shims while the LSP architecture
  is pre-release.
