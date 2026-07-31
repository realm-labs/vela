# Rust/Vela Interop Hard-Switch Acceptance — 2026-07-31

Status: accepted. E0-E5 are complete with no compatibility path.

## Accepted Outcome

- MIR and bytecode contain no compiler-driven last-use, scope-edge,
  branch-edge, overwrite, or pre-await Host release producer.
- Authored `host::release` remains strict. Authored
  `host::try_release(value) -> bool` releases a live group, returns `false`
  only for a group already released in the same root, and preserves all other
  errors.
- Scoped Views, MutViews, borrowed returns, projected children, and lazy Host
  iterators are nameable explicit resources. Child-before-parent order and
  alias-group invalidation are enforced.
- Every await checks the complete active scoped-resource table before polling,
  including dead locals and ready futures.
- `service::base::*` and `service::pinned::*` are the only Service compiler
  namespaces. They are not values, and the former contextual receiver
  spellings have no aliases.
- Every admitted Service Host signature has typed sync/async Rust default,
  Vela-selected, pinned nested, target-base, and return-restoration paths.
  Non-`'static`, non-`Sync` Host parameters use the reviewed root-local erased
  reborrow boundary without exposing Rust references to Vela.
- Controlled HostAccess adapter receivers continue to use their registered
  synchronous method vtable when no typed receiver lease exists. This does not
  retry permission, alias, parameter-lease, or invocation failures.
- Portable program, Service bundle, and detached Service metadata format
  version 2 reject version 1 before linking, staging, or activation. Rejection
  leaves the active generation unchanged.

## Executable Matrix

| Matrix | Primary proof |
|---|---|
| ER-01–ER-04 | `vela_macros::interop_exports` proves last use, scope exit, branch convergence, and dead locals do not release. |
| ER-05–ER-06 | compiler tests reject discarded and unnameable scoped producers. |
| ER-07–ER-14 | interop export, optional borrowed-return, scoped host, iterator, panic, error, and future-drop tests cover invalidation and teardown. |
| ER-15–ER-21 | strict/idempotent release tests cover true/false results and preservation of non-expiry errors. |
| AW-01–AW-08 | borrowed collection async, optional borrowed-return, Service async, cancellation, and ready/pending tests cover the complete await table. |
| OI-01–OI-09 | ordinary export, Host method, collection, reflection, bindgen, and `interop_round_trip` tests cover Value/Host conversion and write-through. |
| ST-01–ST-15 | Service Rust-default, interop, scoped-host, async, activation, selection, UI, and `service_hard_switch_fixture` tests cover total dispatch and rejection. |
| Artifact rejection | portable bytecode and portable Service activation tests mutate version 2 headers to version 1 and require rejection before publication. |

Complex children yielded by scoped Host iterators have an additional runtime
regression proof: the iterator keeps a shared freeze lease on its source,
children reborrow the source explicitly, and releasing the cursor before a
live child returns `BorrowStillInUse`.

## Representative Fixtures And Benchmarks

- `interop_round_trip` prints
  `interop_round_trip result=6 level=6 recorded=5` after a conditional strict
  release converges through `host::try_release`.
- `service_hard_switch_fixture` prints
  `rust=1711 rule=1741 delta1=1741 delta2=1741 snapshot=1741 vela_methods=3 rollback=5->4`
  while exercising Rust default, Vela patch, typed base, pinned generation,
  exact-base Deltas, folded Snapshot, old roots, and rollback.
- `service_boundary_baseline` contains
  `generated_vela_service_base_dispatch`, `borrowed_return_release`, and
  `borrowed_return_try_release` rows. A quick execution completed all three,
  and the full benchmark target compiled in the workspace bench gate.

## Validation

The following commands passed on Rust 1.97.1
`x86_64-pc-windows-msvc`:

```text
cargo fmt --all -- --check
cargo fmt --manifest-path examples/Cargo.toml -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo doc --workspace --all-features --no-deps
cargo bench --workspace --all-features --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo run --manifest-path examples/Cargo.toml --bin interop_round_trip
cargo run --manifest-path examples/Cargo.toml --bin service_hard_switch_fixture
node editors/vscode/scripts/validate-package.js
(cd editors/tree-sitter-vela && npx --yes tree-sitter-cli@0.25.10 generate)
git diff --exit-code -- editors/tree-sitter-vela/src
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

Focused acceptance also passed for portable artifact rejection, portable
Service activation, optional borrowed returns, explicit Host iterators and
await, complex iterator children, generated adapter dispatch, architecture
file-size policy, and all 634 `vela_engine` library tests.

Structural source audits found no `MirBorrowReleaseSchedule`,
`emit_automatic_release`, `requires_opaque_host_dispatch`, old opaque-base
placeholder, or `host::is_live` intrinsic. Remaining `base.method` text is an
intentional negative binding fixture or unrelated Rust/local collection code;
remaining `services.*` text is ordinary generated Rust API use, not accepted
Vela contextual syntax.

Miri remains unavailable on the installed stable toolchain. The unsafe erased
reborrow boundary is therefore covered by focused type/generation/alias,
lifecycle, async, cancellation, panic, reentry, and source-audit proof rather
than a Miri gate.
