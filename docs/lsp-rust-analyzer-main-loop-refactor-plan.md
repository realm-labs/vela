# Rust-Analyzer-Style LSP Main Loop Refactor Plan

> **Track:** native LSP protocol and main-loop architecture cleanup
> before further editor-behavior debugging
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release `vela_lsp_server`
> internals are allowed. Do not preserve the current hand-written JSON-RPC,
> stdio framing, protocol params/results, or synchronous `match method`
> dispatcher as compatibility shims. Preserve product contracts:
> analysis-only editor tooling, no runtime script execution for LSP queries,
> no live host-state reads, no `TypeRegistry` mutation, no Rust `&mut`
> exposure, no script-language generics, no monkey patching, HostAccess
> safety, reflection permissioning, source-spanned diagnostics, hot-reload
> ABI/schema checks, and no editor feature that changes language or runtime
> semantics.

---

## 0. Codex Goal

```text
/goal Execute the complete rust-analyzer-style LSP main-loop refactor plan in
docs/lsp-rust-analyzer-main-loop-refactor-plan.md from the first unchecked
phase/task through final acceptance. This goal is complete only when every
phase checklist item in this execution document and every acceptance criterion
in Section 7 is complete and validated; it is not complete after adding
`lsp-server`, after migrating initialize alone, after adding a task pool alone,
or after any single checkpoint. On each turn or resume, read docs/goal.md,
docs/architecture.md, docs/architecture/*.md, docs/architecture/lsp.md,
docs/lsp-implementation-plan.md, docs/lsp-clean-architecture-refactor-plan.md,
docs/progress.md, docs/decisions.md, and this execution document, inspect the
current git diff, then choose the smallest verifiable task that advances the
earliest incomplete phase. Implement that task, validate it with the focused
tests named in this document plus any relevant workspace checks, update this
plan's checklist/progress notes and durable docs when status or decisions
change, commit a small Conventional Commit checkpoint, and continue to the next
incomplete task rather than shrinking the goal to the checkpoint just finished.
Use ~/CLionProjects/rust-analyzer as the local rust-analyzer reference root for
architecture comparison. The most relevant reference files are
crates/rust-analyzer/src/bin/main.rs,
crates/rust-analyzer/src/main_loop.rs,
crates/rust-analyzer/src/global_state.rs,
crates/rust-analyzer/src/handlers/dispatch.rs,
crates/rust-analyzer/src/handlers/request.rs,
crates/rust-analyzer/src/handlers/notification.rs,
crates/rust-analyzer/src/lsp/to_proto.rs, and
crates/rust-analyzer/src/lsp/from_proto.rs. Borrow the protocol and editor
server architecture model, not Rust-only semantics: do not add macro
expansion, borrow checking, Rust trait solving, proc macros, Cargo project
modeling, flycheck, or script-language generics to Vela. Preserve the existing
`vela_language_service` boundary as the editor-neutral analysis surface and
keep editor packages thin launchers. If a real external decision blocks
progress, update docs/blocked.md and leave the goal active or blocked
explicitly; otherwise keep advancing the next unchecked task until the entire
plan is complete.
```

---

## 1. Purpose

The native LSP now has broad feature coverage, but its protocol boundary is
still hand-written. `vela_lsp_server` currently owns custom JSON-RPC request
types, custom response envelopes, custom stdio Content-Length framing, custom
params/result structs, and a synchronous `match method` dispatcher. That shape
made early vertical progress easy, but it now creates protocol-consistency and
debuggability risk.

The visible VS Code stalls should not be debugged only by guessing individual
slow handlers. First align the server architecture with the model used by
rust-analyzer: a typed LSP transport, a main loop with explicit mutable global
state, immutable request snapshots, typed request/notification dispatch,
request queues, cancellation, task scheduling lanes, and protocol conversion
modules. After that, profiling and user-facing LSP behavior can be evaluated
against a cleaner server model.

This plan is an LSP-server architecture cleanup track, not a language-service
semantic rewrite. Existing `vela_language_service` query APIs remain the
semantic source for completion, diagnostics, hover, definitions, symbols,
references, rename, code actions, formatting, semantic tokens, and inlay hints.

---

## 2. Goals

- [ ] Replace production hand-written JSON-RPC and stdio framing with
  `lsp-server`.
- [ ] Replace production hand-written protocol params/results with
  `lsp-types` where the upstream protocol type exists.
- [ ] Introduce a rust-analyzer-style `GlobalState` as the only owner of
  mutable server state.
- [ ] Introduce `GlobalStateSnapshot` for read-only request handlers.
- [ ] Introduce typed `RequestDispatcher` and `NotificationDispatcher`.
- [ ] Split request execution into main-thread mutable handlers, latency
  handlers, formatting handlers, and worker handlers.
- [ ] Track incoming request state in a request queue.
- [ ] Implement cancellation through request IDs and service generation tokens.
- [ ] Implement stale-result handling with retry for retryable requests and
  `ContentModified` for non-retryable requests.
- [ ] Rebuild request profiling at the main-loop/task boundary so logs show
  received, queued, task-started, task-ended, responded, stale, retried, and
  cancelled states.
- [ ] Preserve current VS Code and Zed packages as thin launchers around the
  native server.
- [ ] Keep `vela_language_service` free of LSP, editor, filesystem watcher,
  and process transport types.

---

## 3. Non-Goals

- [ ] Do not rewrite `vela_language_service` semantics as part of this track.
- [ ] Do not introduce Salsa as a requirement for this refactor.
- [ ] Do not add Rust macro expansion, borrow checking, Rust trait solving,
  proc macros, Cargo project discovery, or flycheck.
- [ ] Do not change parser, HIR, compiler, VM, HostAccess, reflection, hot
  reload, or runtime semantics.
- [ ] Do not add a custom full IDE product.
- [ ] Do not make browser/WASM tooling shape the native server architecture.
- [ ] Do not keep legacy protocol structs or response envelopes in production
  paths once their typed replacements are verified.

---

## 4. rust-analyzer References To Borrow

Use the local checkout as the source reference root:

```text
~/CLionProjects/rust-analyzer
```

Borrow these architecture ideas:

- `crates/rust-analyzer/src/bin/main.rs`: stdio connection startup through
  `lsp_server::Connection`, initialize handshake, and main-loop entry.
- `crates/rust-analyzer/src/main_loop.rs`: event loop, LSP message routing,
  task result collection, request registration, shutdown handling, and typed
  request/notification dispatch.
- `crates/rust-analyzer/src/global_state.rs`: central mutable server state,
  request queue, snapshot creation, response sending, cancellation, diagnostics
  publishing, and drop-order ownership.
- `crates/rust-analyzer/src/handlers/dispatch.rs`: typed request and
  notification dispatch, parameter extraction, error projection, panic
  boundary, background task spawning, retry policy, and unknown method
  handling.
- `crates/rust-analyzer/src/handlers/request.rs`: feature handlers taking
  either mutable global state or immutable snapshots.
- `crates/rust-analyzer/src/handlers/notification.rs`: mutable notification
  handlers for document, config, workspace, and watched-file changes.
- `crates/rust-analyzer/src/lsp/to_proto.rs` and
  `crates/rust-analyzer/src/lsp/from_proto.rs`: clear protocol conversion
  boundary instead of feature handlers constructing protocol JSON directly.

Do not borrow these Rust-specific systems:

- Macro expansion or proc macro server integration.
- Cargo workspace discovery, build-script support, flycheck, or test runner.
- Rust borrow checking, trait solving, associated item lookup, or import
  insertion semantics that do not apply to Vela.
- Rust-specific editor commands unless a later Vela-specific UX problem
  justifies an explicit counterpart.

---

## 5. Target Architecture

The target production layout for `vela_lsp_server` is:

```text
main.rs
  CLI flags, version/help, lsp_server::Connection::stdio(), main_loop entry

main_loop.rs
  event loop, LSP message dispatch, task result collection, shutdown/exit loop

global_state.rs
  mutable server state, request queue, task pools, snapshots, sending helpers

handlers/dispatch.rs
  typed RequestDispatcher and NotificationDispatcher

handlers/request.rs
  typed request handlers for LSP features

handlers/notification.rs
  typed notification handlers for document/config/workspace/file events

lsp/from_proto.rs
  lsp-types -> vela_language_service conversion

lsp/to_proto.rs
  vela_language_service -> lsp-types conversion

profile.rs
  debug-only JSONL profiler for main-loop and task boundary events
```

`GlobalState` owns:

```text
sender to the LSP client
incoming request queue
server launch configuration
workspace roots and editor configuration
Workspace overlays and disk snapshots
LanguageServiceDatabases
open document set
config/schema diagnostics and watched schema/config documents
semantic token projection/cache
task pools for latency, formatting, and worker requests
cancellation handles keyed by request ID
shutdown/exited flags
profile sink
```

`GlobalStateSnapshot` carries read-only request state:

```text
WorkspaceSnapshot
LanguageServiceDatabases clone or equivalent immutable snapshot
workspace config
open document set
semantic token projection/cache view when needed
generation token
```

Until a deeper service snapshot model is needed, use the existing
`LanguageServiceDatabases: Clone` implementation for snapshot-based read-only
tasks. If this becomes too expensive, add a later service-level immutable
snapshot or Arc-backed database representation as a measured follow-up.

Request scheduling policy:

```text
main-thread mutable:
  initialize, shutdown, exit, workspace reload-like internal requests,
  document open/change/close, configuration, workspace folders, watched files,
  cancel request

latency lane:
  completion, completion resolve, hover, signature help, semantic tokens

formatting lane:
  document formatting, range formatting, on-type formatting

worker lane:
  definitions, references, rename, code actions, call hierarchy, symbols,
  folding, selection ranges, inlay hints, workspace symbols
```

Retry policy:

```text
retryable:
  completion, completion resolve, semanticTokens/full,
  document symbols, folding ranges, workspace symbols

non-retryable:
  hover, signature help, definition/declaration/typeDefinition, references,
  rename, code actions, call hierarchy, semanticTokens/range, inlay hints,
  formatting requests
```

The implementer may adjust a method between retryable and non-retryable only
with a test that proves the editor-visible behavior and with a note in this
document or `docs/decisions.md`.

---

## 6. Phased Execution Plan

### Phase 1: Protocol Dependencies And Typed Transport Shell

- [ ] Add workspace dependencies for `lsp-server`, `lsp-types`, `anyhow`, and
  `crossbeam-channel`. Add `tracing` only if it is used in the same
  checkpoint.
- [ ] Keep the existing CLI flags: `--stdio`, `--root`, `--schema`,
  `--profile`, `--profile-slow-ms`, `--no-watch-files`, `--version`, and
  `--help`.
- [ ] Make `main.rs` start `lsp_server::Connection::stdio()` for real stdio
  server mode.
- [ ] Add a typed in-memory test harness for LSP messages so tests no longer
  need custom Content-Length frame parsing.
- [ ] Keep the old `stdio::run_stdio_with_configuration` only as a temporary
  compatibility wrapper during this phase. Mark it for deletion in Phase 9.
- [ ] Add tests proving the typed transport shell responds to initialize and
  exits cleanly.

Validation:

```bash
cargo test -p vela_lsp_server stdio
cargo test -p vela_lsp_server lifecycle
```

### Phase 2: GlobalState And Typed Lifecycle Dispatch

- [ ] Introduce `global_state.rs` with `GlobalState`, request queue, launch
  configuration, workspace state, language-service databases, shutdown/exited
  flags, and send/respond helpers.
- [ ] Introduce `main_loop.rs` with event loop over `lsp_server::Message`.
- [ ] Introduce `handlers/dispatch.rs` with typed `RequestDispatcher` and
  `NotificationDispatcher`.
- [ ] Migrate `initialize`, `initialized`, `shutdown`, `exit`, and
  `$/cancelRequest` to typed dispatch.
- [ ] Preserve current lifecycle behavior for repeated initialize, malformed
  initialize, shutdown before initialize, requests after shutdown, exit, and
  unsupported methods.
- [ ] Preserve `--no-watch-files` and empty host schema behavior.

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
```

### Phase 3: Typed Mutable Notifications

- [ ] Migrate `textDocument/didOpen`, `textDocument/didChange`,
  `textDocument/didClose`, and `textDocument/didSave` to `lsp-types`.
- [ ] Migrate `workspace/didChangeConfiguration` to typed settings extraction
  while preserving nested `vela` settings support.
- [ ] Migrate `workspace/didChangeWorkspaceFolders`.
- [ ] Migrate `workspace/didChangeWatchedFiles` with existing final-state
  coalescing semantics.
- [ ] Move watcher registration to typed `RegisterCapability` and
  `DidChangeWatchedFilesRegistrationOptions`.
- [ ] Preserve diagnostics publication for open documents, config documents,
  and schema documents.

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server close_overlay
cargo test -p vela_lsp_server schema_reload
cargo test -p vela_lsp_server workspace_folders
```

### Phase 4: Typed Read-Only Request Migration

- [ ] Create `lsp/from_proto.rs` for `Url`, `Position`, `Range`, formatting
  options, and request-specific params conversion into service inputs.
- [ ] Create `lsp/to_proto.rs` for diagnostics, completion, hover,
  definitions, symbols, semantic tokens, references, rename edits, code
  actions, call hierarchy, folding, selection ranges, formatting edits, and
  inlay hints.
- [ ] Migrate completion and completion resolve first.
- [ ] Migrate hover, signature help, definition, declaration, type definition,
  references, prepare rename, rename, call hierarchy, document highlight,
  document symbols, workspace symbols, folding, formatting, range formatting,
  on-type formatting, selection range, semantic tokens full/delta/range, code
  action, and inlay hint.
- [ ] Remove feature-handler construction of raw `serde_json::Value` responses
  as each feature migrates.
- [ ] Preserve current advertised capabilities unless a test proves an
  existing capability is incorrect.

Validation:

```bash
cargo test -p vela_lsp_server completion
cargo test -p vela_lsp_server hover
cargo test -p vela_lsp_server definition
cargo test -p vela_lsp_server references
cargo test -p vela_lsp_server rename
cargo test -p vela_lsp_server code_action
cargo test -p vela_lsp_server formatting
cargo test -p vela_lsp_server semantic_tokens
cargo test -p vela_lsp_server inlay
```

### Phase 5: Task Pools And Scheduling Lanes

- [ ] Add task result enum for background request responses.
- [ ] Add latency, formatting, and worker execution lanes.
- [ ] Run main-thread mutable handlers synchronously with `&mut GlobalState`.
- [ ] Run read-only handlers from `GlobalStateSnapshot`.
- [ ] Ensure document changes and cancellation notifications can be processed
  while long read-only requests are pending.
- [ ] Ensure formatting uses the formatting lane and cannot starve behind
  normal worker requests.
- [ ] Add tests that simulate a queued long request and a following document
  change or cancel notification.

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server formatting
cargo test -p vela_lsp_server completion
```

### Phase 6: Cancellation, Stale Results, And Retry Policy

- [ ] Track incoming request IDs in the request queue.
- [ ] Store cancellation handles by request ID for in-flight background tasks.
- [ ] Cancel unknown or completed IDs as no-response no-ops.
- [ ] Carry `GenerationToken` through background tasks.
- [ ] Discard stale results when the current generation differs.
- [ ] Retry retryable stale requests once using a fresh snapshot.
- [ ] Return LSP `ContentModified` for non-retryable stale requests.
- [ ] Return LSP `RequestCancelled` for cancelled in-flight requests.
- [ ] Add tests for cancelled before start, cancelled while running, stale
  retry, stale non-retry, unknown cancel, and completed cancel.

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server completion
cargo test -p vela_lsp_server semantic_tokens
```

### Phase 7: Protocol Projection Cleanup

- [ ] Delete or empty production use of custom protocol params in
  `protocol.rs` once `lsp-types` replacements exist.
- [ ] Delete production use of custom `RequestId`, `JsonRpcMessage`,
  `JsonRpcResult`, `success_response`, and `error_response`.
- [ ] Keep `serde_json` only for extension payloads, completion resolve data,
  configuration settings, schema artifact JSON, and tests.
- [ ] Ensure no LSP protocol types leak into `vela_language_service`.
- [ ] Add an assertion or package validation where practical that
  `vela_language_service` does not depend on `lsp-types`.

Validation:

```bash
rg -n "JsonRpcMessage|JsonRpcResult|success_response|error_response" crates/vela_lsp_server/src
cargo test -p vela_lsp_server
cargo test -p vela_language_service
```

The `rg` command should return no production references after cleanup. Test
helper references are allowed only if they describe external JSON fixtures.

### Phase 8: Main-Loop Profiling And Debugging

- [ ] Move profile support out of the old stdio transport and into a dedicated
  `profile.rs`.
- [ ] Keep the VS Code settings `vela.server.profile.enabled`,
  `vela.server.profile.path`, and `vela.server.profile.slowMs`.
- [ ] Write JSONL events for session start, request received, queued,
  task started, task ended, response sent, stale discarded, retried, and
  cancelled.
- [ ] Include method, request ID, document URI when available, generation,
  queueMs, handleMs, writeMs, totalMs, outputBytes, lane, and status.
- [ ] Preserve the ability to identify a stuck handler from an unmatched or
  incomplete event sequence.
- [ ] Document how to compare server handler time with VS Code-side stalls.

Validation:

```bash
cargo test -p vela_lsp_server profile
node editors/vscode/scripts/validate-package.js
```

### Phase 9: Close-Out Cleanup, Docs, And Packaging

- [ ] Delete obsolete manual stdio transport code.
- [ ] Delete obsolete custom JSON-RPC code.
- [ ] Update `docs/architecture/lsp.md` with the new RA-style main-loop
  boundary.
- [ ] Update `docs/lsp-implementation-plan.md` if its long goal prompt needs
  to reference this execution document.
- [ ] Update `docs/progress.md` only when current milestone status or durable
  LSP architecture status changes.
- [ ] Update `docs/decisions.md` if the dependency, request scheduling, stale
  retry, or profiling model is accepted as durable architecture.
- [ ] Build a release VSIX after all focused and full validations pass.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node editors/vscode/scripts/validate-package.js
cd editors/vscode && npm run package:release
```

---

## 7. Acceptance Criteria

- [ ] Production `vela_lsp_server` no longer uses hand-written
  `JsonRpcMessage`, `JsonRpcResult`, custom `RequestId`, custom
  `success_response`, custom `error_response`, or custom Content-Length parser
  for normal stdio server operation.
- [ ] Production request and notification handlers use `lsp-types` typed
  params and typed result projection where upstream protocol types exist.
- [ ] Lifecycle behavior remains covered for initialize, initialized,
  shutdown, exit, repeated initialize, malformed initialize, pre-initialize
  requests, requests after shutdown, unsupported methods, and unsupported
  notifications.
- [ ] Watcher registration remains dynamic and typed, respects client
  capability support, ignores empty host schema, and respects
  `--no-watch-files`.
- [ ] Diagnostics, completion, hover, signature help, definitions, symbols,
  semantic tokens, references, rename, code actions, call hierarchy,
  formatting, selection ranges, folding, and inlay hints remain
  behavior-compatible with current fixtures.
- [ ] Latency-sensitive read-only requests do not block document-change or
  cancel notifications in the main loop.
- [ ] Formatting requests use a dedicated execution lane.
- [ ] Cancellation and stale-generation behavior are observable in tests.
- [ ] Profile JSONL distinguishes queue time, handler time, response write
  time, stale, cancelled, retried, and completed request states.
- [ ] `vela_language_service` remains editor-neutral and has no dependency on
  `lsp-server` or `lsp-types`.
- [ ] VS Code and Zed packages remain thin launchers or fallback syntax
  packages, not semantic analysis implementations.

Acceptance must be validated with:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server stdio
cargo test -p vela_lsp_server completion formatting semantic_tokens inlay
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node editors/vscode/scripts/validate-package.js
cd editors/vscode && npm run package:release
```

---

## 8. Validation Commands

Focused checks while migrating individual phases:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server stdio
cargo test -p vela_lsp_server completion formatting semantic_tokens inlay
node editors/vscode/scripts/validate-package.js
```

Full close-out checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd editors/vscode && npm run package:release
```

Use narrower tests for intermediate commits, but do not close the full goal
until the full close-out checks pass or a real external blocker is recorded in
`docs/blocked.md`.

---

## 9. Checkpoint Rules

- [ ] Work from the earliest incomplete phase.
- [ ] Keep checkpoints small and conventional-commit each verified slice.
- [ ] Prefer replacing obsolete internals over supporting parallel legacy and
  new protocol paths.
- [ ] Preserve current user-facing LSP behavior unless a fixture shows it is
  wrong and the change is part of an explicit task.
- [ ] Do not touch unrelated user work. In particular, if
  `examples/src/bin/modules/game/reward.vela` is dirty, treat it as user work
  unless explicitly instructed otherwise.
- [ ] Update this document's checklist only after the relevant focused tests
  pass.
- [ ] Update durable docs only for durable status or architecture changes, not
  routine per-commit notes.

---

## 10. First Execution Tasks

Use the repository task template when starting implementation.

```text
Task: Add protocol dependencies and a typed stdio smoke harness.
Context: This starts the rust-analyzer-style LSP main-loop refactor. The
relevant crate is `vela_lsp_server`; reference rust-analyzer's
`crates/rust-analyzer/src/bin/main.rs` and `crates/rust-analyzer/src/main_loop.rs`.
Expected behavior:
  - `vela_lsp_server` depends on `lsp-server` and `lsp-types`.
  - real stdio server startup uses `lsp_server::Connection::stdio()`.
  - a test harness can drive typed LSP messages without custom Content-Length
    parsing.
  - initialize still returns the current server capabilities and serverInfo.
Tests:
  - lsp_server_stdio_smoke_test or its typed replacement
  - lifecycle initialize fixtures
Do not change:
  - Do not migrate all request handlers in this first task.
  - Do not change `vela_language_service`.
  - Do not change VM, host, reflection, or language semantics.
Validation:
  cargo test -p vela_lsp_server stdio
  cargo test -p vela_lsp_server lifecycle
```

```text
Task: Introduce `GlobalState` and typed lifecycle dispatch.
Context: The current server owns mutable state directly on `LspServer` and
dispatches through hand-written `match method`. The refactor needs a
rust-analyzer-style state owner and typed dispatcher before feature migration.
Expected behavior:
  - lifecycle requests and notifications dispatch through typed request and
    notification dispatchers.
  - repeated initialize, malformed initialize, shutdown, exit, unsupported
    methods, and cancellation preserve current fixture behavior.
  - dynamic watched-file registration still runs after `initialized`.
Tests:
  - cargo test -p vela_lsp_server lifecycle
Do not change:
  - Do not migrate feature requests yet except lifecycle support needed by the
    dispatcher.
  - Do not preserve the old dispatcher as a production compatibility path.
Validation:
  cargo test -p vela_lsp_server lifecycle
```

```text
Task: Migrate document sync notifications to typed handlers.
Context: Document sync drives workspace overlays and diagnostics, so it must
move to typed notifications before background read-only request scheduling.
Expected behavior:
  - didOpen, didChange, didClose, didSave, configuration, workspace folders,
    and watched-file notifications use `lsp-types` params.
  - open overlays remain authoritative over disk snapshots.
  - closed documents restore disk diagnostics or clear scratch diagnostics.
  - watched-file batches keep final-state coalescing.
Tests:
  - cargo test -p vela_lsp_server lifecycle
  - cargo test -p vela_lsp_server close_overlay
  - cargo test -p vela_lsp_server workspace_folders
Do not change:
  - Do not change diagnostic semantics.
  - Do not change project configuration precedence.
Validation:
  cargo test -p vela_lsp_server lifecycle close_overlay workspace_folders
```

```text
Task: Migrate completion as the first latency-lane read-only request.
Context: Completion is latency-sensitive and already has rich
`vela_language_service` coverage. It is the safest first feature request to
move through `GlobalStateSnapshot`, typed params, typed result projection, and
the latency lane.
Expected behavior:
  - textDocument/completion and completionItem/resolve use `lsp-types`.
  - completion runs from a read-only snapshot.
  - document change and cancel notifications can be processed while completion
    is pending.
  - current completion fixtures still pass.
Tests:
  - cargo test -p vela_lsp_server completion
  - cargo test -p vela_language_service completion
Do not change:
  - Do not change completion ranking or item semantics in this task.
  - Do not introduce LSP types into `vela_language_service`.
Validation:
  cargo test -p vela_lsp_server completion
  cargo test -p vela_language_service completion
```

```text
Task: Replace profile JSONL with main-loop and task-boundary profiling.
Context: The existing profiler measures only the old stdio `handle_json`
boundary. After the RA-style main loop exists, profiling must identify queue,
task, stale, cancel, retry, and response phases.
Expected behavior:
  - profile events include session_start, request_received, request_queued,
    task_started, task_ended, response_sent, request_cancelled,
    request_stale, and request_retried where applicable.
  - events include method, request ID, document URI when available,
    generation, lane, queueMs, handleMs, writeMs, totalMs, outputBytes, and
    status.
  - VS Code profile settings keep working.
Tests:
  - cargo test -p vela_lsp_server profile
  - node editors/vscode/scripts/validate-package.js
Do not change:
  - Do not make profiling enabled by default.
  - Do not require VS Code protocol trace to diagnose server stalls.
Validation:
  cargo test -p vela_lsp_server profile
  node editors/vscode/scripts/validate-package.js
```
