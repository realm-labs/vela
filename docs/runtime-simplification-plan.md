# Runtime Simplification And Feature Removal Plan

Status: checkpoints A-C implemented and focused gates pass. Service/Provider,
script task/async surfaces are removed; D-E remain open. Full repository
acceptance is still pending. R02 replacement interop is deferred.

## Target And Authority

Vela becomes a synchronous, interpreted dynamic scripting language with reliable
script hot reload. Rust interoperability admits only serialized values and
opaque objects. Neither shared nor mutable caller-borrowed Rust objects may
enter script execution, including when hidden behind a handle or view.

This scope supersedes conflicting Service, Provider, async, borrowed-host, and
JIT requirements in the previous roadmap and architecture. Existing code,
tests, milestone acceptance records, and subsystem contracts still describe the
previous implementation until their corresponding removal checkpoint lands.
Checkpoints B-C remove Service/Provider and script tasks/async; borrowed-host
and path removal must still land with checkpoint D. Historical records are not current support.

Keep repository artifacts domain-neutral. The purpose of this plan is to reduce
the language and embedding contract, without reproducing another host ABI.

This pass performs removals and the cleanup needed to keep retained behavior
runnable. It does not implement new features, a replacement opaque-object
system, new value-conversion APIs, or a new re-entry model. The value/object
boundary above constrains the eventual embedding surface; implementing missing
parts is deferred to a separate follow-up, not a prerequisite or acceptance
condition for this removal pass. Existing interop may remain only where it does
not depend on removed capabilities. Otherwise remove the affected entrypoints
and callers rather than preserve them through compatibility wrappers.

## Confirmed Scope

Remove:

- Rust/Vela Service hotfixing, generated service domains, Rust-default
  composition, Snapshot/Delta inheritance, Service publication, folding, and
  conditional rollback.
- Compiler-owned `service::base::*` and `service::pinned::*` calls.
- Provider discovery, selection, provider-specific compilation, and handles.
- Script `async fn`, `.await`, async native/host registration, `call_async`,
  suspended execution, and the CLI async executor mode.
- Scoped detached tasks, continuations, child Runtime scheduling, and task
  lifecycle observation.
- Shared/mutable Rust borrow injection, borrowed returns, borrowed collection
  views, retained leases, and script-visible borrow release operations.
- JIT roadmap work and JIT-only preparation interfaces. Cranelift code
  generation is not implemented today; existing interpreter optimization is
  a separate retained capability.

Keep:

- Dynamic typing, optional type contracts, functions, closures, structs,
  enums, traits, pattern matching, collections, iterators, and Option/Result.
- Ordinary modules, visibility, manifests, workspaces, and path dependencies.
- Existing synchronous native functions and value conversion that do not depend
  on removed capabilities. Existing opaque-object constructors/methods may
  remain only if they meet the reduced boundary without a replacement system.
- Ordinary script hot reload, compatible script-state preservation, stable
  identities, ABI checks, and exact old-generation ownership for active calls
  and retained closures.
- Script GC, finite execution/memory/call-depth budgets, permissions,
  diagnostics, LSP, browser execution, and justified interpreter optimizations.
- Controlled script reflection and metadata/calls for any retained opaque
  surface. Reflection must not recover host layout, borrowed children, or path
  access. This pass does not add an opaque reflection schema.

Rust hosts may use their own executors and call Vela synchronously after work
completes. LSP worker threads, protocol cancellation, and editor request
scheduling are outside the removed script execution surface. Debugger scope,
ordinary script traits, and package loading are not removed by this decision.

## Rust Boundary And Deferred Follow-Up

These constraints apply to retained interop and any future replacement. They do
not require implementing missing boundary features during this removal pass.

| Boundary | Required semantics |
|---|---|
| Serialized value | Conversion produces script-owned data. Script changes do not implicitly mutate the Rust source; Rust explicitly receives or decodes a result. Serialization need not use JSON text. |
| Opaque object | Script stores object identity and invokes an explicitly registered synchronous surface. Its lifetime does not depend on a caller-stack borrow. Rust host state remains outside the script GC. |
| Native implementation | Rust may borrow internally for one invocation. Such a borrow cannot become a script value, borrowed argument binding, retained view, or borrowed return. |

Remove borrow-shaped wrappers as well as direct reference entrypoints. An
opaque wrapper around a call-scoped borrowed object does not satisfy the new
boundary. A handle must not silently serialize the Rust object
or recreate the old transparent field/collection access model.

A future opaque-object design may use methods-only exposure, explicit
getters/setters, host-managed storage, and identity-checked handles. No concrete
storage or registration design is approved here. Do not implement these as
replacements for deleted paths in this pass.

A separate follow-up must resolve the following before adding a replacement:

- Object ownership, retention, invalidation, destruction, and Runtime teardown;
  whether handles may be stored in state, containers, and closures.
- Object identity, alias behavior, Runtime affinity, and invalid/stale handles.
- Native receiver access and same-session synchronous re-entry, without
  restoring script-retained borrows or conflicting Rust accesses.
- Serialized graphs, unsupported cycles, nested opaque values, conversion
  failure, and budget charging. Do not assume serialized graphs preserve the
  alias/cycle behavior of existing internal heap-copy operations.
- Opaque-object compatibility across script reload and its reflection/tooling
  schema. Do not promise migration or transparent serialization of host state.

These are deferred design questions, not checkpoint A work or removal-pass
acceptance requirements. Existing handle, storage, and method-registration code
is an audit starting point, not proof that the future boundary is implemented.

## Removal Checklist

R01 and R03-R07 are implemented at checkpoints A-C; R02 is deferred. R08-R16
remain open; full repository acceptance belongs to E. Remove a
feature's declarations, public exports, compiler metadata, runtime dispatch,
portable encoding, tooling projection, examples, and build registrations
together with its implementation.

| ID | Work | Acceptance |
|---|---|---|
| R01 | Replace obsolete product/architecture requirements, agent instructions, long-term goal text, milestone targets, and active plan links. | The removal-only scope and reduced synchronous contract are authoritative; deferred features and historical implementation proof are labeled accurately. |
| R02 | Deferred: design and implement missing serialized-value and opaque-object boundaries in a separate follow-up. | Not required to close this removal pass; do not add replacement APIs or examples here. |
| R03 | Delete Service/domain macros, schemas, default composition, Snapshot/Delta, publication, folding, rollback, and Service deployment bundles. | Public APIs and production dispatch have no Service-specific paths. |
| R04 | Delete static Service namespaces, hidden implementation targets, generated typed thunks, and Service ABI identities. | Old special calls fail explicitly; ordinary functions and methods still work. |
| R05 | Delete Provider discovery/catalogs, selection, compile requests, handles, and provider-only ABI facts. | Ordinary package/dependency compilation, imports, and reload still work. |
| R06 | Delete scoped spawn/then, continuations, child Runtime admission/reuse, schedulers, and task observation. | Scripts cannot start background execution; no detached-task artifact data remains. |
| R07 | Delete script async syntax/semantics, async callable metadata, native registration, Runtime futures, polling/resume machinery, and CLI `--async`. | Old async source and registration fail explicitly; execution is synchronous. |
| R08 | Delete `with_host_ref`, `with_host_mut`, borrowed root injection, scoped projection, and erased reborrow/slice machinery. | Caller-borrowed data cannot enter through direct APIs, macros, constructors, or wrappers. |
| R09 | Delete borrowed returns and collection protocols, including reference envelopes, slices, View/MutView, and lazy borrowed host iterators. | Remaining arguments/results do not carry caller borrows; scripts retain their own collections and iterators. |
| R10 | Delete borrow provenance, retained lease trees, parent/child release ordering, `host::release`/`host::try_release`, and suspension checks. | No script-managed borrow resources remain; remove entrypoints that cannot work safely without them. |
| R11 | Delete transparent HostPath/PathProxy/HostTargetPlan read/write/RMW and their adapter plumbing and callers. | Old compound paths cannot survive via reflection or compatibility aliases; no replacement object calls are required. |
| R12 | Simplify TypeBinding, registration macros, native signatures, capabilities/effects, reflection, and exported schemas. | Only retained value/object/synchronous facts are required; opaque layout remains hidden. |
| R13 | Remove old `extern state` host bindings that depend on deleted capabilities; simplify reload metadata and cleanup without adding a replacement binding model. | Ordinary VM state and old closure generations remain valid; old borrowed extern bindings cannot bypass removal. |
| R14 | Remove dead HIR/MIR operations, bytecode instructions, verifier branches, IDs, artifact fields, profile/debug tables, and serializers. | Current artifacts round-trip; obsolete executable formats reject before activation when the format changes. |
| R15 | Remove JIT milestones, restricted-JIT-only interfaces, and backend assumptions. | Verified MIR, budget/root analyses, caches, scalar blocks, and superinstructions needed by the interpreter remain intact. |
| R16 | Update LSP, VS Code, CLI, WASM, bilingual site docs, examples, tests, fuzz seeds, benchmarks, Cargo targets, and CI. | Retained behavior has current proof; removed features have no advertised or executable entrypoints. |

Do not remove whole shared crates based on their names. In particular,
`vela_host`, `vela_package`, `vela_hot_reload`, reflection, and TypeBinding need
responsibility-level review before module/crate deletion. For any retained
object surface, retain type, identity, permission, and invalidation checks even
when borrow checks disappear. If retaining that surface requires a new object
system, remove the dependent surface instead.

## Implementation Entry Points

Paths identify audit starting points, not an exhaustive file deletion list.

| Area | Starting points |
|---|---|
| Service | `crates/vela_engine/src/service/`, `crates/vela_macros/src/service.rs`, `crates/vela_macros/src/service_domain.rs`, `crates/vela_hir/src/service_impl.rs` |
| Provider | `crates/vela_hir/src/provider.rs`, `crates/vela_engine/src/source/package/provider.rs`, `crates/vela_engine/src/runtime/provider.rs` |
| Async/tasks | `crates/vela_engine/src/task/`, `crates/vela_engine/src/runtime/detached_task.rs`, `crates/vela_engine/src/typed/async_functions.rs`, `crates/vela_vm/src/async_resume.rs` |
| Borrow/path ABI | `crates/vela_host/src/`, `crates/vela_engine/src/interop/`, `crates/vela_engine/src/host_lease.rs`, `crates/vela_engine/src/runtime/execution_host/` |
| Compiler/artifacts | `crates/vela_common/src/`, `crates/vela_mir/src/`, `crates/vela_bytecode/src/artifact/`, `crates/vela_bytecode/src/compiler/` |
| JIT preparation | `crates/vela_mir/src/jit.rs` and its consumers |
| User-facing contracts | `crates/vela_language_service/`, `crates/vela_lsp_server/`, `crates/vela_cli/`, `crates/vela_playground_wasm/`, `editors/vscode/`, `site/`, `examples/`, `tests/lsp_matrix/` |

## Runnable Checkpoints

1. **A: Removal contract and retained baseline.** Rewrite active contracts and
   agent instructions for this removal-only pass. Identify existing tests for
   the retained synchronous language, value/native calls, and script reload.
   Record deletion dependencies and any interop surface that must disappear
   with them. Do not design or implement a replacement object system.
2. **B: Service and Provider removal.** Remove the two feature families and
   their compiler/artifact/tooling branches. Prove ordinary functions, methods,
   modules, packages, and script reload still work.
3. **C: Task and async removal.** Remove background admission first, then
   synchronous/async split machinery. Prove bounded synchronous execution and
   deliberate rejection of removed syntax and APIs.
4. **D: Borrow and path removal.** Remove borrowed roots/returns/views,
   transparent paths, leases, explicit borrow release, and dependent extern
   bindings, registrations, and callers. Preserve existing independent
   value/native calls; do not add opaque replacements to keep old examples or
   APIs alive. Prove removed borrowing paths are unavailable.
5. **E: Repository acceptance.** Finish dead metadata, JIT preparation,
   dependencies, site/tooling/examples, and validation. Audit portable formats;
   incompatible format changes and old-format rejection must already accompany
   their owning removal checkpoints. Record the actual supported baseline and
   separately deferred interop work, without claiming a replacement is complete.

Each checkpoint consists of small Conventional Commits and must leave a
runnable retained-language slice. Move feature-specific test expectations with
their removal; preserve the safety property through replacement or rejection
tests. Never delete a failing test merely to obtain a green run. Preserve
unrelated staged edits and accepted editor evidence; update affected contracts
and recapture affected evidence rather than marking stale evidence accepted.

## Required Verification

- Existing retained value conversions and synchronous native calls work;
  script-owned value changes do not implicitly write back to Rust. Do not add a
  new serializer or conversion API to satisfy removal acceptance.
- Caller-borrowed roots, wrappers, returns, views, and script-managed leases
  have no remaining executable or registration entrypoints.
- If an existing opaque surface remains, its type, identity, invalidation,
  lifetime, and permission checks remain effective without removed machinery.
  A new object surface is not required for acceptance.
- Permissions, finite VM budgets, and source-spanned errors remain enforced on
  retained execution paths.
- Script state survives compatible reload; old closures retain old code and
  new entry calls use new code. No new host-object migration or retention
  policy is introduced by this pass.
- Ordinary imports, packages, traits, script collections/iterators, script
  reflection, retained synchronous native calls, CLI, WASM, and supported LSP
  operations still work.
- Removed APIs, compiler-owned paths, syntax, macro signatures, and executable
  artifact formats are rejected at the appropriate boundary. Update or remove
  feature-specific fixtures and examples with their feature; preserve retained
  behavior and boundary rejection coverage, not obsolete success expectations.

Run focused subsystem tests at each behavioral change. At checkpoint closure,
use [validation.md](validation.md) as the command source of truth, including:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
```

Also run affected example, editor/site, artifact-codec, benchmark-harness, and
fuzz validation from [validation.md](validation.md). Search production exports
and dispatch for obsolete capabilities; do not require generic words such as
`async`, `task`, or `service` to vanish from unrelated Rust tooling or history.
