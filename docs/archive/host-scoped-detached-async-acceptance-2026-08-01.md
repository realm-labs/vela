# Host-Scoped Detached Async Acceptance — 2026-08-01

Status: accepted. M20.75 Batches A-F are complete with no compatibility path.

## Accepted Outcome

- `task::spawn_scoped(worker(args...))` and
  `task::spawn_scoped_then(worker(args...), continuation)` are the only Vela
  task forms. Both require static function identity; no target string,
  TaskHandle, Future value, join, script cancellation, manual resume, or
  unscoped spawn exists.
- Synchronous ordinary functions and generated Service patches can admit async
  work without changing their ABI. Each child owns an isolated Runtime,
  recursively detached values, finite policy, exact linked artifact, and, for
  Service roots, the complete originating Service generation.
- Workers may await nested Vela, native, provider, I/O, and pinned Service
  calls. `TaskSpawn` and the transitive worker/continuation effect closure are
  checked against Engine, artifact, Service, and host-scope ceilings.
- HostRef, HostPath/PathProxy-backed values, borrowed views, scoped resources,
  closures/upvalues, live iterators, and hidden non-detachable values reject
  before admission. Owned graphs preserve aliases and cycles across transfer.
- Workers publish only after child Runtime teardown. Optional continuations run
  later as fresh synchronous roots with owned `Result<T, task::Error>` input
  and freshly acquired trailing host arguments.
- Version 3 ordinary programs, Service bundles, and detached deployment
  metadata preserve static targets, ABI, detachability, effects, feature bits,
  and Service origin. Versions 1 and 2 reject before linking, staging, or
  activation.
- Scope-local diagnostic IDs, structured lifecycle events, saturating metrics,
  and bounded exact-artifact Runtime pooling remain host-only. Cached Runtime
  shells clear every mutable owner and rerun artifact initialization before
  reuse; observer panic is contained.

## Executable Proof

| Area | Primary proof |
|---|---|
| Static language and effects | HIR, analysis, MIR, verifier, bytecode, linker, hot-reload, macro, and Service-schema tests cover static shape, asyncness, continuation ABI, `TaskSpawn`, and transitive ceilings. |
| Ownership and isolation | Engine task tests cover owned graph alias/cycle transfer, hidden HostRef rejection, independent VM state/budgets, missing scope, capacity, deadline, host-call limit, cancellation, error, panic, pending future drop, and clean Runtime reuse. |
| Service generations | `service_detached_task` suspends an old worker across publication, observes old Rust-pinned result 106 and new Vela-pinned result 1006, then resumes both continuations on their exact generations. |
| Safe-point lifecycle | Request-scope tests cover bounded completion queues, shutdown before context reclamation, cancellation/completion races, one-way suppression, and fresh resume arguments. |
| Stress and observation | Concurrent stress covers 32 tasks across success, panic, cancellation, direct drop, capacity refusal, artifact pin teardown, and recursive spawn quota exhaustion. Lifecycle tests cover unique IDs, terminal metrics, observer panic containment, and cache reset. |
| Tooling and portability | Portable corruption/rejection, sealed reflection, LSP diagnostics/completion/hover/navigation/references/call hierarchy/signature/semantic tokens, CLI schema, and generated Service deployment tests cover their owned layers. |

The runnable `scoped_service_task` example returns `immediate=50` from a
synchronous hotfix, awaits host I/O in a detached worker, calls
`service::base` and `service::pinned`, then prints
`continuation=106 turn=7 task_id_count=1 pool_hits=1 pool_misses=1` after host
safe-point delivery.

The interpreter-only `scoped_task_execution` benchmark separates owned-graph
admission/copy, fresh Runtime, pooled Runtime, first pending poll, Service
nested dispatch, and continuation delivery. The 500-iteration macOS arm64
smoke baseline was approximately 78.8, 28.6, 27.5, 14.8, 768.6, and 18.6
microseconds per operation respectively. These are comparison rows, not an
M20.75 performance threshold.

## Validation

The following gates passed on Rust 1.97.1 `aarch64-apple-darwin`:

```text
cargo fmt --all -- --check
cargo fmt --manifest-path examples/Cargo.toml -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo doc --workspace --all-features --no-deps
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
node editors/vscode/scripts/validate-package.js
(cd editors/tree-sitter-vela && npx --yes tree-sitter-cli@0.25.10 generate)
git diff --exit-code -- editors/tree-sitter-vela/src
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

The architecture size-policy test and reviewed unsafe-boundary source audit
pass. Structural searches found no compatibility alias, target-string path,
global/thread-local task authority, unbounded completion queue, or
framework-specific task API in Vela core. M20.75 added no unsafe block, so its
lifecycle did not require a new Miri or sanitizer boundary.
