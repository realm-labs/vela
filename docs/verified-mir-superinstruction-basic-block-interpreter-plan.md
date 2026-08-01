# Verified-MIR Superinstruction And Basic-Block Interpreter Plan

> **Track:** post-M20 non-JIT interpreter structural optimization, before M22
> Cranelift JIT
>
> **Status:** planned execution contract; no batch is accepted yet
>
> **Compatibility policy:** pre-release hard switch. Internal instruction,
> verifier, artifact, profiler, and frame-location shapes may change without a
> compatibility layer. The first portable plan representation raises ordinary
> and Service artifact formats to version 4 and rejects versions 1-3.
>
> **Supersedes:** the remaining execution work in
> [typed-scalar-bytecode-optimization-plan.md](typed-scalar-bytecode-optimization-plan.md).
> Its completed verified i64 opcodes, 16-byte `Value`, linked operands,
> execution-mode specialization, and benchmark harness remain prerequisites;
> its bytecode-adjacency superinstruction prompt must not be resumed.
>
> **Lead scope:** proven i64 scalar branches, straight-line blocks, and
> single-entry/single-latch loops. Complex calls, HostAccess, reflection,
> allocation, task admission, and suspension remain ordinary VM boundaries.

## Persistent Goal

```text
/goal Implement the Verified-MIR Superinstruction And Basic-Block Interpreter
Plan in docs/verified-mir-superinstruction-basic-block-interpreter-plan.md.
Treat docs/goal.md as the product roadmap, docs/architecture.md as the
technical contract, docs/progress.md as current milestone state, and
docs/performance.md as the measurement contract.

Build one production interpreter with two execution granularities: ordinary
linked instructions for complex or ineligible operations, and verified-MIR-
selected superinstructions, scalar blocks, and scalar loop regions for proven
hot paths. Do not create a second VM, public optimization toggle, legacy
interpreter mode, bytecode-adjacency peephole pass, JIT, source/HIR query from
the backend, speculative type system, or benchmark-name special case.

Selection must consume MirBackendHandoff and its sealed CFG, program-point
facts, effects, value/root liveness, safepoints, budget schedule, guards,
source origins, and exact generation targets. Every selected unit must carry a
machine-verifiable one-to-one coverage map for the MIR operations, terminator,
budget sites, trap/source points, and exits it implements. Reject missing,
duplicate, reordered, or cross-boundary coverage before a LinkedArtifact can
be executed.

Preserve exact Vela semantics: checked arithmetic, trap and side-effect order,
source-spanned errors, execution-unit charging on the same semantic boundary,
GC root visibility, debugger-ready source points, profiler ownership,
HostAccess permissions, call-scoped leases, async suspension, scoped-task
isolation, and exact hot-reload/Service generations. A selected block may not
contain allocation, call, HostAccess, reflection, task, await, safepoint, or
unknown dynamic dispatch in the first accepted version. A selected loop must
charge every taken budgeted edge before continuing; it may not turn per-
iteration limits into one aggregate charge.

Use a measurement-first hard-retention policy. Freeze stable same-toolchain
before baselines and opcode/CFG inventories, profile the dominant stacks,
implement one bounded selection family, run fresh-build interleaved candidates,
and revert candidates that miss their batch performance gate or introduce a
stable guardrail regression. Do not retain speculative machinery merely
because its structure looks cleaner. Preserve the current ordinary instruction
fallback for semantically ineligible code, but expose no production flag that
selects the pre-optimization backend.

When selected plans become portable, atomically raise the ordinary program,
Service artifact, and Service deployment bundle formats to version 4. Encode
all execution plans and verification sidecars required to load without MIR;
reject versions 1-3 without inference or compatibility readers. Plans, caches,
profiles, and counters remain exact-generation data shared by all Runtimes for
that artifact and must not be duplicated per Actor.

Execute Batches A-G in order. At each batch, run focused compiler, verifier,
VM, artifact, hot-reload, async, Service, and benchmark checks appropriate to
the changed boundary; update docs/decisions.md only for accepted durable
decisions, docs/performance.md only for baseline/threshold/exit changes, and
docs/progress.md only when active focus or milestone status changes. Commit
each coherent verified checkpoint with a Conventional Commit. Do not mark the
track complete until the semantic matrix, stable performance gates, artifact
v4 hard switch, full repository validation, and archived acceptance report all
pass.
```

## 0. Objective And Exit Outcome

The current interpreter already performs the important semantic preparation:

```text
HIR and analysis facts
  -> verified MIR with CFG, effects, liveness, safepoints, and budgets
  -> linked typed operands and specialized i64 instructions
  -> one generation-pinned register VM
```

The remaining scalar cost is structural. A short loop repeatedly fetches a
128-byte `Instruction`, enters the large linked-instruction `match`, reads and
writes 16-byte `Value` slots, and returns to dispatch for each compare,
arithmetic operation, branch, budget stub, and jump. Profiles and rejected
candidates recorded in [decisions.md](decisions.md) show that the current safe-
Rust dispatch body is a local optimum: unchecked register access, partial
instruction shrinking, a complete 64-byte instruction encoding, and extra
inlining all failed their measurements. M20 inline caches also do not move
static pure-language rows whose targets are already linked.

This track changes the unit of interpretation instead of continuing to tune
the large `match` in place:

```text
complex/cold operation -> ordinary dispatch; short sequence -> superinstruction
pure scalar basic block -> one outer dispatch plus compact scalar execution
eligible scalar loop    -> one outer dispatch plus an internal bounded loop
```

The exit outcome is one interpreter. `drive_linked_frame` remains the only
production frame driver and retains ordinary instructions as the semantic
fallback. New `RunScalarBlock`/`RunScalarLoop`-style units call focused
executors from that same driver; they do not form a parallel Runtime, stack,
heap, call engine, or public execution mode.

Completion requires both correctness and measured value:

- selected units implement exactly the verified MIR they cover;
- ineligible functions retain current behavior through ordinary instructions;
- scalar dispatch count falls materially on the frozen lead workloads;
- the stable scalar-suite geometric mean improves by at least 25%;
- `scalar_branch_loop` and `range_iteration` each improve by at least 35%;
- no stable non-target interpreter, host, Service, async, compile-memory, or
  Actor-memory guardrail regresses by more than 5% without an explicit accepted
  trade-off; and
- Lua 5.4 ratios are reported on equivalent embedded workloads, but matching
  Lua on every microbenchmark is not an exit requirement for this track.

Thresholds are retention gates, not permission to overfit. If the complete
implementation misses them, remove the unproductive execution family and
retain only independently proven smaller wins.

## 1. Goals And Non-Goals

### 1.1 Goals

- Select physical execution units directly from verified MIR rather than
  inferring facts from emitted bytecode order.
- Reduce large-dispatch frequency for proven scalar branches and loops.
- Keep ordinary dynamic code, calls, host operations, and suspension on the
  existing interpreter path.
- Preserve exact execution-unit budget placement and partial-progress
  semantics.
- Retain precise source points for traps, profiling, and future debugger side
  exits.
- Keep selected plans immutable, deterministic, generation-qualified, and
  shared across Runtimes.
- Encode portable execution plans so loading never needs HIR, analysis, source,
  or process-local MIR.

### 1.2 Non-goals

This track does not add:

```text
machine-code generation, Cranelift, native traces, speculation, or deoptimization
a second production interpreter or public optimization switch
bytecode-adjacency peepholes or source/HIR queries in the physical backend
new Vela syntax, numeric conversions, arithmetic semantics, or generics
fusion across HostAccess, reflection, allocation, call, task, or await
cross-generation plan reuse or suspended-frame migration
benchmark-name, source-text, function-name, or package-name special cases
an exhaustive opcode Cartesian product
```

The initial accepted block language is intentionally smaller than Vela. It is
not a new user-visible IR and must not become a second semantic implementation
of collections, objects, calls, errors, GC, or host state.

## 2. Current Repository Anchors

The implementation starts from these active boundaries:

- `crates/vela_mir/src/verifier/mod.rs` owns `OwnedVerifiedMirProgram`,
  `MirFunctionAnalyses`, and `MirBackendHandoff`.
- `crates/vela_mir/src/function.rs`, `operations.rs`, and `facts.rs` own the
  CFG, statements, terminators, effects, program-point facts, and liveness.
- `crates/vela_bytecode/src/compiler/mir_backend/core.rs` currently walks each
  MIR block and emits each statement/terminator directly.
- `crates/vela_bytecode/src/compiler/mir_backend/core/physical.rs` already
  demonstrates MIR-fact-driven instruction selection for `I64Add`, immediate
  arithmetic, shapes, and slots.
- `crates/vela_bytecode/src/linked.rs` owns linked instruction and code-object
  layout; one `Instruction` currently has a measured 128-byte stride.
- `crates/vela_bytecode/src/artifact.rs` binds verified MIR functions to exact
  linked executables and verifies budget mapping.
- `crates/vela_bytecode/src/portable.rs` owns portable program format version
  3 and deliberately strips process-local MIR.
- `crates/vela_vm/src/linked_execution.rs` owns the one production frame
  driver and exhaustive instruction dispatch.
- `crates/vela_vm/src/frame.rs` and `frame/registers.rs` own register/frame
  storage and pooling.
- `crates/vela_vm/benches/external_compare.rs` and the `baseline` harness own
  the pure-language, cache, and external-runtime comparison surface.
- `crates/vela_engine` benchmark suites own HostAccess, Service, async,
  detached-task, Actor memory, and concurrency guardrails.

The existing `I64CmpImmJumpIfFalse` linked/runtime shape has verifier and VM
support but no production MIR selection site. It is a suitable first vertical
proof if Batch A confirms the corresponding MIR pattern is frequent enough;
its existence does not exempt it from the measurement gate.

## 3. Normative Semantic Invariants

### 3.1 Observable equivalence

For every selected unit and the ordinary instruction sequence it replaces:

```text
same return value or terminal error category
same checked arithmetic and division/remainder behavior
same state visible after every possible trap
same order and count of observable effects
same source span and call-stack attribution
same branch, loop, break, continue, try, and return behavior
same execution-unit, memory, collection, and call-depth limit behavior
same GC-reachable values at every permitted collection point
same generation, service, capability, lease, and host authority
same result in unbounded, budgeted, profiled, async, and reloadable execution
```

Fusing operations never grants permission to reorder them. If operation two
traps, writes completed by operation one remain visible and operation three
does not run. A selector may reject a candidate that cannot encode this
precisely; it may not weaken the behavior to make fusion convenient.

### 3.2 One production interpreter

Ordinary instructions and selected units are variants of one linked executable
format driven by one `ExecutionSession`, frame stack, register file, heap,
budget, host context, profiler, and error model. There is no production
`legacy`/`optimized` Runtime flag and no second source-to-VM route.

A `cfg(test)` selector-disable policy or test-support constructor may exist to
prove equivalence. It must not be exported by the embedding API, serialized as
a deployment option, or used by production Engine execution.

### 3.3 Exact-generation ownership

Selected plans belong to their owning `LinkedCodeObject` and
`LinkedArtifact`. Plan handles are dense generation-local indexes, never
stable cross-artifact IDs. Old frames, closures, async roots, detached workers,
and continuations retain old plans through the same artifact owner they retain
today. Reload never edits or rebases a selected plan.

Caches, hotness, profile counters, and any plan-specific metadata follow the
existing M20 ownership classification. Immutable plans are shared per exact
generation. Mutable counters live in generation execution data, not in every
Actor Runtime and not in global/thread-local state.

## 4. Selection Architecture

### 4.1 Backend input

The selector accepts only `MirBackendHandoff` plus the function and its sealed
`MirFunctionAnalyses`. It may consume:

```text
MirBasicBlock CFG and terminators
MirStatement operations and destinations
MirProgramPointFacts and declared MirValueType
MirEffect
value and root liveness
MirSafepoint identities and live-root sets
MirBudgetSchedule and MirBudgetSite
guards and try-region structure
source/debug origins and lexical availability
sealed target IDs and signatures
```

It may not consume HIR nodes, source text, analysis databases, emitted
instruction adjacency, benchmark identity, or current Runtime values.

### 4.2 Selected function plan

The exact private Rust names may evolve, but the accepted model has these
logical parts:

```rust
struct SelectedFunctionPlan {
    units: Box<[SelectedUnit]>,
    block_entries: Box<[SelectedUnitId]>,
    coverage: Box<[SelectedCoverage]>,
    source_points: Box<[SelectedSourcePoint]>,
}

enum SelectedUnit {
    Ordinary(MirUnitRange),
    Superinstruction(SuperinstructionPlan),
    ScalarBlock(ScalarBlockPlan),
    ScalarLoop(ScalarLoopPlan),
}
```

`Ordinary` is a compile-time selection result, not a runtime wrapper around
MIR. It causes the existing MIR backend to emit the canonical ordinary linked
instructions. Selected plans never retain borrowed MIR pointers.

### 4.3 Coverage proof

Every non-ordinary unit carries enough compile/link metadata to prove a one-to-
one implementation relation:

```rust
struct SelectedCoverage {
    function: MirFunctionId,
    blocks: Box<[MirBlockId]>,
    statements: Box<[MirStatementId]>,
    terminators: Box<[MirBlockId]>,
    budget_sites: Box<[MirBudgetSite]>,
    safepoints: Box<[MirSafepointId]>,
    exits: Box<[SelectedExitCoverage]>,
}
```

The compiler and linked-artifact verifier independently require:

- every selected statement and terminator belongs to the declared function;
- no MIR operation is omitted or covered twice;
- selection respects CFG predecessor/successor and try-region structure;
- every budget site is implemented exactly once at the correct semantic
  position;
- a unit declaring no safepoint covers no operation that requires one;
- every exit targets a valid selected-unit or ordinary-instruction entry;
- every trapping sub-operation has a valid source point;
- all registers, constants, plan handles, and linked targets resolve; and
- portable plans reproduce the same verified physical coverage without
  needing MIR at load time.

Portable artifacts cannot repeat the full process-local MIR proof. They carry
a sealed, deterministic physical coverage manifest produced by the compiler;
the decoder and linker verify its internal completeness, bounds, hashes, and
plan correspondence. Source compilation additionally verifies that manifest
against owned verified MIR before publishing the artifact.

### 4.4 Region construction

Selection proceeds deterministically:

1. enumerate MIR blocks in stable function order;
2. identify hard boundaries and CFG entries;
3. form maximal eligible straight-line scalar regions inside each block;
4. match profile-approved short superinstruction recipes first;
5. represent remaining sufficiently large eligible sequences as scalar blocks;
6. identify natural single-entry/single-latch scalar loops whose complete body
   is eligible;
7. prefer a loop region only when it replaces its component scalar blocks and
   preserves every edge charge/source point; and
8. emit ordinary instructions for every unmatched operation.

Selection is deterministic for identical verified MIR and compiler version.
Runtime hotness does not rewrite code or install speculative units in this
track.

## 5. Eligibility And Split Rules

### 5.1 Initial scalar operation set

The first accepted compact scalar language may contain only proven operations
from a bounded family:

```text
Value/boolean move where identity and Missing semantics are explicit
i64 checked add, subtract, and multiply
i64 checked remainder with proven nonzero or explicit trap support
i64 immediate variants
i64 comparisons
boolean not/truth values whose input fact is exact
constant scalar loads that allocate nothing
unconditional jump and proven bool/i64 comparison branch exits
proven i64 range cursor/termination mechanics
```

An operation is selected only when all incoming CFG paths establish the facts
required by its compact representation. A typed parameter guard remains at
the ordinary entry boundary unless the normal call-site proof already removes
it. The selector does not treat a currently observed value as a proof.

### 5.2 Hard boundaries

The first accepted selector splits before and after:

```text
script, closure, native, stdlib, provider, or Service call
dynamic callable or dynamic method dispatch
HostAccess read, write, mutate, remove, release, or call
reflection read, write, or call
allocation, materialization, formatting, aggregate, or closure construction
task admission or task-result machinery
await, suspension, resume, or other async boundary
GC safepoint
try propagation or a try-region edge not explicitly modeled by the unit
iterator/callback operation that may suspend internally
state or externally observable effect in the initial version
unknown/dynamic type guard or slow path
debugger-required side exit that the selected plan cannot represent
```

Operations that merely may trap are not automatically excluded. They are
eligible only when the compact executor preserves exact order, partial writes,
error category, and per-operation source point. Batch D starts with the
narrower nontrapping/checked-i64 subset and expands only with tests and profile
evidence.

### 5.3 Minimum-size and profitability rules

The selector must not wrap every one- or two-op block in an indirection-heavy
plan. Batch A freezes a cost model; until measurements justify different
values, candidates require:

```text
superinstruction: at least two eliminated outer dispatches
scalar block: at least three compact operations or two plus a fused exit
scalar loop: at least two eliminated outer dispatches per taken iteration
```

The plan records accepted/rejected candidate counts and reasons for benchmark
inspection. Profitability affects only physical selection, never semantics.

## 6. Physical Representation

### 6.1 Short superinstructions

Small fused families whose operands fit the existing instruction payload may
remain direct `InstructionKind` variants. The lead candidate is:

```text
proven i64 compare against immediate + sole-use conditional branch
  -> I64CmpImmJumpIfFalse
```

Selection comes from MIR definition/use and terminator facts. The compiler does
not emit `I64CmpImm` and later inspect adjacent bytecode to remove it.

Every family requires:

- a stable MIR recipe and sole-use/liveness proof;
- verifier coverage for all operands and targets;
- exact trap/source semantics;
- a dispatch-count reduction report;
- fresh-build interleaved benchmark evidence; and
- removal if its stable candidate misses the retention gate.

Do not grow `InstructionKind` beyond its guarded payload/stride merely to fit a
wide recipe. Operand-rich plans use dense handles into immutable per-code-
object tables.

### 6.2 Scalar block plans

The intended linked shape is logically:

```rust
struct LinkedScalarBlockPlan {
    operations: Box<[ScalarOp]>,
    exit: ScalarExit,
    source_points: Box<[SourcePointId]>,
}

enum ScalarOp {
    LoadScalar { dst: Register, constant: ScalarConstantId },
    Move { dst: Register, src: Register },
    I64Add { dst: Register, lhs: Register, rhs: Register },
    I64AddImm { dst: Register, lhs: Register, imm: i64 },
    I64Sub { /* bounded operands */ },
    I64Mul { /* bounded operands */ },
    I64Compare { /* bounded operands */ },
    BoolNot { dst: Register, src: Register },
}

enum ScalarExit {
    Fallthrough(InstructionOffset),
    Jump(ChargedTarget),
    BoolBranch { condition: Register, passed: ChargedTarget,
                 failed: ChargedTarget },
    I64CompareBranch { /* operands, op, two charged targets */ },
}
```

The compact operation representation is separately layout-tested. It must not
reuse the rejected whole-program 64-byte instruction experiment: a block pays
one table lookup and then amortizes compact operand access across several
operations, while ordinary calls and constructors retain their direct
operands.

The VM enters a block through one ordinary instruction such as
`RunScalarBlock { plan }`, borrows the register file once, executes the compact
operations in order, resolves one exit, and returns the next ordinary
instruction offset. The block executor may use a small exhaustive match,
static handler table, or measured specialized handler functions; the retained
choice must win the same benchmark gate. Guaranteed tail-call optimization may
not be assumed.

### 6.3 Scalar loop plans

A loop plan is accepted only for a natural loop with:

```text
one entry header
one latch/backedge
explicit finite CFG exits
no irreducible control flow
no call, allocation, safepoint, host, reflection, task, or await
only eligible scalar blocks and explicitly modeled branches
complete per-edge budget coverage
no live capability whose lifecycle changes inside the region
```

Logical representation:

```rust
struct LinkedScalarLoopPlan {
    header: ScalarCondition,
    body: Box<[ScalarOp]>,
    continue_target: ChargedLoopEdge,
    exits: Box<[ScalarLoopExit]>,
    source_points: Box<[SourcePointId]>,
}
```

The executor stays inside the loop plan while the modeled backedge is taken.
It exits to ordinary interpreter offsets for `break`, function return staging,
or any non-region successor. `continue` maps to the exact charged latch path.
Nested loops, multiple latches, internal try regions, and loop-carried dynamic
guards are deferred until a later measured extension.

Loop plans do not assert termination. They preserve the existing Runtime's
bounded or explicitly unbounded execution policy. In budgeted mode, every
taken semantic backedge charges before the next iteration; in unbounded mode,
the const execution mode removes inactive charging exactly as today.

## 7. Runtime Execution Contract

### 7.1 One frame driver

The production shape remains:

```rust
match instruction.kind {
    InstructionKind::RunScalarBlock { plan } => {
        ip = execute_scalar_block::<CHARGE_BUDGET, PROFILE>(
            code.scalar_block(plan),
            frame,
            budget,
            profiler,
        )?;
    }
    InstructionKind::RunScalarLoop { plan } => {
        ip = execute_scalar_loop::<CHARGE_BUDGET, PROFILE>(/* same authority */)?;
    }
    ordinary => execute_existing_instruction(ordinary),
}
```

Move scalar execution into focused modules rather than growing
`linked_execution.rs`. Do not duplicate call dispatch, pending operations,
heap ownership, HostAccess, reentry, async resume, or error-stack assembly.

### 7.2 Register and value safety

Linked verification proves all selected register operands in bounds. The first
implementation nevertheless uses the existing checked register APIs: the
repository has measured unchecked indexing slower. `unsafe` indexing or typed
lane reinterpretation is out of scope unless a later profile identifies a new
dominant bound and an interleaved candidate wins materially.

Every compact typed operation still verifies its runtime tag when a malformed
host/test entry could violate a function contract. A later design may hoist a
shared entry guard when MIR and call-site contracts prove it, but this track
does not reinterpret the Rust `Value` enum or store unchecked payload pointers.

### 7.3 Trap order and source points

Each compact operation that can fail maps to its own `SelectedSourcePoint`.
The block executor returns the error immediately and attaches that point before
later operations run. Already completed register writes remain completed.

Selected source points also retain enough stable logical order for future
debugger stepping. The fast no-debug mode performs no per-operation breakpoint
lookup. The plan format reserves subpoint identity and permits a future
instrumented executor to side-exit at those points without reconstructing MIR
or de-fusing the deployment artifact.

### 7.4 Profiling

Aggregate hotness and cache ownership remain physical-unit/generation data.
When `PROFILE` is false, selected execution performs no profile branch. When
`PROFILE` is true, the instrumented compact executor records the selected unit
and its logical subpoints according to the versioned profile layout. Reports
must distinguish:

```text
ordinary instruction hits
superinstruction hits and eliminated-dispatch count
scalar block entries and compact-op count
scalar loop entries, iterations, exits, and charged backedges
```

Profile data may not rewrite plans at runtime in this track. Any future tiering
decision is M22 input and remains exact-generation metadata.

## 8. Budget, GC, Debug, And Effects

### 8.1 Execution-unit budgets

MIR semantic work units remain authoritative. Physical dispatch count never
becomes the budget unit.

- A charge before the first operation may attach to the selected unit entry.
- A charge between operations splits the unit unless the compact format has an
  exact ordered charge point.
- A conditional edge charge belongs only to the matching `ChargedTarget`.
- A loop backedge charge occurs on every taken backedge before continuing.
- Trap-before-effect and effect-before-later-trap ordering remains unchanged.
- The verifier rejects dropped, duplicated, moved, or aggregated budget sites.

Memory, collection-growth, host-call, call-depth, deadline, and task-scope
limits remain in their existing helpers because initial selected units do not
perform those operations.

### 8.2 GC and root liveness

Initial scalar blocks and loops may not allocate, collect, call, suspend, or
create a heap reference. Existing heap references may remain live in untouched
registers while a scalar unit executes; no GC occurs inside the unit. Every
operation requiring a safepoint splits selection and executes through the
ordinary boundary, which uses verified root liveness exactly as today.

The selector and verifier require an empty safepoint coverage set for initial
selected units. A future allocation-capable block design is a separate plan,
not an incremental relaxation hidden inside this track.

### 8.3 Debug readiness

M21 debugger hooks are not implemented here, but this track must not make them
impossible. Every selected operation retains a source point, lexical
availability projection, and stable subpoint order. Region construction splits
at any already-declared debugger boundary. The future debug executor may stop
at a subpoint and retain a `(unit, subpoint)` frame location, while production
no-debug execution continues without those checks.

This track does not implement DAP, breakpoints, stepping, or debug-mode hot
patching.

### 8.4 Host and effect boundaries

No initial selected unit performs HostAccess, reflection, Service dispatch,
native calls, event emission, time, random, I/O, task admission, or state
mutation. Consequently it cannot cache a permission as a grant, retain a
HostRef or lease beyond its current lifetime, reorder externally visible
effects, or bypass capability checks.

Later host-aware superinstructions require a separate profile-backed design
and must continue calling the canonical HostAccess helpers. They are not
implicitly authorized by completion of this plan.

## 9. Hot Reload, Async, Service, And Portability

### 9.1 Hot reload and Service generations

Selected plans are immutable code-generation content. Existing frames retain
their exact `Arc<LinkedArtifact>`; new roots select the newly published
artifact. A sparse Vela Service patch and its Rust defaults still compose into
one complete generation. Nested `service::base` and `service::pinned` calls do
not cross generations because scalar selection adds no dispatch authority.

Reload ABI comparison continues to use semantic function/schema/service
contracts. A different physical selection for semantically compatible code is
not itself an ABI rejection, but it changes the artifact checksum and exact
executable generation.

### 9.2 Async and scoped tasks

`await`, provider/native future entry, task admission, and continuation resume
are hard region boundaries. A selected unit never suspends, so ordinary frame
IP remains externally stable between units. Async roots and detached workers
execute selected synchronous regions through their own existing Runtime and
retain the exact artifact selected at admission.

Cancellation and deadline checks that currently occur at semantic budget,
call, await, or host safe points remain at those points. A scalar loop may not
hide a required cancellation/deadline check; Batch A inventories the current
check schedule and the selector models every such boundary before loop regions
are enabled.

### 9.3 Portable artifact version 4

The first serialized selected-plan representation is one atomic hard switch:

```text
portable Vela program format: 3 -> 4
portable Service artifact:     3 -> 4
Service deployment bundle:     3 -> 4
```

Version 4 encodes deterministic unlinked scalar/super/loop plans, compact
operands, exits, source points, profile layout, physical coverage manifest,
required feature bits, and all ordinary instruction data required for ineligible
regions. Decoding applies size/count/depth limits before allocation and rejects
invalid plan handles, registers, constants, targets, exits, source points,
coverage, budgets, and feature combinations before linking or activation.

Versions 1-3 reject immediately. There is no compatibility reader, plan
inference, load-time MIR reconstruction, or fallback that silently expands a
version 4 plan into old instructions. `PortableProgramArtifact::from_linked`
round-trips the canonical plan rather than discarding it. Ordinary and Service
artifact checksums include plan content and metadata.

## 10. Unsafe Policy

Safe Rust is sufficient for the initial selector, verifier, plans, and compact
executor. No batch is authorized to add `unsafe` merely for unchecked register
access, enum payload reinterpretation, computed-goto emulation, or lifetime
convenience.

If profiling later proves a specific safe abstraction is the dominant
remaining cost, an unsafe candidate requires all of:

1. a separate documented invariant and smallest possible private module;
2. a safe construction boundary reachable only after linked verification;
3. a `SAFETY:` proof on every block;
4. malformed artifact, unwind, trap, and hot-reload lifetime tests;
5. Miri/sanitizer coverage where supported;
6. repository unsafe-boundary audit registration; and
7. fresh-build interleaved evidence that it beats the safe implementation by
   enough to justify permanent audit cost.

The measured unchecked-register regression controls until a new representation
changes that premise.

## 11. Execution Batches

### Batch A — Freeze baselines, profiles, and semantic boundaries

Deliverables:

- capture stable same-toolchain, same-machine before baselines using the
  repository capture helper and fresh release builds;
- retain current quick Vela/Lua results only as directional context;
- add opcode, MIR-block, CFG-edge, budget-site, safepoint, trap, and source-
  point inventory for the lead workloads;
- profile `scalar_branch_loop`, `range_iteration`, `function_calls`,
  `recursive_countdown`, and `float_math_loop`;
- inventory the exact cancellation/deadline/safe-point schedule relevant to a
  loop that stays inside one selected unit;
- define stable benchmark checksums and a scalar-suite geometric-mean report;
- record instruction count, outer dispatch count, code bytes, artifact bytes,
  compile time, peak compile RSS, and Runtime/Actor memory; and
- record candidate sequence frequencies without adding selection code.

Frozen lead and guardrail rows include at least:

```text
external_compare:
  scalar_branch_loop, range_iteration, function_calls, recursive_countdown,
  float_math_loop, array_scan, map_string_index_lookup_update,
  object_field_methods, string_methods

baseline:
  scalar/range dispatch, script calls, direct closures, GC pacing,
  managed heap, host boundary aggregate/detail

engine:
  interop, service_boundary_baseline, async_execution,
  scoped_task_execution, actor_memory, actor_concurrency
```

Checkpoint:

```text
every lead workload has a reproducible MIR/dispatch inventory and checksum
profiles identify large dispatch/value plumbing rather than assumed helpers
before results and toolchain metadata are retained outside current docs
candidate recipes are chosen from measured frequency
no runtime or artifact behavior has changed
```

### Batch B — Add selector and coverage verification with ordinary output

Deliverables:

- introduce a focused MIR physical-selection module accepting only
  `MirBackendHandoff`;
- partition every function into deterministic selected units while marking all
  units ordinary;
- implement coverage, boundary, CFG, liveness, source-point, and budget-site
  verification independent of bytecode adjacency;
- expose test-only selection reports with candidate/rejection reasons;
- add malformed coverage fixtures for omissions, duplicates, wrong function,
  invalid exits, moved budget sites, swallowed safepoints, and source mismatch;
- keep generated bytecode byte-for-byte canonical where no selection is
  enabled; and
- split modules before any active file crosses the repository size policy.

Checkpoint:

```text
every verified MIR operation maps once to an ordinary selected unit
selection reads no HIR, source text, runtime values, or emitted adjacency
coverage verifier rejects every malformed fixture before link/execution
ordinary workload bytecode, checksums, and performance remain within noise
```

### Batch C — MIR-native short superinstructions and artifact v4

Deliverables:

- select the first measured short recipe directly from MIR definition/use,
  liveness, facts, and terminator structure;
- prefer `I64CmpImmJumpIfFalse` as the first proof only if Batch A confirms its
  frequency; otherwise choose the highest-frequency equally bounded recipe;
- implement unlinked/linked forms, linker projection, verifier coverage,
  source/trap behavior, cache/profile classification, and disassembly/testing;
- atomically raise ordinary program, Service artifact, and deployment bundle
  formats to version 4 and reject versions 1-3;
- encode the physical coverage/source metadata needed for the first selected
  unit without portable MIR;
- add structural dispatch-count and stable before/after measurements; and
- revert any recipe that misses its focused retention gate.

Retention gate for each recipe:

```text
matching workload eliminates the predicted outer dispatches
focused stable mean improves by at least 5% or contributes indispensably to a
  later accepted block family with separately demonstrated combined evidence
no stable scalar/guardrail row regresses by more than 5%
all artifact v4 rejection and round-trip tests pass
```

Checkpoint:

```text
at least one profile-justified MIR-native superinstruction is retained
no production bytecode peephole or old artifact reader exists
source and portable compilation produce equivalent linked plans
```

### Batch D — Compact scalar basic blocks

Deliverables:

- add compact scalar op, exit, source-point, and plan tables with guarded
  layouts and bounded decoding;
- add `RunScalarBlock` to the one linked interpreter and a focused scalar
  executor module;
- select only eligible three-or-more-op regions and fused exits;
- preserve checked arithmetic, partial writes, error spans, budget entry/exit
  sites, and profiled logical subpoints;
- prove no allocation, call, safepoint, HostAccess, reflection, task, await, or
  state effect enters an accepted block;
- verify plan handles/registers/constants/exits at unlinked, portable, linked,
  and artifact boundaries;
- add test-only ordinary-versus-selected differential fixtures across success,
  overflow, branch, break, continue, and malformed entry values; and
- measure plan lookup, compact-op loop, instruction/code bytes, compile time,
  and artifact size against the frozen baseline.

Retention gate:

```text
selected blocks reduce outer dispatch count by at least 40% on their covered
  straight-line regions
scalar-suite stable geometric mean improves by at least 15% from Batch A
no target-independent guardrail regresses stably by more than 5%
selected execution allocates zero bytes per block entry after warmup
```

Checkpoint:

```text
ordinary and scalar-block units interoperate in one frame driver
every hard boundary exits through the canonical ordinary VM helper
no public selector/runtime toggle or second interpreter exists
```

### Batch E — Single-entry scalar loop regions

Deliverables:

- recognize natural single-entry/single-latch loops from verified CFG rather
  than instruction offsets;
- represent header condition, compact body, continue/latch, break exits,
  source points, and conditional edge charges explicitly;
- execute taken iterations inside one focused loop unit without returning to
  the large dispatch match;
- charge every budgeted backedge before continuing and preserve exact
  exhaustion/partial-progress behavior;
- preserve overflow/trap source location and do not run later operations after
  failure;
- model or split every cancellation/deadline/safe-point boundary found in
  Batch A;
- reject multiple-latch, irreducible, nested, dynamic, allocating, calling,
  host, reflection, task, await, and try-region loops in the initial version;
- add budget-at-iteration-N, overflow-at-iteration-N, break, continue, empty,
  one-element, inclusive/exclusive range, and unbounded-mode tests; and
- measure outer dispatches per iteration, loop entries/iterations, latency,
  throughput, and tails.

Retention gate:

```text
eligible loops eliminate at least 60% of prior outer dispatches per iteration
scalar_branch_loop and range_iteration each improve at least 35% from Batch A
scalar-suite geometric mean improves at least 25% from Batch A
budgeted/unbounded checksums and exact failure iteration match ordinary proof
no stable guardrail regression exceeds 5% without an accepted trade-off
```

Checkpoint:

```text
one selected loop may run many iterations but crosses every semantic budget,
  cancellation, deadline, and trap boundary at the same logical point
ineligible loops remain canonical ordinary instructions
```

### Batch F — Reload, async, Service, profiling, and portability closure

Deliverables:

- prove old/new `ProgramVersion` generations retain independent immutable
  plans across accepted, rejected, staged, activated, and rolled-back reloads;
- prove closures and active frames keep old plans while new calls use new plans;
- prove ready/pending async roots, provider calls, detached workers, and
  safe-point continuations preserve exact plans and resume only between units;
- prove Service Snapshot/Delta/fold/rollback and nested
  `service::base`/`service::pinned` calls remain generation-coherent;
- complete version 4 ordinary/Service/deployment round trips, checksums,
  corruption limits, feature bits, and v1-3 rejection at every public load,
  stage, and activation entry;
- finalize unit/subpoint profiler ownership and output rows;
- prove 1, 100, and 10,000 Runtimes share plan memory through the exact
  generation and do not duplicate mutable plan state;
- add fuzz seeds for plan handles, operand ranges, coverage manifests, exits,
  source points, and payload limits; and
- update architecture and durable decision docs to the accepted physical
  model.

Checkpoint:

```text
reload, async, detached-task, and Service behavior is unchanged except speed
portable load requires no MIR and cannot infer or expand missing plan metadata
plan memory is generation-shared and Runtime-local overhead stays bounded
profile data never crosses or mutates an exact generation
```

### Batch G — Acceptance, cleanup, and release decision

Deliverables:

- remove temporary selectors, unused candidate opcodes, dead plan variants,
  obsolete version 3 readers/fixtures, compatibility flags, and benchmark-only
  production branches;
- audit that no production bytecode adjacency peephole, source/HIR backend
  query, global plan authority, or second interpreter remains;
- rerun stable fresh-build interleaved before/after comparisons with exact
  checksums and noise floors;
- compare embedded Vela and Lua 5.4 on equivalent workloads and report ratios
  without mixing process-backed rows;
- run the acceptance matrix and complete repository validation;
- update `docs/progress.md`, `docs/performance.md`, `docs/decisions.md`, and
  relevant architecture docs with only durable conclusions;
- archive detailed baseline, candidate, rejected-family, and final reports; and
- commit the final accepted documentation checkpoint.

Checkpoint:

```text
all semantic, artifact, reload, async, Service, memory, and performance gates pass
the stable scalar-suite geometric mean improves at least 25%
scalar_branch_loop and range_iteration each improve at least 35%
no unexplained stable guardrail regression above 5% remains
ordinary fallback covers every ineligible operation through the one VM
```

## 12. Test And Acceptance Matrix

### Selection and verification

- deterministic selection for identical verified MIR;
- exact one-to-one statement, terminator, edge, budget, and source coverage;
- missing, duplicate, reordered, cross-function, and invalid-exit coverage
  rejects before execution;
- guards, joins, liveness, try regions, and safepoints split as declared;
- unknown/dynamic facts fall back to ordinary instructions;
- no selector reads HIR/source or emitted instruction adjacency.

### Scalar semantics

- add/sub/mul/rem success and exact checked failure behavior;
- immediate and register forms agree;
- true/false branches, fallthrough, break, continue, return staging, empty and
  one-iteration loops agree;
- writes before a trap remain, later writes do not occur;
- source span and stack frame identify the same logical operation;
- malformed typed entry produces a structured type error, not UB or panic.

### Budgets and limits

- charge at entry, internal split, conditional edge, and loop backedge maps to
  the exact `MirBudgetSite`;
- exhaustion at iteration 1, N, and final edge matches ordinary execution;
- unbounded execution contains no active per-operation budget branch;
- memory, collection, call-depth, host-call, deadline, and task quotas remain
  unchanged on ordinary boundaries;
- cancellation/deadline observation is not hidden inside a selected loop.

### GC and resources

- selected units contain no allocation or safepoint;
- live heap refs in untouched registers survive entry/exit and later GC;
- selected execution leaks no roots, frames, register buffers, plan refs, or
  generation owners;
- block/loop entry allocates nothing after warmup;
- host slots, leases, and borrowed views cannot be created, released, or
  escaped inside initial selected units.

### Reload, async, tasks, and Service

- old active frame/closure and new root select their respective plans;
- rejected/stale reload changes neither plan nor authority;
- pending async frame resumes between units on the exact old artifact;
- detached child and continuation retain the origin artifact and Service
  generation;
- Runtime pool reset clears mutable counters/state without copying plans;
- nested Service calls remain pinned and atomic publication remains unchanged.

### Artifact and tooling

- version 4 source/linked/portable round trips produce equivalent plans and
  checksums;
- versions 1-3 reject at ordinary, Service, and deployment entries;
- malformed size/count/depth, plan handle, register, constant, target, exit,
  source, profile, and coverage data rejects transactionally;
- reflection may report physical counts for diagnostics but cannot invoke or
  mutate plans;
- disassembly/test support renders selected unit and subpoint facts;
- LSP remains source-semantic and does not depend on physical selection;
- fuzz and architecture size/unsafe audits pass.

### Performance and memory

- same commit/toolchain/profile, fresh builds, interleaved runs, exact
  checksums, and recorded noise floors;
- interpreter-only, profile-only, cache-enabled, block, host, Service, async,
  and external-runtime rows remain separated;
- selected-unit count, compact-op count, eliminated dispatches, block/loop
  entries, loop iterations, exits, plan bytes, artifact bytes, allocations,
  compile time, and peak RSS are reported;
- scalar-suite geometric mean improves at least 25%;
- lead scalar/range rows improve at least 35%;
- non-target stable regressions above 5% are rejected or explicitly accepted
  with evidence and a named follow-up;
- 10,000-Actor plan memory remains generation-shared and under the existing
  ceiling.

## 13. Benchmark Protocol

Use the repository measurement-first loop:

```text
capture stable baseline -> profile -> implement one family -> fresh-build
interleaved candidate -> verify checksums/noise -> retain or revert
```

Minimum focused commands include:

```bash
tools/perf/capture_external_compare.py \
  --name verified-mir-scalar-before \
  --baseline verified_mir_scalar_macos_aarch64 \
  -- \
  --runtime vela,lua54 --iterations 500000 --repeats 5 --warmup 2 \
  scalar range function recursive float

tools/perf/profile_external_compare.sh \
  --runtime vela --iterations 500000 --repeats 1 --warmup 1 scalar

cargo bench -p vela_vm --bench baseline -- --quick scalar
cargo bench -p vela_vm --bench baseline -- --quick range
cargo bench -p vela_vm --bench external_compare -- --quick \
  --runtime vela,lua54
cargo bench -p vela_engine --bench interop -- --quick
cargo bench -p vela_engine --bench service_boundary_baseline
cargo bench -p vela_engine --bench async_execution -- --quick
cargo bench -p vela_engine --bench scoped_task_execution
cargo bench -p vela_engine --bench actor_memory -- memory
cargo bench -p vela_engine --bench actor_concurrency
```

If the capture helper accepts only one workload substring per invocation,
capture separate files rather than changing its CLI only for this plan. Stable
acceptance uses the harness's stable iteration shape and multiple fresh process
runs. Randomly seeded map/closure rows require seed control or a measured
process-level noise floor before attribution.

Do not write routine raw output into `docs/performance.md`. Store local data in
`perf-results/`, checked-in regression inputs in `perf-baselines/`, and detailed
accepted/rejected history in one archived acceptance report when needed.

## 14. Validation Commands

Ordinary batch work uses focused tests and `cargo test-fast`. Relevant focused
commands include:

```bash
cargo test -p vela_mir --all-features
cargo test -p vela_bytecode --all-features
cargo test -p vela_vm --all-features
cargo test -p vela_hot_reload --all-features
cargo test -p vela_engine --all-features
cargo test -p vela_vm --test integration external_compare_contract
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo bench -p vela_vm --bench baseline --no-run
cargo bench -p vela_vm --bench external_compare --no-run
cargo bench -p vela_engine --no-run
```

Before each accepted implementation checkpoint, run the relevant subset of
[validation.md](validation.md). Before Batch G completion, run at least:

```bash
cargo fmt --all -- --check
cargo fmt --manifest-path examples/Cargo.toml -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --manifest-path examples/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo test --manifest-path examples/Cargo.toml \
  --all-features --no-fail-fast
cargo doc --workspace --all-features --no-deps
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
node editors/vscode/scripts/validate-package.js
(cd editors/tree-sitter-vela && \
  npx --yes tree-sitter-cli@0.25.10 generate)
git diff --exit-code -- editors/tree-sitter-vela/src
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

Also run repository architecture file-size and unsafe-boundary audits. If a new
unsafe boundary is accepted, run its Miri/sanitizer targets where supported and
record unavailable toolchains explicitly.

## 15. Completion State

This track is complete only when verified MIR selects profitable larger
execution units without introducing a second semantic engine; selected units
preserve exact budgets, traps, roots, source points, reload generations, async
boundaries, and Service behavior; artifact version 4 loads the same plans
without MIR and rejects old formats; stable scalar workloads meet the retention
thresholds; all unrelated guardrails and repository gates pass; and the final
accepted architecture is documented with detailed history archived outside
current status docs.

The result is deliberately not a JIT. It is a generation-pinned register VM
whose physical interpreter can execute either one complex instruction or one
verified scalar region per outer dispatch. M22 may later consume the same
verified MIR and eligibility facts for machine code without changing the
language, HostAccess, hot-reload, async, or Service contracts established here.

## 16. First Task Template

```text
Task: Freeze the verified-MIR structural interpreter baseline.
Context: This is Batch A of the Verified-MIR Superinstruction And Basic-Block
Interpreter Plan. The current VM already has typed i64 instructions; the next
decision requires exact MIR/dispatch and performance evidence before adding a
selector.
Expected behavior:
  - scalar_branch_loop, range_iteration, function_calls, recursive_countdown,
    and float_math_loop report stable MIR blocks, selected candidate sequences,
    outer instruction dispatches, budget sites, safepoints, source points, and
    checksums;
  - stable same-toolchain before captures exist for Vela and embedded Lua 5.4;
  - current cancellation/deadline observation points relevant to a scalar loop
    are inventoried;
  - no compiler, artifact, or runtime semantics change.
Tests:
  - cargo test -p vela_vm --test integration external_compare_contract
  - cargo test -p vela_bytecode --all-features
  - focused tests for any new test-support inventory API
Do not change:
  - do not add selected-unit or superinstruction emission;
  - do not add a production optimization flag;
  - do not alter bytecode/artifact versions;
  - do not update durable performance conclusions from quick runs.
Validation:
  - cargo fmt --all -- --check
  - cargo clippy -p vela_bytecode --all-targets --all-features -- -D warnings
  - cargo clippy -p vela_vm --all-targets --all-features -- -D warnings
```
