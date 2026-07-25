# Rust/Vela Service Patchability Acceptance

The service patchability completion plan reached P7 acceptance on 2026-07-25.
The accepted boundary keeps unchanged Rust callers and direct Rust defaults,
while sparse Vela Snapshot/Delta generations can orchestrate registered Rust
services and Host operations. Direct, optional, and fallible borrowed returns
are executable for the admitted exact-parameter shapes; unsupported projected
or nested borrowed returns are compile-time errors.

The `service_hotfix_coverage` example is the consolidated executable proof. It
covers RustDefault, sparse Snapshot, two exact-base Deltas, same-generation
nested calls, old-root isolation, stale and ABI-incompatible rejection, folded
Snapshot, conditional rollback, direct/optional/fallible Host returns,
zero-copy Host arguments, call-scoped Host construction and reclamation,
owned/shared collection lowering, and mutable copy-back rejection.

## Final Gate

The following commands passed from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
node editors/vscode/scripts/validate-package.js
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

The architecture size gate also passed after moving signature tests and
service-dispatch schema construction into focused submodules. The
generated-path audit found no selected borrowed-return placeholders, and the
unsupported nested-borrow audit found the intentional compile-fail fixtures.

The website build emitted only Vite's advisory chunk-size warning; it produced
all 147 static pages successfully.
