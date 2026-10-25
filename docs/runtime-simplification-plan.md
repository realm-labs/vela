# Runtime Simplification And Feature Removal Plan

Status: scope recorded; implementation has not started.

## Target And Authority

Vela becomes a synchronous, interpreted dynamic scripting language with reliable
script hot reload. Rust interoperability admits only serialized values and
opaque objects. Neither shared nor mutable caller-borrowed Rust objects may
enter script execution, including when hidden behind a handle or view.

This scope supersedes conflicting Service, Provider, async, borrowed-host, and
JIT requirements in the previous roadmap and architecture. Existing code,
tests, milestone acceptance records, and subsystem contracts still describe the
previous implementation until their corresponding removal checkpoint lands.
This document does not claim that any runtime feature has already been removed.

Keep repository artifacts domain-neutral. The purpose of this plan is to reduce
the language and embedding contract, without reproducing another host ABI.

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
- Synchronous native functions and opaque-object constructors/methods.
- Ordinary script hot reload, compatible script-state preservation, stable
  identities, ABI checks, and exact old-generation ownership for active calls
  and retained closures.
- Script GC, finite execution/memory/call-depth budgets, permissions,
  diagnostics, LSP, browser execution, and justified interpreter optimizations.
- Controlled script reflection and registered opaque-object metadata/calls.
  Reflection must not recover host layout, borrowed children, or path access.

Rust hosts may use their own executors and call Vela synchronously after work
completes. LSP worker threads, protocol cancellation, and editor request
scheduling are outside the removed script execution surface. Debugger scope,
ordinary script traits, and package loading are not removed by this decision.

## Rust Boundary

| Boundary | Required semantics |
|---|---|
| Serialized value | Conversion produces script-owned data. Script changes do not implicitly mutate the Rust source; Rust explicitly receives or decodes a result. Serialization need not use JSON text. |
| Opaque object | Script stores object identity and invokes an explicitly registered synchronous surface. Its lifetime does not depend on a caller-stack borrow. Rust host state remains outside the script GC. |
| Native implementation | Rust may borrow internally for one invocation. Such a borrow cannot become a script value, borrowed argument binding, retained view, or borrowed return. |

The replacement must reject borrow-shaped wrappers as well as direct reference
entrypoints. An opaque wrapper around a call-scoped borrowed object does not
satisfy the new boundary. A handle must not silently serialize the Rust object
or recreate the old transparent field/collection access model.

Recommended first implementation: methods only, explicit getters/setters,
host-managed object storage, and identity-checked handles. Replace transparent
host field/index read-modify-write with registered operations. For example:

```vela
fn handle(player, reward) {
    let level = player.level();
    player.set_level(level + 1);
    player.grant(reward);
}
```

Before implementing the replacement, record the concrete rules for:

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

These are implementation decisions to resolve in checkpoint A. Methods-only
exposure and host-managed storage are recommendations, not already implemented
or independently approved storage designs.

## Removal Checklist

Every item below remains open. Remove a feature's declarations, public exports,
compiler metadata, runtime dispatch, portable encoding, tooling projection,
examples, and build registrations together with its implementation.

| ID | Work | Acceptance |
|---|---|---|
| R01 | Replace obsolete product/architecture requirements, agent instructions, long-term goal text, milestone targets, and active plan links. | The synchronous value/object contract is authoritative; historical implementation proof is labeled accurately. |
| R02 | Implement serialized-value and opaque-object registration/call boundaries. | A runnable source-to-VM example round-trips a value and calls a state-changing opaque method without borrowed injection. |
| R03 | Delete Service/domain macros, schemas, default composition, Snapshot/Delta, publication, folding, rollback, and Service deployment bundles. | Public APIs and production dispatch have no Service-specific paths. |
| R04 | Delete static Service namespaces, hidden implementation targets, generated typed thunks, and Service ABI identities. | Old special calls fail explicitly; ordinary functions and methods still work. |
| R05 | Delete Provider discovery/catalogs, selection, compile requests, handles, and provider-only ABI facts. | Ordinary package/dependency compilation, imports, and reload still work. |
| R06 | Delete scoped spawn/then, continuations, child Runtime admission/reuse, schedulers, and task observation. | Scripts cannot start background execution; no detached-task artifact data remains. |
| R07 | Delete script async syntax/semantics, async callable metadata, native registration, Runtime futures, polling/resume machinery, and CLI `--async`. | Old async source and registration fail explicitly; execution is synchronous. |
| R08 | Delete `with_host_ref`, `with_host_mut`, borrowed root injection, scoped projection, and erased reborrow/slice machinery. | Caller-borrowed data cannot enter through direct APIs, macros, constructors, or wrappers. |
| R09 | Delete borrowed returns and collection protocols, including reference envelopes, slices, View/MutView, and lazy borrowed host iterators. | Arguments/results use the value/object boundary; scripts retain their own collections and iterators. |
| R10 | Delete borrow provenance, retained lease trees, parent/child release ordering, `host::release`/`host::try_release`, and suspension checks. | Object lifetime and invocation safety work without script-managed borrow resources. |
| R11 | Replace transparent HostPath/PathProxy/HostTargetPlan read/write/RMW and adapter plumbing with opaque registered calls. | Host state mutation uses the new object boundary; old compound paths cannot survive via reflection or compatibility aliases. |
| R12 | Simplify TypeBinding, registration macros, native signatures, capabilities/effects, reflection, and exported schemas. | Only retained value/object/synchronous facts are required; opaque layout remains hidden. |
| R13 | Redesign or remove old `extern state` host bindings; simplify reload metadata and cleanup. | Ordinary VM state and old closure generations remain valid; old borrowed extern bindings cannot bypass removal. |
| R14 | Remove dead HIR/MIR operations, bytecode instructions, verifier branches, IDs, artifact fields, profile/debug tables, and serializers. | Current artifacts round-trip; obsolete executable formats reject before activation when the format changes. |
| R15 | Remove JIT milestones, restricted-JIT-only interfaces, and backend assumptions. | Verified MIR, budget/root analyses, caches, scalar blocks, and superinstructions needed by the interpreter remain intact. |
| R16 | Update LSP, VS Code, CLI, WASM, bilingual site docs, examples, tests, fuzz seeds, benchmarks, Cargo targets, and CI. | Retained behavior has current proof; removed features have no advertised or executable entrypoints. |

Do not remove whole shared crates based on their names. In particular,
`vela_host`, `vela_package`, `vela_hot_reload`, reflection, and TypeBinding need
responsibility-level review before module/crate deletion. Retain object type,
identity, permission, and invalidation checks even when borrow checks disappear.

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

1. **A: Contract and replacement vertical slice.** Resolve the object rules
   above, rewrite active contracts/agent instructions, and implement a minimal
   synchronous native/value/object example. Keep the old runtime proof until
   its replacement has coverage; do not add public compatibility aliases.
2. **B: Service and Provider removal.** Remove the two feature families and
   their compiler/artifact/tooling branches. Prove ordinary functions, methods,
   modules, packages, and script reload still work.
3. **C: Task and async removal.** Remove background admission first, then
   synchronous/async split machinery. Prove bounded synchronous execution and
   deliberate rejection of removed syntax and APIs.
4. **D: Borrow and path replacement.** Migrate callers to checkpoint A's
   boundary; remove borrowed roots/returns/views, transparent paths, leases,
   explicit borrow release, and old extern bindings. Prove identity, lifetime,
   mutation, and error safety through opaque objects.
5. **E: Repository acceptance.** Finish portable format changes, dead metadata,
   JIT preparation, dependencies, site/tooling/examples, and validation. Replace
   this transition status with the actual supported baseline.

Each checkpoint consists of small Conventional Commits and must leave a
runnable retained-language slice. Move feature-specific test expectations with
their removal; preserve the safety property through replacement or rejection
tests. Never delete a failing test merely to obtain a green run. Preserve
unrelated staged edits and accepted editor evidence; update affected contracts
and recapture affected evidence rather than marking stale evidence accepted.

## Required Verification

- Serialized changes do not implicitly write back; explicit returned values
  decode correctly and unsupported boundary shapes fail with useful errors.
- Opaque aliases observe the same object; wrong-type, stale, invalid, and
  foreign-Runtime handles fail under the chosen ownership policy.
- Object retention/reclamation, root failure, and native invocation/re-entry
  preserve Rust access safety; caller-borrowed wrappers remain inadmissible.
- Registered mutation works, opaque fields stay inaccessible under the chosen
  surface, and permissions, budgets, and source-spanned errors remain enforced.
- Script state survives compatible reload; old closures retain old code and
  new entry calls use new code. Host objects follow their explicit lifetime
  policy rather than implicit migration.
- Ordinary imports, packages, traits, script collections/iterators, reflection,
  synchronous native calls, CLI, WASM, and supported LSP operations still work.
- Removed APIs, compiler-owned paths, syntax, macro signatures, and executable
  artifact formats are rejected at the appropriate boundary.

Run focused subsystem tests at each behavioral change. At checkpoint closure:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Also run affected example, editor/site, artifact-codec, benchmark-harness, and
fuzz validation from [validation.md](validation.md). Search production exports
and dispatch for obsolete capabilities; do not require generic words such as
`async`, `task`, or `service` to vanish from unrelated Rust tooling or history.
