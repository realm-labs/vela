# Host-Scoped Detached Async Execution Plan

This document is the executable design and implementation plan for detached
async work started by Vela code. It extends the sequential executor-neutral
async model in [async-execution-model-plan.md](async-execution-model-plan.md)
without introducing an executor, actor API, or business-domain API into the
language core.

The feature is a hard switch. There are no compatibility aliases, legacy task
names, dual runtime paths, string-target fallbacks, or old artifact readers.
`unsafe` Rust is permitted where the scoped host lifetime cannot be represented
in safe Rust, but only behind the audited boundary defined in section 10.

## Persistent Goal

```text
/goal Implement M20.75 Host-Scoped Detached Async Execution according to
docs/host-scoped-detached-async-execution-plan.md. Preserve Vela's
domain-neutral language and executor-neutral runtime: script code may start a
statically linked ordinary async function as host-scoped detached work, and may
optionally request a Vela continuation that the host resumes at a safe point.
Do not expose TaskHandle, Future values, join, script cancellation, manual
resume, unscoped spawn, shared-memory task concurrency, target strings, or
framework-specific Actor/Request APIs. A detached child owns a fresh Runtime,
owned arguments, independent budgets, and an exact immutable execution
generation; it never carries HostRef, PathProxy, scoped leases, closures, live
iterators, or other call-scoped capabilities out of its parent call.

Make the generated Service application the complete first-class embedding of
this model. Any hotfixable Service method, including a synchronous method, may
start permitted detached async work without changing its Rust sync/async ABI.
The child pins the originating complete Service generation and linked artifact,
and nested service::base/service::pinned calls remain on that generation.
Ordinary module functions and Service selections continue to publish as one
atomic candidate; do not create a second hotfix mechanism.

Keep task scheduling and lifecycle in a host-provided scope. Core crates define
the capability and execution protocol but do not choose Tokio, an actor
mailbox, a request executor, or a GUI loop. Enforce TaskSpawn effects,
capability ceilings, bounded task admission, cancellation, result/error
observation, and continuation safe-point delivery. If unsafe lifetime erasure
is required, confine it to one documented module, audit every invariant and
drop path, add compile-fail and lifecycle tests, and enumerate the boundary in
the repository unsafe audit. Replace obsolete generated APIs directly; add no
compatibility shim.

Execute the batches in order. At every checkpoint run the focused tests, keep
docs/progress.md current, record durable decisions in docs/decisions.md, and
commit a small verified Conventional Commit. Do not mark M20.75 complete until
the complete acceptance matrix and repository validation pass.
```

## 1. Problem And Outcome

Vela can currently suspend an async root or an awaited nested call, but a
synchronous call has no way to start work that outlives its own stack. This is
especially limiting for emergency Service patches: the Rust Service ABI may be
synchronous while the repair must query a database, call a remote service,
delay, retry, or compute in the background and later re-enter host-owned state.

The completed feature provides two domain-neutral operations. Their source
shape is frozen as a suspended static call specification, not a function path
plus an `args` tuple:

```vela
task::spawn_scoped(worker(arg1, arg2));
task::spawn_scoped_then(worker(arg1, arg2), continuation);
```

They are compiler-owned static forms, not ordinary dynamically replaceable
functions. The first argument is syntax representing a future invocation; it
does not call `worker` in the parent. `worker` and `continuation` are statically
resolved function paths, and `arg1, arg2` use ordinary call argument checking.
Parentheses are required even for a zero-argument worker. This avoids tuple
packing, function values, target strings, and function overloading.

`spawn_scoped` admits an owned detached invocation into the current host task
scope and returns synchronously. `spawn_scoped_then` additionally registers a
Vela continuation. The worker may suspend and await any permitted ordinary,
native, host, provider, database, RPC, or pinned Service call. Completion is
reported to the host scope. A continuation is invoked only when that host scope
chooses a safe point and supplies a fresh resume context.

The word `scoped` means lifecycle-scoped, not stack-borrowing. A detached child
may outlive the spawning Vela frame, so everything transferred to it is owned.
The host scope bounds admission, lifetime, cancellation, and result delivery;
it does not permit a Rust borrow to escape.

## 2. Semantic Contract

### 2.1 Static target and ordinary function model

- A worker is an ordinary declared `async fn`. There is no separate task
  function declaration kind.
- A continuation is an ordinary declared synchronous function whose first
  parameter is the exact owned `Result<T, task::Error>` corresponding to the
  worker result `T`. Any remaining parameters are host-resume parameters
  declared by the embedding's sealed continuation contract. It executes as a
  new root turn and cannot itself suspend.
- Worker and continuation paths resolve to stable `FunctionId` values during
  HIR lowering. Dynamic values, reflection results, closures, strings, and
  computed member paths are rejected.
- A synchronous caller may spawn an async worker. Spawning does not make the
  caller async and does not alter a Service method's Rust ABI.
- Any ordinary function reached from a Service root may use the operation when
  its verified effect ceiling permits it. The feature is not restricted to
  `#[service_impl]` bodies.
- An async worker may call and await other async functions normally. This is
  one fresh async root using the existing frame driver, not a copied executor
  or a second async implementation.

### 2.2 Return and error model

`task::spawn_scoped(...)` returns unit after successful admission. Admission
failure is a synchronous structured runtime error; it cannot silently drop the
requested work. The initial contract does not expose a script TaskHandle or an
admission `Result`, because a handle would create join/cancel/lifetime
semantics that are intentionally absent. Hosts that need a non-fatal admission
policy expose an explicit registered wrapper or choose a scope policy whose
admission cannot fail under the declared limit.

Worker completion is represented internally as:

```text
DetachedOutcome = Ok(DetachedValue) | Err(DetachedTaskError)
```

The host scope always observes terminal success, Vela error, trap, cancellation,
deadline, panic containment, and dropped-executor cases. Completed host effects
are not rolled back. A task with no continuation still emits the host's normal
completion/telemetry event; failures must not become unobserved futures.

For `spawn_scoped_then`, the continuation receives one sealed Result-like
owned value derived from `DetachedOutcome`. It cannot receive a panic payload,
host pointer, Runtime, dispatcher, or executor object. Error categories and
source/generation metadata remain available through controlled value fields and
host diagnostics without exposing internal Rust types.

### 2.3 Continuation semantics

The continuation is not run on the worker executor at arbitrary time. Worker
completion enqueues a `ContinuationInvocation` containing the pinned callable,
owned outcome, and its sealed trailing-parameter contract into the originating
host scope. At a host safe point:

1. the host acquires the context required for one new turn;
2. the scope validates that the owner is still live and the task was not
   cancelled;
3. Vela creates a fresh root invocation for the statically linked continuation;
4. the continuation receives its owned outcome plus freshly constructed
   trailing arguments through the normal registered Host/Value boundary; and
5. root teardown releases all resources before the next turn.

The continuation and worker pin the same originating linked artifact and, when
present, the same complete Service generation. This matches an already-started
async frame: hot reload does not rewrite the continuation target underneath a
pending child. A host that wants a fresh-generation message handler should use
its normal mailbox/request dispatch after observing completion; that is a host
adapter policy, not `spawn_scoped_then` semantics.

The worker never receives the actor/request/UI context used by the
continuation. The continuation never receives the worker Runtime. No
`ActorContext`, request guard, or framework name appears in Vela core.

The Engine embedding API exposes the trailing callable contract before resume.
The host adapter must either construct every declared argument with the normal
sealed TypeBinding/HostAccess rules or cancel the invocation with a structured
resume-binding error. For example, an actor adapter may create new call-scoped
HostRefs for its actor and handler context; a request adapter may provide a new
request context. These references live only for the continuation root and are
released at its teardown. Vela core sees typed registered parameters, not a
universal context object or type-based ambient lookup.

### 2.4 Service hotfix example

An authored synchronous Service method may remain synchronous while its Vela
replacement starts a database-backed repair:

```vela
async fn load_repair(account_id: Int) -> Repair {
    let row = database::load_repair(account_id).await?;
    normalize_repair(row).await
}

fn apply_repair(
    result: Result<Repair, task::Error>,
    account: Host Account,
    turn: Host TurnContext,
) -> Unit {
    match result {
        Ok(repair) => service::pinned::inventory::apply(account, repair),
        Err(error) => turn.report_task_error(error),
    }
}

#[service_impl]
impl AccountService {
    fn request_repair(account_id: Int) -> Unit {
        task::spawn_scoped_then(load_repair(account_id), apply_repair);
    }
}
```

`database::load_repair` and the two Host types are host registrations, not Vela
builtins. The host's lifecycle adapter supplies fresh `account` and `turn`
parameters when it accepts `ContinuationInvocation`; the worker never holds
them. Both worker and continuation use the Service generation that contained
this `request_repair` selection. If the host instead converts completion into
a normal new Service request, that new request deliberately pins the then-current
generation and does not use `apply_repair` as the scoped continuation.

### 2.5 Explicit non-goals

The milestone does not add:

```text
TaskHandle or Future as a Vela value
join, select, race, detach-from-scope, script cancellation, or task enumeration
unscoped or process-global spawn
async closures or escaping borrowed captures
shared Runtime, heap, state cells, HostAccess table, or VM stack across tasks
manual poll, yield, suspend, resume, or coroutine bytecode
hot migration of worker or continuation frames
dynamic/string task targets
framework-specific actor, request, database, or UI APIs
implicit retries or rollback of completed host effects
```

## 3. Ownership And Isolation

### 3.1 Detachable values

The compiler and runtime use one authoritative `Detachability` fact. Values
transferred into a detached child or returned from it must be recursively owned
and independent of the parent Runtime.

Admitted families include scalar values and recursively detachable owned
strings, bytes, records, enums, arrays, maps, sets, Option, Result, and tuples.
GC-managed values are deep-detached into a new task transfer image while
preserving aliases and cycles within the transferred graph. The transfer is
budgeted and transactional: admission publishes no task when validation,
budgeting, or copying fails.

The boundary representation is an owned Runtime-independent
`DetachedValueImage`, not `VelaValue`. Admission exports the parent arguments
and the child imports them into its heap. Before child teardown, a successful
result is exported into a new image; a continuation later imports that image
into its own fresh root Runtime. The image uses sealed linked type identities
and is not a general serializer, persistence format, or cross-artifact value
coercion path.

Rejected families include:

```text
HostRef, HostPath, PathProxy, HostAccess-backed views
shared/exclusive Rust views and any active scoped lease
call-scoped or lazy Host iterator resources
closures, captured upvalues, bound methods, and dynamic callables
Runtime, execution session, task scope, dispatcher, or provider lease handles
borrowed slices, strings, collection views, and live external resources
values whose sealed TypeBinding storage or child graph is not detachable
```

The same proof applies to worker arguments, worker result, continuation-bound
outcome payloads, task-local state initialization inputs, and host completion
payloads. Runtime checks remain mandatory even when static facts prove common
cases, because `Any`, reflection, and host-returned values can hide a rejected
representation.

### 3.2 Fresh Runtime per active child

Each admitted task owns a fresh `Runtime` with:

- its own VM stack, frames, managed heap, root set, state cells, extern
  bindings, HostRef table, leases, execution session, and cancellation state;
- an immutable shared `Arc<LinkedArtifact>` and sealed registries;
- an exact generation identity and task metadata;
- independent execution, memory, collection-growth, call-depth, wall-clock,
  and host-call budgets; and
- no access to the parent Runtime's mutable state unless the host explicitly
  exposes shared external state through registered, concurrency-safe APIs.

This is runtime isolation, not an executor copy. Vela core produces a future
that drives the fresh Runtime; the host task scope chooses where and how to
poll it. Runtime allocation may later use a bounded idle cache, but reuse must
perform complete root teardown and generation/policy rebinding. Correctness
must not depend on pooling.

VM `state` is per Runtime. A detached child therefore does not observe or
mutate the parent's VM state cells. Shared business state must cross registered
host APIs, `extern state`, database/RPC calls, or a continuation context. This
rule prevents accidental shared-memory concurrency.

### 3.3 No parent borrow crosses admission

Task construction follows prepare/commit semantics:

1. statically resolve targets and contracts;
2. validate the active host task scope and policy;
3. validate effects and capabilities;
4. validate and detach the complete argument graph;
5. reserve task and resource budgets;
6. construct the child execution capsule and future; and
7. atomically admit it to the host scope.

Before step 7, no task is visible. After step 7, the child owns every value it
needs. No reference into the parent Runtime, frame, stack, heap, HostAccess
table, native call context, or temporary argument buffer remains.

## 4. Host Scope Protocol

Core owns an executor-neutral protocol, conceptually:

```rust,ignore
pub trait ScopedTaskHost: Send + Sync {
    fn admit(
        &self,
        task: ScopedTask,
    ) -> Result<(), TaskAdmissionError>;
}

pub struct ScopedTask {
    pub metadata: TaskMetadata,
    pub future: ScopedTaskFuture,
    pub completion: ScopedCompletion,
}
```

Final names are chosen in Batch A. The protocol must express ownership,
cancellation, deadline, completion, and optional safe-point continuation
delivery without naming an executor. It must not expose Runtime mutation or a
pollable task handle to scripts.

The scope owns:

- maximum active tasks and bounded admission queues;
- task IDs used only for host diagnostics/tracing;
- cancellation when the host lifecycle ends;
- executor integration and wakeups;
- completion observation and panic containment;
- safe-point delivery for continuations; and
- policy narrowing for budgets, effects, and allowed targets.

The scope is an owned capability propagated through the execution context. It
must not use a process global, thread local, implicit Tokio handle, or mutable
singleton. Ordinary `Runtime::call` without a scope remains valid, but executing
a task builtin there fails deterministically with `TaskScopeUnavailable`.

Dropping the scope cancels all children, wakes pending futures, and retains the
backing lifetime authority until every child future and queued continuation has
finished dropping. The host cannot reclaim actor/request/context storage merely
because cancellation was requested.

## 5. Effect, Capability, And Budget Contract

Add a domain-neutral `TaskSpawn` effect/capability bit. Static verification of
a spawn site includes:

```text
TaskSpawn
union transitive effects of the worker
union transitive effects of the continuation, when present
```

This prevents a pure-looking synchronous wrapper from bypassing its patch
effect ceiling by moving I/O or host mutation into a child. Recursive task call
graphs use the existing fixed-point effect analysis and reject unbounded or
unresolved dynamic cycles.

At admission, the effective child authority is the intersection of:

```text
caller's active effect ceiling
engine capability profile
originating linked-artifact policy
host task-scope policy
Service method/domain ceiling, when present
```

The child may narrow authority but never widen it. Continuation authority is
checked separately at safe-point entry against both its sealed effects and the
resume context supplied by the host.

Every scope configures finite limits for active task count, queued completion
count, execution units, memory, collection growth, call depth, host calls, and
deadline/timeout behavior. `CallOptions` remains explicit and gains or composes
a task policy rather than receiving a domain-specific default. Recursive spawn
cannot create an unbudgeted execution tree: child admission consumes from a
bounded scope quota and each child receives finite independent budgets.

The Service schema hard-switches from one overloaded effect field to two facts:

```text
RustDefaultEffects   effects performed by the registered Rust body
PatchEffectCeiling   maximum effects allowed in a Vela replacement
```

`RustDefaultEffects` remains truthful metadata and drives direct Rust checks.
`PatchEffectCeiling` is ABI/policy and may deliberately be a strict superset so
an emergency patch can do work the normal Rust implementation did not need.
The generated Service-domain builder must require one explicit emergency patch
ceiling and apply it to every hotfixable Service method, with an optional
explicit method-level narrowing. To satisfy the game-server requirement that
any Service can perform an emergency async repair, that domain ceiling includes
at least `TaskSpawn` and the registered I/O effects needed by repair workers.
There is no silent default and no inference from the current Rust body.

This widening does not authorize execution by itself. Engine capabilities and
the host task-scope policy still narrow every actual call/admission, and staged
Vela effects must remain within `PatchEffectCeiling`. Changing the ceiling is a
Service schema/ABI change that requires a new host build or an explicitly
compatible artifact contract; a live patch cannot widen it.

## 6. Complete Service Integration

### 6.1 One pinned execution capsule

The generated immutable Service generation gains one owned execution capsule,
conceptually `PinnedServiceExecution`, containing:

```text
ServiceSetId and ServiceGenerationId
Arc<LinkedArtifact>
Arc<dyn ServiceCallDispatcher>
Runtime factory or ServiceRuntimeBinding
sealed registries and Engine authority
CallOptions, detached-task policy, and Service patch effect ceilings
generation tracing/diagnostic metadata
```

The capsule is the authority inherited by a Service-rooted detached child. It
is constructed once for the complete generation and shared by generated
composite adapters. Runtime/cache/options/dispatcher ownership must not be
duplicated independently in every generated Service adapter.

The generation still owns no mutable active Runtime. Each invocation leases a
root Runtime as today; each detached child creates or leases a distinct clean
Runtime from the capsule. Concurrent children can progress without a Runtime
mutex and cannot enter the caller's leased Runtime.

### 6.2 Exact-generation behavior

A detached child started anywhere below a Service root inherits the exact
originating capsule. Its ordinary Vela calls, nested
`service::base::method(...)`, and
`service::pinned::service_name::method(...)` all resolve against that capsule.
Publishing a new Service generation while the child is pending has no effect
on the child or its continuation. A later root pins the new generation.

Ordinary Vela functions, Service method selections, state/schema facts, task
targets, and continuation targets remain members of the same linked candidate.
Snapshot/Delta staging validates their complete call/effect graph, and one CAS
publishes them atomically. There is no separate task patch table or per-target
replacement slot.

### 6.3 Generated application hard switch

`#[service_domain]` must generate the task-capable request/turn scope as the
only Service application entry model and require the domain-wide emergency
patch effect ceiling during construction. Existing `app.with_request` and
`app.with_request_async` signatures may be replaced directly by one coherent
scope object or explicit scoped forms chosen in Batch A. Do not keep old
aliases or internally branch between scoped and unscoped Service execution.

The generated entry boundary:

1. pins one complete Service generation;
2. binds the host lifecycle's `ScopedTaskHost` and resume-context factory;
3. leases the root Runtime;
4. installs the owned scope/capsule authority in the execution context;
5. invokes unchanged authored Rust Service traits and Vela selections; and
6. restores/drops the root while detached children remain owned by the host
   scope, not by the borrowed root Runtime.

Business Service traits gain no Runtime parameter, task-host parameter,
ActorContext parameter, or per-method generated spawn function. One generated
domain integration serves every Service and every statically linked worker.

### 6.4 Nested calls and Rust defaults

`NativeCallContext` currently carries borrowed same-session Service dispatch.
The implementation must separate:

- borrowed current-session authority used for immediate nested calls; and
- owned detached execution authority copied into a new child capsule.

Both resolve to the same immutable dispatcher and generation, but only the
owned capsule crosses task admission. Authored Rust defaults remain ordinary
sync/async trait methods. If a registered Rust host function needs to initiate
the same generic operation, it may invoke an Engine embedding API with the
current owned scope/capsule; Service traits themselves are not rewritten.

## 7. Compiler, Bytecode, Artifact, And Tooling Shape

HIR represents each builtin with dedicated nodes carrying stable worker and
continuation `FunctionId` values and verified argument facts. MIR/bytecode use
dedicated task-admission operations; they do not lower through reflective
`CallValue`, a native target string, or hidden source rewriting.

The linker records:

- worker and continuation target slots;
- exact callable ABI and asyncness;
- detachability contracts for arguments/results;
- transitive effect summaries;
- Service-generation requirements; and
- artifact feature/version requirements.

Portable program, Service bundle, and detached deployment metadata hard-switch
from format version 2 to version 3. Version 3 encodes the task target table,
detachability contracts, transitive effects, continuation ABI, Service
execution requirements, and required feature bits. Version 1 and 2 artifacts
are rejected before linking, staging, or activation. Do not infer missing task
metadata, rewrite old bytecode, or retain a compatibility loader.

Reflection may report whether a function is a valid detached target, its
effects, and its owned input/result contract. Reflection cannot dynamically
start it. LSP completion, hover, signature help, navigation, diagnostics,
semantic tokens, references, and call hierarchy understand both task forms and
their static target restrictions. Diagnostics must name the target, rejected
value path, effect/capability, scope requirement, and source span.

## 8. Failure And Cancellation Semantics

Required structured categories include:

```text
TaskScopeUnavailable
TaskAdmissionDenied / TaskCapacityExceeded
TaskTargetNotStatic / TaskTargetNotAsync
TaskContinuationInvalid
TaskValueNotDetachable { path, kind }
TaskEffectDenied / TaskCapabilityDenied
TaskBudgetExceeded / TaskDeadlineExceeded
TaskCancelled { reason }
TaskWorkerError / TaskWorkerTrap / TaskWorkerPanicked
TaskContinuationError / TaskContinuationPanicked
TaskGenerationUnavailable
```

Exact public names are frozen with the implementation. Errors retain source
and generation metadata where available and redact host internals.

Cancellation is cooperative at existing VM/native await safe points. Dropping
the worker future must run Runtime, Service lease, provider future, extern
binding, and host-resource cleanup through RAII. A native future that ignores
cancellation may delay final drop; therefore host integrations must document
their cancellation behavior and the scope must retain required lifetime
authority until drop completes.

A worker panic is caught at the host task boundary and converted to a terminal
outcome/diagnostic. A continuation panic is likewise contained at its root
turn. Neither unwinds through an executor or host event loop. Effects completed
before any failure remain committed.

## 9. Implementation Batches

### Batch A: Implement the frozen language and authority contracts

Deliver:

- the frozen suspended-call grammar for `task::spawn_scoped` and
  `task::spawn_scoped_then`;
- HIR target identity and `Detachability` facts;
- `TaskSpawn` effect and transitive effect rules;
- host scope, execution capsule, task policy, outcome, and error types;
- continuation ABI and exact-generation policy;
- portable artifact version 3 schema and version 1/2 rejection contract; and
- compile-fail proof for dynamic targets and rejected value families.

Checkpoint:

```text
parser/HIR/analysis tests cover both forms and recovery
effect analysis proves worker and continuation closure
contract tests reject dynamic targets, sync workers, async continuations, and borrowed values
architecture docs and decisions contain no framework-specific public API
```

### Batch B: Ordinary detached-call vertical slice

Deliver one non-Service path from a synchronous Vela function through task
admission to a fresh Runtime. The worker must await a deliberately pending
registered native fixture, return an owned nested value, and be cancelled
cleanly when its scope ends.

Checkpoint:

```text
sync caller returns after admission without becoming async
worker uses existing async frame driver and a fresh isolated Runtime
owned graph transfer preserves aliases/cycles and charges budgets
missing scope, capacity, deadline, cancellation, error, panic, and dropped-future paths are observed
parent and child VM state/heap/HostRef tables are isolated
```

### Batch C: Whole-Service-generation integration

Deliver the generation-owned execution capsule, generated application hard
switch, task policy sealing, and exact-generation nested dispatch. Remove
duplicated per-adapter runtime/options/dispatcher ownership where the new
capsule supersedes it.

Checkpoint:

```text
a synchronous Vela Service patch starts an async worker without changing the Rust trait ABI
the worker awaits a host I/O fixture and calls service::base and service::pinned
all nested calls use the originating Service generation
hot reload during suspension leaves old child/continuation old and new roots new
ordinary helper functions below a Service root can spawn
Rust-only defaults and Vela-selected roots share one generated scope model
no per-Service/per-message spawn function or second patch path exists
```

### Batch D: Safe-point continuation and host lifecycle

Deliver completion queues, host resume-context acquisition, new-turn
continuation entry, cancellation races, and teardown ordering. Provide one
generic actor-style example adapter and one request-style test adapter without
putting either vocabulary in Vela core APIs.

Checkpoint:

```text
continuation never runs on the background worker context
continuation receives owned Ok/Err outcome and a fresh host context
owner shutdown before completion cancels and prevents unsafe re-entry
completion versus cancellation race has one terminal outcome
queued continuation pins its generation until execution or cancellation
no parent borrow or worker Runtime survives into the continuation turn
```

### Batch E: Hardening, tooling, and portability

Deliver artifact encoding/rejection, reflection facts, LSP support, tracing,
metrics, structural audits, examples, and bounded concurrency tests. Add a
benchmark separating admission/copy cost, fresh/pooled Runtime cost, pending
poll cost, Service nested dispatch, and continuation delivery.

Checkpoint:

```text
portable version 3 ordinary and Service artifacts preserve all task metadata
version 1 and 2 reject before link/stage/activation
LSP diagnoses invalid targets, values, effects, and continuation shape
reflection reports but cannot dynamically invoke task targets
stress tests cover concurrent tasks, cache reuse, cancellation, panic, and teardown
benchmark records stable interpreter-only task rows without claiming performance acceptance prematurely
```

### Batch F: Acceptance and cleanup

Delete superseded generated application APIs and temporary implementation
bridges. Run the full matrix in section 11, update progress/decisions, and write
an archived acceptance report with only durable results.

Checkpoint:

```text
no compatibility alias, target-string path, global/thread-local task authority, or unbounded queue remains
source audits find unsafe only in explicitly enumerated reviewed boundaries
full repository validation passes
docs/progress.md marks M20.75 complete and links the acceptance report
```

## 10. Unsafe Boundary

Safe Rust is preferred. Permission to use `unsafe` is limited to the case where
the host must erase a non-`'static` lifecycle lifetime while an owned scope
guarantees that backing storage outlives every worker future and queued
continuation. The intended shape is one focused private module owned by the
embedding/host boundary, not by parser, HIR, compiler, bytecode, VM dispatch,
or generated business code.

The boundary is acceptable only if all of these invariants are executable:

1. admission increments or owns a lifetime token before erasure;
2. scope shutdown requests cancellation but retains backing storage;
3. every future and completion record releases exactly one token on every
   success, error, panic, cancellation, and drop path;
4. continuation context is reacquired at a safe point and is never retained;
5. no erased pointer is `Send`/`Sync` unless the concrete host contract proves
   the required property;
6. no parent Runtime, frame, HostRef table, or Rust borrow is stored in the
   detached capsule; and
7. final backing storage reclamation occurs only after the token count reaches
   zero and completion queues are drained.

Every unsafe block requires a local `SAFETY:` proof. The module requires a
module-level invariant comment, focused unwind/drop/race tests, compile-fail
tests for escaping lifetimes, and Miri/sanitizer execution where supported. It
must be listed by the repository unsafe-boundary source audit. Unsafe may not
be used merely to avoid designing owned transfer values or to make a borrowed
future appear `'static` without lifecycle ownership.

## 11. Acceptance Matrix

### Language and ordinary execution

- sync ordinary function admits an async worker and returns unit;
- async ordinary function can also admit a child without awaiting it;
- worker awaits nested Vela, native, method, provider, database/RPC fixtures;
- static target IDs survive modules, reload, version 3 serialization, and diagnostics;
- dynamic/string/closure targets and synchronous workers are rejected;
- no scope fails before worker invocation;
- no TaskHandle/Future/cancel/join/manual-resume surface exists.

### Ownership and isolation

- scalar and nested owned values transfer in both directions;
- aliasing/cycles within owned GC graphs are preserved;
- HostRef, PathProxy, borrowed view, closure, live iterator, and scoped resource
  reject statically when known and dynamically when hidden by `Any`;
- failed transfer admits no task and leaks no reservation;
- parent/child state, heap, roots, leases, and budgets are independent;
- teardown returns pooled Runtime state to a provably clean baseline.

### Service and hot reload

- every generated Service can use the domain task policy without a per-method
  Rust wrapper;
- a sync Service selection starts pending async work without ABI change;
- child `service::base` and `service::pinned` calls use the exact origin;
- old pending worker and continuation remain old across two publications;
- new root/child uses new generation;
- failed candidate or stale activation starts no work and changes no authority;
- Snapshot, Delta, rollback, and folded Snapshot preserve task target behavior;
- ordinary functions and Service selections publish atomically.

### Effects, limits, and failure

- `TaskSpawn` and worker/continuation transitive effects cannot bypass method,
  artifact, Engine, or scope ceilings;
- every hotfixable Service receives the explicit domain emergency patch
  ceiling even when its Rust default is synchronous/pure, while
  `RustDefaultEffects` remains truthful and separate;
- task count, queue, execution, memory, call-depth, host-call, and deadline
  limits are finite and tested;
- recursive spawn exhausts a bounded quota;
- success, Vela error, trap, timeout, cancellation, panic, and future drop each
  produce exactly one observed terminal outcome;
- completed host effects are not rolled back;
- scope shutdown and completion races leak no Runtime, generation, dispatcher,
  lease, waker, or host lifetime token.

### Continuation

- outcome is fully owned and path-validated;
- continuation runs only at a host safe point in a fresh root turn;
- fresh host context is available through ordinary registered boundaries;
- worker cannot access continuation-only context;
- cancellation before delivery prevents continuation execution;
- continuation error/panic is isolated and observed;
- continuation cannot suspend, escape its context, or migrate generations.

### Tooling and structure

- parser, HIR, analysis, MIR, verifier, linker, VM, Engine, macros, reflection,
  LSP, artifact, CLI/examples, and Service tests cover their owned layer;
- public names and docs remain domain-neutral;
- generated code has one capsule/scope path and no per-message function family;
- relevant files stay within architecture size policy;
- unsafe source audit and lifecycle proof pass.

## 12. Validation Commands

Each batch runs its focused crate tests. The phase checkpoint runs at least:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --manifest-path examples/Cargo.toml --all-features
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
```

Also run the repository structural, artifact, website, editor-extension,
benchmark-build, and fuzz-build gates named in [validation.md](validation.md).
If the unsafe boundary is implemented, run its source audit and Miri/sanitizer
targets on supported toolchains and record any unavailable platform/toolchain
in the acceptance report rather than silently skipping it.

## 13. Exit State

M20.75 is complete only when a host can install one generic bounded task scope,
then any permitted ordinary function or hotfixed Service implementation can
start an owned async workflow, await arbitrary registered async work, preserve
the exact complete hot-reload generation, and optionally resume Vela at a safe
host turn with fresh context. The result must require neither a message-specific
Rust function nor a Service-specific task wrapper, and it must leave no
framework vocabulary, borrowed state, unbounded execution, or compatibility
path in Vela core.
