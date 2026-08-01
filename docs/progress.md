# Progress

This file records current implementation truth, the active checkpoint, and the
remaining gaps. It is not a changelog.

Detailed progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Completed execution plans and acceptance reports live under
[archive](archive/); routine implementation history belongs in Git.

## Current Focus

The Rust/Vela interop checkpoint is complete under the
[final interop and explicit-release hard switch](rust-vela-interop-final-shape-hard-switch-plan.md).
E0-E5 removed compiler-driven Host release, added authored
`host::try_release(value) -> bool`, made await validate the complete active
resource table, completed typed `service::base`/`service::pinned` dispatch,
rejected old artifacts, and passed the repository acceptance matrix. The
[acceptance report](archive/rust-vela-interop-hard-switch-acceptance-2026-07-31.md)
owns the detailed proof. There is no compatibility release mode, legacy
artifact loader, contextual Service alias, or second Service dispatch path.

The active implementation focus is M20.75 Batch F acceptance and cleanup.
M20.5 incremental HIR re-lowering remains its prior in-progress
checkpoint and resumes after the active product goal or at an explicit focus
change.

M20.75 host-scoped detached async execution is fully designed; Batches A-E
are accepted, and Batch F is in progress. Its
[execution plan](host-scoped-detached-async-execution-plan.md) hard-switches
Vela and generated Service applications to a domain-neutral bounded host task
scope. It permits synchronous ordinary functions and Service patches to admit
statically linked async workers on isolated Runtimes, with exact Service
generation pinning and optional safe-point continuations. No compatibility
surface, TaskHandle, shared Runtime, dynamic target, or framework-specific API
is planned. HIR now records both task forms as non-escaping lexical
capabilities and rejects dynamic/non-function shapes, synchronous workers, and
asynchronous continuations before compilation. `TaskSpawn` is now a first-class
capability and effect bit across MIR, binding schemas, registry/reflection
metadata, Service validation, tooling schemas, and hot-reload ABI comparison.
Dedicated MIR task operations now preserve the worker arguments and stable
worker/continuation identities without executing the worker in the parent
Runtime. They require a safepoint, charge the call budget, and contribute the
worker/continuation effect closure to the spawning root. Later Batch A work
adds the frozen continuation ABI and authority contracts, then hard-switches
portable artifacts from current version 2 to version 3. One shared
`Detachability` fact now classifies recursively owned values, statically
rejects known Host references, borrowed views, iterators, and callables with a
nested contract path, and preserves mandatory runtime checking for `Any` and
opaque storage. The continuation ABI is also sealed: its first parameter must
be the exact owned `Result<WorkerReturn, task::Error>`, while trailing
parameters are preserved separately as fresh host safe-point resume inputs.
Semantic analysis treats only the statically owned worker-call position as
detached; ordinary async calls still require `await`.

Batch D adds an owned `ScopedTaskCompletion` and bounded host completion-queue
protocol. A worker publishes only after its isolated Runtime is dropped; the
completion retains the exact ordinary or complete Service generation until a
host safe point consumes or cancels it. `resume` creates a new synchronous root,
prepends an owned `Result<T, task::Error>` without flattening aliases or cycles,
and accepts only freshly constructed trailing `CallArgs`. Cancellation is
one-way and makes later resume a no-op. The generic actor-style example and
request-lifecycle race adapter keep framework vocabulary outside Vela core.
Verified MIR call-graph closure, rather than provisional callable descriptors,
now seals worker and continuation effects into artifact metadata, so nested
database/IO/Host work cannot bypass Engine, policy, or Service ceilings. No new
unsafe boundary was required.

Rust embedding now has one public registration vocabulary: every derived or
generated Value/Host uses `register_type::<T>()`, callable bundles use
`register_exports(...)`, and each generated service domain owns one application
builder. `Service<dyn Trait>` fields declare the domain schema; concrete default
instances, service-owned Runtime leasing, call options, Engine sealing, schema
validation, and the initial generation converge at `.build()`. Business Host
contexts do not carry Runtime authority. `ScriptHost` emits its Host object
contract directly. The former service-set registration/construction surface,
Host/Value-specific builder aliases, and shape-specific `script_*` callable
macros have been removed without compatibility shims.

Phase status:

- E0 accepted: the final explicit-release, namespaced Service capability, and
  typed-base totality contract is frozen in the interop plans.
- E1 accepted: last-use, lexical-scope, branch-edge, and pre-await automatic
  release scheduling is deleted. Authored strict `host::release` and narrowly
  idempotent `host::try_release -> bool` lower to distinct dedicated
  MIR/bytecode operations; root teardown remains unchanged.
- E2 accepted: sealed callable facts identify View, MutView, and lazy Host
  iterator resources, including resource transfer through iterator adapters.
  Discarded and unnamed producers fail before execution, and tooling exposes
  both authored release operations without liveness inference.
- E3 accepted: every await checks the complete active scoped-resource table
  before polling ready or pending targets. Dead locals still block; explicit
  release permits suspension; root Host futures and teardown remain RAII.
- E4 accepted: compiler-owned `service::base::*` and `service::pinned::*`
  replace the contextual receivers without aliases. Generated sync and async
  typed thunks invoke non-`'static`, non-`Sync` Host defaults through one
  reviewed root reborrow boundary; pinned Rust/Vela chaining, target base,
  old-root isolation, cancellation, and panic cleanup are executable.
- E5 accepted and superseded by M20.75 portability: its format version 2
  explicit-release gate passed the representative ordinary/Service fixtures,
  release/base benchmark rows, structural audits, and repository matrix. The
  active artifact contract is now version 3.
- S0 accepted: the migration inventory, executable fixture, and boundary
  baselines are frozen.
- S1 accepted: the callable-level replacement model is deleted without aliases
  or a compatibility path.
- S2 accepted: one sealed `TypeBinding` registry, compact root-local `HostRef`
  slots, prepared typed thunks, and allocation-free common-arity preflight are
  validated.
- S3 accepted: standard Rust type bindings, borrowed collection views,
  collection protocols, prepared host operations, and the phase-wide gate are
  complete.
- S4 accepted: generated Rust-only service contracts publish and pin one
  complete immutable generation with direct zero-VM Rust defaults.
- S5 accepted as a foundation: sparse Vela implementations, exact-base Delta
  inheritance, static Service dispatch, custom Values, host-backed
  collections, scoped borrowed returns, and atomic nested reborrow are
  validated in one mixed Rust/Vela generation. The active spelling is
  `service::base::*` / `service::pinned::*`.
- S6 accepted: async lifecycle/lease proof, immutable deployment bundles and
  dry-run diagnostics, service-only handler/rule/event roles, CLI/LSP service
  and TypeBinding metadata, replacement examples, active-Vela benchmarks, and
  the phase-wide gate are complete.
- S7 accepted: the representative host-framework chain publishes two
  exact-base Deltas and an equivalent folded Snapshot through one unchanged
  async caller; registered constructors/methods, nested views and grouping,
  business Result, old/new in-flight roots, publication-only rollback, stable
  boundary measurements, and the final repository gate are complete.
- M20.75 Batches A-E accepted; Batch F in progress: the complete language, ownership, effect, Service-generation,
  continuation, host-lifecycle, unsafe-audit, and acceptance contract is frozen
  in the host-scoped detached async execution plan. Static HIR task shapes and
  target asyncness are implemented, and `TaskSpawn` now propagates through the
  compiler/host effect and capability model. Static compile targets also retain
  exact worker/continuation identity. Dedicated MIR task operations capture
  arguments in the parent turn, carry a safepoint and call-budget charge, and
  close effective effects over both static targets. Static detachability and
  runtime-check requirements are sealed into each task target and reverified
  in MIR. The compiler-owned `task::Error` and exact continuation outcome plus
  trailing resume contract are likewise sealed and reverified. The
  executor-neutral host admission protocol, finite task policy, owned execution
  capsule, exact ordinary/Service generation identity, authority intersection,
  outcome, cancellation, and structured error contracts are implemented.
  Portable program, Service bundle, and deployment metadata now hard-switch to
  version 3; linked and portable artifacts seal and validate task feature bits,
  static target slots, callable ABI/asyncness, detachability, transitive
  effects, continuation ABI, and originating-Service requirements. Versions 1
  and 2 reject before linking or activation. Batch B completed the task
  bytecode operation, owned graph transfer, scope installation, and an ordinary
  fresh-Runtime vertical slice. Dedicated unlinked/linked task bytecode now
  preserves static worker/continuation handles and owned argument preparation,
  participates in verification and call budgeting, and reaches an explicit VM
  task boundary. An ordinary Runtime without installed scope deterministically
  reports `TaskScopeUnavailable`. `CallOptions` can now install one explicit
  owned `TaskScope`; a synchronous caller admits the prepared operation through
  `ScopedTaskHost`, returns immediately, and the admitted future constructs a
  fresh Runtime from the exact artifact. One runtime-independent
  `DetachedValueImage` transfers all roots together, preserves cross-argument
  aliases and cycles, rejects hidden HostRef/callable/iterator/proxy values with
  nested paths, and transactionally charges export/import budgets. The focused
  isolation proof leaves parent VM state at 100 while the child observes its
  independent initial state. A deliberately pending native worker returns a
  nested owned result through the same async session driver. Scope absence,
  capacity refusal, host-call limits, deadline, explicit cancellation, direct
  future drop, worker error, and Rust panic are executable; all cleanup drops
  the pending child Runtime/native future before publishing a terminal result.
  Batch C integrates one generated `PinnedServiceExecution` capsule with the
  exact whole Service generation. Every generated application now requires an
  explicit finite task scope and emergency patch ceiling containing
  `TaskSpawn`; `RustDefaultEffects` and `PatchEffectCeiling` are separate schema
  and compiler facts. Ordinary helpers inherit their unique Service origin,
  while each child restores the exact dispatcher, artifact, Runtime binding,
  options, and generation identity. The executable proof suspends on host I/O,
  reloads, then observes 106 through the old Rust-pinned generation and 1006
  through the new Vela-pinned generation without changing a Rust trait ABI.
  Batch D delivers owned completion records, bounded host completion queues,
  fresh-root safe-point continuation delivery, one-way cancellation, and
  generation pinning until delivery or cancellation. Continuations receive an
  owned `Result<T, task::Error>` plus fresh trailing host arguments, never run
  on the worker context, and retain no parent Runtime or borrow.
  Batch E hardens version 3 portability, sealed reflection facts, static LSP
  diagnostics/navigation, and host-only lifecycle observation. Scope-local
  task IDs, structured events, saturating metrics, bounded exact-artifact
  Runtime pooling, concurrent teardown stress, recursive quota exhaustion, an
  interpreter-only benchmark harness, and a runnable Service hotfix example
  are covered. Pooled Runtimes clear all mutable owners and rerun artifact
  initialization before reuse; observer failure is contained and no task ID or
  control handle enters Vela.
- P0-P3 accepted for service return totality: recursive macro diagnostics now
  reject nested, exclusive-envelope, projected-child, and otherwise
  non-executable borrowed returns. Exact direct parameters, direct borrowed
  collection parameters, `Option<&T>`, and `Result<&T, E>` execute through
  Rust defaults, nested Vela calls, and unchanged Rust callers. The controlled
  terminal sink validates the exact call-scoped HostRef and reuses the
  authored Rust borrow without unsafe reference fabrication. Projected Host
  children remain an ordinary Host-method capability and are not Service
  return types.
- P4 accepted for target-directed construction and lowering: sealed storage
  chooses Value temporaries or Host leases; registered call-scoped Host
  constructors feed shared and exclusive Rust service parameters and reclaim
  their objects at root teardown; Runtime-owned constructors remain explicit;
  transformed owned collections lower recursively; script-owned mutable
  copy-back rejects before authored Rust runs; and Host collection views retain
  zero-copy identity and write-through.
- P5 accepted for lifetime, permission, and dispatch parity: direct and nested
  service calls retain atomic alias preflight and async leases; old roots keep
  their pinned generation; scoped children cannot escape through state, root
  returns, closures, async suspension, dynamic calls, or reflection; and
  `service::base`/`service::pinned` are compiler-owned static namespace paths
  that cannot become dynamic or reflected callable values; `base` and
  `services` remain ordinary local names.
- P6 accepted for runnable coverage: `service_hotfix_coverage` drives one
  unchanged async Rust caller through RustDefault, a sparse Snapshot, two
  exact-base Deltas, old-root isolation, rejected stale/ABI-incompatible
  candidates, a folded Snapshot, and conditional rollback. The same fixed
  transcript covers direct/optional/fallible Host returns to Rust,
  same-generation nesting, zero-copy Row arguments, call-scoped Host
  reclamation, owned/shared collection lowering, and mutable copy-back
  rejection.
- P7 accepted for final validation: formatting, workspace and example Clippy,
  all-feature workspace and example tests, documentation, benchmark builds,
  fuzz binaries, VS Code packaging, website checks/build, architecture size
  policy, and generated-path structural audits all pass.
- Cross-cutting host-method checkpoint accepted: grouped `#[vela_macros::methods]`
  exports accept explicit additive `effects(...)`, so read-only receivers may
  truthfully declare event, time, random, I/O, or reflection effects without
  falling back to the older, less complete method adapter path.
- Cross-cutting service-domain ergonomics checkpoint accepted:
  `#[service_domain]` generates one application builder, retains stateful Rust
  default instances, exposes request-safe-point pinning through
  `app.with_request` / `app.with_request_async`,
  and centralizes revisioned multi-file Snapshot source and bundle deployment
  behind `app.patches()`. `PatchEdit::Put/Remove` submits only changed virtual
  files while compilation consumes the complete checksummed `PatchRevision`;
  exact-base edits, source-state rollback, complete replacement after
  source-less bundle activation, and portable compile/load are covered.
  The removed `#[service_set]`, default-type field attribute, split
  register/new construction, public generation construction, and `stage_rust`
  APIs, plus the single-string `stage_snapshot_source` entry, have no aliases.
- Cross-cutting public-API cleanup checkpoint accepted: the standalone
  administrative script ABI/bundle model is deleted in favor of dedicated
  Services, and the embedding prelude now exposes the ordinary Engine/Runtime
  and Service Patch authoring path rather than reflection, HIR, HostAccess, or
  service-controller internals.
- Cross-cutting Runtime API cleanup checkpoint accepted: ordinary and
  reloadable execution now share `compile_source` plus `Runtime::builder`;
  `with_hot_reload` promotes that linked program to generation zero, and
  `stage_reload` plus `activate_reload` replace the split compile/stage/apply
  method families without compatibility aliases.
- Cross-cutting embedding ergonomics checkpoint accepted: generated Service
  applications expose sync and async one-request closures that pin exactly one
  generation; Rust type plus inherent exports have one combined registration
  helper; and Rust bindgen uses one schema-only builder instead of a separate
  options object and free generation function.
- Cross-cutting call-scoped Host checkpoint accepted: schema-only
  `register_host_type` registration seals stable Host contracts without a Rust
  `TypeId`; `with_host_mut` accepts `Send`, non-`Sync`, non-`'static` objects
  through one exclusive root lease; erased sync/async Host methods dispatch
  without `Any`; their detached `HostCallValue` boundary round-trips derived
  Rust Value records, enums, and collections through the standard typed
  codecs through the single existing Host method ABI; and generated Services
  keep authored
  `&mut RequestContext<'_, A>` signatures without a Runtime slot or authority
  implementation. The focused regression holds an exclusive Host lease across
  a pending Rust future and reborrows the context after resume.

S3 provides recursive standard bindings; exact owned/shared/exclusive
View and MutView facts; scoped reborrow for borrowed collections; prepared
field, index, and key access; call-scoped Array, Map, and Set iterators with
frozen traversal structure and live prepared reads; terminal iterator fold and
collection; prepared Array searches; live read-only Array, Map, and Set
callback traversal, including Array and Map grouping; bounded collection
projections; complex child views with
exact nested identity and lifetime enforcement; and immediate write-through
for the implemented Array, Map, and Set mutations. User-defined Sequence,
MapLike, and SetLike adapters reuse the same protocol, traversal, callback,
budget, and mutation paths. Bulk clear/extend/retain operations preflight
budgets, conversions, and stale snapshots before mutation. The explicit
standard collection matrix covers owned round trips, shared reads and mutation
rejection, fixed mutable replacement and growth rejection, growable mutable
write-through, Bytes views, and distinct BTree/Hash ABI. The S3 exit
proof covers the complete element/key method surface, resumable traversal,
dense typed element methods, lease-aware dynamic caches, and target resolution
independent of element count. The generated Rust-only service generation
creates no `HostRef`, performs no VM entry, and allocates nothing after root
pinning when a method selects the Rust default.

S5 adds explicit internal service-call targets that are invisible to ordinary
source registration, one immutable dispatcher per published generation, and
same-session re-entry for `base` and pinned `services` calls. The acceptance
fixture constructs a custom `PatchCommand` Value in Vela, preserves a mutable
Vec identity through Rust defaults and Vela selections, proves immediate
write-through and old-root isolation, routes a Vela-selected scoped borrowed
return into another Rust service, and rejects duplicate exclusive aliases
before business Rust executes.

S6 now preserves ordinary authored Rust async traits while generating a hidden
object-safe dispatcher returning `Send` service futures. One actor-owned,
mutex-free Runtime slot is removed from its host context for the duration of a
Vela-selected call and restored on completion, cancellation, drop, or unwind.
The pinned dispatcher/artifact and complete host lease set survive suspension;
the fixture proves direct host write-through, awaited Rust `base`, isolated
actors, old/new-root generation behavior, and non-rollback of effects already
performed before cancellation or panic.

S7 integrates that model into one domain-neutral handler/rule/inventory/reward/
event chain. The host pins once per request; business code holds no target
strings, Runtime values, patch branches, or Vela adapters. A rule Snapshot and
two successive Deltas form one complete generation, then fold into an
equivalent Snapshot. The chain exercises a registered Value constructor,
registered Host methods, mutable actor references, DTO slices, nested Array/
Map values, View/MutView grouping and write-through, Result propagation, and
async handling. Read-only Value slices materialize only at the Vela boundary
and decode into invocation-scoped Rust slices for same-generation `base`;
Host-backed and mutable collections retain HostRef identity and leases.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M7 | Complete | Source-to-VM-to-HostAccess-to-reload vertical slice, budgets, managed heap, and GC roots are validated. |
| M8-M18 | Complete enough | Language, HIR, host bridge, reflection, stdlib, embedding, reload, diagnostics, examples, and benchmark foundations satisfy their checkpoints. |
| M19 | Complete enough | Remaining interpreter costs belong to later cache, layout, or backend work. |
| M19.5 | Complete enough | Cache-ready IDs, linked bytecode, profile ownership, and prepared host paths are validated. |
| M20 | Complete enough | Actor Runtime/cache ownership, lifetime, reload, and concurrency gates are accepted. |
| M20.5 | In progress | Per-keystroke latency is fixed for requests and diagnostics; the HIR rebuild is still whole-workspace. |
| M20.75 | In progress | Batches A-C are accepted: static task contracts, v3 artifacts, isolated child Runtimes, transferable graphs, finite lifecycle/error paths, and exact generated Service-generation execution are validated. Batch D safe-point continuation delivery is active. |
| Rust/Vela service interop | Complete | S0-S7, P0-P7, and E0-E5 are accepted; explicit release, typed Service namespaces, artifact v2 rejection, and repository proof are complete. |
| M21 | Not started | Debugger runtime hooks and DAP integration. |
| M22 | Not started | Cranelift JIT after interpreter, cache, and debugger contracts stabilize. |
| M23 | Not started | Release hardening, public documentation, validation, and performance targets. |

## Current Baseline

### Language And Runtime

- Vela uses lossless syntax, HIR, analysis facts, verified MIR, linked bytecode,
  and one production interpreter route.
- Functions, closures, records, enums, traits, pattern matching, loops,
  iterators, parameterized collections, Option/Result, and controlled
  reflection have executable coverage.
- Execution, memory, call-depth, collection-growth, and registered host-call
  budgets are enforced.
  Script objects use non-moving managed storage; Rust host state stays outside
  the script GC.
- `LinkedArtifact` is the sole production executable generation. Sync and async
  execution share one explicit frame driver, and old generations remain pinned
  across active or suspended calls.
- Async outer calls accept an optional `CallControl` and cooperative host-clock
  deadline. Hosts can observe running/pending/terminal state and poll count;
  cancellation wakes the task and drops execution through existing RAII
  cleanup without rolling back completed effects.
- Ordinary detached child admission now gives every child an isolated Runtime,
  owned transferable graph, finite host lifecycle policy, exact linked
  artifact, contained panic/error outcome, and no parent borrow. Exact
  whole-Service-generation execution and fresh-root safe-point continuations
  are accepted; portability, tooling, stress, and benchmark hardening remain
  the active M20.75 work.

### Host Boundary And Embedding

- Scripts mutate Rust-owned state only through `HostRef`, `HostPath`,
  `PathProxy`, `HostTargetPlan`, and call-scoped `HostAccess`.
- Host reads, writes, compound mutations, methods, permissions, generations,
  lease conflicts, retained borrows, and same-session re-entry are covered.
- The compiler emits no proven-last-use, scope-edge, branch-edge, overwrite, or
  pre-await Host releases. Only authored strict `host::release`, authored
  idempotent `host::try_release`, terminal Service transfer, and root teardown
  release retained scoped capabilities.
- Generated synchronous functions and methods support direct `&T`,
  `Option<&T>`, and `Result<&T, E>` scoped returns for registered host-backed
  types. Successful envelopes preserve receiver or unique-parameter
  provenance, owner leases, generation, and read-only access; `None`/`Err`
  create no HostRef. Persistent state, root-result, closure, async-suspend,
  dynamic, and reflection paths enforce the same non-escape boundary, and
  async borrowed-return signatures are rejected during macro expansion.
- Centralized `external_host` companions can publish typed read-only fields
  through one `vela_fields!` block. Vela uses property syntax for those fields,
  and dispatch remains statically registered with no runtime field-name lookup.
  Scoped method/property HostRef producers require a nameable handle and
  explicit release; scalar and owned-value path chaining remains supported.
- One sealed `TypeBinding` model supplies stable identity, ABI, codecs,
  constructors, methods, fields, protocols, and owned/shared/exclusive
  representation facts to runtime, reflection, compiler analysis, and LSP.
- The former callable-level replacement implementation is absent. Generated
  `#[service]` and `#[service_domain]` contracts provide sealed schemas,
  instance-supplied direct Rust defaults, whole-generation
  staging/publication, request-scope root pinning, and conditional rollback.
  The generated application joins Engine and domain construction, while its
  patch facade owns the routine virtual-workspace-to-publication path. Sparse
  Vela methods compile to stable hidden targets, bind to one verified artifact,
  and execute through generated Snapshot and exact-base Delta adapters with
  explicit Runtime authority. Delta inheritance rebinds all Vela targets to
  one artifact; explicit `RustDefault`, stale-base and stale-source rejection,
  effect ceilings, failure-without-fallback, and rollback are covered.

### Standard Library, Tooling, And Proof

- Arrays, maps, sets, strings, bytes, iterators, Option/Result, math, context,
  deterministic time, controlled random, stdio, and sandboxed filesystem
  helpers have runtime and analysis coverage.
- The native language service and LSP cover diagnostics, completion, signature
  help, hover/navigation, symbols, semantic tokens, references, rename, code
  actions, formatting, inlay hints, watching, cancellation, and schema reload.
- Runnable examples, conformance fixtures, fuzz targets, benchmark harnesses,
  and documentation provide end-to-end proof.
- Durable performance rules and current baseline summaries live in
  [performance.md](performance.md); detailed measurements live under
  [archive](archive/).

## Accepted Interop Baseline

### Service Patchability Totality

The admitted borrowed-return matrix remains executable for direct `&T`, direct
`&mut T`, `Option<&T>`, and `Result<&T, E>` when the return is the exact direct
Host parameter. Vela-selected outer Rust calls restore the authored borrow
without fabricating references; nested borrowed containers and projected
children fail during macro expansion.

The admitted Service boundary is now total: non-`'static` call-scoped Host
parameters reach sync and async Rust defaults through generated typed thunks,
and pinned calls may select Rust or Vela before a target patch calls its own
base. The contextual receiver spellings are rejected; only
`service::base::*` / `service::pinned::*` are compiler-owned paths. Portable
format version 3 rejects both pre-explicit-release and pre-task-metadata
artifacts before activation. The accepted contract and gates are in the
[final interop plan](rust-vela-interop-final-shape-hard-switch-plan.md).

Shared custom service parameters now use one storage-directed boundary:
sealed Value storage decodes one invocation-local temporary, while sealed Host
storage acquires a shared lease. The same generated Rust caller serializes a
borrowed Value or injects a Host without a patch-specific branch. This works
for synchronous and async service defaults. Host constructors declare
`CallScoped` or `RuntimeOwned`; call-scoped objects are reclaimed at root
teardown and their lifetime enters Type ABI and exported schema facts.
Transformed Value arrays lower to owned `Vec<T>` or temporary `&[T]`; mutable
script-owned arrays still fail before the authored Rust body, while Host
collection views retain zero-copy write-through.

CLI/LSP schema metadata reports each Host service parameter's reachable
`Injected`, `Constructible`, and `ProducedBorrow` origins from the same sealed
service and TypeBinding facts. Dispatch/lifetime parity and the consolidated
runnable coverage demo are accepted. The P7 repository-wide validation and
final documentation audit are complete.

The archived
[completion plan](archive/rust-vela-service-patchability-completion-plan.md)
owns the
signature whitelist, total-admission invariant, representation-directed
parameter construction, target-directed collection lowering, focused test
matrix, and domain-neutral `service_hotfix_coverage` demo. Durable handles,
cross-root borrows, borrowed children across async suspension, and arbitrary
nested borrowed containers remain outside that plan.
The intended final Rust, Vela, and deployment authoring form is consolidated
in the
[service patchability usage guide](rust-vela-service-patchability-usage.md).

## Active Gaps

### Parameterized Container Contracts

The runtime supports nested Array/Map/Set/Iterator facts, recursive guards,
budgeted deep checks, value-keyed storage, compiler-owned mutator checks,
macro inference, serde/reflection preservation, ABI comparison, contract
stamps, and lazy iterator item guards. The remaining work is an explicit
acceptance audit against
[container-type-hints-plan.md](container-type-hints-plan.md) and
[value-keyed-map-set-plan.md](value-keyed-map-set-plan.md).

### M20.5 Incremental Model

The named editor-visible failure was that every keystroke rebuilt the whole
workspace's `AnalysisFacts` once per request and every background request deep
copied the databases. On a 128-module fixture a keystroke cost 5.9 s and a
completion 11.2 s. Facts are now memoized per workspace generation, flow
narrowing and schema validation no longer scale with the square of workspace
size, and snapshots share the databases behind an `Arc`. The same keystroke
costs 346 ms, completion 2.7 ms, hover 0.2 ms. Evidence and the harness are in
[performance.md](performance.md).

The remaining named gap is the HIR layer: any invalidated module still forces a
full `ModuleGraph` rebuild, so `database_update` stays superlinear (222 ms at
128 modules, 871 ms at 256). Making it incremental requires stable HIR ids
across re-lowering, which is the prerequisite for per-module fact reuse as
well. `did_change` also still computes and publishes diagnostics on the message
loop rather than a worker lane.

Other known follow-ups are broader method/schema call-site classification and
suppression of future hints across dynamic `Any` boundaries.

### Deferred Tracks

- M21 debugger/DAP work waits for stable runtime debug contracts.
- M22 Cranelift JIT waits for M20/M21 close-out and consumes the verified
  MIR/linked-artifact contract.
- Typed scalar superinstructions require profile evidence and temporary-register
  liveness.
- Persistent host iterator handles require an explicit lifetime model.
- Persistence, replication, cross-Runtime sharing, structural state migration,
  async-frame migration, and initializer dependency reads remain out of scope.

## Validation

Every implementation commit runs the focused test for its changed behavior.
A phase acceptance checkpoint runs the repository and later-phase gates from
[the hard-switch plan](rust-vela-service-hard-switch-plan.md#5-validation-commands).
Use the relevant subset of [validation.md](validation.md) during implementation.
The phase-closing commit or acceptance report records the commands and final
result; a routine feature commit does not claim phase acceptance from focused
tests alone.

Miri remains unavailable on the installed stable Rust 1.97.1
`x86_64-pc-windows-msvc` toolchain. The erased-borrow boundary relies on its
focused lifecycle, async, lease/re-entry, and source-audit proof until that
changes.

## Next Up

1. Continue M20.75 Batch E with portability corruption tests, tooling
   diagnostics, structural audits, and benchmark coverage.
2. Resume M20.5 incremental HIR re-lowering after the active detached-async
   goal or an explicit focus change.
3. Audit the parameterized container and value-keyed Map/Set plans against
   their explicit acceptance matrices.
4. Keep the shorter Runtime-owned host reclamation policy as a non-blocking
   post-S2 optimization follow-up.

## Update Rules

- Update this file only when current focus, phase status, supported baseline,
  validation expectations, or remaining gaps change.
- Do not append per-commit notes, method-by-method chronology, benchmark logs,
  or rejected candidates.
- Routine implementation commits should not modify this file or the execution
  plan.
- Keep accepted-phase detail in its acceptance report or Git history. Archive
  additional history only when Git is insufficient.
- Keep `Current Focus`, `Active Gaps`, and `Next Up` mutually consistent.
- Use one coherent Conventional Commit per independently verifiable behavior.
  Record focused validation in the commit body when it is not obvious; use one
  explicit checkpoint commit for a phase-wide validation result.
- Fold immediate fixups into their triggering change before shared integration
  when history has not already been published.
