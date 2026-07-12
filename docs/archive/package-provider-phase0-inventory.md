# Package And Provider Phase 0 Inventory

Recorded before the package implementation edits on 2026-07-12.

## Baseline

- `cargo fmt --all -- --check`: pass.
- `cargo clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo test --workspace`: one LSP close/restore assertion failed while the
  rest of the observed suites passed. The focused `vela_lsp_server` suite
  subsequently passed, so this was recorded as a transient baseline failure.
- examples Clippy and tests: pass, including 30 runnable examples.
- `cargo bench --workspace --no-run`: pass.

## Legacy Surfaces

- Handwritten manifest parser and `[workspace].roots` model:
  `vela_language_service::project`; final owner `vela_package`, deleted in
  Phase 1.
- Module identity and `ModulePath`-only indexes: `vela_hir::module_graph`;
  `ModulePath` moves to `vela_package` in Phase 1 and indexes become
  `ModuleKey`-based in atomic Phase 2.
- Global `script` stable identities: `vela_def::script` and callers across
  HIR, analysis, bytecode, reflection, hot reload, Engine, and tooling;
  replaced atomically in Phase 2.
- Capability vocabulary: `vela_engine::permission`; final owner
  `vela_common`, moved in Phase 1. Effect conversion remains in Engine until
  package effect validation lands.
- Source and reload front doors: `vela_engine::source`, `vela_engine::reload`,
  and Runtime staging helpers; convenience APIs remain front doors but build
  reserved package graphs in Phase 2, with package requests added in Phase 3.
- Trait, impl, and attributes: HIR owns resolved trait/impl metadata, while
  `HirAttribute` flattened arguments; structured arguments and provider
  validation belong to Phase 4.
- Executable generation metadata: `vela_bytecode::LinkedArtifact` is the sole
  linked owner, `vela_hot_reload` constructs versions/updates from artifacts,
  and Engine Runtime owns current-image handles. Package/provider metadata is
  attached at this boundary in Phases 3-6.

The affected active-file size audit had no new package implementation files and
the pre-existing reviewed exceptions remained unchanged. Dependency review
confirmed language service did not depend on Engine and Engine was the only
production source/link orchestrator.
