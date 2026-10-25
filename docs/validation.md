# Validation

Commands here target the reduced synchronous language after checkpoints A-E.
They are instructions, not claims that gates passed; [progress.md](progress.md)
and the [removal acceptance report](archive/runtime-simplification-acceptance.md)
record actual results and the separate strict/native-editor gaps. No opaque-object replacement, new conversion API or new
re-entry model is required for removal acceptance.

## Fast And Full Workspace Lanes

```bash
cargo test-fast
```

The fast alias skips macro UI compile tests, the workspace unsafe-source audit
and VM conformance fixtures. It does not replace checkpoint gates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo doc --workspace --all-features --no-deps
```

## Retained Baseline And Removal Boundaries

Run the focused retained slice before broad gates:

```bash
cargo test -p vela_engine --lib engine_installs_registered_native_functions_into_vm
cargo test -p vela_engine --lib ordinary_package
cargo test -p vela_engine --lib runtime_state_decl_persists_vm_owned_value_and_rust_updates
cargo test -p vela_engine --test integration args::
cargo test -p vela_engine --test integration hot_reload::retained_
cargo test -p vela_vm --lib execution_core
cargo test -p vela_examples --manifest-path examples/Cargo.toml --test removed_entrypoints
cargo test -p vela_bytecode --features artifact-codec
cargo test -p vela_bindgen
cargo test -p vela_language_service
cargo test -p vela_lsp_server
cargo test -p vela_cli
cargo test -p vela_playground_wasm
cargo check -p vela_playground_wasm --target wasm32-unknown-unknown --release
```

Portable program **8**, Rust-binding schema **7** and tooling schema **4** must
round-trip current retained facts and reject incompatible versions before use,
linking or activation. Keep source/API/macro rejection plus export/dispatch
and artifact audits: a passing retained baseline alone does not prove absence
of Service/Provider, async/tasks, borrowed/path/lease or dependent host bindings.
Native host-read/write effects/permissions, script reflection, GC/budgets,
modules/packages/state and exact old-generation dispatch remain positive proof.

## Runnable Examples

Cargo metadata exposes **14 runnable bins**, plus the example library. Verify
actual targets rather than preserving a removed bin to satisfy a historical
count:

```bash
cargo metadata --manifest-path examples/Cargo.toml --no-deps --format-version 1
```

All currently retained bins are:

```bash
cargo run --manifest-path examples/Cargo.toml --bin native_function
cargo run --manifest-path examples/Cargo.toml --bin serde_value
cargo run --manifest-path examples/Cargo.toml --bin modules
cargo run --manifest-path examples/Cargo.toml --bin package_dependency_demo
cargo run --manifest-path examples/Cargo.toml --bin script_state
cargo run --manifest-path examples/Cargo.toml --bin container_type_hints
cargo run --manifest-path examples/Cargo.toml --bin io_stdlib
cargo run --manifest-path examples/Cargo.toml --bin hot_reload_function_swap
cargo run --manifest-path examples/Cargo.toml --bin hot_reload_function_swap_invalid
cargo run --manifest-path examples/Cargo.toml --bin bad_schema_duplicate_field
cargo run --manifest-path examples/Cargo.toml --bin generic_type_hint_denied
cargo run --manifest-path examples/Cargo.toml --bin random_permission_denied
cargo run --manifest-path examples/Cargo.toml --bin reflect_schema_mutation_denied
cargo run --manifest-path examples/Cargo.toml --bin reflect_unknown_field_denied
```

Expected-error bins assert their rejection and exit successfully. These are
owned-value, script-state, package, reflection and reload examples—not Rust
write-through or replacement-object acceptance demos.

## Benchmarks And Fuzzing

Use [performance.md](performance.md) for live targets and measurement modes.
Build harnesses separately from potentially long measurements:

```bash
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
```

With `cargo-fuzz` installed, select bounded parser/portable-plan runs:

```bash
cargo fuzz run parser -- -max_total_time=30
cargo fuzz run portable_plan -- -max_total_time=30
```

## Documentation, Syntax And Site

For documentation-only changes, check whitespace and local Markdown targets and
heading anchors in affected docs (including links retained in historical files).
Use a local link audit; distinguish pre-existing historical dead code links from
new active broken links. Preserve relative links when moving archive snapshots.
There is currently no repository-wide Markdown formatter/link command; Rust fmt
does not format Markdown or EBNF.

```bash
git diff --check -- AGENTS.md docs
npm --prefix site run test:docs
npm --prefix site run test:syntax
npm --prefix site run build
```

The site's `test:docs` checks site placeholders and reduced-surface claims,
not `docs/**` links. A passing
site check is not full documentation link acceptance. For changed editor syntax
or package surfaces also run:

```bash
node editors/vscode/scripts/validate-package.js
(cd editors/tree-sitter-vela && npx --yes tree-sitter-cli@0.25.10 generate)
git diff --exit-code -- editors/tree-sitter-vela/src
```

Grammar changes require the owning matrix inventory/fingerprint review; updating
a hash is not test evidence. The reviewed
[E inventory migration](../tests/lsp_matrix/reviews/checkpoint-e-inventory.md)
records retired rules, exact old/new contract hashes and reopened evidence while
preserving the immutable execution baseline. Inventory validity does not mean
strict/native acceptance.

## Native LSP And Installed Editors

```bash
node --test "scripts/lsp-matrix/*.test.js"
node scripts/lsp-matrix/run.js --run
npm --prefix editors/vscode ci
npm --prefix editors/vscode test
```

Linux CI uses `xvfb-run -a npm --prefix editors/vscode test`. The
[editor guide](../editors/vscode/README.md#automated-editor-tests) owns version
profiles, bounded assertions and retained logs. `--strict` is full matrix
acceptance and remains red while required cells lack reviewed current evidence;
see [the strategy](lsp-test-strategy.md).

If the complete installed-editor command reaches its finite timeout, bounded
partitions may establish its individual checks only when the existing merge
validator confirms identical input/server/platform provenance and exactly one
passing result for every declared check. Preserve raw runs and report the
aggregate timeout; do not claim an exit-zero aggregate run or strict matrix
acceptance from partitions.

Provider/command evidence does not establish actual native input/render proof.
Recapture affected removed-feature/schema/grammar evidence without invalidating
unrelated retained LSP work. Do not claim full native-editor acceptance from
backend tests, a site build, historical records or this consolidation.
