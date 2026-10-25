# Architecture

## Authority And Transition Status

Vela's target is a synchronous, interpreted dynamic language with reliable
script hot reload. [goal.md](goal.md) defines the product; the
[runtime simplification plan](runtime-simplification-plan.md) defines removal
checkpoints A-E. This entrypoint is the cross-subsystem technical contract and
supersedes conflicting detailed contracts and historical milestone plans.

**Checkpoint A consolidates requirements, not runtime implementation.** Existing
code, tests, artifacts, and subsystem docs still include capabilities scheduled
for removal. [progress.md](progress.md) records that implementation evidence;
it is not authority to extend Service/Provider, async/tasks, borrowed-host,
transparent-path, or JIT scope. Do not infer completed runtime removal from
this document.

This pass removes capabilities and keeps retained behavior runnable. It does
not implement new opaque-object storage/registration, new value-conversion
APIs, an opaque reflection schema, or a new re-entry model. Missing boundary
features belong to a separate follow-up, not checkpoint A or removal acceptance.

## Retained Compile And Runtime Pipeline

```text
.vela source / ordinary package graph
    -> lexer / parser / lossless CST / AST
    -> resolver / HIR / semantic facts
    -> verified MIR / bytecode compiler
    -> linked artifact / immutable executable generation
    -> synchronous VM / call frames / script heap / GC / finite budgets
    -> script-owned results and existing independent synchronous native calls
```

- `vela_syntax` owns parsing; `vela_hir` owns source-set ingestion, module graphs,
  and resolution. Source-set mode and function selection belong to the same
  source set. The bytecode compiler accepts built HIR and must not parse source
  text or depend on `vela_syntax`.
- `vela_engine` is the production source orchestrator and registry-aware linker
  for source/file/directory/package and script-reload APIs. It passes a
  `LinkedArtifact` into `vela_hot_reload`; that crate owns version construction,
  ABI/policy comparison, and update generation, not source/HIR compilation.
  Ingestion, compilation, and linking failures remain structured Engine errors;
  only linked ABI/policy outcomes enter the staged reload report path.
- `vela_package` owns dependency-light manifest/project IO. Engine and tooling
  share structured `vela.toml`, deterministic source tables, package graphs,
  and `PackageId + ModulePath` identity. Ordinary imports, visibility,
  workspaces, and path dependencies do not require Provider discovery or
  selection. Remove provider-only compile/link facts, not package loading.
- Production execution uses verified linked bytecode through one interpreter
  route. Unlinked bytecode is a compiler/verifier/linker or fixture format, not
  an alternate runtime. Verified MIR and analyses used by the interpreter stay;
  restricted-JIT-only interfaces do not.
- Source files use `.vela`; portable bytecode-only files use `.vbc`. Executable
  format changes must reject obsolete incompatible formats before linking or
  activation, in the checkpoint that changes them. Do not retain a legacy
  interpreter, synthesize deleted metadata, or silently upgrade removed paths.

## Language And Runtime Ownership

Retain dynamic typing and optional contracts; functions and closures; script
records, enums, shapes, traits and dispatch; patterns, loops, collections,
iterators, and Option/Result. Restricted builtin parameterized hints are not
general script-language generics. Ordinary script traits remain independent of
the removed Provider and Service systems.

An immutable executable generation owns linked code and handles, verified MIR
where retained, indexes, source maps, cache/profile layouts, and selected
interpreter plans. Runtime-local mutable state owns the script heap, roots,
compatible Vela `state`, and active synchronous execution. Multiple runtimes may
share an artifact without sharing script-visible mutable state.

Script GC is non-moving and owns script objects, not Rust host state. Frames,
closures, retained script values, and live generation state must remain rooted
correctly. Retain finite execution-unit, memory, call-depth, collection-growth,
and registered native-call accounting, permissions, and source-spanned errors.
Removed leases or suspension checks must not be replaced by unbounded or
unchecked execution paths.

The host may schedule its own external work and then make a synchronous Vela
call. Script execution has no async declarations/await, native futures,
suspension/resume machinery, detached children, continuations, or task
observation in the target. LSP workers, editor request cancellation, and host
executor internals are not part of this removed execution surface.

## Reduced Rust Boundary

These are constraints on retained interop and future work, not a mandate to
implement missing APIs in this removal pass:

| Surface | Contract |
|---|---|
| Serialized value | Existing conversion produces script-owned data. Script changes do not implicitly mutate the Rust source; Rust explicitly receives/decodes a result. Serialization need not be JSON text. |
| Existing opaque object | Only retain an existing surface if identity and explicitly registered synchronous calls work without removed capabilities. Its lifetime must not depend on a caller-stack borrow; Rust state remains outside script GC. No replacement object system is required. |
| Native implementation | Rust may borrow internally during one invocation, but that borrow cannot become a script value, borrowed argument binding, retained view, or borrowed return. |

Remove shared/mutable caller-borrowed roots, including `with_host_ref` and
`with_host_mut`, borrowed returns and envelopes, slices/views and lazy borrowed
host iterators, erased reborrow/projection machinery, retained lease trees,
parent/child release ordering, and `host::release` / `host::try_release`.
A handle or opaque wrapper around a call-scoped borrowed object is not an
acceptable escape hatch.

Remove transparent HostPath/PathProxy/HostTargetPlan reads, writes, compound
mutations and adapter plumbing. Neither reflection nor a compatibility alias
may reconstruct this model. A handle must not silently serialize a host object
or expose its fields/collections as the old path model did. Script-owned record
and collection access remains ordinary language behavior.

Audit TypeBinding, native signatures, macros, constructors, methods, effects,
permissions, reflection, and exported schemas by responsibility. Keep only
facts needed by existing independent synchronous value/object surfaces. If a
surface depends on deleted borrowing/path machinery, remove its registrations,
entrypoints, and callers instead of implementing a substitute. Remove affected
`extern state` host bindings without adding a replacement binding model;
ordinary VM-owned state and script reload remain supported.

Retain type, identity, invalidation, lifetime, and permission checks for any
existing opaque surface that survives. Removing borrow checks is not permission
to bypass other safety checks. Do not delete whole shared crates merely because
their current names or modules refer to host access.

Object ownership/retention/destruction, Runtime affinity, invalidation, alias
behavior, same-session synchronous re-entry, serialized graphs/cycles and budget
charging, object reload compatibility, and opaque reflection/tooling schemas
remain [separate follow-up questions](runtime-simplification-plan.md#rust-boundary-and-deferred-follow-up).
Existing handles or internal heap-copy operations do not establish those
contracts, and this pass must not promise host-object migration or retention.

## Script Hot Reload And Exact Generations

- `ProgramVersion` and its linked artifact identify one immutable executable
  generation. Stable names/semantic IDs are ABI comparison keys; dense function
  handles, state indexes, cache slots, profile sites, and physical plan offsets
  are valid only with their exact owner.
- A public entry call resolves against the active generation. A frame pins the
  owner chosen at entry; a closure retains its creation-generation owner and
  handle. Nested calls and native dispatch from old code use that owner, never
  the current generation's similarly named function or dense handle.
- Accepted activation at an explicit safe point changes subsequent entry calls.
  It does not switch executing bytecode or rebase old closures, plans, caches,
  or counters. Rejected/stale candidates leave active code and state unchanged.
- Compatible VM state cells survive reload without rerunning existing
  initializers or arbitrary module top-level side effects. Added VM state is
  staged; incompatible state contracts reject. Retained old closures keep the
  state slots their old generation needs, including state removed from the
  active generation. Reclaim old-only state/code when no live owners need it;
  a removed closure-valued state root must not retain itself indefinitely.
- Code, interpreter plans, state ownership, and GC/reclamation must obey the
  same exact-generation lifetime. Service generations, borrowed extern
  bindings, suspended frames, or detached workers are not prerequisites for
  retaining these ordinary script semantics.

This is script-code reload, not Rust Service hotfixing or host-object migration.
Delete Service schemas/domains, Rust defaults and Snapshot/Delta composition,
publication/folding/rollback, generated thunks, hidden targets, and
`service::base::*` / `service::pinned::*`. Delete Provider catalogs, selection,
handles, and provider-specific ABI/compilation without removing ordinary
functions, methods, traits, imports, or script reload.

## Reflection, Tooling, And Interpreter Optimization

Script reflection retains metadata for types, fields, methods, variants, traits,
modules, and functions, with controlled value reads/writes/calls. The registry
is structurally read-only at runtime: no monkey patching or type-structure
mutation. Any retained opaque metadata/calls must honor permissions and cannot
reveal host layout, borrowed children, or host-path access. Do not invent a new
opaque schema to preserve deleted reflection operations.

Retain CLI and browser/WASM execution, `vela_analysis`, the platform-independent
`vela_language_service`, native `vela_lsp_server`, and thin editor clients.
Language service snapshots and workers remain separate from runtime execution.
Revise removed syntax/schema/tooling projections and recapture affected editor
evidence in their owning checkpoints; preserve unrelated accepted LSP evidence.
Debugger runtime hooks and DAP integration remain a planned retained track, not
a reason to keep async or JIT machinery.

Interpreter optimization stays profile-driven. Verified MIR, liveness/root and
budget analyses, linked-artifact ownership, inline caches, scalar blocks,
superinstructions, source points, and profile layouts remain when they serve
the interpreter. Optimized paths must preserve ordinary instruction behavior,
logical budget charges, errors, roots, and exact-generation ownership. Remove
JIT-only preparation and backend assumptions; there is no Cranelift/JIT milestone
or machine-code acceptance target. Compare representative retained workloads
without obsolete borrowed-path or Service performance requirements.

## Removal Dependencies And Verification

The [removal checklist](runtime-simplification-plan.md#removal-checklist) owns
the complete inventory; the critical ordering is:

1. **B: Service/Provider.** Remove generated/public declarations together with
   compiler metadata, hidden/static targets, dispatch, artifacts, tooling, and
   callers. Preserve ordinary modules/packages and script generation ownership.
2. **C: Tasks/async.** Remove background admission before async registration,
   call/poll/resume paths, metadata, and CLI mode. Keep finite synchronous calls
   and independent editor workers.
3. **D: Borrows/paths.** Remove roots, returns/views, leases/release, transparent
   adapters, dependent extern bindings, macros/signatures, and examples together.
   Keep only independent native/value calls; remove unsafe dependents rather
   than retain them behind wrappers or add replacement objects.
4. **E: Acceptance.** Finish dead IDs, MIR/bytecode/verifier branches, artifact
   fields, serializers, debug/profile tables, dependencies, JIT-only preparation,
   tooling, site docs, examples, benchmarks, and fuzz fixtures. Format rejection
   must already accompany each owning change, not wait until E.

Checkpoint A identifies contracts and baseline tests only. A passing baseline
is not proof of absent APIs: each behavioral checkpoint also needs deliberate
rejection tests and an export/dispatch/artifact audit. Do not remove safety tests
merely to obtain a green run; update obsolete success expectations with their
feature while preserving retained behavior and boundary rejection coverage.

## Existing Retained Baseline Tests

These are existing test entrypoints, not a newly implemented interop contract.
Some fixtures currently import old machinery; adapt those fixtures during its
owning removal without preserving the obsolete production APIs.

| Existing source | Retained proof to preserve |
|---|---|
| [Engine native compiler tests](../crates/vela_engine/src/tests/native/compiler_options.rs) | `engine_installs_registered_native_functions_into_vm`: registered synchronous scalar native calls. |
| [Argument conversion tests](../crates/vela_engine/tests/args.rs) | `script_arg_conversions_preserve_exact_scalar_tags`, byte-buffer, tuple, Option/Result, set and map conversions; avoid treating mixed HostRef cases as retained contracts. |
| [Ordinary package tests](../crates/vela_engine/src/source/package/tests.rs) | `ordinary_package_imports_public_dependency_function`, `ordinary_dependency_body_reload_updates_root_package_calls`, and ABI rejection without active-image advance. |
| [Runtime state tests](../crates/vela_engine/src/tests/runtime_args/managed_values.rs) | `runtime_state_decl_persists_vm_owned_value_and_rust_updates`; extern-host-binding cases are not retained requirements. |
| [Engine reload tests](../crates/vela_engine/tests/hot_reload.rs) | `retained_closure_pins_old_generation_across_handle_layout_reload`, `retained_old_closure_reads_state_removed_from_active_generation`, and `retained_old_closure_keeps_native_dispatch_and_rejected_reload_owner`. |
| [VM execution tests](../crates/vela_vm/src/tests/execution_core.rs) | Execution-unit charging, stop-before-next-instruction, and call-depth limits on recursive scripts. |

Run focused tests per change and the formatting, Clippy, workspace-test and
affected platform/artifact/example gates in [validation.md](validation.md) at
checkpoint closure. Report commands actually run, not inferred acceptance.

## Implementation Architecture Hygiene

Keep focused crate/module responsibilities and ordinary source/test files under
1200 lines unless a concrete reviewed exception is listed in
[architecture/file-size-exceptions.md](architecture/file-size-exceptions.md).
Use Rust `mod` boundaries for handwritten code, `include!` only for generated
code, normal module resolution with justified `#[path]` exceptions, and explicit
production imports. Split large functions, extract cohesive parameter structs,
and prefer enum-driven dispatch or focused helper types to accumulating patches.

Remove obsolete internal APIs instead of adding aliases, duplicate paths, or
migration wrappers solely to preserve old callers. This does not weaken script
hot reload ABI checks or executable-format rejection. Keep durable docs compact;
raw measurements and implementation chronology belong in Git or labeled archives.

## Subsystem References And Old Implementation Contracts

The files below have not been rewritten by checkpoint A. They document the old
implementation and may contain mixed retained details and superseded targets.
They are useful audit/history references, **not independent active requirements**.
Only their retained semantics consistent with this entrypoint and the removal
plan remain applicable. In particular, old JIT, Service, Provider, async/task,
borrow/path/lease, and borrowed extern-state clauses must not guide new work.

- Language/analysis references: [language semantics](architecture/language.md),
  [primitive hints/guards](architecture/primitives-type-hints-and-guards.md),
  [dynamic dispatch](architecture/dynamic_method_dispatch.md).
- Runtime references: [runtime/GC/bytecode](architecture/runtime.md),
  [hot reload](architecture/hot-reload.md), [reflection](architecture/reflection.md),
  [stdlib/embedding](architecture/stdlib-and-embedding.md),
  [tooling/performance/security/testing](architecture/tooling-performance-security-testing.md),
  [LSP](architecture/lsp.md), [identity refactor](architecture/clean-identity-refactor.md).
- Old host/Service/Provider contracts and plans (not implementation targets):
  [host registration](architecture/host-and-registration.md),
  [packages and providers](packages-and-providers.md),
  [service model](architecture/rust-vela-service-model.md),
  [service hard switch](rust-vela-service-hard-switch-plan.md),
  [explicit-release hard switch](rust-vela-interop-final-shape-hard-switch-plan.md),
  [interop implementation](rust-vela-interop-hard-switch-implementation.md),
  [interop authoring/deployment](rust-vela-interop.md).
- Old execution/state plans (not implementation targets):
  [async](async-execution-model-plan.md),
  [detached async](host-scoped-detached-async-execution-plan.md),
  [state storage](state-storage-model-plan.md),
  [actor/cache history](archive/actor-runtime-cache-execution-plan.md).

The [archived goal/milestones](archive/goal-before-runtime-simplification.md)
preserve detailed old targets and demos. Historical acceptance records, old
`Complete` milestone labels, and deferred-track lists in progress/performance
docs establish prior implementation evidence only where scope now conflicts;
they neither require restoring removed capabilities nor prove their removal.
