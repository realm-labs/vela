# Rust-Analyzer-Style LSP Main Loop Refactor Plan

> **Track:** native LSP protocol and main-loop architecture cleanup
> before further editor-behavior debugging
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release `vela_lsp_server`
> internals are allowed. Do not preserve the current hand-written JSON-RPC,
> stdio framing, protocol params/results, or synchronous `match method`
> dispatcher as compatibility shims. Stdio remains the default editor transport.
> A TCP listener may be added only as an explicit debug/remote-integration
> transport that enters the same typed main loop, defaults to loopback-only
> binding, and never creates a second protocol stack. Preserve product contracts:
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
crates/rust-analyzer/src/config.rs,
crates/rust-analyzer/src/reload.rs,
crates/rust-analyzer/src/line_index.rs,
crates/rust-analyzer/src/lsp/to_proto.rs, and
crates/rust-analyzer/src/lsp/from_proto.rs. Borrow the protocol and editor
server architecture model, not Rust-only semantics: do not add macro
expansion, borrow checking, Rust trait solving, proc macros, Cargo project
modeling, flycheck, or script-language generics to Vela. Preserve the existing
`vela_language_service` boundary as the editor-neutral analysis surface and
keep editor packages thin launchers. rust-analyzer's production LSP entry is
stdio-only in this local checkout, so Vela's optional TCP listener is a Vela
debug/remote-integration extension, not an RA compatibility requirement. When
implemented, TCP must feed the same typed message loop, request queue,
GlobalState, handlers, cancellation, profiling, and protocol conversion modules
as stdio; it must default to loopback-only binding and must not expose an
unauthenticated non-loopback listener unless a separate explicit opt-in flag is
added. If a real external decision blocks progress, update docs/blocked.md and
leave the goal active or blocked explicitly; otherwise keep advancing the next
unchecked task until the entire plan is complete.
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

Stdio remains the editor default because that is the common LSP deployment
shape and the shape used by rust-analyzer's local production entrypoint. Vela
may additionally provide a loopback TCP listener for debugging, attach-first
workflows, external LSP harnesses, or remote editor integration. That TCP mode
is only acceptable if it is a transport wrapper over the same typed server
architecture rather than a parallel implementation.

This plan is an LSP-server architecture cleanup track, not a language-service
semantic rewrite. Existing `vela_language_service` query APIs remain the
semantic source for completion, diagnostics, hover, definitions, symbols,
references, rename, code actions, formatting, semantic tokens, and inlay hints.

---

## 2. Goals

- [ ] Replace production hand-written JSON-RPC and stdio framing with
  `lsp-server`.
- [ ] Keep stdio as the default editor transport.
- [ ] Add an optional loopback TCP debug transport that reuses the same typed
  main loop and cannot diverge from stdio behavior.
- [ ] Replace production hand-written protocol params/results with
  `lsp-types` where the upstream protocol type exists.
- [ ] Introduce a rust-analyzer-style `GlobalState` as the only owner of
  mutable server state.
- [ ] Introduce `GlobalStateSnapshot` for read-only request handlers.
- [ ] Introduce typed `RequestDispatcher` and `NotificationDispatcher`.
- [ ] Model dispatcher entry points after RA's `on_sync_mut`, `on_sync`,
  worker, latency-sensitive, and formatting-thread request categories.
- [ ] Split request execution into main-thread mutable handlers, latency
  handlers, formatting handlers, and worker handlers.
- [ ] Track incoming request state in a request queue.
- [ ] Implement cancellation through request IDs and service generation tokens.
- [ ] Implement stale-result handling with retry for retryable requests and
  `ContentModified` for non-retryable requests.
- [ ] Add a single `line_index`/position-encoding boundary for LSP
  `Position`/`Range` conversion and service byte/span offsets.
- [ ] Add a `ConfigChange`-style configuration pipeline that separates launch
  config, editor config, and workspace config application.
- [ ] Add an explicit reload/diagnostics scheduler boundary for watched files,
  schema/config changes, workspace roots, generation bumps, and open-file
  diagnostic priority.
- [ ] Rebuild request profiling at the main-loop/task boundary so logs show
  received, queued, task-started, task-ended, responded, stale, retried, and
  cancelled states.
- [ ] Add RA-style tracing/log-file diagnostics that do not write to stdout and
  can be correlated with profile JSONL request events.
- [ ] Add a typed in-memory message harness for lifecycle, cancellation, stale
  result, task result, stdio smoke, and TCP smoke coverage.
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
- [ ] Do not make TCP the default editor transport.
- [ ] Do not expose an unauthenticated non-loopback TCP listener by default.
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
- `crates/rust-analyzer/src/config.rs`: central configuration shape and
  change-application model. Borrow the separation of durable config state from
  change payloads, not Cargo-specific settings.
- `crates/rust-analyzer/src/reload.rs`: explicit reload orchestration for file
  system changes, project changes, diagnostics scheduling, and generation
  changes. Borrow the boundary, not Cargo/flycheck behavior.
- `crates/rust-analyzer/src/line_index.rs`: one protocol-position boundary for
  converting between LSP positions/ranges and internal offsets.
- `crates/rust-analyzer/src/lsp/capabilities.rs`: capability construction and
  client-capability gating separated from request handlers.
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
  CLI flags, version/help, stdio/tcp transport selection, main_loop entry

transport.rs
  stdio and optional loopback TCP LSP transport setup, IO thread ownership,
  and conversion into the shared lsp_server::Message channel shape

main_loop.rs
  event loop, LSP message dispatch, task result collection, shutdown/exit loop

global_state.rs
  mutable server state, request queue, task pools, snapshots, sending helpers

config_change.rs
  launch/editor/workspace config change payloads and application order

reload.rs
  watched-file/config/schema/workspace-root reload orchestration,
  generation bumps, diagnostics scheduling, open-file priority

line_index.rs
  LSP position-encoding and range conversion boundary

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

tracing.rs
  stderr/log-file diagnostics, startup environment summaries, request spans
```

`GlobalState` owns:

```text
sender to the LSP client
incoming request queue
server launch configuration
active transport kind and transport metadata
workspace roots and editor configuration
Workspace overlays and disk snapshots
LanguageServiceDatabases
open document set
config/schema diagnostics and watched schema/config documents
line-index and position-encoding cache
reload queue and diagnostic publication scheduler
semantic token projection/cache
task pools for latency, formatting, and worker requests
cancellation handles keyed by request ID
shutdown/exited flags
profile sink and tracing sink
```

`GlobalStateSnapshot` carries read-only request state:

```text
WorkspaceSnapshot
LanguageServiceDatabases clone or equivalent immutable snapshot
workspace config
open document set
semantic token projection/cache view when needed
generation token
position encoding
line-index view for open and disk-backed files
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

Dispatcher API requirements:

```text
on_sync_mut:
  mutable main-thread lifecycle, document, config, workspace, watcher, reload

on_sync:
  cheap read-only handlers that should finish immediately from a snapshot

on_latency_sensitive:
  typing-critical read-only handlers such as completion and signature help

on_fmt_thread:
  formatting-only lane with no starvation behind normal worker requests

on_worker:
  non-latency read-only feature handlers
```

Panic, cancellation, invalid params, stale generations, and unknown methods
must be projected by the dispatcher/main loop instead of each feature handler
constructing ad hoc JSON errors. Read-only background handlers should be wrapped
in a panic boundary and converted into LSP errors without poisoning the main
loop.

Transport policy:

```text
stdio:
  default editor and package transport, RA-aligned production path

tcp:
  optional debug/remote-integration transport, explicit --listen <addr>,
  loopback-only by default, same message loop as stdio, no second dispatcher
```

The TCP listener must reject or require an explicit unsafe opt-in for
non-loopback addresses. It must not add authentication-sensitive semantics to
the language server itself; production editor packages should keep launching
stdio unless a later packaging decision explicitly changes that contract.

Position and encoding policy:

```text
line_index.rs:
  owns LSP Position/Range <-> internal offset/span conversion

encoding:
  preserve UTF-16 behavior for default LSP clients and record negotiated
  position encoding when client capabilities provide it
```

No feature handler should manually derive byte offsets from LSP line/character
pairs after this boundary exists.

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

### Phase 1: Protocol Dependencies, Typed Stdio, And TCP Debug Transport

- [x] Add workspace dependencies for `lsp-server`, `lsp-types`, `anyhow`, and
  `crossbeam-channel`. Add `tracing` only if it is used in the same
  checkpoint.
- [x] Keep the existing CLI flags: `--stdio`, `--root`, `--schema`,
  `--profile`, `--profile-slow-ms`, `--no-watch-files`, `--version`, and
  `--help`.
- [x] Add explicit TCP debug flags, for example `--listen <host:port>`, while
  keeping stdio as the default editor path.
- [x] Reject non-loopback TCP bind addresses by default or require a separate
  explicit unsafe opt-in flag before accepting them.
- [x] Introduce `transport.rs` so stdio and TCP setup produce the same typed
  message-loop input and share response writing, IO thread ownership, shutdown
  joining, profiling, and tracing behavior.
- [x] Make `main.rs` start `lsp_server::Connection::stdio()` for real stdio
  server mode.
- [x] Add a TCP listener smoke path that accepts one client connection and then
  enters the same typed main loop used by stdio.
- [x] Add a typed in-memory test harness for LSP messages so tests no longer
  need custom Content-Length frame parsing.
- [x] Keep the old `stdio::run_stdio_with_configuration` only as a temporary
  compatibility wrapper during this phase. Mark it for deletion in Phase 9.
- [x] Add tests proving the typed transport shell responds to initialize and
  exits cleanly over stdio, in-memory channels, and loopback TCP.

Validation:

```bash
cargo test -p vela_lsp_server stdio
cargo test -p vela_lsp_server tcp
cargo test -p vela_lsp_server lifecycle
```

### Phase 2: GlobalState And Typed Lifecycle Dispatch

- [~] Introduce `global_state.rs` with `GlobalState`, request queue, launch
  configuration, workspace state, language-service databases, shutdown/exited
  flags, and send/respond helpers.
  - `GlobalStateSnapshot` now clones immutable launch configuration,
    workspace snapshot, language-service databases, workspace roots, open
    document IDs, generation, and lifecycle flags for future read-only
    request handlers. Migrating read-only handlers to consume snapshots
    remains open for Phase 5.
  - Typed queued-cancellation state now lives in `GlobalState`'s
    `RequestQueue` instead of the legacy server wrapper. In-flight task
    cancellation handles remain open for Phase 6.
  - Typed initialized, shutdown, and exited lifecycle flags now live in
    `GlobalState`, with temporary legacy-wrapper synchronization for paths
    still routed through `handle_legacy_json`.
  - `RequestQueue` now tracks incoming request IDs as typed `RequestId` values
    instead of stringified IDs, preparing it for later in-flight task
    cancellation and stale-result bookkeeping.
  - Client work-done progress support, dynamic watched-file registration
    support, and semantic-token projection state now live in `GlobalState` and
    `GlobalStateSnapshot`, with temporary legacy-wrapper mirroring until
    Phase 4/5 handler migration removes the old request path.
  - Typed `initialized` now performs dynamic watched-file registration through
    `GlobalState` capability state, and the obsolete typed legacy
    `initialized_lsp` bridge has been removed.
  - Typed `shutdown` and `exit` now update `GlobalState` directly while
    mirroring legacy lifecycle flags, and their obsolete typed legacy bridge
    methods have been removed.
  - Dynamic watched-file registration state now lives in `GlobalState` and
    `GlobalStateSnapshot`, with mirroring to the legacy wrapper only for
    remaining legacy notification paths.
  - The launch/config watcher-enabled setting now lives in `GlobalState` and
    `GlobalStateSnapshot`; typed watcher registration reads that owner while
    the legacy wrapper is kept synchronized for remaining legacy paths.
  - Workspace roots now live in `GlobalState`, drive typed workspace-folder
    changes and watcher registration, and are mirrored back to the legacy
    wrapper for remaining non-typed handlers.
  - Open document IDs now live in `GlobalState` and `GlobalStateSnapshot`;
    the temporary legacy document-sync path mirrors them back after legacy
    handling while typed watched-file scheduling and progress gating read the
    `GlobalState` owner.
  - Editor configuration now lives in `GlobalState` and `GlobalStateSnapshot`
    after launch, initialize, and typed configuration changes, with temporary
    mirroring from legacy paths.
  - Workspace configuration and schema-path lookup now live in `GlobalState`
    and `GlobalStateSnapshot`; typed watcher registration and watched-file
    scheduling read the `GlobalState` owner while the legacy wrapper remains
    synchronized for remaining reload application paths.
  - `GlobalState` now keeps synchronized workspace snapshot and
    `LanguageServiceDatabases` mirrors for `GlobalStateSnapshot`; the legacy
    wrapper remains the temporary mutation backend for document sync and
    reload application until their typed migrations land.
- [x] Introduce `main_loop.rs` with event loop over `lsp_server::Message`.
- [x] Introduce `handlers/dispatch.rs` with typed `RequestDispatcher` and
  `NotificationDispatcher`.
- [x] Implement explicit dispatcher APIs for `on_sync_mut`, `on_sync`,
  latency-sensitive background requests, formatting-lane requests, and worker
  requests.
- [~] Centralize invalid params, panic, cancellation, stale generation,
  `ContentModified`, `RequestCancelled`, method-not-found, and unknown
  notification projection in dispatch/main-loop code.
  - Typed request handler panics are now caught at the dispatcher boundary and
    projected as JSON-RPC internal errors; typed notification handler panics
    are caught as no-response notification failures. Legacy feature-handler
    panic paths remain open until their Phase 4/5 typed migration.
  - Typed request parameter decode failures now use the shared dispatcher
    `InvalidParams` projection (`-32602`) while invalid lifecycle state remains
    `InvalidRequest`.
  - Shared dispatcher helpers now project typed `RequestCancelled` (`-32800`)
    and `ContentModified` (`-32801`) LSP errors, preparing stale-result
    handling to reuse central JSON-RPC error construction when Phase 6 adds
    background tasks.
- [x] Migrate `initialize`, `initialized`, `shutdown`, `exit`, and
  `$/cancelRequest` to typed dispatch.
- [x] Preserve current lifecycle behavior for repeated initialize, malformed
  initialize, shutdown before initialize, requests after shutdown, exit, and
  unsupported methods.
- [x] Preserve `--no-watch-files` and empty host schema behavior.

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
```

### Phase 2.5: Config, Position, Reload, And Trace Boundaries

- [x] Introduce a `ConfigChange`-style pipeline separating immutable launch
  configuration, client/editor configuration, workspace `vela.toml`
  configuration, schema paths, watcher state, and effective config.
- [x] Apply configuration changes through `GlobalState`, not directly inside
  feature request handlers.
- [~] Introduce `line_index.rs` as the only LSP `Position`/`Range` conversion
  boundary, preserving UTF-16 client behavior and recording negotiated position
  encoding when available.
  - Ranged `didChange`, legacy request parameter conversion in `queries.rs`,
    and call-hierarchy item decoding now resolve through `line_index.rs` with
    UTF-16, surrogate-pair, CRLF, oversized range, and member-completion
    regression coverage. Response projection, negotiated encoding storage, and
    future `lsp/from_proto.rs` / `lsp/to_proto.rs` ownership remain open.
- [x] Introduce `reload.rs` or an equivalent reload scheduler for watched-file,
  schema, config, workspace-root, and disk-source changes.
  - `ReloadScheduler` now coalesces typed watched-file batches, classifies
    config/schema/source targets, assigns reload generations, records
    open-document priority metadata, and schedules workspace-root reload work
    before `GlobalState` applies the existing config/schema/source mutations.
- [~] Keep reload work generation-based and open-file-prioritized so watcher
  activity cannot block typing-sensitive notifications.
  - Scheduler drain now processes open-document watched-file work before other
    reload work while preserving stable order inside priority groups. It still
    applies reload work synchronously on the main loop, so non-blocking watcher
    activity remains open.
- [x] Introduce tracing/log-file startup and request-span diagnostics that use
  stderr or explicit log files, never stdout.
  - `--log <jsonl-path>` now enables typed main-loop trace JSONL with
    `session_start`, `message_received`, and `response_sent` events carrying
    method, request ID, document URI, lane, output counts, launch settings, and
    transport metadata. The trace sink writes only to the configured file.
- [x] Add tests for config application order, position conversion edge cases,
  watched-file reload scheduling, and trace/profile opt-in behavior.

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server workspace_folders
cargo test -p vela_lsp_server schema_reload
cargo test -p vela_lsp_server profile
```

### Phase 3: Typed Mutable Notifications

- [x] Migrate `textDocument/didOpen`, `textDocument/didChange`,
  `textDocument/didClose`, and `textDocument/didSave` to `lsp-types`.
  - `textDocument/didOpen` now uses typed `lsp-types` params through
    `GlobalState`, updates the open-document mirror, and preserves existing
    diagnostics publication.
  - `textDocument/didChange` now uses typed `lsp-types` params through
    `GlobalState`, including full-text and ranged edit application through the
    existing line-index conversion boundary.
  - `textDocument/didClose` now uses typed `lsp-types` params through
    `GlobalState`, removes open overlays from the global mirror, and preserves
    disk-snapshot restoration or scratch diagnostic clearing.
  - `textDocument/didSave` now uses typed `lsp-types` params through
    `GlobalState`; it remains a no-response no-op because save events are not
    advertised or required for correctness.
- [x] Migrate `workspace/didChangeConfiguration` to typed settings extraction
  through the `ConfigChange` pipeline while preserving nested `vela` settings
  support.
  - `workspace/didChangeConfiguration` uses typed
    `DidChangeConfigurationParams` through `GlobalState` and the
    `ConfigChange` pipeline.
- [x] Migrate `workspace/didChangeWorkspaceFolders`.
  - `workspace/didChangeWorkspaceFolders` uses typed
    `DidChangeWorkspaceFoldersParams` through `GlobalState` and the reload
    scheduler.
- [x] Migrate `workspace/didChangeWatchedFiles` with existing final-state
  coalescing semantics and reload scheduler ingestion.
  - `workspace/didChangeWatchedFiles` uses typed
    `DidChangeWatchedFilesParams` through `GlobalState` and preserves reload
    scheduler ingestion.
- [x] Move watcher registration to typed `RegisterCapability` and
  `DidChangeWatchedFilesRegistrationOptions`.
  - Dynamic watched-file registration is built from typed
    `RegisterCapability`, `RegistrationParams`,
    `DidChangeWatchedFilesRegistrationOptions`, and `FileSystemWatcher`
    values before JSON-RPC serialization.
- [x] Preserve diagnostics publication for open documents, config documents,
  and schema documents.
  - Existing open-document, config, schema, and close-overlay diagnostic
    publication fixtures remain the Phase 3 compatibility guard.

Validation:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server close_overlay
cargo test -p vela_lsp_server schema_reload
cargo test -p vela_lsp_server workspace_folders
```

### Phase 4: Typed Read-Only Request Migration

- [~] Create `lsp/from_proto.rs` for `Url`, `Position`, `Range`, formatting
  options, and request-specific params conversion into service inputs using
  the shared `line_index.rs` conversion boundary.
  - Initial `lsp/from_proto.rs` now owns URI-to-`DocumentId`, UTF-16
    position/range conversion through `line_index.rs`, formatting option
    copying, and typed text-document position/range input helpers. Existing
    feature handlers still need to migrate onto this boundary.
  - `textDocument/completion` now uses typed `CompletionParams` through the
    latency-sensitive dispatch category and converts its nested text-document
    position through `lsp/from_proto.rs`.
  - `textDocument/definition`, `textDocument/declaration`, and
    `textDocument/typeDefinition` now convert typed `GotoDefinitionParams`
    and its protocol aliases through `lsp/from_proto.rs`.
  - `textDocument/references` now converts typed `ReferenceParams` through
    `lsp/from_proto.rs`.
  - `textDocument/documentHighlight` now converts typed
    `DocumentHighlightParams` through `lsp/from_proto.rs`.
  - `textDocument/documentSymbol` now converts typed
    `DocumentSymbolParams` through `lsp/from_proto.rs`.
  - `workspace/symbol` now converts typed `WorkspaceSymbolParams` query text
    through `lsp/from_proto.rs`.
  - `textDocument/foldingRange` now converts typed `FoldingRangeParams`
    through `lsp/from_proto.rs`.
  - `textDocument/selectionRange` now converts typed `SelectionRangeParams`
    through `lsp/from_proto.rs`.
  - `textDocument/formatting`, `textDocument/rangeFormatting`, and
    `textDocument/onTypeFormatting` now convert typed formatting params
    through `lsp/from_proto.rs`.
  - `textDocument/prepareRename` now converts typed
    `TextDocumentPositionParams` through `lsp/from_proto.rs`.
  - `textDocument/rename` now converts typed `RenameParams` through
    `lsp/from_proto.rs`.
  - `textDocument/prepareCallHierarchy` now converts typed
    `CallHierarchyPrepareParams` through `lsp/from_proto.rs`.
  - `callHierarchy/incomingCalls` and `callHierarchy/outgoingCalls` now
    decode typed `CallHierarchyItem` params through `lsp/from_proto.rs`.
  - `textDocument/semanticTokens/full`,
    `textDocument/semanticTokens/full/delta`, and
    `textDocument/semanticTokens/range` now convert typed semantic token
    params through `lsp/from_proto.rs`.
  - `textDocument/codeAction` now converts typed `CodeActionParams` through
    `lsp/from_proto.rs`.
  - `textDocument/inlayHint` now converts typed `InlayHintParams` through
    `lsp/from_proto.rs`.
- [~] Create `lsp/to_proto.rs` for diagnostics, completion, hover,
  definitions, symbols, semantic tokens, references, rename edits, code
  actions, call hierarchy, folding, selection ranges, formatting edits, and
  inlay hints.
  - Initial `lsp/to_proto.rs` now projects completion lists into typed
    `lsp_types::CompletionResponse` values. Broader feature projections and
    legacy raw JSON helpers remain to migrate.
  - Navigation definitions now project through typed `lsp_types::Location`
    values for definition, declaration, and type-definition responses.
  - References now project through typed `lsp_types::Location` arrays.
  - Document highlights now project through typed
    `lsp_types::DocumentHighlight` values.
  - Document symbols now project through typed
    `lsp_types::DocumentSymbolResponse::Nested` values.
  - Workspace symbols now project through typed
    `lsp_types::WorkspaceSymbolResponse::Nested` values, preserving Vela
    detail metadata in `data.detail` because upstream `WorkspaceSymbol` has
    no top-level `detail` field.
  - Folding ranges now project through typed `lsp_types::FoldingRange`
    values with typed `FoldingRangeKind` categories.
  - Selection ranges now project recursive parent chains through typed
    `lsp_types::SelectionRange` values.
  - Formatting responses now project service text edits through typed
    `lsp_types::TextEdit` values.
  - Prepare rename now projects through typed
    `lsp_types::PrepareRenameResponse` values.
  - Rename now projects through typed `lsp_types::WorkspaceEdit` values with
    `changes`, versioned `documentChanges`, and change annotations.
  - Prepare call hierarchy now projects through typed
    `lsp_types::CallHierarchyItem` values.
  - Incoming and outgoing call hierarchy now project through typed
    `lsp_types::CallHierarchyIncomingCall` and
    `lsp_types::CallHierarchyOutgoingCall` values.
  - Semantic token full, delta, and range responses now project through typed
    `lsp_types::SemanticTokensResult`,
    `lsp_types::SemanticTokensFullDeltaResult`, and
    `lsp_types::SemanticTokensRangeResult` values while preserving the
    existing Vela semantic token projection/cache behavior.
  - Code actions now project through typed `lsp_types::CodeActionResponse`
    values and reuse the existing typed workspace-edit projection.
  - Inlay hints now project through typed `lsp_types::InlayHint` values with
    typed label, kind, position, and padding fields.
- [x] Migrate completion and completion resolve first.
  - `textDocument/completion` now uses typed request params through
    `GlobalState` and typed completion result projection through
    `lsp/to_proto.rs`.
  - `completionItem/resolve` now uses typed `CompletionItem` params through
    `GlobalState` and typed result projection through `lsp/to_proto.rs`;
    resolve payload parsing still reuses the temporary JSON payload helper
    until completion resolve data is replaced by a typed extension payload.
  - Validated with `cargo test -p vela_lsp_server completion_resolve`,
    `cargo test -p vela_lsp_server completion`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service completion`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
- [x] Migrate hover, signature help, definition, declaration, type definition,
  references, prepare rename, rename, call hierarchy, document highlight,
  document symbols, workspace symbols, folding, formatting, range formatting,
  on-type formatting, selection range, semantic tokens full/delta/range, code
  action, and inlay hint.
  - `textDocument/hover` now uses typed `HoverParams` through `GlobalState`
    and typed `lsp_types::Hover` projection through `lsp/to_proto.rs`.
    Validated with `cargo test -p vela_lsp_server hover`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service hover`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/signatureHelp` now uses typed `SignatureHelpParams`
    through `GlobalState` and typed `lsp_types::SignatureHelp` projection
    through `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server signature`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service signature`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/definition`, `textDocument/declaration`, and
    `textDocument/typeDefinition` now use typed params through `GlobalState`
    and typed `lsp_types::Location` projection through `lsp/to_proto.rs`.
    Validated with `cargo test -p vela_lsp_server definition`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service definition`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/references` now uses typed `ReferenceParams` through
    `GlobalState` and typed `lsp_types::Location` array projection through
    `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server references`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service references`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/prepareRename` now uses typed
    `TextDocumentPositionParams` through `GlobalState` and typed
    `lsp_types::PrepareRenameResponse` projection through `lsp/to_proto.rs`.
    Validated with `cargo test -p vela_lsp_server prepare_rename`,
    `cargo test -p vela_lsp_server rename`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_language_service rename`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/rename` now uses typed `RenameParams` through
    `GlobalState` and typed `lsp_types::WorkspaceEdit` projection through
    `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server rename`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service rename`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/prepareCallHierarchy` now uses typed
    `CallHierarchyPrepareParams` through `GlobalState` and typed
    `lsp_types::CallHierarchyItem` projection through `lsp/to_proto.rs`.
    Validated with `cargo test -p vela_lsp_server call_hierarchy`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service call_hierarchy`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `callHierarchy/incomingCalls` and `callHierarchy/outgoingCalls` now use
    typed params through `GlobalState`, decode typed `CallHierarchyItem`
    inputs through `lsp/from_proto.rs`, and project typed incoming/outgoing
    call results through `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server call_hierarchy`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service call_hierarchy`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/documentHighlight` now uses typed
    `DocumentHighlightParams` through `GlobalState` and typed
    `lsp_types::DocumentHighlight` projection through `lsp/to_proto.rs`.
    Validated with `cargo test -p vela_lsp_server document_highlight`,
    `cargo test -p vela_lsp_server references`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service document_highlight`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/documentSymbol` now uses typed `DocumentSymbolParams`
    through `GlobalState` and typed
    `lsp_types::DocumentSymbolResponse::Nested` projection through
    `lsp/to_proto.rs`; the now-unused raw `on_sync` dispatcher branch was
    removed. Validated with `cargo test -p vela_lsp_server document_symbol`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service document_symbols`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `workspace/symbol` now uses typed `WorkspaceSymbolParams` through
    `GlobalState` and typed
    `lsp_types::WorkspaceSymbolResponse::Nested` projection through
    `lsp/to_proto.rs`; Vela workspace-symbol detail strings are now carried
    as extension payload `data.detail` in both typed and legacy projection
    helpers. Validated with `cargo test -p vela_lsp_server workspace_symbol`,
    `cargo test -p vela_lsp_server symbols`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service workspace_symbols`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/foldingRange` now uses typed `FoldingRangeParams` through
    `GlobalState` and typed `lsp_types::FoldingRange` projection through
    `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server folding`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service folding`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/selectionRange` now uses typed `SelectionRangeParams`
    through `GlobalState` and typed `lsp_types::SelectionRange` projection
    through `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server selection`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service selection`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/formatting`, `textDocument/rangeFormatting`, and
    `textDocument/onTypeFormatting` now use typed formatting params through
    `GlobalState` and typed `lsp_types::TextEdit` projection through
    `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server formatting`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service formatting`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/semanticTokens/full`,
    `textDocument/semanticTokens/full/delta`, and
    `textDocument/semanticTokens/range` now use typed params through
    `GlobalState` and typed semantic-token result projection through
    `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server semantic_tokens`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server typed_semantic_token_dispatch_projects_full_delta_and_range`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service semantic_tokens`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/codeAction` now uses typed `CodeActionParams` through
    `GlobalState` and typed `lsp_types::CodeActionResponse` projection
    through `lsp/to_proto.rs`. Validated with
    `cargo test -p vela_lsp_server code_action`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server typed_code_action_dispatch_projects_quickfix_edits`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service code_action`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/inlayHint` now uses typed `InlayHintParams` through
    `GlobalState` and typed `lsp_types::InlayHint` projection through
    `lsp/to_proto.rs`; the now-unused legacy request-dispatch helpers were
    removed after all Phase 4 request registrations became typed. Validated
    with `cargo test -p vela_lsp_server inlay`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo test -p vela_lsp_server typed_inlay_hint_dispatch_projects_parameter_hints`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_language_service inlay`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
- [x] Remove feature-handler construction of raw `serde_json::Value` responses
  as each feature migrates.
  - `textDocument/codeAction` and `textDocument/inlayHint` compatibility
    handlers now serialize typed `lsp/to_proto.rs` projections, and their
    obsolete raw JSON adapter modules were removed. Validated with
    `cargo test -p vela_lsp_server code_action`,
    `cargo test -p vela_lsp_server inlay`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/semanticTokens/full`,
    `textDocument/semanticTokens/full/delta`, and
    `textDocument/semanticTokens/range` compatibility handlers now serialize
    typed `lsp/to_proto.rs` projections, and the duplicate raw semantic-token
    response encoder was removed. Validated with
    `cargo test -p vela_lsp_server semantic_tokens`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/formatting`, `textDocument/rangeFormatting`, and
    `textDocument/onTypeFormatting` compatibility handlers now serialize
    typed `lsp/to_proto.rs` text-edit projections, and the obsolete raw
    formatting adapter module was removed. Validated with
    `cargo test -p vela_lsp_server formatting`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/foldingRange` compatibility handling now serializes typed
    `lsp/to_proto.rs` folding-range projections, and the obsolete raw folding
    adapter module was removed. Validated with
    `cargo test -p vela_lsp_server folding`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/selectionRange` compatibility handling now serializes typed
    `lsp/to_proto.rs` selection-range projections, and the obsolete raw
    selection adapter module was removed. Validated with
    `cargo test -p vela_lsp_server selection`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/signatureHelp` compatibility handling now serializes typed
    `lsp/to_proto.rs` signature-help projections, and the obsolete raw
    signature adapter module was removed. Validated with
    `cargo test -p vela_lsp_server signature`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/hover` compatibility handling now serializes typed
    `lsp/to_proto.rs` hover projections, and the obsolete raw hover adapter
    module was removed. Validated with
    `cargo test -p vela_lsp_server hover`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/definition`, `textDocument/declaration`, and
    `textDocument/typeDefinition` compatibility handling now serializes typed
    `lsp/to_proto.rs` location projections, and the obsolete raw navigation
    adapter module was removed. Validated with
    `cargo test -p vela_lsp_server definition`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/references` and `textDocument/documentHighlight`
    compatibility handling now serializes typed `lsp/to_proto.rs` reference
    and highlight projections, and the obsolete raw references adapter module
    was removed. Validated with
    `cargo test -p vela_lsp_server references`,
    `cargo test -p vela_lsp_server document_highlight`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/prepareRename` and `textDocument/rename` compatibility
    handling now serializes typed `lsp/to_proto.rs` prepare-rename and
    workspace-edit projections, and the obsolete raw rename adapter module
    was removed. Validated with
    `cargo test -p vela_lsp_server prepare_rename`,
    `cargo test -p vela_lsp_server rename`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/prepareCallHierarchy`,
    `callHierarchy/incomingCalls`, and `callHierarchy/outgoingCalls`
    compatibility handling now serializes typed `lsp/to_proto.rs`
    call-hierarchy projections, and the obsolete raw response builders were
    removed from the call-hierarchy adapter. The remaining helper only decodes
    legacy custom call-hierarchy item params until custom protocol params are
    retired. Validated with `cargo test -p vela_lsp_server call_hierarchy`,
    `cargo test -p vela_lsp_server lsp::from_proto::tests::call_hierarchy_item_converts_ranges_and_document_id`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/documentSymbol` and `workspace/symbol` compatibility
    handling now serializes typed `lsp/to_proto.rs` symbol projections, and
    the obsolete raw symbols adapter module was removed. Validated with
    `cargo test -p vela_lsp_server document_symbol`,
    `cargo test -p vela_lsp_server workspace_symbol`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `textDocument/publishDiagnostics` now uses typed
    `lsp_types::Diagnostic` and `lsp_types::PublishDiagnosticsParams`
    projection through `lsp/to_proto.rs`, while preserving Vela's diagnostic
    extension data and optional publication error extension at the transport
    envelope. Validated with `cargo test -p vela_lsp_server diagnostics`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - `completionItem/resolve` compatibility handling now deserializes legacy
    params into `lsp_types::CompletionItem` and serializes the typed
    `lsp/to_proto.rs` resolved-item projection instead of mutating raw JSON
    response values. The resolve payload itself remains a Vela extension
    payload. Validated with
    `cargo test -p vela_lsp_server completion_resolve`,
    `cargo test -p vela_lsp_server lsp::to_proto::tests`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
  - A production helper audit now finds no remaining raw feature response
    builders; remaining `lsp_*` production helpers are lifecycle/capability
    support or document-change conversion utilities. Custom protocol params,
    JSON-RPC envelopes, and extension payload cleanup remain tracked by
    Phase 7 rather than this feature-response item.
- [x] Preserve current advertised capabilities unless a test proves an
  existing capability is incorrect.
  - Phase 4 request migration preserved existing advertised capabilities; no
    initialize capability shape changed during the typed read-only request
    migration.

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

- [x] Add task result enum for background request responses.
  - `TaskResult::Response(JsonRpcResult)` now exists as the main-loop task
    response envelope, and the current synchronous main loop sends handler
    results through `GlobalState::send_task_result(...)`. This prepares the
    receiver shape for future latency/formatting/worker lanes without changing
    scheduling behavior in the same checkpoint. Validated with
    `cargo test -p vela_lsp_server task_result`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
- [x] Add latency, formatting, and worker execution lanes.
  - `TaskScheduler` now owns separate latency, formatting, and worker lane
    workers. `GlobalState` owns the scheduler, and the typed main loop selects
    between client messages and lane `TaskResult` receivers, with a ready
    formatting task checked before blocking. Feature-handler routing onto
    snapshots and lane categories remains tracked by the next Phase 5
    checklist items. Validated with `cargo test -p vela_lsp_server task`,
    `cargo test -p vela_lsp_server next_event`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo test -p vela_lsp_server formatting`,
    `cargo test -p vela_lsp_server completion`, `cargo fmt --all -- --check`,
    and `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
- [x] Run main-thread mutable handlers synchronously with `&mut GlobalState`.
  - `RequestDispatcher::on_sync_mut_typed` routes lifecycle requests such as
    `initialize` and `shutdown` on the main loop with `&mut GlobalState`, and
    `NotificationDispatcher::on_sync_mut_typed` routes initialized, exit,
    cancellation, document sync, configuration, workspace-folder, watched-file,
    and save notifications through the same synchronous mutable boundary.
    Latency, formatting, and worker request categories remain separate from
    this mutable-main-thread checklist item and are tracked by the following
    snapshot/lane items. Validated with
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
- [~] Run read-only handlers from `GlobalStateSnapshot`.
  - `textDocument/completion`, `completionItem/resolve`,
    `textDocument/hover`, `textDocument/signatureHelp`,
    `textDocument/semanticTokens/full`,
    `textDocument/semanticTokens/full/delta`, and
    `textDocument/semanticTokens/range`, `textDocument/formatting`,
    `textDocument/rangeFormatting`, `textDocument/onTypeFormatting`,
    `textDocument/definition`, `textDocument/declaration`,
    `textDocument/typeDefinition`, `textDocument/references`,
    `textDocument/documentHighlight`, `textDocument/documentSymbol`,
    `workspace/symbol`, `textDocument/foldingRange`, and
    `textDocument/selectionRange`, `textDocument/prepareRename`, and
    `textDocument/rename` now dispatch through snapshot-specific dispatcher
    branches, clone a `GlobalStateSnapshot`, and query the snapshot-owned
    `LanguageServiceDatabases`, `WorkspaceSnapshot`, and semantic-token
    projection state without mutating `GlobalState` or the legacy `LspServer`.
    The obsolete mutable typed completion, hover, signature-help,
    semantic-token, formatting, navigation, reference, highlight, symbol,
    folding, selection-range, and rename wrappers were removed. Call
    hierarchy, code actions, and inlay hints still need the same snapshot
    migration before this checklist item can close. Validated with
    `cargo test -p vela_lsp_server completion`,
    `cargo test -p vela_lsp_server hover`,
    `cargo test -p vela_lsp_server signature`,
    `cargo test -p vela_lsp_server semantic_tokens`,
    `cargo test -p vela_lsp_server formatting`,
    `cargo test -p vela_lsp_server definition`,
    `cargo test -p vela_lsp_server references`,
    `cargo test -p vela_lsp_server document_highlight`,
    `cargo test -p vela_lsp_server document_symbol`,
    `cargo test -p vela_lsp_server workspace_symbol`,
    `cargo test -p vela_lsp_server folding_range`,
    `cargo test -p vela_lsp_server selection_range`,
    `cargo test -p vela_lsp_server prepare_rename`,
    `cargo test -p vela_lsp_server rename`,
    `cargo test -p vela_lsp_server lifecycle`,
    `cargo fmt --all -- --check`, and
    `cargo clippy -p vela_lsp_server --all-targets -- -D warnings`.
- [ ] Name task threads or task spans by lane and request method so profile and
  trace output can identify where work is running.
- [ ] Use a latency-sensitive main-loop thread and adequate stack sizing when
  needed by parser/analysis workloads; measure before expanding stack sizes
  globally.
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
- [ ] Move trace/log setup into an explicit module that can write to stderr or
  a log file without polluting stdio protocol output.
- [ ] Keep the VS Code settings `vela.server.profile.enabled`,
  `vela.server.profile.path`, and `vela.server.profile.slowMs`.
- [ ] Preserve or add a VS Code-accessible way to see trace/log output for
  startup args, transport kind, request routing, queue events, and task lane.
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
  boundary, stdio default, and optional loopback TCP debug transport.
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
  for normal stdio or TCP server operation.
- [ ] Stdio remains the default editor transport and uses the same main loop as
  any optional TCP debug transport.
- [ ] TCP mode is explicit, loopback-only by default, covered by smoke and
  lifecycle tests, and cannot route through a separate dispatcher.
- [ ] Production request and notification handlers use `lsp-types` typed
  params and typed result projection where upstream protocol types exist.
- [ ] Request dispatch exposes RA-style mutable, read-only, latency-sensitive,
  formatting, and worker categories rather than feature handlers branching on
  raw method strings.
- [ ] Invalid params, panics, cancellations, stale generations, retry,
  method-not-found, and notification errors are projected through shared
  dispatch/main-loop code.
- [ ] LSP position/range conversion goes through one `line_index` boundary and
  preserves default UTF-16 behavior.
- [ ] Launch config, editor config, workspace config, schema paths, and watcher
  settings flow through a `ConfigChange`-style application pipeline.
- [ ] Watched-file, schema, config, workspace-root, and disk-source changes go
  through an explicit reload/diagnostics scheduler with generation bumps and
  open-file priority.
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
- [ ] Tracing/logging never writes to stdout in stdio mode and can be
  correlated with profile JSONL request IDs, methods, lanes, and generations.
- [ ] Typed in-memory harnesses cover lifecycle, cancellation, stale result,
  task result, stdio smoke, and TCP smoke paths without relying on manual
  Content-Length fixture parsing for core behavior.
- [ ] `vela_language_service` remains editor-neutral and has no dependency on
  `lsp-server` or `lsp-types`.
- [ ] VS Code and Zed packages remain thin launchers or fallback syntax
  packages, not semantic analysis implementations.

Acceptance must be validated with:

```bash
cargo test -p vela_lsp_server lifecycle
cargo test -p vela_lsp_server stdio
cargo test -p vela_lsp_server tcp
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
cargo test -p vela_lsp_server tcp
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
Task: Add optional loopback TCP debug transport.
Context: rust-analyzer's current production LSP entry is stdio-only, but Vela
needs a debug/remote-integration transport for attach-first debugging and
external LSP harnesses. TCP must be a transport wrapper over the same typed
main loop, not a second protocol implementation.
Expected behavior:
  - `vela_lsp_server --listen 127.0.0.1:0` binds a loopback TCP listener and
    reports the selected address through trace/log output.
  - one connected TCP client can send initialize/shutdown/exit through the same
    typed main loop used by stdio.
  - non-loopback bind addresses are rejected unless a later explicit unsafe
    opt-in flag is implemented and tested.
  - VS Code and Zed continue to launch stdio by default.
Tests:
  - tcp initialize/shutdown smoke test
  - lifecycle initialize fixtures through the shared typed harness
Do not change:
  - Do not fork request dispatch, profiling, cancellation, or protocol
    conversion for TCP.
  - Do not make TCP the default editor package transport.
Validation:
  cargo test -p vela_lsp_server tcp
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
  - dispatcher entry points distinguish mutable main-thread handlers,
    read-only snapshot handlers, latency-sensitive tasks, formatting tasks,
    and worker tasks.
  - invalid params, panics, cancellation, stale generations, unsupported
    methods, and unsupported notifications are projected by dispatch/main-loop
    code, not individual feature handlers.
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
Task: Add config, line-index, reload, and tracing boundaries.
Context: Before feature handlers are migrated, protocol conversion and mutable
workspace updates need the same shared boundaries RA uses: a configuration
change pipeline, a position conversion module, a reload scheduler, and
stdout-safe tracing.
Expected behavior:
  - launch flags, editor settings, workspace config, schema paths, and watcher
    options apply through one `ConfigChange`-style path.
  - LSP `Position`/`Range` conversion is centralized in `line_index.rs` and
    preserves default UTF-16 behavior.
  - watched-file, schema, config, workspace-root, and disk-source changes enter
    a reload scheduler that bumps generations and prioritizes open-file
    diagnostics.
  - trace/log output goes to stderr or an explicit file and can identify
    startup args, transport kind, request method/id, generation, and task lane.
Tests:
  - cargo test -p vela_lsp_server lifecycle
  - cargo test -p vela_lsp_server workspace_folders
  - cargo test -p vela_lsp_server schema_reload
  - cargo test -p vela_lsp_server profile
Do not change:
  - Do not change language-service query semantics.
  - Do not log to stdout in stdio mode.
Validation:
  cargo test -p vela_lsp_server lifecycle
  cargo test -p vela_lsp_server workspace_folders
  cargo test -p vela_lsp_server schema_reload
  cargo test -p vela_lsp_server profile
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
