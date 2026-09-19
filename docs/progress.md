# Progress

This file records current implementation truth, the active checkpoint, and the
remaining gaps. It is not a changelog.

Detailed progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Completed execution plans and acceptance reports live under
[archive](archive/); routine implementation history belongs in Git.

## Current Focus

Current implementation focus is the M20.5 local LSP coverage goal under the
[execution plan](lsp-test-execution-plan.md). The execution checkpoint owns
accepted batches and the next local task. Incremental HIR re-lowering remains an open
follow-up and may be required by the local scale gate. The post-M20
[Verified-MIR superinstruction and basic-block interpreter plan](verified-mir-superinstruction-basic-block-interpreter-plan.md)
is complete.
Batch A is accepted with newly captured quiet-machine scalar, VM, Host,
Service, async, scoped-task, Actor memory/concurrency, compile-resource, and
portable-size baselines. Earlier overloaded-machine data is excluded. Profiles
and dynamic instruction counts select `I64CmpImmJumpIfFalse` as the first
bounded MIR-native proof while identifying `range_iteration` scalar blocks and
loops as the primary throughput lever. Artifact version 4 is already occupied,
so portable selected plans hard-switch all three surfaces to version 5.
Batch B is accepted: a deterministic `MirBackendHandoff`-only selector maps
every function and block to dense ordinary units, and an independent verifier
re-derives exact statement/terminator, budget/edge, safepoint/root, CFG exit,
source, liveness, and debug coverage before unchanged bytecode lowering. The
five-row runtime geometric mean is +0.302%, compile resources remain within
noise, and portable bytes/checksums are unchanged. Batch C is accepted: the
first MIR-native i64 compare-immediate branch removes exactly 606 predicted
outer dispatches, improves its focused scalar row by 8.86% and the complete
scalar row by 10.30%, and leaves the five-row geometric mean 1.91% faster than
Batch A. Portable program, Service artifact, and deployment formats are now v5
and reject versions 1-4. Batch D is accepted under its
[archived report](archive/verified-mir-interpreter-batch-d-acceptance-2026-08-09.md).
Bounded v5 op/exit/source/charged-target tables and one `RunScalarBlock` entry
execute eligible cyclic three-or-more-op Bool/i64 blocks through the existing
frame driver after short-superinstruction priority. Independent selection,
source-link, physical-plan, portable, budget, trap, profiling, break/continue,
malformed-entry, and ordinary-versus-selected proofs pass. The executor's two
private unchecked slot helpers remain guarded by all verifier layers; tags,
checked arithmetic, partial writes, logical profiling, budget order, exits, and
trap spans remain canonical. The quiet-machine five-row geometric mean is
18.856% faster than Batch A, scalar/range improve 33.487%/47.229%, target-
independent guardrails remain within 5%, and 10,000 additional block entries
allocate zero incremental bytes. Batch E is accepted under its
[archived report](archive/verified-mir-interpreter-batch-e-acceptance-2026-08-09.md).
Verified CFG predecessor/dominator facts select only proven-i64, single-entry,
single-latch scalar range regions; dynamic, safepointed, multi-exit, and
multiple-latch shapes remain ordinary. The one scalar executor preserves exact
header/edge/backedge budgets, later-iteration traps, portable metadata, and
logical loop profile events. `range_iteration` profiled outer dispatch falls
97.45% from Batch D, one versus 10,001 loop iterations adds zero allocations,
and the quiet-machine five-row geometric mean is 34.401% faster than Batch A;
scalar/range improve 38.347%/79.043%. Batch F is accepted under its
[archived report](archive/verified-mir-interpreter-batch-f-acceptance-2026-08-28.md).
Old/new ordinary and Service generations, closures, ready/pending async roots,
providers, detached workers, continuations, Snapshot/Delta/fold/rollback,
generation-owned physical profiling, v5 corruption/round trips, and shared-plan
memory are covered without a second execution path. The portable-plan fuzz
target seeds plan handles, operands, coverage, exits, source points, and payload
limits and runs in CI. Batch G and the complete track are accepted under the
[final acceptance report](archive/verified-mir-interpreter-final-acceptance-2026-08-28.md).
The final scalar/range rows improve 37.634%/75.796% from Batch A and the frozen
five-row geometric mean improves 32.215%; semantic, artifact, reload, async,
Service, memory, structural, and repository gates pass. Production retains one
frame driver, verified-MIR-owned selection, version 5 portable plans, exact-
generation profiles, and ordinary instructions as the universal fallback.

The Rust/Vela interop checkpoint is complete under the
[final interop and explicit-release hard switch](rust-vela-interop-final-shape-hard-switch-plan.md).
E0-E5 removed compiler-driven Host release, added authored
`host::try_release(value) -> bool`, made await validate the complete active
resource table, completed typed `service::base`/`service::pinned` dispatch,
rejected old artifacts, and passed the repository acceptance matrix. The
[acceptance report](archive/rust-vela-interop-hard-switch-acceptance-2026-07-31.md)
owns the detailed proof. There is no compatibility release mode, legacy
artifact loader, contextual Service alias, or second Service dispatch path.

M20.75 is complete under the
[host-scoped detached async acceptance report](archive/host-scoped-detached-async-acceptance-2026-08-01.md).
All six batches, the complete acceptance matrix, structural audits, examples,
benchmarks, and repository gates pass without a compatibility path or new
unsafe boundary.

M20.75 host-scoped detached async execution is accepted. Its
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
Dedicated MIR task operations preserve the worker arguments and stable
worker/continuation identities without executing the worker in the parent
Runtime. They require a safepoint, charge the call budget, and contribute the
worker/continuation effect closure to the spawning root. Batch A froze the
continuation ABI and authority contracts and hard-switched portable artifacts
from version 2 to version 3. One shared
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

Rust embedding now has one public registration model: derived/generated and
manual Values/Hosts produce `TypeRegistration<T>`, individual methods and
method groups produce `MethodRegistration<T>`/`MethodsRegistration<T>`, and free-function modules produce
`ModuleRegistration`. Applications collect all three in `VelaBindings` and
install it once. Each generated service domain still owns one application
builder. `Service<dyn Trait>` fields declare the domain schema; concrete default
instances, service-owned Runtime leasing, call options, Engine sealing, schema
validation, and the initial generation converge at `.build()`. Business Host
contexts do not carry Runtime authority. `ScriptHost` emits its Host object
contract directly. The former service-set registration/construction surface,
Host/Value-specific builder aliases, and shape-specific `script_*` callable
macros have been removed without compatibility shims.

Vela's supported embedding surface is Rust-only. The former built-in C ABI
crate and its duplicate value, ownership, error, and async boundaries are
removed; downstream adapters may wrap the Rust API without becoming a core
compatibility contract.

Arbitrary concrete Rust types now participate in that same model without a
business newtype or `ScriptHostObject` implementation.
`TypeRegistration::<T>::host(path)` installs the opaque Host identity,
`MethodRegistration::<T>::shared`/`exclusive` installs the explicitly selected
surface, and `#[vela(host = path)]` projects a derived parent field through an
internal erased scoped wrapper. The end-to-end fixture uses `VecDeque<i64>`
directly and proves both shared and exclusive registered methods.

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
  active artifact contract is now version 5; version 4 added static HostRef
  collection iteration shape to the accepted version 3 task contract, and
  version 5 adds canonical selected physical-plan coverage.
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
- M20.75 Batches A-F accepted: the complete language, ownership, effect,
  Service-generation, continuation, host-lifecycle, unsafe-audit, and
  acceptance contract is frozen in the host-scoped detached async execution
  plan. Static HIR task shapes and
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
  Batch F aligned semantic-input tests with the earlier analysis diagnostics,
  completed source and generated-path audits, and passed the full repository,
  examples, documentation, benchmark-build, fuzz-build, editor, Tree-sitter,
  and website matrix. The archived acceptance report owns the durable proof.
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
| M20.75 | Complete | Static scoped task forms, isolated owned execution, exact Service generations, safe-point continuations, v3 portability, tooling, observation, stress, and repository gates are accepted. |
| Rust/Vela service interop | Complete | S0-S7, P0-P7, and E0-E5 are accepted; explicit release, typed Service namespaces, active artifact v3 rejection, and repository proof are complete. |
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
  are accepted together with v3 portability, tooling, stress, lifecycle
  observation, bounded clean Runtime reuse, benchmarks, and runnable examples.

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
format version 4 rejects versions 1 through 3 before activation; it includes
the explicit-release/task metadata and HostRef iteration-shape hard switches.
The accepted contract and gates are in the
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

Installed VSIX smoke coverage now exercises activation, bundled-server startup,
package discovery, local/cross-file navigation, F12, unsaved edits, Unicode
paths/ranges, hover, and completion. The Windows/Linux CI gate and logs are
documented in the [editor test guide](../editors/vscode/README.md#automated-editor-tests).
Navigation projects target byte columns to UTF-16. Other outgoing protocol
ranges still need a Unicode projection audit; this checkpoint does not accept
all editor features or schema/remote workflows. Initial discovery currently
loads package manifests at workspace roots; manifest-free disk discovery and
broader multi-root lifecycle coverage remain follow-ups.

The [LSP test strategy](lsp-test-strategy.md) now owns proactive coverage work:
41 protocol rows and the current grammar/token/AST contracts expand into an
executable requirement inventory. CI checks contract drift and exact evidence
references; reports distinguish candidate tests, mapped assertions, executed
proof, and unreviewed requirements. P0 inventory is implemented; P1 semantic
mapping and P2 range/edit transformation coverage remain open. Ordinary CI is
not full acceptance while the separate strict gate reports unresolved cells.
The active execution plan supports alternating Windows/macOS development with
shared batch/child progress and independent registered profiles and audit records.
Each strict run requires complete fresh proof on its selected local profile,
including actual input/render proof and generated/scale checks. B16's
additional environments are deferred follow-up work and do not block B19 local
acceptance. The local package-path baseline is restored. The versioned ownership inventory registers
1327 semantic obligations, 136 local interaction obligations and 14 infrastructure/
later acceptance obligations, with contract/route drift self-tests. Scoped strict
acceptance and the machine-readable checkpoint enforce fresh proof for accepted
batches, explicit reopening and exact remaining scope. B01 shared Unicode,
LF/CRLF, marker/edit and disk/overlay fixtures now feed both Rust layers and the
installed VSIX (nine scenarios). The local Input/Render driver now proves keyboard, pointer, visible candidate and
exact final document/caret behavior. Evidence validation binds current source,
profile, fixture, driver and installed VSIX/server bytes. B00 and B01 passed their combined
strict acceptance. B02 has executable proof for all 120 currently registered
service/protocol obligations. The installed local driver now covers all eight
UX02 command/input obligations, including F12/back, declaration/type palettes,
dirty Unicode source and unknown targets. Passive client logs bind these actions
to exact request positions and response URI/ranges. Installed declaration and
type-definition provider checks now cover exact dirty Unicode LF/CRLF ranges and
unknown nulls. UX03 modifier-click reaches the rendered target. Native Peek
follow/dismiss/unknown workflows now pass on the unlocked local desktop,
including actual context-menu input, exact preview text and aligned highlight,
full target selection, restored source focus, and null-response no-jump proof.
The driver temporarily selects ABC and restores the original input source on
success or failure. The execution checkpoint records fresh strict batch
acceptance. B14 still owns the later semantic
partition completeness review.

B03 is accepted; B04 is active. Current completion proof covers source/schema/stdlib
types and members, enum construction and patterns, named arguments, import and
namespace aliases, direct dependency types, and compiler-owned Service paths.
Shared service/protocol fixtures use frozen complete candidate sets, canonical
resolve metadata and documentation, Unicode LF/CRLF edits, applied references
and receiver members. Local shadowing, private/ambiguous/missing owners, malformed
syntax, erased receivers and occupied argument slots have explicit negatives.
Parameter mapping is shared with signature and expected-argument queries;
expression and parameter-name edits preserve distinct same-name roles.
Type annotations preserve resolvable qualified insertions and semantic source
identity, including dependency aliases and crate paths. Enum tuple entries follow
the existing parameter grammar; empty/name slots are not type annotations.

Source annotations now resolve registered leaf types recursively in declaration
scope across builtin containers, tuples, functions, locals, globals and source
fields/method returns. Same-path source declarations own both positive and
negative results; schema short-name fallback is excluded. Source method lookup
resolves implementation and default-trait targets in their owning module instead
of matching short-name suffixes. Forty-one additional shared callable cases cover
nested type/parameter contracts, Provider implementations/default methods, known
native parameter prefixes, returned members and registry/source collisions.
Nineteen analysis cases check complete structural facts, including hidden
Iterator element types and collection mutation capabilities. Six schema states
check field type/name/doc replacement, missing and invalid metadata, recovery and
persistent-versus-fresh service equivalence. Native positional arguments beyond
known metadata receive no invented parameter names or types; this does not claim
a new variadic registration contract. Detailed child evidence remains in the
execution checkpoint and Git.

Root `service::` completion now offers `base` only for a unique registered
static origin and `pinned` for a nonempty Service set. Compiler-owned namespace
identity prevents ordinary source/schema names from supplying candidates or docs.
Twenty-nine shared cases cover whole-token edits, existing calls, helper origins,
restricted contexts and metadata absence under Unicode LF/CRLF. Nine lifecycle
states cover dirty removed/ambiguous/recovered callers and replaced/empty/invalid/
recovered Service sets, with persistent-versus-fresh service equivalence.

Static task path completion now rejects unknown source/schema operations and
keeps literal task call signatures builtin-owned. Outer operands stay positional;
ordinary aliased source functions and nested worker calls retain their own
parameter contracts. Continuation completion inserts a static function path,
while worker and nested expression completions retain call insertion. Shared
fixtures cover these distinctions, whole-token edits and Unicode LF/CRLF.

Source expression and type completion share a declaration-address resolver that
checks the exact HIR declaration in the current package/import scope. Dependency
functions and constants retain usable dependency prefixes, shadowed local paths
use `crate::`, and inaccessible/transitive declarations cannot leak into lists.
Twelve shared package cases verify complete sets, explicit edits, resolved docs,
applied signatures, owned parameter names and exact definition targets under
Unicode LF/CRLF. Direct qualified dependency paths now navigate without requiring
their spelling to change during import expansion.

Member call facts and navigation retain the receiver's source declaration ID,
including trait hints and same-name dependency types. Qualified/aliased function
returns and inherent/default method returns retain their declaration's package;
local bindings preserve those returned receiver facts. Twenty-four shared package
cases assert complete member sets, edits, signatures, named parameters, resolved
docs and exact applied definitions under Unicode LF/CRLF. Parameter inlay hints
use the same receiver identity and project byte positions to UTF-16 correctly.

Required/default trait method returns and awaited source calls now preserve
their source type identity. Block, if/else-if, match and direct lambda results
share value-path inference; branches with the same declaration preserve that
owner through later completion and navigation. Forty shared package cases check
these flows, exact receiver facts and complete candidate sets under Unicode
LF/CRLF. Member context uses the syntax receiver before lexical recovery, so
semicolons and braces inside a receiver no longer truncate its range.

Local assignment flow now carries possible source declarations alongside type
facts. Twenty shared cases cover reassignment, retained aliases, branch/match/loop
joins and chained method results across same-name package types. Completion merges
shared field types deterministically; ambiguous fields have no arbitrary definition.
Unprefixed HIR paths stay in the requesting package regardless of source insertion
order, and a for-loop keeps a following expression statement outside its body.

Async callable completion preserves the owning source/schema signature's
asyncness, including same-name methods in different packages. Eighteen shared
awaited/unawaited cases check full candidate sets, applied signatures and
parameters, scoped function inlays, and missing-await diagnostics. Trait calls
validate against the declared signature even without an implementation body.
Diagnostic ranges and their labels/repair hints use each document's current text
for protocol UTF-16 projection; invalid ranges cannot be emitted as byte columns.

Twenty-five shared await-context cases retain complete callable choices across
sync functions, lambdas, sync/async impl and trait methods, and nested async
blocks. Fifty-eight candidate applications preserve signatures, parameter
completion, inlays and definitions despite invalid enclosing await contexts.
Exact syntax diagnostic codes, messages, ranges and labels distinguish the
await token, a parenthesized operand and an inner unawaited async call under
Unicode LF/CRLF. Parser diagnostics are compared with explicit fixture oracles.

Static task operand completion now filters workers to declared async functions
and continuations to synchronous functions with the matching required outcome
parameter. Twenty-four shared cases cover qualified/imported targets, unknown
workers, dynamic/schema exclusions, nested expressions and existing call syntax.
Applied candidates preserve resolve ownership and task/await diagnostics; a
changed worker result retains the exact incompatible-continuation diagnostic.
Qualified and imported callable details retain source/schema asyncness.

Sync-only stdlib callback slots now insert function references and exclude known
async function targets. Thirty-six shared cases cover Array/Map/Set,
Option/Result/Iterator, named and reordered fold parameters, source/schema/import
ownership, exact applied definitions and Unicode LF/CRLF edits. Dynamic callback
values remain available; ordinary source/schema methods and nested expressions
keep ordinary call insertion. Same-owner reflection signature alternatives remain
one callable, while compiler-owned task operations cannot become callback values.
Global stdlib candidates now carry explicit builtin ownership for resolve queries.

Function and method completion preserves an existing argument list, including
intervening comments and whitespace, instead of inserting a second call. Twenty-one
shared factory/reference cases apply 82 candidates across source, schema, method,
import and reflection owners. Exact applied text, definitions and await diagnostics
distinguish factory calls from direct sync callback references and new call snippets.
Dynamic callable values keep their existing behavior; this does not infer callback
arity or execute a factory during analysis.

Thirty-six shared task-admission cases apply completion edits and check complete
candidate sets plus exact diagnostic codes, messages, ranges and related labels.
Owned and erased arguments remain available; nested callable/iterator/HostRef
values retain their admission errors. Worker parameter and result errors keep
distinct source ranges. Registered effect ceilings cover individual capabilities,
recursive worker/continuation calls, denied TaskSpawn and absent-ceiling metadata.
Trailing continuation parameters remain a host-resume contract. Both test layers
run Unicode LF/CRLF cases; these static queries do not prove runtime admission.

Eighteen shared callback-contract cases retain declared zero/one/two/defaulted
parameter signatures, dynamic callback values and conservative map result facts.
Direct expression/block lambdas preserve proven record members through eager and
iterator maps; named callbacks, factories, bound local closures and erased values
do not invent result members. Both layers check exact choices, edits, resolve and
definitions under Unicode LF/CRLF. This is static completion proof, not callback
runtime admission or a new callable compatibility policy.

Stdlib callback analysis now orders argument facts by registered parameter slots.
Sixteen shared queries retain fold results and contextual parameters through named
reordering, distinguish two lambdas by their roles, and reject specialization from
unrelated or ambiguous arguments. Executable analysis independently checks the
same slot and return boundaries; queries do not invoke callbacks.

The [completion coverage review](lsp-completion-coverage-review.md) indexes S5
proof and the remaining acceptance work. Shared-method queries now cover merged
details, all signature alternatives and exact/null definitions across package
joins. Incomplete source origins survive field/method return chains and cannot
prove a unique definition; source branches exclude colliding schema signatures.
Protocol tests explicitly query null definitions after applying candidate edits.
Seventeen abrupt-flow cases now exclude unreachable tail receivers and retain
reachable explicit lambda returns. One HIR traversal supplies normal values,
invocation returns and control flags; source origins follow the same result sets.
Local assignment joins now exclude returning branches, stop at exits and retain
loop exit snapshots. Match guards carry their evaluated state to later arms.
Loop headers now include normal/continue backedges under a shared analysis budget;
break/return exits remain terminal. Match values and local environments share
the irrefutable-pattern boundary and exclude unreachable later arms.
Schema callable replacement now has shared sync/async, parameter, return-owner,
metadata-removal/restoration and retained-resolve proof with fresh service checks.
Missing, malformed and unsupported schema files now clear host authoring facts
while retaining source candidates and controlled diagnostics through recovery;
completion's missing/stale-schema state requirements have concrete mappings.
Source callable disk changes, deletion and recreation now have shared root/dep
ownership proof, fresh service/protocol comparisons, exact applied signatures
and definition targets, and diagnostic recovery. Source resolve remains empty
and never borrows colliding schema documentation.
Dirty dependency edits, external disk writes/deletion, save and close/reopen now
have shared disk/overlay oracles and fresh-workspace comparisons. Document sync
also republishes diagnostics for affected open importers; closing a deleted
dependency no longer leaves their old diagnostics visible.
Six source recovery states now retain neighboring completion/signature/definition
facts through malformed declarations, repair and close restoration, with fresh
service/protocol comparisons and real syntax diagnostic publication/clearing.
The protocol rejects old/duplicate didChange versions before applying edits or
publishing diagnostics, preserves signed versions in workspace edits, and resets
the version lifetime on close/open. Deterministic completion/resolve task tests
cover cancellation before publication, current-generation retry, bounded stale
retry failure and successful subsequent requests without stale metadata.
S5 completion/resolve now has a reviewed partition-to-test map covering callable
owners, named/defaulted parameters, active slots and dynamic/unresolved boundaries.
All named-parameter candidates are lightweight and protocol resolve preserves
their full content without parsing; standard callable fixtures also check exact
post-edit signatures and exclude colliding schema documentation.
S10 completion/resolve now has a reviewed ownership map, including local and
parameter candidates without lazy identities, source/schema/builtin modules,
and known Any-return callables whose downstream members remain empty. Actual
protocol resolve preserves candidate fields and uses only owned schema docs.
S8/S9 completion now has reviewed import and recovery coverage. Functions in
`use` insert plain paths; unfinished records at EOF retain owned field candidates
without borrowing closed records. Shared fixtures also check source-backed schema
alias targets, unknown contexts and repaired syntax.
S11 completion now ties repeated queries and incremental cache counters to exact
current candidates. Shared LF/CRLF transitions cover body edits, changed imports
and declarations, transitive invalidation, empty sets and restoration; separate
task tests prove cancellation and bounded stale-generation retry at publication.
S13 completion now has direct template/application and empty-dot inventory proof.
Declaration completion survives preceding trivia; statement keyword prefixes work
in nested blocks, methods and lambdas while operand positions stay expressions.
S14 completion now has reviewed structured-context and item-projection coverage.
Type locations follow CST ownership and direct argument separators; loop/pattern
bindings use their introduction boundaries; reordered named arguments keep their
active parameter index aligned with expected name/type facts. Shared LF/CRLF
fixtures assert exact scopes, candidates, edits and applied reference owners.
S6 completion now has reviewed pattern/control-flow coverage. Record-pattern
labels use their nearest source/schema owner, exclude used fields and preserve
the current label's replacement range. Nested and colon-separated value patterns
remain distinct; applied shorthand fields introduce exact local binding targets.
S1 completion now has reviewed declaration/default contexts. Queries use the
current body's canonical bindings, including initializers, and retain a default
body at an identifier's end boundary. Required trait defaults use canonical
analysis-only HIR roots with owned parameter edits and shadow checks.
S2 completion now has reviewed body/default scope coverage. Eighty-eight shared
LF/CRLF Unicode queries cover nested bindings, captures, guards and all five
compound assignments in four declaration forms. Existing matrices cover
assignment/return/loop flow, callbacks and body templates. Equal-span body
selection prefers inner lambdas; typed-lambda defaults preserve their own colon
and comma boundaries. HIR ownership/import refresh and MIR runtime rejection
have direct tests; required methods remain bodyless.
S7 completion now has reviewed literal/operator coverage. One hundred thirteen
shared LF/CRLF Unicode queries cover value/result types, operands, indexing,
literal-content exclusions and owned Map keys/values. Map values retain expression
contexts; raw and malformed literal content excludes code candidates while
closed interpolation expressions remain eligible. Unclosed Map CSTs retain their
last value. Map-key suggestions use the existing enum owner resolver, preserve
current-key eligibility and replace whole identifiers; applied bare keys retain
their existing logical-string semantics and null definition result.
Completion-resolve editor smoke checks lazy owned documentation, snippets and
whole-identifier edits on dirty Unicode LF/CRLF documents. UX04 separately checks
actual Enter/Tab acceptance, resolved documentation, exact text/caret and undo;
Escape closes suggestions without a completion edit. B03 is accepted.
B04.01 adds an 18-position shared reference/highlight/rename coordinate matrix:
complete sets, shadow separation, actual applied edits and repeated/restored queries
on Unicode LF/CRLF sources. Qualified uses retain terminal-token ranges and are
included in renames; import terminal queries resolve their declaration. Protocol
results project to UTF-16 per document, with client versions only for open files.
B04.02 adds 16 LF/CRLF declaration-rename collision scenarios at both layers.
Bare uses reject capture by visible locals, including parameters in default
expressions under the existing canonical binding rules. Qualified paths, retained
aliases and disjoint scopes remain eligible; applied edits preserve exact call
ownership. The broad S2 cells remain unreviewed pending the other partitions.
Resume B04.03 with local-target rename capture and disjoint-scope coverage.
B00-B03 acceptance snapshots stay fixed; the remaining whole-batch inventory is
1230 requirements while B04 is open.
Windows x64 has installed VSIX and native input coverage for B01/UX02/UX03/UX04:
suggestion acceptance, F12/back and palettes, modifier-click, context-menu Peek,
exact Unicode targets and no-jump outcomes. The original macOS acceptance is
historical; fresh evidence is required when that machine is next used. The earlier
locked macOS desktop does not imply a Windows blocker; see [blocked.md](blocked.md).
Both input profiles author the explicit all-off quickSuggestions setting while
retaining exact observed-setting checks. B16 remains deferred.

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
`aarch64-apple-darwin` toolchain. The pre-existing erased-borrow boundary relies
on its focused lifecycle, async, lease/re-entry, and source-audit proof until
that changes. Batch D's new private unchecked scalar-register access instead
relies on unlinked/portable/linked verifier proofs, malformed-entry tests, and
the fixed non-resizing frame layout; Miri must be added when the toolchain
provides it.

## Next Up

1. Continue LSP B04 references, highlights and rename coverage from
   the execution checkpoint; B00-B03 are accepted. Then complete B05-B15 and
   B17-B19; B16 stays deferred.
2. Address incremental HIR re-lowering and moving `did_change` diagnostics off
   the message loop when required by local state/scale acceptance.
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
