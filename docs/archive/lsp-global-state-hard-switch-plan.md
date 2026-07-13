# LSP GlobalState Single-Owner Hard-Switch Plan

> Status: completed on 2026-07-12
>
> Scope: `crates/vela_lsp_server`, its tests, and the active LSP architecture
> documentation
>
> Policy: this is a breaking internal refactor. Do not preserve `LspServer`,
> state mirroring, or legacy test-dispatch APIs for compatibility.

---

## 0. Codex Goal

Use this prompt to execute the hard switch:

```text
/goal Execute docs/lsp-global-state-hard-switch-plan.md end to end as one
atomic breaking cutover. Treat docs/goal.md as the product roadmap,
docs/architecture.md and docs/architecture/lsp.md as the architecture contract,
docs/decisions.md as durable design decisions, docs/progress.md as the rolling
status source, and this plan as the complete execution and acceptance contract.

Begin with the read-only Phase 0 inventory and baseline. Once implementation
edits begin, keep the entire ownership move in one uncommitted cutover across
turns and resumes. Inspect and continue the existing dirty worktree on every
resume. Do not create per-phase or small compatibility-preserving commits, do
not reset partial work, and do not stop after adding a typed test harness,
moving one state category, migrating only some tests, or making a focused test
subset pass. Intermediate compilation failures are allowed while production
state, handlers, tests, and modules move together.

Make GlobalState the sole mutable LSP coordinator and sole owner of live
workspace, language-service databases, disk sources, configuration,
diagnostics, capabilities, watcher, lifecycle, request, cancellation, reload,
task, and outbound-message state. Keep GlobalStateSnapshot immutable and bound
to one authoritative generation. Split production logic by responsibility
instead of moving the legacy implementation wholesale into global_state.rs.

Add one typed in-memory TestServer harness that exercises the same lifecycle
gates, request queue, handlers::dispatch, task/result path, and response
emission as the production main loop. Migrate every legacy LSP test family to
that harness without weakening semantic or protocol assertions. Then delete
LspServer, its test-only dispatcher and local JSON parameter model, all manual
state synchronization, all mirrored fields, obsolete legacy-only modules and
helpers, and the oversized inline test block. Do not add traits, adapters,
aliases, feature gates, dual writes, fallback dispatch, or renamed wrappers to
keep the old and new architectures alive together.

Preserve typed stdio and loopback TCP behavior, lifecycle and invalid-message
semantics, cancellation and stale-generation handling, UTF-16/CRLF document
edits, overlay and watched-file behavior, configuration precedence, schema
reload, diagnostics and progress, all editor feature results, panic/error
projection, profiling, and tracing. Keep editor tooling analysis-only and do
not change Vela language or runtime semantics.

After the legacy half is deleted, run every focused, transport, zero-hit,
file-size, Clippy, workspace, and examples validation gate in this plan. Update
docs/architecture/lsp.md, docs/decisions.md, docs/progress.md, architecture/CI
guards, and this plan so documentation and implementation describe the same
final state. Create exactly one implementation commit after all completion
criteria pass: `refactor(lsp)!: hard switch to GlobalState ownership`. Do not
mark the goal complete while any LspServer, self.server delegation, legacy sync
helper, legacy test dispatcher, mirrored live state, oversized unreviewed LSP
file, unchecked plan item, or failing validation remains.
```

---

## 1. Objective

Finish the typed LSP migration by making `GlobalState` the only mutable server
coordinator and the only owner of live workspace/configuration/diagnostic
state. Remove the legacy `LspServer` implementation, all manual state
synchronization, and the legacy test harness.

The finished production path must be:

```text
stdio or loopback TCP
  -> lsp_server::Message
  -> main_loop
  -> typed request/notification dispatch
  -> one mutable GlobalState
  -> immutable GlobalStateSnapshot for read-only work
  -> vela_language_service
  -> typed LSP projection and response
```

There must be no second mutable server object behind `GlobalState` and no test
path that implements a separate request dispatcher.

## Atomic Execution Mode

This plan is not a sequence of compatibility-preserving commits. The numbered
phases below are an implementation checklist and dependency order inside one
hard-switch batch. They are not mergeable checkpoints.

Execution rules:

1. Phase 0 is read-only inventory and baseline measurement.
2. After implementation begins, keep the work in one uncommitted cutover until
   production ownership, typed tests, legacy deletion, documentation, and
   architecture gates all reach the final shape.
3. The worktree may temporarily fail to compile while fields, handlers, and
   tests move together. Do not add adapters or dual-write compatibility code
   merely to keep an intermediate state green.
4. Do not commit a state containing both the new authoritative owner and a
   functioning legacy `LspServer` path.
5. Do not stop after introducing `TestServer`, moving one state category, or
   migrating only some test families. Those are internal work steps, not
   deliverables.
6. Run focused compiler/tests opportunistically to diagnose the cutover, but
   require the validation gates only after the legacy half has been deleted.
7. Produce one coherent breaking implementation commit after all completion
   criteria and zero-hit gates pass. Documentation and CI enforcement land in
   that same commit so no committed revision claims an architecture it does
   not have.

This plan deliberately trades intermediate compatibility for a shorter,
clearer migration. Git history already preserves the old implementation; the
source tree does not need to preserve it in parallel.

## 2. Current Problem

The typed production entry already runs through `main.rs`, `main_loop.rs`,
typed dispatch, and `GlobalState`, but `GlobalState` still owns a legacy
`LspServer`:

```rust
pub(crate) struct GlobalState {
    // typed coordinator fields
    server: LspServer,
    workspace_snapshot: WorkspaceSnapshot,
    databases: LanguageServiceDatabases,
    workspace_roots: BTreeSet<String>,
    open_documents: BTreeSet<DocumentId>,
    // duplicated config, capabilities, watcher, and lifecycle state
}
```

The current implementation:

- constructs `GlobalState` by cloning fields out of `LspServer`;
- delegates config, workspace mutation, reload, and diagnostics back to
  `LspServer`;
- mirrors lifecycle, capability, watcher, root, document, and config fields;
- refreshes workspace snapshots and databases through manual sync helpers;
- keeps most protocol feature tests on `LspServer` rather than the production
  typed dispatcher;
- keeps roughly 2,450 lines of inline `global_state.rs` tests in the same file
  as roughly 1,350 lines of production code.

This creates two possible views of live state. A new mutation path can update
the legacy wrapper but not `GlobalState`, or update `GlobalState` but not the
legacy wrapper. Snapshot queries, diagnostics, watcher registration, and
lifecycle gates can then observe different state.

## 3. Target Ownership Model

### 3.1 Mutable ownership

`GlobalState` owns each mutable category exactly once:

| State | Final owner |
|---|---|
| lifecycle flags | `GlobalState` |
| request queue and cancellation | `GlobalState` |
| task/reload schedulers | `GlobalState` |
| workspace overlays and source versions | `GlobalState` |
| language-service databases | `GlobalState` |
| disk source records | `GlobalState` |
| workspace roots and open documents | `GlobalState` |
| editor/workspace configuration | `GlobalState` |
| config/schema diagnostic tracking | `GlobalState` |
| client capabilities and semantic-token projection | `GlobalState` |
| watcher enablement and registration state | `GlobalState` |
| outbound typed message sender | `GlobalState` |

A focused internal component such as `WorkspaceState` may group workspace,
database, disk-source, and diagnostic fields, but it must be owned only by
`GlobalState`. It must not become another coordinator, contain lifecycle or
request-queue state, or be mirrored into parallel fields.

### 3.2 Snapshot ownership

`GlobalStateSnapshot` is an immutable point-in-time query input. It may clone
snapshot-safe handles and immutable data from `GlobalState`, but it must not:

- contain mutable protocol state;
- be written back into `GlobalState`;
- require synchronization with another live server object;
- provide mutation methods;
- read current state after it has been dispatched to a worker.

Every snapshot must carry one authoritative workspace/database generation.
Mutation and snapshot publication must not expose a half-applied document,
configuration, schema, or reload update.

### 3.3 Focused logic modules

Do not move the legacy `LspServer` implementation wholesale into
`global_state.rs`. Split production responsibilities along existing ownership
boundaries. The final physical layout should resemble:

```text
vela_lsp_server/src/
  lib.rs                         # module index and deliberate public exports
  global_state.rs                # coordinator and snapshot construction
  global_state/
    documents.rs                 # didOpen/didChange/didClose state transitions
    diagnostics.rs               # diagnostic collection/publication
    configuration.rs             # ConfigChange application and precedence
    project_state.rs             # workspace/database/disk/schema state
    tests/
      lifecycle.rs
      documents.rs
      configuration.rs
      reload.rs
      snapshots.rs
  reload.rs                      # scheduling and reload work descriptions
  handlers/dispatch.rs           # typed protocol routing
  main_loop.rs                   # event pump only
  tests/
    support.rs                   # typed in-memory production-path harness
    ...                          # feature families
```

Exact filenames may follow the implementation discovered during migration,
but the boundaries and single-owner rule are mandatory. Ordinary source and
test files must remain below the repository's 1,200-line threshold unless an
exception is explicitly reviewed and documented.

## 4. Hard-Switch Rules

1. Do not introduce a trait abstraction whose only implementations are
   `GlobalState` and `LspServer`.
2. Do not rename `LspServer` to another compatibility wrapper.
3. Do not keep old request/notification helpers as aliases around typed
   dispatch.
4. Do not keep mirrored lifecycle, capability, workspace, config, or document
   fields during the final state.
5. Do not let tests call feature handlers through a path unavailable to the
   production main loop.
6. Do not replace typed `lsp_types` params/results with raw JSON values.
7. Do not change LSP-visible behavior as part of this refactor unless an
   existing behavior is demonstrably inconsistent with the active architecture
   contract. Record any intentional behavior correction separately.
8. Do not broaden TCP exposure, change cancellation semantics, weaken stale
   generation checks, or alter language-service query semantics.
9. Transitional code may exist only as an uncommitted worktree state during the
   atomic cutover. It must not be committed, released, or described as a
   completed checkpoint.

## 5. Behavior Matrix To Preserve

Before deleting legacy code, preserve focused coverage for:

| Area | Required behavior |
|---|---|
| initialize | launch defaults, client options, capabilities, repeated initialize error |
| initialized | watched-file registration happens at most once |
| shutdown/exit | correct request/notification shape and post-shutdown gating |
| cancellation | queued, in-flight, unknown, completed, and reused IDs |
| document sync | open/change/close overlays, versions, ranged UTF-16 edits, CRLF |
| close overlay | disk content reappears after an open overlay closes |
| configuration | launch/editor/`vela.toml` precedence and schema selection |
| workspace folders | add/remove roots and generation updates |
| watched files | coalescing, source/config/schema upsert/remove, open-file protection |
| schema reload | schema diagnostics, config changes, database generation |
| diagnostics | open files, config files, schema files, progress wrapping |
| snapshots | immutable generation-consistent reads and stale-result rejection |
| request families | completion, hover, signature, navigation, references, symbols |
| edit families | formatting, code action, rename, selection, inlay hints |
| semantic features | semantic tokens and call hierarchy |
| transport | stdio and loopback TCP use the same typed main loop |
| errors | invalid params, method not found, panic projection, no-response notifications |
| observability | profiling and trace event identity/timing remain stable |

Tests should compare typed response/notification values at the protocol
boundary. They must not assert internal field mirroring after the hard switch.

## 6. Phase 0: Baseline And Inventory

### Tasks

- [x] Record the current focused and full LSP test baseline.
- [x] Inventory every production `self.server` access in `global_state.rs`.
- [x] Classify each non-test `LspServer` method as configuration, workspace
      mutation, reload, diagnostics, or shared utility.
- [x] Inventory every test file that constructs `LspServer` and group it by
      feature family.
- [x] Identify tests that bypass typed `handlers::dispatch` or the main-loop
      lifecycle gates.
- [x] Freeze the behavior matrix above with focused typed tests where coverage
      currently exists only through the legacy harness.
- [x] Record file sizes for `lib.rs`, `global_state.rs`, and affected test
      families.

### Phase 0 Inventory (2026-07-12)

- Baseline: `cargo test -p vela_lsp_server` passes 530 library tests, 7 binary
  tests, and 1 stdio integration test; `cargo clippy -p vela_lsp_server
  --all-targets -- -D warnings` passes; `cargo test -p
  vela_language_service` passes 520 tests with 2 explicit scale tests ignored.
- Production ownership: `global_state.rs` contains 37 `self.server` accesses.
  Configuration delegates through `apply_config_change`; document/workspace
  mutation delegates through open/change/close and workspace-root updates;
  reload delegates watched source/config/schema upsert and removal; diagnostics
  delegate open/workspace publication; lifecycle and capabilities are mirrored
  by initialize/shutdown/exit/watcher assignments and sync helpers.
- Legacy test surface: 64 files under `src/tests` construct or name
  `LspServer`. They cover every family in Phase 5 and route through the
  test-only `handle_message`, `handle_request`, and `handle_notification`
  dispatcher in `tests.rs`, bypassing the production request queue, task path,
  and `handlers::dispatch`.
- Typed-path coverage already present in `global_state.rs`, `handlers`,
  `transport`, and `main_loop` freezes lifecycle, document sync,
  configuration/reload, diagnostics, cancellation, stale-generation retry and
  rejection, panic projection, stdio, and loopback TCP behavior before the
  ownership move.
- File sizes: `lib.rs` is 935 lines, `global_state.rs` is 3,813 lines, and
  `tests.rs` is 1,241 lines. Additional active files above the 1,200-line limit
  are `lsp/to_proto.rs` (1,619) and `tests/signature.rs` (1,251).

### Baseline commands

```bash
cargo test -p vela_lsp_server
cargo clippy -p vela_lsp_server --all-targets -- -D warnings
cargo test -p vela_language_service

rg -n '\bLspServer\b|self\.server|sync_.*legacy|legacy_server' \
  crates/vela_lsp_server/src --glob '*.rs'
rg -l '\bLspServer\b' crates/vela_lsp_server/src/tests --glob '*.rs'
wc -l crates/vela_lsp_server/src/lib.rs \
  crates/vela_lsp_server/src/global_state.rs
```

### Exit gate

- [x] Every production delegation and every legacy test family has an owner in
      the migration checklist.
- [x] Typed-path tests cover lifecycle, document sync, configuration, reload,
      diagnostics, cancellation, and stale generations before ownership moves.

## 7. Phase 1: Introduce The Typed Production Test Harness

The test harness must exercise the same typed dispatch and state transitions as
stdio/TCP without opening sockets.

### Tasks

- [x] Add one crate-private `TestServer` under `tests/support.rs`.
- [x] Make `TestServer` own a real `GlobalState`, typed message channel, and
      deterministic task draining controls.
- [x] Route request and notification helpers through the same lifecycle gates,
      request queue, `handlers::dispatch`, and response emission used by the
      production main loop.
- [x] Provide typed helpers for initialize, notify, request, drain tasks, and
      collect outbound messages.
- [x] Keep raw JSON only for final protocol-shape assertions or extension
      payloads that have no upstream typed representation.
- [x] Add parity tests proving the harness and the in-memory production main
      loop produce the same initialize, document-sync, feature-request,
      cancellation, shutdown, and exit results.
- [x] Do not model the old `LspServer` API or reuse its name.

### Exit gate

- [x] New tests have no reason to instantiate `LspServer`.
- [x] The typed harness proves it executes production lifecycle and dispatch
      gates rather than calling `GlobalState` feature methods directly.

## 8. Phase 2: Move Workspace And Database Ownership

This phase removes the most dangerous mirrored state first.

### Tasks

- [x] Move the mutable `Workspace` into `GlobalState` or its uniquely owned
      `project_state` component.
- [x] Move `LanguageServiceDatabases` into the same authoritative owner.
- [x] Move disk source records, config diagnostic records, config documents,
      and schema documents out of `LspServer`.
- [x] Build `GlobalStateSnapshot` only from the authoritative workspace and
      databases after a mutation completes.
- [x] Replace persistent `workspace_snapshot` mirroring with either an
      authoritative current snapshot updated at one explicit commit point or
      snapshot-on-demand. Do not retain a second mutable workspace.
- [x] Define one mutation commit helper if needed to advance workspace/database
      generation atomically and invalidate stale tasks.
- [x] Route typed document open/change/close directly to this state.
- [x] Preserve overlay close behavior, source versions, UTF-16 ranged edits,
      and generation increments.
- [x] Delete `sync_workspace_analysis_from_legacy_server` as soon as all its
      callers use the authoritative state.

### Required tests

- [x] open/change/close update one workspace and one database generation;
- [x] snapshots before a mutation remain immutable;
- [x] snapshots after a mutation see the complete update;
- [x] stale worker results are discarded after a document generation change;
- [x] close restores disk-backed content without a transient empty project;
- [x] ranged edits preserve existing UTF-16 and CRLF semantics.

### Exit gate

- [x] `GlobalState` no longer reads workspace or databases through
      `LspServer`.
- [x] No production workspace/database synchronization helper remains.

## 9. Phase 3: Move Configuration, Reload, And Diagnostics

### Configuration tasks

- [x] Move effective editor/workspace configuration ownership into
      `GlobalState`.
- [x] Apply `ConfigChange` directly to the authoritative state.
- [x] Preserve launch defaults, initialize options, editor settings,
      `vela.toml` discovery, and authoritative config precedence.
- [x] Remove copies between `server.config`, `editor_config`,
      `workspace_config`, and workspace roots.

### Reload tasks

- [x] Make `ReloadScheduler` produce work descriptions only.
- [x] Apply watched-file and workspace-root work directly through
      `GlobalState`/project-state mutation methods.
- [x] Move watched source upsert/remove and schema reload logic out of
      `LspServer`.
- [x] Ensure open overlays win over watched disk changes.
- [x] Commit generation changes once per logical reload batch.

### Diagnostic tasks

- [x] Move diagnostic collection and publication to a focused diagnostics
      module operating on explicit `GlobalState`/snapshot inputs.
- [x] Preserve config, schema, project, missing-import, and open-document
      diagnostics.
- [x] Preserve work-done progress wrapping and custom diagnostic error payloads.
- [x] Ensure diagnostic publication cannot refresh or mutate analysis as a
      hidden side effect.

### Exit gate

- [x] Production configuration, reload, and diagnostics no longer call
      `LspServer`.
- [x] Workspace roots, open documents, config, watcher state, and diagnostic
      state each have one owner.

## 10. Phase 4: Remove Lifecycle And Capability Mirroring

### Tasks

- [x] Keep initialized, shutdown, and exited flags only in `GlobalState`.
- [x] Keep work-done progress, watched-file registration, semantic-token
      projection, and watcher enablement only in `GlobalState`.
- [x] Remove all direct assignments to corresponding `LspServer` fields.
- [x] Delete `sync_client_capabilities_to_legacy_server`.
- [x] Delete the test-only `sync_from_legacy_server` helper.
- [x] Rewrite tests that create impossible divergent state by mutating
      `state.server` directly; express those scenarios through typed messages.

### Required tests

- [x] repeated initialize and initialized behavior;
- [x] notification-shaped lifecycle misuse;
- [x] post-shutdown request rejection and final exit;
- [x] watcher registration capability and disable flag combinations;
- [x] semantic-token projection follows initialize capabilities;
- [x] no response is emitted for notification failures.

### Exit gate

- [x] No lifecycle, capability, watcher, root, document, or config field is
      mirrored between objects.
- [x] No function name contains `legacy` or `sync_*server` in production LSP
      code.

## 11. Phase 5: Migrate Legacy Test Families

Migrate tests by coherent family so failures identify missing production-path
behavior. Do not perform one unreviewable global textual replacement.

### Migration order

- [x] lifecycle, initialization, workspace folders, and file watching;
- [x] document sync, close overlay, incremental edits, config, and schema reload;
- [x] completion and completion resolve;
- [x] hover and signature help;
- [x] definition, declaration, type definition, references, and highlights;
- [x] document/workspace symbols, folding, selection, and formatting;
- [x] rename and code actions;
- [x] semantic tokens and inlay hints;
- [x] call hierarchy;
- [x] degradation, cancellation, panic, tracing, and protocol-shape fixtures.

### Per-family rules

- [x] Replace `LspServer::new()` with the typed `TestServer` harness.
- [x] Send typed requests/notifications through production dispatch.
- [x] Assert outbound typed messages and language-service results.
- [x] Remove direct mutation/assertion of `LspServer` private fields.
- [x] Preserve all semantic assertions; do not weaken fixtures to make the
      migration pass.
- [x] Split files that remain above 1,200 lines by feature responsibility.
- [x] Run the focused family tests before moving to the next family.

### Exit gate

```bash
rg -n '\bLspServer\b' crates/vela_lsp_server/src/tests --glob '*.rs'
```

The command must return no matches.

## 12. Phase 6: Delete The Legacy Half

### Tasks

- [x] Delete the `LspServer` struct and implementation from `lib.rs`.
- [x] Delete test-only legacy request dispatch and local JSON parameter structs.
- [x] Delete legacy response/request/notification helper functions that are no
      longer used by the typed harness.
- [x] Delete obsolete `client.rs`, `queries.rs`, or other modules only if their
      remaining contents are exclusively legacy. Move still-valid focused
      utilities to their owning typed modules.
- [x] Remove legacy labels from `architecture_tests.rs`; replace them with
      assertions for the final typed-only module boundary.
- [x] Reduce `lib.rs` to module declarations and deliberate public exports.
- [x] Split `global_state.rs` production logic and inline tests according to the
      target module layout.
- [x] Remove obsolete file-size exceptions if the final files no longer need
      them.

### Mandatory zero-hit gates

```bash
rg -n '\bLspServer\b|server:\s*LspServer|self\.server' \
  crates/vela_lsp_server/src --glob '*.rs'

rg -n 'sync_.*legacy|legacy_server|handle_legacy|legacy.*bridge' \
  crates/vela_lsp_server/src --glob '*.rs'

rg -n 'legacy JSON fixture|legacy lifecycle|legacy query|legacy stdio' \
  crates/vela_lsp_server/src --glob '*.rs'
```

All commands must return no matches.

### Exit gate

- [x] The crate has one typed dispatcher and one mutable `GlobalState` owner.
- [x] Tests exercise that production architecture through the typed harness.
- [x] No compatibility wrapper, alias, duplicated dispatcher, or mirrored state
      remains.

## 13. Phase 7: Architecture And CI Close-Out

### Tasks

- [x] Update `docs/architecture/lsp.md` to describe the implemented ownership
      and snapshot commit model, not a future target.
- [x] Update `docs/decisions.md` with the final single-owner decision and typed
      test-harness rule.
- [x] Compact `docs/progress.md`: remove stale statements that temporary legacy
      synchronization remains and record the hard-switch checkpoint.
- [x] Add an architecture test or CI zero-hit gate preventing `LspServer`,
      legacy sync helpers, and a second test dispatcher from returning.
- [x] Audit `serde_json` use again; allow it only at typed protocol projection,
      extension payload, tracing/profiling, and final shape-test boundaries.
- [x] Audit all active LSP implementation/test files against the 1,200-line
      threshold.

### Exit gate

- [x] Documentation, architecture tests, and implementation describe the same
      ownership model.
- [x] The repository prevents the deleted legacy architecture from returning.

## 14. Validation Gates

The phases are not required to remain independently green. During the atomic
cutover, use focused `cargo check` or test filters only when they help diagnose
the current edit. After all production and test references to the legacy half
have been removed, run the focused gate:

```bash
cargo fmt --all -- --check
cargo clippy -p vela_lsp_server --all-targets -- -D warnings
cargo test -p vela_lsp_server
cargo test -p vela_language_service
```

After the atomic cutover, also run the transport and binary checks:

```bash
cargo test -p vela_lsp_server --test stdio_transport
cargo run -p vela_lsp_server -- --version
```

Run full repository validation before declaring the hard switch complete:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy --manifest-path examples/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path examples/Cargo.toml
```

If the VS Code package is present in the checkout, also run its existing
package validation and release build commands recorded by that package.

### Final Validation (2026-07-12)

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`: passed.
- `cargo test -p vela_lsp_server`: passed with 533 library tests, 7 binary
  tests, 1 stdio integration test, and doctests.
- `cargo test -p vela_language_service`: passed with 520 tests and 2 explicit
  scale tests ignored, plus doctests.
- `cargo test -p vela_lsp_server --test stdio_transport`: passed.
- `cargo run -p vela_lsp_server -- --version`: passed and reported
  `vela_lsp_server 0.1.0`.
- All mandatory legacy zero-hit searches returned no matches; the architecture
  guard, JSON-boundary guard, and 1,200-line file-size guard passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed.
- `cargo clippy --manifest-path examples/Cargo.toml --all-targets -- -D
  warnings`: passed.
- `cargo test --manifest-path examples/Cargo.toml`: passed, including all 30
  runnable example tests.
- `npm run validate` and `npm run package:release` under `editors/vscode`:
  passed and produced the release VSIX.

## 15. Completion Criteria

The plan is complete only when all of the following are true:

- [x] Production stdio and TCP enter the same typed main loop.
- [x] `GlobalState` is the sole mutable protocol coordinator.
- [x] Workspace, databases, disk sources, configuration, diagnostics,
      capabilities, watcher state, and lifecycle state each have one live owner.
- [x] `GlobalStateSnapshot` is immutable and generation-consistent.
- [x] No manual live-state synchronization helper remains.
- [x] `LspServer` no longer exists.
- [x] No legacy test dispatcher or compatibility wrapper remains.
- [x] All LSP feature tests use the typed production-path harness.
- [x] Lifecycle, cancellation, reload, diagnostics, stale generation, panic,
      stdio, and TCP behavior remain covered.
- [x] LSP production and test files satisfy the active file-size policy.
- [x] Focused and full validation gates pass.
- [x] Architecture, decisions, progress, and CI gates reflect the final state.

## 16. Atomic Commit Policy

Do not create per-phase implementation commits. Complete the ownership move,
typed test migration, legacy deletion, file split, documentation update, and CI
gates in one cutover, validate the final tree, then create one commit:

```text
refactor(lsp)!: hard switch to GlobalState ownership
```

The commit body should summarize the removed legacy surface, the new ownership
model, typed test-harness migration, zero-hit results, and validation commands.
No compatibility shim should be added before or after this commit.
