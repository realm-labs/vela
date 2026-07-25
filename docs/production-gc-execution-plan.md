# Production GC Execution Plan

> Status: Ready for execution
>
> Execution status: Not started
>
> Primary scope: VM heap, Engine actor Runtime, configuration, observability,
> correctness stress tests, and latency/memory benchmarks
>
> Production-v1 target: actor-local, non-moving, fully incremental mark-sweep
> with bounded work, barriers, debt scheduling, hard limits, and no ordinary
> call-return full collection
>
> Evidence-gated follow-on: non-moving young collection plus incremental major

This replaces "mark once, incrementally sweep" with a collector that remains
correct and paced while the mutator runs between steps. It is an architecture
and runtime-safety plan, not a narrow optimization task.

## 0. Codex Goal

Use this prompt to execute the plan:

```text
/goal Execute docs/production-gc-execution-plan.md to its Production-v1
completion gate. Treat it as one persistent multi-turn implementation goal.
Continue automatically across tasks, turns, tests, benchmarks, documentation
updates, and commits until every required Production-v1 checklist item is
checked, every required validation gate passes, the durable architecture and
progress docs describe the implemented collector, and all plan-owned changes
are committed with Conventional Commits.

Start every turn from the repository instructions and the next unchecked item
in this plan. The instruction to choose the smallest verifiable task controls
the next implementation unit, not when the goal ends. Completing one phase,
one test, one benchmark, one refactor, or one commit is progress only and is
not a valid stopping condition.

Execute required work in this order:

1. Phase A: freeze current behavior, add missing adversarial tests, metrics,
   and representative GC/actor latency and memory baselines.
2. Phase B: split the heap/collector responsibilities and centralize all
   allocation, root admission, and heap-edge mutation behind barrier-capable
   APIs without changing observable behavior.
3. Phase C: implement the bounded incremental mark state machine, resumable
   per-object tracing, epoch-based marks, and incremental sweep.
4. Phase D: enable and verify root, insertion, allocation, and async/reentry
   barriers across every heap mutation and root-lifetime boundary.
5. Phase E: replace unconditional call-end full collection with allocation-debt
   scheduling, actor-turn/idle safe points, explicit full collection, and
   emergency pressure handling.
6. Phase F: finish memory accounting, configuration, telemetry, heap-slot
   trimming, failure atomicity, and Runtime-facing control APIs.
7. Phase G: run stress, differential, async/reload, latency, throughput, actor
   memory, and full workspace acceptance gates; fix every regression before
   completion.
8. Phase H: evaluate the generational evidence gate. Record the measurements
   and decision. Implement Phase H generational work only when the gate passes;
   a recorded "not yet justified" result does not block Production-v1.

Do not add a moving collector, shared cross-Runtime heap, script-visible
finalizers, GC-dependent HostRef or lease cleanup, Rust host state tracing,
script threads, or a second VM execution route. Do not weaken memory budgets,
HostAccess, hot-reload generation ownership, reflection permissions, async
rooting, verified safepoints, or conservative interpreter frame rooting.

Implement the long-term internal API directly. This repository is pre-release:
remove obsolete heap mutation and pacing APIs instead of keeping compatibility
shims. Preserve unrelated worktree changes. Keep ordinary source files below
1200 lines, and split heap, collector, pacing, root, metrics, and test
responsibilities before they become oversized.

Never mark this goal complete while any of the following is true:

- a required Production-v1 checklist item in Phases A-G is unchecked;
- mark work can traverse an unbounded aggregate in one incremental step;
- incremental marking can run without complete mutation, allocation, and root
  barriers;
- `max_pause_micros` or its replacement claims a pause bound that the
  implementation does not enforce;
- an ordinary persistent Runtime root call performs a full-heap collection
  solely because the call returned;
- any live frame, protected caller, pending callback/iterator, persistent state,
  retained VelaValue, async suspension, or reentry result can be missed as a
  root;
- a HostRef or PathProxy host target, Rust host object, host lease, database
  object, or network resource is traced or finalized as script-owned memory;
- allocation or in-place collection mutation can exceed a hard memory limit
  after partially mutating the heap;
- no-allocation actor calls perform heap-size-proportional GC work;
- required stress, benchmark, full validation, documentation, or file-size
  gates have not passed;
- docs/progress.md or this plan still reports Production-v1 as unfinished;
- plan-owned changes remain uncommitted.

If an attempt fails, diagnose it, add the smallest regression that captures the
failure, and continue with another in-scope implementation. Report blocked only
when progress genuinely requires an external product decision or unavailable
production workload. Goal completion means Production-v1 is implemented,
measured, documented, fully validated, and committed. Phase H is complete when
its evidence decision is recorded; generational implementation is required
only if the evidence gate defined by this plan passes.
```

## 1. Objective

The collector must make script allocation safe and predictable for a
game-server actor model:

```text
many independent actor Runtimes
        |
        +-- one actor-local ScriptHeap each
        +-- one turn executing at a time
        +-- short-lived per-event temporaries
        +-- long-lived Vela state and closures
        +-- HostRef leaves pointing to Rust-owned state
        `-- latency and memory budget per call/turn
```

Production-v1 is complete when:

```text
correct reachability under arbitrary safe-point interleaving
+ bounded incremental mark and sweep work
+ barriers for every new heap edge and root
+ allocation-debt scheduling instead of per-call full GC
+ hard memory-budget failure atomicity
+ actor-local observability and operational controls
+ stress and P95/P99 performance evidence
----------------------------------------------------------
= a production-ready non-moving incremental collector
```

Generational collection is an optimization over this correct base. It must not
be used to avoid implementing incremental marking, barriers, metrics, or major
collection pacing.

## 2. Current State And Confirmed Gaps

The existing implementation already provides:

- `GcRef { index, generation }` stable handles;
- a non-moving `Vec<HeapEntry>` arena and free-list slot reuse;
- stale-reference rejection through slot generations;
- tracing for strings/bytes leaves, tuples, arrays, maps, sets, records, enums,
  closures, iterators, and PathProxy leaves;
- frame, protected caller, persistent state, retained `VelaValue`, and dynamic
  reentry root paths;
- cycle collection;
- shallow heap-byte accounting tied to `ExecutionBudget`;
- full collection and resumable sweep-by-slot tests;
- actor-local heaps with no Rust host state under script GC.

Confirmed gaps that this plan must close:

1. Marking is monolithic. `step_gc` performs the complete transitive mark
   before it returns and only sweep is resumable.
2. `GcBudget::max_pause_micros` is stored but not consulted. The default
   microsecond budget permits unlimited sweep slots and therefore does not
   enforce the advertised pause shape.
3. `HeapValue::trace_refs` traverses a complete aggregate at once. A single
   large array/map/set/record/closure/iterator can exceed any future pause
   target even if the outer mark loop is incremental.
4. General heap mutations do not pass through a collector write barrier.
   Incremental mark cannot safely be enabled until every heap-edge insertion
   path participates.
5. Raw mutable heap access makes barrier completeness difficult to audit.
6. Dynamic root admission has a focused incremental-mark hook, but root
   admission is not yet one exhaustive policy covering frames, protected
   roots, state, retained values, async results, and reentry.
7. an ordinary linked Runtime completion invokes `collect_full_with_budget`;
   repeated short calls can therefore do work proportional to heap high-water
   size even when they allocate nothing.
8. Collection triggering is a threshold check, not an allocation-debt model
   that guarantees collector progress relative to allocation.
9. Sweep visits the arena high-water length. Trailing free slots are not
   trimmed after collection.
10. GC statistics are last-step diagnostics rather than an operational cycle,
    pause, survival, allocation, and pressure telemetry surface.
11. There is no randomized reachability oracle, mutation/barrier stress suite,
    or large-live-object pause-bound test.
12. Existing `gc_pacing` is a useful regression workload but does not isolate
    mark pause, major/full pause, sparse high-water scanning, retained actor
    state, or no-allocation short-call behavior.

## 3. Product Requirements

### 3.1 Required behavior

- Every script heap belongs to exactly one Runtime.
- The Runtime may move between executor workers only under exclusive actor
  ownership; GC never runs concurrently with its mutator.
- `GcRef` remains stable for the lifetime of its object.
- Unreachable cycles are reclaimed.
- HostRef and PathProxy host targets remain external leaves.
- A live linked closure keeps its `LinkedArtifact` generation alive; reclaiming
  the closure releases that ownership normally.
- Active and suspended async sessions retain every reachable script object.
- GC work is charged independently from script execution units but is bounded
  by its own deterministic budget.
- Hard script-memory limits fail before a heap mutation becomes externally
  visible.
- Explicit full collection remains available for shutdown, tests, diagnostics,
  idle maintenance, and emergency pressure handling.
- Ordinary call completion does not imply full collection.
- The collector exposes enough metrics to diagnose memory growth and tail
  latency without inspecting private heap data.

### 3.2 Latency contract

Production-v1 uses two limits:

```rust,ignore
pub struct GcStepBudget {
    pub max_work_units: u32,
    pub soft_deadline_micros: Option<u64>,
}
```

`max_work_units` is the deterministic correctness and test contract.
`soft_deadline_micros` is an operational early-stop hint checked at a bounded
interval. The API must not describe the deadline as a strict wall-clock bound,
because one primitive allocation/free and clock scheduling can overshoot it.

Work units must cover:

- one root value inspected;
- one heap reference marked or rejected as stale;
- one bounded edge chunk visited;
- one sweep slot inspected;
- one reclaimed object finalized internally by dropping its Rust-owned
  `HeapValue` payload;
- bounded side-table cleanup.

No primitive unit may hide an unbounded scan. Aggregate tracing must be
resumable by edge cursor.

### 3.3 Memory contract

Use distinct concepts:

```text
live heap bytes       currently owned HeapValue shallow storage
reserved heap bytes   capacity/reservation attributable to script values
GC metadata bytes     arena, roots, mark stack, remembered metadata
soft trigger bytes    start/increase collection pressure
hard limit bytes      reject before mutation if recovery cannot make room
```

Production-v1 may keep the existing public memory-budget meaning if it is
documented precisely, but all allocation and growth paths must use the same
meaning. Do not count Rust host state as script bytes.

### 3.4 Explicit non-goals

Production-v1 does not add:

- moving or compacting live objects;
- a shared heap across actors or Runtime instances;
- concurrent GC threads;
- script-visible weak references or ephemerons;
- script-visible finalizers;
- GC-driven HostRef release, host lease release, transaction cleanup, or other
  external effects;
- conservative scanning of arbitrary Rust memory;
- migration of heap objects between hot-reload generations;
- a new JIT, coroutine, or async-frame migration path;
- general script-language generics.

## 4. Target Architecture

### 4.1 Module ownership

Split the current heap implementation before adding the state machine. The
exact filenames may adapt to the existing crate layout, but ownership must be
equivalent to:

```text
vela_vm/src/heap.rs or heap/mod.rs
    public heap facade, GcRef access, allocation/read APIs

vela_vm/src/heap/object.rs
    HeapEntry, HeapObject, mark epoch/color, age, size metadata

vela_vm/src/heap/value.rs
    HeapValue tracing vocabulary and resumable TraceCursor

vela_vm/src/heap/collector.rs
    GcPhase, CollectorState, mark/sweep transitions, full collection driver

vela_vm/src/heap/barrier.rs
    insertion, allocation, and root-admission barrier policy

vela_vm/src/heap/pacing.rs
    GcConfig, GcStepBudget, allocation debt, deadline checks

vela_vm/src/heap/metrics.rs
    cycle/step reports and cumulative telemetry

vela_vm/src/heap/tests/
    handle, tracing, barriers, pacing, budget, stress, and regression tests

vela_vm/src/heap_execution.rs
    active execution roots, safe-point integration, dynamic root guards

vela_engine/src/runtime/vm_states.rs
    persistent state and VelaValue root ownership
```

No ordinary source file may cross 1200 lines as a result of this work.

### 4.2 Collector state machine

The target collector state is:

```rust,ignore
enum GcPhase {
    Idle,
    MarkRoots,
    MarkGray,
    RemarkRoots,
    Sweep,
}

struct CollectorState {
    cycle_id: u64,
    phase: GcPhase,
    mark_epoch: u32,
    root_cursor: RootCursor,
    gray: Vec<GrayWork>,
    sweep_cursor: usize,
    debt_bytes: usize,
}
```

The precise representation can change if tests prove the same invariants. It
must not clear every live object's mark bit at cycle start. Prefer an epoch or
equivalent current-white scheme; define and test wraparound normalization.

State transitions:

```text
Idle
  -- debt/soft pressure --> MarkRoots

MarkRoots
  -- root slice done --> MarkGray

MarkGray
  -- gray empty --> RemarkRoots

RemarkRoots
  -- roots stable for this safe point and gray empty --> Sweep
  -- new gray work --> MarkGray

Sweep
  -- arena slice done, side tables cleaned, tail trimmed --> Idle
```

There are no weak tables or script finalizers in Production-v1, so there is no
unbounded ephemeron convergence or user finalizer phase.

### 4.3 Resumable object tracing

Replace `HeapValue::trace_refs(&mut Vec<GcRef>)` as the collector's primary
primitive with bounded tracing:

```rust,ignore
trait TraceHeap {
    fn trace_slice(
        &self,
        cursor: &mut TraceCursor,
        visitor: &mut dyn FnMut(GcRef),
        edge_budget: u32,
    ) -> TraceProgress;
}
```

`TraceCursor` must support:

- tuple/array element index;
- map entry plus key/value position;
- set element index;
- record/enum field position;
- closure capture position;
- iterator source/callback/capture/snapshot positions;
- leaf completion for string, bytes, and PathProxy.

Container mutation between slices must remain safe through insertion barriers.
Cursor continuation must not become quadratic by restarting an ordered
container scan from zero on each step. If a container representation cannot
provide stable O(1)-amortized cursor progress, change its internal traversal
surface or store an explicit bounded work snapshot whose memory is accounted
as GC metadata.

### 4.4 Barrier invariants

Production-v1 uses an insertion barrier:

```text
if marking is active
and a scanned/black parent receives a reference to an unmarked/white child
then mark the child and enqueue it as gray before the mutation completes
```

Required barrier families:

1. **Allocation barrier**
   - An object allocated during mark/remark joins the current epoch.
   - If it can contain heap references, it is enqueued for bounded tracing.
   - An object allocated during sweep cannot be mistaken for old white garbage.

2. **Heap-edge insertion barrier**
   - Covers array/tuple construction and mutation, map key/value insert or
     replace, set insertion, record/enum field writes, closure capture
     construction, iterator/adaptor state, and every future heap aggregate.

3. **Root-admission barrier**
   - Covers frame/continuation roots, protected caller values, persistent state,
     retained `VelaValue`, pending callback/iterator state, async resume
     materialization, and reentry values before child roots are released.

4. **Root-removal behavior**
   - Removing a root never attempts eager reclamation.
   - The object becomes collectible in the current cycle only when removal is
     known to precede root snapshot/remark; otherwise it is reclaimed later.

Centralize these barriers. Do not scatter direct color manipulation through
array, map, set, field, callback, and async modules.

### 4.5 Mutation API

Raw mutable `HeapValue` access is incompatible with an auditable barrier
contract. Replace production uses of `ScriptHeap::get_mut` with one of:

```rust,ignore
heap.mutate(reference, mutation_context, |object| { ... })

heap.array_push(reference, value, budget)
heap.map_insert(reference, key, value, budget)
heap.record_set(reference, field, value, budget)
```

The chosen API must:

- preflight memory and collection-size limits;
- identify every inserted `Value`;
- run the insertion barrier before publishing the edge;
- perform the mutation;
- update shallow/reserved byte accounting;
- roll back precharges on failure;
- invalidate container contract caches only after successful mutation;
- make bypasses discoverable by a zero-hit source audit.

Test-only raw mutation helpers may exist under `#[cfg(test)]` when their names
make barrier bypass explicit.

### 4.6 Root enumeration

One root visitor must cover:

```text
current execution frame registers
protected caller frames
pending comparison/callback/iterator/ordering state
execution arguments and explicit caller roots
Runtime Vela state
retained VelaValue handles
dynamic async/reentry roots
final root return while being retained/materialized
```

Avoid constructing and deduplicating a `BTreeSet` of roots on every step. Mark
epochs naturally suppress duplicate work. Root storage may remain in focused
registries, but collector entry must consume them through one exhaustive root
visitor or snapshot contract.

Interpreter root reporting remains conservative over frame registers.
Verified MIR `root_live_before` facts are a future JIT input and must not be
used to narrow interpreter roots in this plan.

### 4.7 Scheduling

Collection scheduling is driven by allocation debt:

```text
allocation/growth adds debt
GC work repays debt
soft heap growth starts a cycle
safe points perform bounded work
idle/tick-end may grant a larger budget
hard pressure requests emergency progress
```

Safe points include:

- verified instruction safe points already used by the interpreter;
- native/host/reflection return boundaries;
- async suspend/resume and reentry boundaries;
- actor turn/tick end;
- explicit Runtime GC calls.

No-allocation calls should perform zero or constant GC work when there is no
outstanding debt/cycle. An active cycle must still make progress even if the
mutator temporarily stops allocating; actor-turn and idle safe points provide
that progress.

### 4.8 Full and emergency collection

`collect_full` must drive the same state machine to completion with an explicit
unlimited or caller-supplied budget. Do not maintain a second reachability
algorithm.

Emergency behavior:

1. Preflight the requested allocation/growth.
2. If it would exceed the hard limit, perform a complete collection only when
   the execution context can supply the full current root set.
3. Recompute available memory.
4. Retry the preflight once.
5. Return structured `BudgetExceeded::MemoryBytes` without mutating if it still
   cannot fit.

Low-level `ScriptHeap` APIs without an execution root context must not invent
an empty root set for emergency collection.

### 4.9 Runtime call completion

Separate three cases:

1. **Temporary managed execution**
   - Materialize an `OwnedValue`, then dropping the entire temporary heap is
     sufficient. A full GC solely to release every object is unnecessary.

2. **Persistent actor Runtime returning `VelaValue`**
   - Admit the return into retained roots before releasing execution roots.
   - Perform only scheduled bounded GC work.

3. **Persistent actor Runtime returning/materializing `OwnedValue`**
   - Materialize while execution roots remain live.
   - Remove transient roots afterward.
   - Perform only scheduled bounded GC work.

Explicit state removal, last `VelaValue` drop, reload pruning, and actor idle
may increase collection urgency but do not each require synchronous full GC.

### 4.10 Configuration and telemetry

Replace the misleading current pacing surface with a configuration whose names
match behavior:

```rust,ignore
pub struct GcConfig {
    pub step: GcStepBudget,
    pub idle_step: GcStepBudget,
    pub heap_growth_factor: f64,
    pub min_trigger_bytes: usize,
    pub debt_work_ratio: u32,
    pub trim_trailing_free_slots: bool,
}
```

Exact fields may change after baseline data, but configuration must validate:

- finite growth factor at least `1.0`;
- nonzero work budget when automatic GC is enabled;
- no integer overflow in threshold/debt calculations;
- explicit disabled/automatic/manual mode rather than magic maximum values.

Expose a cheap snapshot:

```rust,ignore
pub struct GcMetrics {
    pub cycle_id: u64,
    pub phase: GcPhase,
    pub live_objects: usize,
    pub heap_slots: usize,
    pub free_slots: usize,
    pub live_bytes: usize,
    pub debt_bytes: usize,
    pub total_allocated_bytes: u64,
    pub total_reclaimed_bytes: u64,
    pub total_cycles: u64,
    pub total_steps: u64,
    pub total_work_units: u64,
    pub max_step_work_units: u32,
    pub max_step_micros_observed: u64,
    pub emergency_collections: u64,
}
```

Histograms belong in the host/telemetry integration, not as eagerly allocated
per-actor arrays. The default Runtime must keep GC metadata sparse and lazy.

## 5. Phase A — Baseline, Invariants, And Missing Regressions

Do not change the algorithm in this phase.

### A1. Freeze current semantics

- [ ] Record current `ScriptHeap`, `HeapExecution`, Runtime root, and call-end
  collection behavior in focused tests.
- [ ] Add a test proving stale `GcRef` cannot access a reused slot after many
  reuse cycles and generation changes.
- [ ] Add a graph oracle helper that computes reachability independently from
  collector marks.
- [ ] Add tests for every `HeapValue` edge kind, including nested iterator
  adapters and closure captures.
- [ ] Add hot-reload tests proving live closures pin their old artifact and
  unreachable closures release it after collection.
- [ ] Add async suspension/reentry tests covering parent frames, returned
  values, dynamic root guard clone/drop, cancellation, error, and abort.

### A2. Add adversarial tests that may initially expose gaps

- [ ] Allocate a heap object between partial sweep steps and prove it survives.
- [ ] Insert a new child into an already traced parent between mark steps.
- [ ] Replace map keys/values and record/enum fields between mark steps.
- [ ] Admit and release roots during each collector phase.
- [ ] Trace an aggregate with at least one million logical edges under a tiny
  work budget and assert that one step does not scan the whole object.
- [ ] Grow then clear a heap to create a sparse high-water arena and measure
  sweep work.
- [ ] Run repeated no-allocation root calls over 1, 1,000, and 100,000 live
  objects and record heap work per call.
- [ ] Exercise memory-limit failure for allocation and every mutable collection
  operation, asserting no partial mutation.

### A3. Freeze measurements

- [ ] Extend `gc_pacing` or add focused workloads for:
  - mark-heavy live graph;
  - dead-object sweep;
  - large aggregate trace;
  - sparse arena sweep;
  - persistent short calls;
  - allocation-heavy actor turns;
  - long-lived actor state;
  - async suspend/reentry roots.
- [ ] Record mean, median, P95, P99, allocations, live bytes, heap slots, GC
  work, and checksum where the harness supports them.
- [ ] Run `actor_memory` for 1, 100, and 10,000 Runtime instances.
- [ ] Archive the baseline report under `docs/archive/` and keep only the
  durable conclusion and commands in `docs/performance.md`.

### Phase A validation

```bash
cargo test -p vela_vm heap
cargo test -p vela_vm heap_execution
cargo test -p vela_vm linked_async_execution
cargo test -p vela_engine runtime
cargo bench -p vela_vm --bench baseline -- gc
cargo bench -p vela_engine --bench actor_memory -- memory
cargo bench -p vela_engine --bench actor_concurrency
```

Phase A is complete only when failures are classified as expected plan work,
not hidden or deleted.

## 6. Phase B — Heap And Mutation Architecture

### B1. Split modules

- [ ] Split heap handles/entries, values/tracing, collector, pacing, metrics,
  barriers, and tests into focused modules.
- [ ] Keep public re-exports minimal and intentional.
- [ ] Keep all ordinary source files below 1200 lines.
- [ ] Preserve current full-GC behavior while the split lands.

### B2. Centralize allocation

- [ ] Route production object allocation through `HeapExecution` or another
  root-aware allocation context.
- [ ] Keep raw `ScriptHeap::allocate` only where a root context is genuinely
  unnecessary, such as initialization before values become observable.
- [ ] Centralize slot reuse, generation advancement, byte precharge, and
  container-contract initialization.
- [ ] Define generation overflow behavior and test it without relying on a
  multi-billion-iteration loop.

### B3. Centralize mutation

- [ ] Inventory every `get_mut`, direct `HeapValue` mutation, array/map/set
  mutation helper, field write, iterator construction, closure construction,
  and materializing cache mutation.
- [ ] Replace production raw mutation with barrier-capable mutation APIs.
- [ ] Add a source zero-hit audit for unauthorized raw mutable heap access.
- [ ] Make size/budget updates failure-atomic.
- [ ] Preserve container contract invalidation and type-summary behavior.

### B4. Introduce dormant barrier hooks

- [ ] Add allocation, edge insertion, and root-admission barrier entrypoints.
- [ ] Invoke them exhaustively while collector marking is still monolithic.
- [ ] Add counters/tests proving each mutation family hits the expected hook.
- [ ] Do not enable incremental marking until the barrier audit is complete.

### Phase B validation

```bash
cargo fmt --all -- --check
cargo clippy -p vela_vm -p vela_engine --all-targets -- -D warnings
cargo test -p vela_vm
cargo test -p vela_engine runtime
rg -n "get_mut\\(|HeapValue::.*=" crates/vela_vm/src
```

Commit boundary:

```text
refactor(vm): centralize script heap mutation
```

## 7. Phase C — Fully Incremental Mark And Sweep

### C1. Mark epochs and phases

- [ ] Replace cycle-start full mark clearing with an epoch/current-white
  representation.
- [ ] Add explicit `GcPhase` transitions and cycle IDs.
- [ ] Define object white/gray/black predicates in one module.
- [ ] Test phase transitions with one work unit per step.
- [ ] Test mark epoch wraparound normalization.

### C2. Incremental roots and gray work

- [ ] Make root scanning resumable and work-budgeted.
- [ ] Store gray work with resumable object trace cursors.
- [ ] Charge every inspected root, edge chunk, and heap object.
- [ ] Ensure stale refs in roots/edges are ignored or diagnosed according to
  the existing Value contract without corrupting collector state.
- [ ] Reuse collector buffers across cycles without eager per-actor capacity.

### C3. Resumable aggregate trace

- [ ] Implement bounded tracing for every `HeapValue` variant.
- [ ] Prove a million-edge array cannot consume more than the configured edge
  slice in one step.
- [ ] Prove map/set cursor continuation is linear overall, not quadratic.
- [ ] Cover iterator adapter state and snapshots exhaustively.

### C4. Incremental sweep

- [ ] Preserve slot-budgeted sweep under the new work-unit budget.
- [ ] Protect objects allocated during sweep.
- [ ] Remove container-contract and dependent side-table entries on reclaim.
- [ ] Release memory-budget bytes exactly once.
- [ ] Complete a cycle only after all arena and side-table cleanup is done.

### C5. Full collection through the same state machine

- [ ] Implement full collection as repeated steps over the same phases.
- [ ] Delete the independent monolithic mark/sweep algorithm.
- [ ] Preserve explicit full-GC statistics and memory release semantics.

### Phase C validation

```bash
cargo test -p vela_vm heap
cargo test -p vela_vm heap_execution
cargo test -p vela_vm --test conformance
cargo bench -p vela_vm --bench baseline -- gc
```

Commit boundary:

```text
feat(vm): add bounded incremental garbage collection
```

## 8. Phase D — Barrier And Root Correctness

### D1. Allocation barrier

- [ ] Allocate during every collector phase and verify reachability.
- [ ] Ensure a new aggregate's initial children are traced or barriered.
- [ ] Ensure slot reuse during sweep cannot inherit stale color/epoch/age.

### D2. Heap-edge barrier

- [ ] Cover array push/insert/replace/extend and higher-order materialization.
- [ ] Cover map insert/replace/merge and both key and value edges.
- [ ] Cover set insertion/combination.
- [ ] Cover record/enum construction and field writes.
- [ ] Cover closure captures and iterator/adaptor state.
- [ ] Cover standard-method cache materializing mutation routes.
- [ ] Add a table-driven test that fails when a new mutation operation is
  registered without a barrier classification.

### D3. Root barrier

- [ ] Cover current and caller frames.
- [ ] Cover protected intermediate callback/iterator/ordering state.
- [ ] Cover Runtime persistent state insertion/replacement/removal.
- [ ] Cover `VelaValue` retain/clone/drop.
- [ ] Cover async result materialization, suspension, cancellation, and resume.
- [ ] Cover reentry return admission before child-root truncation.
- [ ] Cover final result retention/materialization ordering.

### D4. Interleaving stress

- [ ] Add a deterministic scheduler that yields GC after every root, edge,
  allocation, mutation, and sweep primitive.
- [ ] Run representative programs under normal scheduling and forced
  one-unit-step scheduling and compare results.
- [ ] Compare collector liveness against the independent graph oracle after
  randomized allocation/mutation/root sequences.
- [ ] Include cyclic graphs, stale handles, slot reuse, errors, and aborts.

### Phase D validation

```bash
cargo test -p vela_vm
cargo test -p vela_engine runtime
cargo test -p vela_engine --test service_async
cargo test --workspace async
```

Commit boundary:

```text
fix(vm): enforce incremental GC barriers
```

## 9. Phase E — Production Scheduling And Call Boundaries

### E1. Allocation debt

- [ ] Define debt accrual from allocation and in-place growth.
- [ ] Define work-unit repayment and threshold recomputation.
- [ ] Guarantee an allocating mutator cannot indefinitely outrun collection
  without reaching soft/hard pressure.
- [ ] Guarantee an active cycle progresses at actor-turn/idle safe points even
  when allocation pauses.
- [ ] Use saturating/checked arithmetic with overflow tests.

### E2. Safe-point policy

- [ ] Integrate bounded steps with interpreter verified safe points.
- [ ] Add native, host, reflection, async, reentry, and actor-turn boundaries.
- [ ] Avoid scanning roots or checking the clock on the hot path when no cycle,
  debt, deadline, or pressure is active.
- [ ] Keep GC work separate from script execution-unit counters.

### E3. Remove ordinary call-end full GC

- [ ] Replace persistent Runtime completion full collection with scheduled work.
- [ ] Admit retained return values before execution-root release.
- [ ] Materialize owned return values before transient-root release.
- [ ] Drop temporary managed heaps directly after materialization.
- [ ] Add no-allocation repeated-call tests proving work does not scale with
  heap size.

### E4. Explicit and emergency collection

- [ ] Expose explicit step and full collection at the Runtime/embedding layer.
- [ ] Add idle/tick-end maintenance entrypoints or policy hooks.
- [ ] Implement hard-limit emergency collect-and-retry exactly once.
- [ ] Preserve original structured error and call frames when memory remains
  exhausted.
- [ ] Never perform emergency collection with an incomplete or invented root
  set.

### Phase E validation

```bash
cargo test -p vela_vm
cargo test -p vela_engine runtime
cargo bench -p vela_vm --bench baseline -- gc
cargo bench -p vela_engine --bench actor_concurrency
```

Commit boundary:

```text
feat(engine): schedule actor-local GC by allocation debt
```

## 10. Phase F — Memory, Pacing, Metrics, And Operations

### F1. Memory accounting

- [ ] Audit `HeapValue::shallow_size_bytes` against actual owned capacities.
- [ ] Choose and document whether collection capacity or length is charged.
- [ ] Include mutation reservation/precharge without double charging.
- [ ] Keep GC metadata accounting separate and observable.
- [ ] Verify `ExecutionBudget::memory_bytes_allocated()` and heap live bytes
  agree under the documented meaning after allocation, mutation, rollback,
  collection, and error.

### F2. Heap-slot high-water handling

- [ ] Trim trailing free entries after completed collection when enabled.
- [ ] Remove trimmed indexes from the free list efficiently.
- [ ] Never move a live entry or change a live `GcRef`.
- [ ] Benchmark sparse high-water heaps before and after trimming.
- [ ] Evaluate free-list duplication/integrity with debug assertions and stress
  tests.

### F3. Work and time pacing

- [ ] Enforce `max_work_units` on every ordinary step.
- [ ] Add an injectable/fake clock for deadline tests.
- [ ] Check the soft deadline at a bounded work interval.
- [ ] Report actual work and observed duration in `GcStepReport`.
- [ ] Rename/remove the current unused `max_pause_micros` surface rather than
  retaining a misleading compatibility alias.

### F4. Metrics and diagnostics

- [ ] Expose cheap cumulative and current-cycle metrics.
- [ ] Do not allocate per-actor histogram arrays by default.
- [ ] Provide host hooks for exporting step/cycle metrics.
- [ ] Add structured diagnostics for invalid configuration.
- [ ] Document operational interpretation and tuning.

### F5. Configuration

- [ ] Add RuntimeBuilder/default configuration wiring.
- [ ] Validate configuration before Runtime construction.
- [ ] Provide conservative defaults based on benchmark evidence.
- [ ] Support automatic, manual, and disabled automatic stepping explicitly.
- [ ] Keep hard execution memory limits independent from GC tuning.

### Phase F validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p vela_vm
cargo test -p vela_engine
cargo bench -p vela_engine --bench actor_memory -- memory
cargo bench -p vela_engine --bench actor_memory -- allocations
```

Commit boundaries:

```text
feat(vm): expose GC pacing and metrics
feat(engine): configure actor Runtime collection
```

## 11. Phase G — Production-v1 Acceptance

### G1. Correctness matrix

- [ ] Empty heap, leaf-only heap, deep chain, wide graph, diamond graph, and
  unreachable cycles.
- [ ] Every HeapValue edge kind.
- [ ] Allocation/mutation/root admission in every collector phase.
- [ ] Slot reuse, stale refs, generation overflow normalization.
- [ ] Persistent state and retained values.
- [ ] Nested calls, callbacks, iterators, errors, try propagation, and abort.
- [ ] Async suspend/resume/cancel and nested reentry.
- [ ] Hot reload with old closure generation retention and later reclamation.
- [ ] HostRef and PathProxy host-target exclusion.
- [ ] Memory and collection-growth limit failure atomicity.
- [ ] Explicit full, automatic incremental, manual, and disabled modes.

### G2. Stress and differential gates

- [ ] At least 1,000 deterministic randomized seeds in the bounded CI stress
  test, with a longer ignored/nightly form.
- [ ] At least 1,000,000 allocation/mutation/root operations in an extended
  local stress run.
- [ ] Reachability oracle matches collector survivors after each forced cycle.
- [ ] One-unit scheduling produces the same script-visible result and errors as
  unlimited-step scheduling.
- [ ] No panic, double reclaim, invalid free-list entry, or budget
  under/over-release.

### G3. Deterministic work gates

- [ ] An ordinary incremental step reports no more than its configured work
  units, except a documented fixed setup constant that is separately bounded.
- [ ] No aggregate trace primitive scans more than its edge slice.
- [ ] No-allocation calls with no active cycle perform zero heap-size-dependent
  GC work.
- [ ] Active cycles make monotonic progress at turn/idle safe points.
- [ ] A full collection terminates under an unlimited budget.

### G4. Performance and memory gates

Freeze toolchain and machine metadata in the acceptance report. Compare against
Phase A and the parent commit.

- [ ] `gc_pacing` checksum remains valid and mean/P95 regressions are explained.
- [ ] Mark-heavy and large-aggregate P99 no longer scale as one monolithic
  step; deterministic work reports prove the bound even when wall-clock samples
  are noisy.
- [ ] Persistent no-allocation short-call GC work is constant rather than
  proportional to 1/1,000/100,000 live-object heaps.
- [ ] Actor concurrency P50/P95/P99 and throughput do not regress by more than
  5% without an accepted correctness or memory rationale.
- [ ] Non-GC scalar and host-boundary baseline means do not regress by more than
  3% without a profile-backed explanation.
- [ ] Default empty Runtime memory does not regress by more than 5% or 128 bytes,
  whichever is larger, without an accepted operational benefit.
- [ ] Allocation-heavy peak script memory remains within the configured hard
  limit plus explicitly measured allocator/GC metadata.
- [ ] Sparse heap slot trimming materially reduces later sweep work without
  invalidating handles.

These are regression gates, not universal production SLAs. Record workload
specific absolute P95/P99 values so hosts can choose their own SLA.

### G5. Source and architecture audits

- [ ] Zero unauthorized production raw `HeapValue` mutation hits.
- [ ] Zero collector paths tracing Rust host state.
- [ ] Zero ordinary persistent call-return unconditional full-GC hits.
- [ ] Zero misleading strict-pause API claims.
- [ ] Zero ordinary source files over 1200 lines without a documented
  exception.
- [ ] No duplicate full-collection reachability algorithm.
- [ ] No second root policy for async/reentry.

### G6. Required full validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo bench -p vela_vm --bench baseline --no-run
cargo bench -p vela_engine --bench actor_memory --no-run
cargo bench -p vela_engine --bench actor_concurrency --no-run
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.vela
```

Run any available sanitizer, Miri, fuzz, or loom-equivalent checks that cover
new unsafe or cross-thread code. The preferred implementation adds no unsafe
collector code and no concurrent heap mutation.

### G7. Documentation and completion

- [ ] Update `docs/architecture/runtime.md` with the actual collector,
  barriers, scheduling, and Runtime API.
- [ ] Update `docs/decisions.md` with the non-moving incremental-v1 decision,
  work-budget semantics, and call-end policy.
- [ ] Update `docs/performance.md` with only durable baseline/acceptance
  conclusions; archive raw reports.
- [ ] Update `docs/progress.md` only when the production GC focus/status changes.
- [ ] Mark this plan Production-v1 complete only after all A-G gates pass.
- [ ] Commit all plan-owned work with coherent Conventional Commits.

Production-v1 completion commit:

```text
feat(vm): complete production incremental GC
```

## 12. Phase H — Evidence-Gated Generational Follow-On

This phase is a required evaluation and an optional implementation. Do not
implement it merely because generational GC is common.

### H1. Evidence gate

Capture representative game-server actor workloads containing:

- short event/tick temporaries;
- persistent actor state;
- collection-heavy callbacks;
- host-boundary-heavy logic;
- async suspension;
- hot-reload retained closures;
- large-structure construction that may have high survival.

- [ ] Capture representative age/survival distributions and model young-GC work
  plus per-Runtime metadata.
- [ ] Record whether the gate passes and the measured implement/defer decision.

The implementation gate passes only if:

- at least 70% of newly allocated script bytes die before surviving two
  completed young collection opportunities in representative allocation-heavy
  workloads; and
- a prototype/model predicts at least 20% lower GC tracing/sweep work or 10%
  higher actor throughput; and
- projected/default per-Runtime metadata stays within the Phase G memory gate;
  and
- major collection remains incrementally paced rather than becoming an
  unbounded stop-the-world path.

If the gate does not pass:

- [ ] Record the workload, survival distribution, and decision in an archive
  report.
- [ ] Keep incremental mark-sweep as the production default.
- [ ] Mark Phase H evaluation complete without implementing generations.

### H2. Target generational design when justified

Use non-moving ages:

```text
Young -> Survivor -> Old
```

Required properties:

- bounded young-generation size/work;
- old-to-young remembered set maintained by the existing insertion barrier;
- young root and remembered-set scan;
- promotion after a measured/configurable survival count;
- incremental major collection using the Production-v1 state machine;
- no stop-the-world full major in the ordinary path;
- fallback to pure incremental mode for high-survival workloads;
- per-Runtime mode/configuration with conservative defaults;
- lazy remembered-set allocation.

### H3. Generational tests

- [ ] Young cycles are reclaimed.
- [ ] Old-to-young edges preserve young objects.
- [ ] Removing/replacing remembered edges does not leak indefinitely.
- [ ] Promotion and slot reuse preserve `GcRef`.
- [ ] Async and retained roots preserve young/old graphs.
- [ ] Major collection reclaims unreachable old cycles.
- [ ] High-survival workload falls back or avoids repeated wasteful minors.
- [ ] Mode switching preserves all live objects.

### H4. Generational acceptance

- [ ] Allocation-heavy actor throughput improves by at least 10% or GC work
  drops by at least 20% on the evidence workloads.
- [ ] P99 does not regress by more than 5%.
- [ ] Empty Runtime and 10,000-actor memory stay within Phase G gates.
- [ ] High-survival workload does not regress by more than 5%, or automatically
  falls back to incremental mode.
- [ ] All Phase G correctness and validation gates remain green.

Possible commit:

```text
perf(vm): add non-moving young-generation GC
```

## 13. Commit And Recovery Strategy

Preferred verified checkpoints:

```text
test(vm): add GC interleaving regressions
refactor(vm): centralize script heap mutation
feat(vm): add bounded incremental garbage collection
fix(vm): enforce incremental GC barriers
feat(engine): schedule actor-local GC by allocation debt
feat(vm): expose GC pacing and metrics
docs: record production GC acceptance
```

Rules:

- Do not mix generational work into Production-v1 collector commits.
- Do not hide a failing barrier test by forcing full collection.
- Do not retain obsolete raw mutation APIs as compatibility shims.
- Do not tune defaults before Phase A baselines exist.
- Do not update `docs/progress.md` as a per-commit changelog.
- Preserve unrelated worktree changes and commit only verified plan scope.

When a regression appears:

1. Minimize it into a collector/root/barrier test.
2. Identify whether the violated owner is heap, execution roots, Runtime roots,
   async reentry, memory budget, or scheduling.
3. Fix the owning abstraction rather than adding a call-site special case.
4. Rerun the focused test, related crate, and the nearest latency/memory row.
5. Continue to the next unchecked item.

## 14. Final Production-v1 Definition

The collector is production-ready for Vela's intended game-server scripting
model only when all of the following are simultaneously true:

```text
non-moving stable GcRef handles
+ fully incremental bounded mark and sweep
+ resumable aggregate tracing
+ exhaustive allocation/edge/root barriers
+ complete sync/async/reentry/persistent roots
+ allocation-debt and actor-safe-point scheduling
+ no ordinary call-end full heap scan
+ hard memory failure atomicity
+ HostRef exclusion and GC-independent host cleanup
+ operational metrics and validated configuration
+ randomized oracle-backed stress tests
+ actor P95/P99, throughput, and memory acceptance
+ full workspace validation
```

Passing unit tests alone is not sufficient. A fast microbenchmark alone is not
sufficient. Production readiness requires correctness under adversarial
interleaving, deterministic bounded work, representative actor-tail evidence,
memory-limit behavior, and an operational surface that reports what the
collector is doing.
