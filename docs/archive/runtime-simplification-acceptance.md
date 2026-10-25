# Runtime Simplification — Removal Acceptance

## Scope And Result

The A–E removal implementation and software gates are complete. This report does
**not** claim full strict/native-editor acceptance, a coverage-guided fuzz
campaign, benchmark performance acceptance, or implementation of deferred opaque
objects, conversion APIs or a new re-entry model.

The supported embedding boundary is existing synchronous owned-value natives and
structural conversions. No independent opaque-object runtime survived D. Rust
may borrow internally during a native invocation; no caller-stack borrow, view,
path, lease or borrowed return becomes a script value.

| Checkpoint | Commit / result |
|---|---|
| A | `ff8163a19` — contracts, deletion dependencies and retained baseline. |
| B | `bc40c9258` — Service/Provider declarations, macros, compilation, dispatch, artifacts, tooling and callers removed. |
| C | `f55b2d770` — detached tasks, async syntax/registration/futures/execution and metadata removed. |
| D | `226900e81` — borrowed roots/returns/views, HostRef/path/lease runtime, dependent macros/registrations and extern bindings removed. |
| E | This checkpoint — JIT-only preparation, dependencies, active contracts, site/editor grammar and consumers cleaned; software and installed-provider partitions verified. |

Retained behavior includes modules/packages/traits, script collections/iterators,
reflection and VM state, GC/budgets, interpreter MIR/liveness/root/cache/physical
plans, CLI/browser/native LSP execution, existing synchronous context re-entry,
and exact old closure/code/state/native-dispatch ownership across reload.

## Safety And Format Proof

- Removed source syntax, special namespaces, declaration attributes, native
  signatures, registration APIs and macros have rejection tests or compile-fail
  doctests/UI cases. Former borrowed type hints reject at the HIR boundary;
  genuinely declared ordinary script types with the same spelling still work.
- Opaque wrappers do not preserve call-scoped borrowing. Dependent constructors,
  methods and storage were removed instead of replaced.
- Native host-read/write effects survive in MIR, observed package capabilities
  and binding ceilings. Regression tests cover declaration/grant separation,
  rejected capability expansion, non-invocation of denied Rust callbacks and
  capability-reducing reload with a retained old closure.
- **Portable program 8**, **Rust-binding schema/checksum 7**, and **tooling schema
  4** reject incompatible formats/removed fields. Portable package metadata is
  still explicitly unsupported; no compatibility decoder was added.
- Restricted JIT input/eligibility and unused future-backend conformance APIs
  are gone. Their generation/layout/analysis/budget proof now uses retained
  interpreter interfaces. `MirBackendHandoff` remains because bytecode compilation
  uses it.
- Reflection's owned primitive/structural carriers and native permissions remain;
  host layout/path recovery and schema mutation do not.

## Final Rust Gates

Final complete workspace command, installed stable Rust **1.99.1**:

```sh
rustup run stable cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=4
```

**4,333 passed, zero failed, two existing ignored, zero filtered out**, including
macro UI, unsafe-boundary audit, VM conformance and doctests. It completed in
965.27 seconds. Earlier broad runs timed out, and the first completed E run
exposed one stale syntax-diagnostic assertion; the assertion was corrected to
require `hir::removed_host_type_hint`, then the complete gate passed.

Also passed:

```sh
rustup run stable cargo fmt --all -- --check
rustup run stable cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
rustup run stable cargo clippy --workspace --all-targets --locked -- -D warnings
rustup run stable cargo doc --workspace --all-features --locked --no-deps
rustup run stable cargo test --manifest-path examples/Cargo.toml --all-features --locked --no-fail-fast
rustup run stable cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features --locked -- -D warnings
rustup run stable cargo check -p vela_playground_wasm --target wasm32-unknown-unknown --release --locked
rustup run stable cargo bench --workspace --no-run --locked
rustup run stable cargo check --manifest-path fuzz/Cargo.toml --bins --all-features --locked
```

Every one of the **14 retained example bins** was also executed successfully.
The schema-mutation rejection example uses `reflect::type_info("i64")`, so it
proves immutable metadata rejection instead of failing on a deleted Host type.

Final stable Clippy required equivalent atomic `try_update` spellings and removal
of five unnecessary closure borrows. The atomic calls were also compile-checked
on 1.98.0. A serde-only test import is now feature-gated; default-feature Clippy
passes as well as the all-feature lane.

Dependency/lock cleanup removed unused edges without package/version/source/
checksum upgrades. Retained default/all-feature builds, examples and fuzz bins
were checked. Fuzz compilation and bounded smoke inputs do not establish a
coverage-guided campaign. `cargo bench --no-run` is build proof, not measurement.

## Site, Grammar And Matrix Gates

Passed: site syntax checks (36 scopes/36 token checks plus removed-keyword
probes), placeholder/reduced-surface documentation checks, Astro build (**145
pages**), VS Code/Zed package validators, Tree-sitter regeneration and corpus
checks, and Zed query compilation. The ignored Zed grammar checkout/local work
was preserved; validation rejects tracked duplicate grammar sources rather than
wiping a user's cache.

Final combined Node command:

```sh
node --test scripts/lsp-matrix/*.test.js editors/vscode/test/symbol-schema.test.js editors/vscode/test/session-paths.test.js editors/vscode/scripts/check-removed-consumers.test.js editors/zed/scripts/*.test.js
```

**321 passed, zero failures/skips.** This includes the explicit inventory
migration, current schema serialization and short private socket-path tests.

The [inventory review](../../tests/lsp_matrix/reviews/checkpoint-e-inventory.md)
and its machine-readable journal record exact retired grammar rules and old/new
contract hashes. All **1,327 semantic / 1,477 execution IDs and owners** remain;
`execution-baseline.json` is unchanged. B02–B08 reopen with original acceptance
snapshots; B00/B01/B09 and unrelated evidence are preserved. The native checkpoint
resumes at B02, with **1,423 execution obligations remaining**. Pending migrated
proofs cannot be promoted from old linked tests.

## Installed VS Code — Bounded Partitions

Installed VS Code **1.137.0**, macOS arm64, private VSIX/extension/user-data
profiles. Two validation defects were fixed without changing retained oracles:

1. Symbol materializers overwrote schema 4 with schema 1. A real stdio comparison
   proved that version alone caused empty hover; current materializers preserve
   authored owned-value facts and omit only unavailable source locations.
2. Deep generated profiles exceeded the macOS IPC socket limit. Short isolated
   `w`/`s`/`l` directories reduced sockets from 119 to 97 bytes. User profiles,
   trust behavior and assertions were not bypassed.

The aggregate `npm test` run timed out after **900 seconds**, with 26 checks
complete and lifecycle unfinished. It is **not** recorded as an exit-zero run.

The lifecycle helper then passed separately in **402.90 seconds**. An attempted
merge with older base evidence correctly rejected changed input hashes. The
main/symbol/syntax base was recaptured under the final matching inputs and
passed in **633.13 seconds**, within its 900-second partition bound. The existing
merge validator then confirmed all **27 declared checks**, each present once,
all passing and with identical input/server/platform/editor provenance:

```text
inputsSha256: 2f6e5d9f954500a42a43c689a007cebefbce1d3338ca504f69d37518b764278a
serverSha256: f38489ef3080edf6242d17c9030df3847a48e58d8f8c42cb1ee5fbe3a4e79fca
platform: darwin
```

Preserved local evidence:

- `editors/vscode/test-results/run-8r_aybxk/merged-results.json`
- `editors/vscode/test-results/run-8r_aybxk/partition-validation.json`
- `editors/vscode/test-results/run-8r_aybxk/partition-harness.js` and `partition-run.log`
- `editors/vscode/test-results/run-X3dDcH/q/results.json` and lifecycle traces

The partition harness is an ignored, recorded invocation of existing suites and
materializers, not changed test assertions or manufactured provenance. Original
runs remain intact. The plain current-editor audit also passed:

```sh
RUSTUP_TOOLCHAIN=stable node scripts/lsp-matrix/run.js --editor-results editors/vscode/test-results/run-8r_aybxk/merged-results.json
```

Report: `target/lsp-matrix/run-fPwLFn/report.md`; **20 verified / 852 mapped /
453 unreviewed / zero failed / two not applicable**. This invocation did not
rerun Rust suites and is not strict acceptance.

## Remaining Acceptance Boundaries

No runtime-removal or software-test failure remains. Full native Input/Render
and strict LSP coverage are a separate retained track with reopened/stale
obligations, not certified by installed-provider partitions. No fresh Windows
or complete physical-input capture is claimed. Historical accepted records do
not establish current-source acceptance.

Deferred opaque-object ownership/storage, new conversions/re-entry, debugger/DAP,
interpreter performance targets and existing language/tooling gaps were not
implemented to complete this removal. Continue from [current progress](../progress.md),
not from removed-feature historical plans.
