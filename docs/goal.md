# Goal

## Authority And Current Scope

Build Vela as a synchronous, interpreted, Hot Reload First dynamic scripting
language in Rust for host-owned business logic. Game servers are a proving
ground, not a source of builtin domain concepts. The language, standard library,
diagnostics, examples, and repository terminology remain domain-neutral.

The [runtime simplification plan](runtime-simplification-plan.md) defines the
accepted removal-only pass and checkpoints A-E. This file is the consolidated
product roadmap; [architecture.md](architecture.md) is the technical contract.
They supersede conflicting old milestone, subsystem, and execution-plan targets.
[progress.md](progress.md) records implementation evidence and gaps; historical
records do not reinstate removed goals.

**A–D runtime removals are implemented** through D commit `226900e81`.
Service/Provider, script async/tasks, borrowed-host access, transparent paths and
dependent bindings are gone. The supported Rust boundary is existing synchronous
owned-value native APIs only; no independent opaque-object runtime survived D.
Replacement objects, new conversion APIs and a new re-entry model are separately
deferred, not removal prerequisites. **A-E removal implementation and software
acceptance are complete**; the [acceptance report](archive/runtime-simplification-acceptance.md)
records the full workspace gates and installed-editor partitions without
claiming complete strict/native LSP coverage.

## Product Goals

1. **Expressive dynamic language.** Retain optional type contracts, functions,
   closures, structs/records, enums, traits, pattern matching, collections,
   iterators, and Option/Result. Restricted builtin parameterized type hints do
   not imply general script-language generics.
2. **Ordinary source organization.** Retain modules, visibility, imports,
   manifests, workspaces, and path dependencies without Service or Provider
   discovery, selection, or special compilation entrypoints.
3. **Reduced synchronous Rust boundary.** Retain existing synchronous native
   calls and existing owned value conversions. Serialized data is script-owned;
   changes do not implicitly write back to Rust. Native host-read/write effects
   and permission checks remain. There is no opaque-object runtime, constructor
   or Rust receiver surface. Host state stays outside script GC; replacement
   object/conversion/re-entry design requires separately authorized follow-up.
4. **Reliable script hot reload.** Preserve stable semantic identities, ABI
   checks, safe-point activation, and compatible script state. New entry calls
   use new code; active calls and retained closures own their exact old
   generation, including nested dispatch, state ownership, and execution plans.
   A stable function name or ID must never silently rebind an old closure.
5. **Controlled reflection.** Retain script metadata queries and permitted
   value reads, writes, and calls. Type structure is immutable at runtime.
   Reflection cannot recover opaque Rust layout, borrowed children, or host
   paths. No new opaque reflection schema is required in the removal pass.
6. **Bounded, testable execution.** Retain non-moving script GC, finite execution,
   memory, call-depth and collection budgets, permissions, and source-spanned
   diagnostics. Rust hosts may run their own work/executors and call Vela
   synchronously afterward; scripts do not schedule background work.
7. **Practical interpreter performance.** Keep verified MIR, root/budget
   analyses, linked bytecode, caches, profiling, scalar blocks, and
   superinstructions justified by interpreter execution. Optimize measured
   retained workloads without preserving JIT-only interfaces or assumptions.
8. **Usable tooling.** Retain CLI and browser/WASM execution, the native language
   service and full LSP capability track, thin editor integrations, and planned
   debugger runtime hooks/DAP integration. Editor workers, protocol cancellation,
   and request scheduling are not script async execution.

## Removed Capabilities And Non-Goals

The reduced product has no:

- Rust/Vela Service hotfixing, generated service domains, Rust-default
  composition, Snapshot/Delta inheritance, Service publication/folding/rollback,
  or compiler-owned `service::base::*` / `service::pinned::*` calls.
- Provider catalogs, discovery, selection, provider-specific compilation,
  handles, or provider-only ABI facts. Ordinary script traits and packages stay.
- Script `async fn`, `.await`, async native/host registration, `call_async`,
  suspension/resume, CLI async execution, detached tasks, continuations, child
  Runtime scheduling, or task lifecycle observation.
- Shared or mutable caller-borrowed Rust roots, borrowed arguments/returns,
  borrowed collection views/iterators, retained leases, or script-visible borrow
  release. A wrapper or handle around a call-scoped borrow is also excluded.
- Transparent HostPath/PathProxy/HostTargetPlan read/write/read-modify-write
  access or adapter paths, including reflection and compatibility aliases.
  Remove dependent extern-state bindings and registrations rather than invent
  replacement bindings.
- JIT roadmap, Cranelift backend milestone, or restricted-JIT-only preparation.
  Cranelift code generation was not implemented; removing that roadmap is not
  evidence of deleting an implemented backend.

Standing non-goals also include general script generics, function overloading,
a script borrow checker, script threads/shared-memory concurrency, arbitrary
`eval`, monkey patching, moving GC, structural state migration, and a custom IDE
product. A built-in C ABI/non-Rust SDK is not part of the embedding contract.
The removed features are not merely deferred MVP goals. Future interop design
is separately deferred; see the [boundary follow-up questions](runtime-simplification-plan.md#rust-boundary-and-deferred-follow-up).

## Design Principles

- Correctness, testability, hot reload semantics, and boundary safety precede
  performance and syntax sugar. Prefer a runnable vertical slice to a broad
  incomplete subsystem.
- The retained loop is `source -> HIR/MIR -> linked bytecode -> synchronous VM
  -> script-owned result`, with existing independent native/value calls and
  script reload tested end to end. Transparent Rust write-through is no longer
  an acceptance requirement.
- Replace obsolete internal APIs and update their callers/tests rather than
  retain compatibility wrappers. Preserve product ABI/reload checks and reject
  incompatible executable artifact formats before activation.
- Keep modules focused, ordinary source/test files under 1200 lines unless a
  reviewed exception is documented, and durable docs compact. History and raw
  measurements belong in Git or clearly labeled archives.
- Every checkpoint needs focused proof. Never delete a failing test merely to
  make validation pass; replace removed success expectations with deliberate
  rejection coverage and preserve the retained safety properties.

## Long-Term Codex Goal

```text
/goal Treat docs/goal.md as the product roadmap, docs/architecture.md as the
technical contract, docs/runtime-simplification-plan.md as the removal-only
checkpoint plan, and docs/progress.md as implementation evidence and gaps.
A-E removal implementation and software gates are complete; use current
progress and docs/archive/runtime-simplification-acceptance.md for evidence and
remaining strict/native Input/Render gaps. Advance only the authorized retained
task with the smallest verifiable changes. Do not restore Service/Provider,
script async/tasks, caller-borrowed Rust roots/arguments/returns/views,
transparent host paths/leases, dependent bindings or JIT-only preparation,
including through wrappers.
Do not build replacement opaque objects, conversion APIs, or a re-entry model.
Retain the dynamic language, ordinary modules/packages/traits, existing
independent synchronous native/value calls, script reflection, non-moving GC,
finite budgets, permissions, interpreter optimizations, CLI/browser execution,
LSP, and planned debugger support. Preserve compatible script state and exact
old executable generations for active calls and retained closures across reload.
Do not add general script generics, runtime schema mutation, monkey patching,
moving GC, or Rust host state under script GC. Keep the project domain-neutral
and modular. Validate focused changes and checkpoint gates from docs/validation.md;
label old evidence and deferred work honestly. Update status/design documents
only when needed and within assigned ownership. Follow the current request's
file and commit limits; use small verified Conventional Commits when authorized.
After removal acceptance, advance retained language/tooling/release gaps without
reactivating the archived roadmap or treating deferred interop as approved work.
```

## Milestones And Checkpoints

The [runnable removal checkpoints](runtime-simplification-plan.md#runnable-checkpoints)
A–E are complete for removal implementation and software gates, not the old
M7-M23 feature schedule. [Current progress](progress.md) and the
[acceptance report](archive/runtime-simplification-acceptance.md) record proof
and the separate strict/native Input/Render gaps. The table below records each
checkpoint's target and required proof. Each checkpoint retained a runnable
synchronous language; format rejection belongs with the checkpoint that changes
the format, not a later cleanup.

| Checkpoint | Target | Required proof |
|---|---|---|
| A | Consolidate removal contracts and identify the existing retained baseline and deletion dependencies. | Active docs/agent constraints agree; retained synchronous language, native/value calls, and script reload have named existing tests. No replacement design or runtime-removal claim. |
| B | Remove Service and Provider families across declarations, compilation, dispatch, artifacts, tooling, and callers. | Ordinary functions, methods, traits, modules, packages, and script reload remain usable; special paths are rejected. |
| C | Remove detached task admission, then async syntax, registration, execution, and CLI mode. | Bounded synchronous execution remains; removed syntax/APIs cannot execute. |
| D | Remove borrowed roots/returns/views, transparent paths, leases/release, and dependent bindings/registrations/callers. | Independent native/value calls remain; no direct, wrapped, reflected, or artifact boundary bypass remains. No replacement object surface required. |
| E | Finish dead metadata, JIT-only preparation, dependencies, tooling/site/examples, and repository acceptance. | Retained behavior, format rejection, and affected repository gates pass; actual supported interop and separately deferred work are recorded. |

After A-E, retained roadmap tracks are language/container conformance, LSP
coverage and incrementality, interpreter measurement/optimization, debugger/DAP,
and release hardening. Previous M20.5/M21/M23 names may identify their historical
context, not reinstate Service, borrowed-host, async, or JIT prerequisites.
No new object ownership, storage, re-entry, serialization-graph, or opaque
reload/reflection contract is approved by this roadmap.

## Acceptance And Historical Evidence

Removal acceptance uses existing supported value/native calls: compile ordinary
modules/packages, execute synchronously under finite budgets, inspect/mutate
script-owned data, and explicitly receive results in Rust without implicit
write-back. Reload must preserve compatible script state, reject incompatible
changes without changing the active owner, run old closures against their exact
old code/state/dispatch/plans, and use new code for new entry calls. Retained
CLI, browser, reflection, and LSP paths need affected regression proof. No new
opaque-object demo or host-object migration is required.

[architecture.md](architecture.md#existing-retained-baseline-tests) identifies
existing baseline tests and [validation.md](validation.md) owns repository gates.
Passing old baseline tests alone does not prove that removed entrypoints are gone.

The [old goal and detailed milestone snapshot](archive/goal-before-runtime-simplification.md)
preserves previous product targets, `/goal`, and acceptance demos as historical
implementation context, not active requirements or proof of current support.
Active subsystem docs describe the reduced baseline. In-place historical plans
carry explicit notices; their old status labels and prompts are not active work.
See the
[architecture reference policy](architecture.md#subsystem-references-and-old-implementation-contracts).
