# MIR And Executable Generation Architecture Plan

> **Track:** verified MIR semantics, linked executable ownership, hot-reload
> generation lifetime, backend-neutral budgets, and M22 JIT input
> **Document status:** Codex goal-mode execution plan
> **Execution status:** Planned. No phase is complete until its checklist and
> validation gate pass.
> **Execution mode:** throughput-first large batches. Intermediate commits may
> fail to compile or test; only batch-completion checkpoints must be green.
> **Supersedes:** the future-ownership, closure, cache rebasing, profile, budget,
> and JIT-input conclusions in
> [mir-lowering-jit-foundation-plan.md](mir-lowering-jit-foundation-plan.md) and
> [runtime-image-state-refactor-plan.md](runtime-image-state-refactor-plan.md).
> Those documents remain historical records of the completed production MIR
> hard switch and first runtime image/state split.

This is a long-term architecture correction, not a short-term regression patch.
Do not implement temporary block-entry fact clearing, stale-closure rejection,
debug-name cache rebasing, bytecode-to-MIR reconstruction, or a second
compatibility execution path. Each phase must move the repository toward the
single target architecture defined here.

The plan is complete only when verified MIR is a sufficient backend contract,
the linker produces one canonical linked artifact, closures and frames pin the
generation that owns their code, runtime mutable state is separated from
generation-owned layouts, execution budgets are backend-neutral, and the same
owned verified MIR can be consumed by the bytecode backend and future M22 JIT.

---

## 0. Codex Goal

Use this prompt to execute the plan:

```text
/goal Execute docs/mir-executable-generation-architecture-plan.md as the active
long-term correctness and JIT-foundation architecture track. Treat docs/goal.md
as the product roadmap, docs/architecture.md and docs/architecture/*.md as the
technical contract, docs/decisions.md as durable design policy, and
docs/progress.md as rolling status. Start from the first unchecked phase and
continue through the entire active execution batch. Do not stop after the
smallest verifiable task and do not spend time restoring a temporarily green
tree between tightly coupled ownership/type migrations.

Do not apply short-term mitigations such as clearing all physical facts at
basic-block boundaries, disabling specialization, rejecting every closure from
an old reload generation, keeping RuntimeImage name-based cache rebasing, or
reconstructing JIT input from source or linked bytecode. Implement the target
ownership and verification model directly.

Make verified MIR an owned, generation-retainable backend contract. Compute
CFG facts, guard refinements, value/root liveness, unique safepoints, lexical
debug availability, and backend-neutral budget points from MIR program points.
Typed operations require exact proven or guard-refined facts. Backends must not
depend on builder layout, arena order, source HIR traversal, or unverified
peephole conventions.

Make the linker the single authority for flattened executable handles,
generation-global cache-site IDs, ProgramImage indexes, cache/profile layouts,
and linked verification. Produce one LinkedArtifact instead of independently
building and later rebasing ProgramImage and LinkedProgram. Keep mutable cache
entries, profile counters, hotness, heap, globals, and active tier selection in
RuntimeState.

Make ProgramVersion own the same-generation verified MIR and linked artifact.
Closures and active frames pin their immutable linked executable generation;
old closures execute old code, while new entry calls after a safe-point reload
use the new generation. Never migrate a closure implicitly by resolving a
stable function name into new code.

Replace bytecode-instruction-count semantics with one explicitly recorded,
backend-neutral execution-unit schedule at MIR program points. Preserve
HostAccess, reflection, GC, call-depth, memory, source-span, and hot-reload
boundaries. Do not add Cranelift machine-code generation in this track, but
leave ProgramVersion with an owned verified input and immutable layouts that
M22 can consume without rerunning HIR/analysis.

Add the required negative verifier tests, source-level runtime regressions,
reload/cache identity tests, and focused benchmarks as part of their large
execution batch. Intermediate commits are recovery markers and may contain
compile errors, failing tests, incomplete caller migration, or temporarily
unused new types. Keep moving through the batch instead of repairing temporary
states for commit cleanliness. Use Conventional Commit messages, keep unrelated
work out, and make the batch-completion commit green. Do not mark the goal
complete while any acceptance item, zero-hit audit, or ownership invariant
remains unproven.
```

---

## 1. Why This Follow-On Exists

The production Heavy-HIR-to-MIR hard switch succeeded: verified MIR is the only
runtime body-lowering route, the direct HIR bytecode backend is deleted, and the
crate dependency direction is clean. Post-completion review found that the
handoff and runtime-generation architecture are not yet strong enough to
support all current semantics or future M22 JIT safely.

Confirmed current defects and contract gaps:

1. The bytecode backend keeps function-global `shapes` and `immediates` while
   emitting blocks in arena order. Facts from one CFG predecessor can therefore
   specialize another predecessor or join. Different record layouts can emit a
   wrong slot access, and an untaken branch constant can become an `I64*Imm`
   operand.
2. Callable contracts lose positional arity during bytecode guard conversion,
   and the runtime guard rejects a closure where analysis has proven that a
   closure satisfies a Function contract.
3. Runtime cache-site rebasing looks up functions by debug name. Flattened
   lambdas are absent from that name index, so local cache-site IDs can collide
   between lambdas. The manual rewrite list also omits cache-bearing instruction
   families such as native calls.
4. A linked closure stores only a generation-local dense function handle.
   Accepted reload replaces the current linked program while retaining the
   script heap, so an old closure can resolve its handle against unrelated new
   code.
5. MIR verification currently permits Dynamic operands in typed operations and
   does not fully encode backend peephole preconditions. Safepoint IDs can be
   referenced by more than one program point, allowing later liveness data to
   overwrite earlier data.
6. MIR liveness is reused for register allocation, GC planning, and debugger
   projection even though value liveness, root liveness, and lexical variable
   availability have different semantics.
7. Production compilation discards MIR after bytecode emission, while M22 is a
   runtime-hot JIT milestone. ProgramVersion has no same-generation verified
   input from which to compile a function that becomes hot.
8. VM instruction count is the current execution-budget unit. A MIR-consuming
   JIT cannot preserve that observable boundary without duplicating bytecode
   selection and layout.
9. Physical backend failures are collapsed into string errors, and the retained
   unqualified record-pattern error has no source span or diagnostic projection.
10. The completed plan had no formal post-hard-switch performance and retained
    generation-memory exit gate.

Existing tests passing does not close these gaps. The required regressions are
cross-CFG, cross-lambda, cross-generation, and malformed-MIR combinations that
the current test matrix does not exercise.

---

## 2. Scope And Non-Goals

This plan includes:

- an owned verified-MIR bundle retained by ProgramVersion;
- backend-neutral CFG facts and guard refinements;
- strict verifier-to-backend invariants;
- distinct value, GC-root, and debugger-availability analyses;
- a lossless callable contract from analysis through runtime guards;
- one linker-owned `LinkedArtifact` and one cache/profile layout authority;
- immutable executable-generation ownership for closures and frames;
- explicit generation-local versus cross-generation identity rules;
- backend-neutral execution-unit budget semantics;
- structured source-spanned MIR/backend diagnostics;
- performance and retained-generation memory baselines;
- JIT input and ownership readiness without generating machine code.

This plan does not include:

- Cranelift code generation, machine-code allocation, or a JIT worker queue;
- speculative optimization passes or a general deoptimizer;
- automatic migration of closures, frames, iterators, or heap objects between
  ProgramVersion values;
- moving GC, async/coroutine reload, script threads, or shared script heaps;
- script-language generics;
- Rust references exposed to scripts;
- HostAccess, reflection permission, schema ABI, or safe-point bypasses;
- preservation of obsolete internal compiler, linker, runtime-image, or budget
  APIs solely for compatibility.

M22 may later add compiled artifacts and side exits, but it must consume the
identities, layouts, budget points, roots, debug data, and generation ownership
defined here.

---

## 3. Target Pipeline And Ownership

The target compile and execution pipeline is:

```text
source
  -> Heavy HIR + AnalysisFacts + CompileTargets
  -> OwnedVerifiedMirBundle
       CFG + facts + refinements + effects + guards
       value/root/debug analyses + safepoints + budget points
  -> bytecode emission
  -> UnlinkedProgram with local physical operands
  -> Linker
  -> LinkedArtifact
       LinkedProgram + ProgramImage + executable indexes
       generation-global cache/profile layouts
       MIR-function-to-linked-handle mapping
  -> ProgramVersion
       verified MIR + linked artifact + ABI + generation identity
  -> RuntimeImage / RuntimeState
       immutable current generation / mutable per-runtime sidecars
```

The future JIT branch is:

```text
ProgramVersion::verified_mir
  -> restricted-function eligibility
  -> Cranelift lowering
  -> generation-owned immutable compiled artifact
  -> runtime-local tier selection, caches, counters, heap, globals, and budget
```

### 3.1 Ownership Table

| Data | Owner | Lifetime / mutation rule |
|---|---|---|
| HIR and AnalysisFacts | compile generation | immutable compile input |
| Owned verified MIR | ProgramVersion | immutable and retained for the generation |
| CFG facts/refinements | verified MIR bundle | recomputable, verified, backend-neutral |
| Linked bytecode | LinkedArtifact | immutable, generation-owned |
| ProgramImage/indexes | LinkedArtifact | built by the same linker pass as bytecode |
| Cache/profile layouts | LinkedArtifact | immutable, generation-local IDs |
| Inline-cache entries | Runtime generation sidecar | mutable and never shared across runtimes or generations |
| Profile counters/hotness | Runtime generation sidecar | mutable and never mixed across generations |
| Compiled artifacts | ProgramVersion in M22 | immutable after publication, generation-keyed |
| Active tier/entry selection | RuntimeState | mutable runtime policy |
| Heap, roots, globals | RuntimeState | mutable and runtime-local |
| Closure code owner | closure heap value | immutable Arc pin to its creation generation |
| Active frame code owner | call frame/execution stack | immutable pin until frame exit |
| Hot-reload current generation | RuntimeImage/runtime reload state | swapped only at safe points |

Immutable layout and mutable state must not be described by the same field or
type. In particular, ProgramVersion owns `CacheSiteLayout` and `ProfileLayout`,
not mutable cache entries or counters. RuntimeState maintains owner-qualified
generation sidecars so retained old code never indexes the current generation's
mutable arrays.

### 3.2 Identity Rules

Use separate identity classes deliberately:

```text
cross-generation semantic identity:
  FunctionId, MethodId, TypeId, FieldId, VariantId, ShapeId, schema IDs

generation-local executable identity:
  MirFunctionId, ScriptFunctionHandle, CacheSiteId, bytecode offset,
  profile slot, compiled-entry index
```

A dense executable handle is valid only together with its owner generation.
Stable semantic identity may compare ABI or map compatible declarations across
reload, but it must not implicitly migrate an old closure or frame into new
code.

---

## 4. Verified MIR Contract

### 4.1 Owned Bundle

Replace the ephemeral borrowed-only production handoff with an owned sealed
artifact, conceptually:

```rust
pub struct OwnedVerifiedMirProgram {
    program: MirProgram,
    analyses: MirAnalysisBundle,
}

pub struct OwnedVerifiedMirBundle {
    roots: BTreeMap<FunctionId, OwnedVerifiedMirProgram>,
}
```

The exact public/internal names may differ, but the following must hold:

- verification seals the program and its analysis generation together;
- bytecode emission borrows the sealed artifact;
- ProgramVersion retains the same sealed artifact;
- nested functions/lambdas remain discoverable from their owning root;
- stable root identity maps to generation-local MIR functions explicitly;
- no consumer can mutate MIR after verification;
- no consumer can fabricate a backend handoff without verification;
- rerunning HIR/analysis is not required after compilation.

Do not add a second JIT-only IR in this track. M22 may introduce a lower
Cranelift-specific IR inside the JIT backend, but its source of truth is this
verified bundle.

### 4.2 CFG Facts And Refinements

Facts are keyed by logical value and MIR program point, never physical register
or code-emission order. The minimum lattice is:

```text
Unreachable
Unknown
Known(Fact)
Conflict
```

Required facts include only backend-neutral information:

- exact primitive/value type when proven;
- non-allocating scalar immediate and its verified definition provenance;
- stable record/variant type and shape identity;
- callable accepted kinds and positional arity;
- option/result/tuple family and arity where proven;
- missing/default sentinel state where represented internally;
- guard-success refinements.

Join preserves `Known(Fact)` only when every reachable predecessor provides the
same fact. Loop backedges participate in fixed-point convergence. Unreachable
predecessors do not erase facts. Conflict and Unknown never authorize a typed,
slot, shape, arity, or immediate specialization.

Physical slot numbers, registers, constant-pool indexes, cache IDs, and
instruction choices remain bytecode-backend concerns.

### 4.3 Guards And Callable Contracts

Keep two explicit guard families:

```text
contract guard:
  mismatch is a language/runtime contract error

specialization guard:
  mismatch follows an equivalent slow CFG edge
```

Callable contracts must carry accepted runtime kinds and optional positional
arity without reinterpreting direction in each layer:

```rust
struct CallableContract {
    accepted_kinds: CallableKindSet,
    positional_arity: Option<u16>,
}
```

The analysis-owned contract for expected Function accepts direct functions and
closures. Expected Closure accepts closures only. MIR, unlinked guards, linked
guards, verifier, VM, diagnostics, reflection metadata, and hot-reload ABI must
all preserve the same set and arity.

### 4.4 Safepoints, Liveness, And Debug Availability

Maintain three distinct analyses:

```text
value liveness:
  register allocation and dead-value decisions

root liveness:
  values that a precise future compiled-frame root map must report

lexical debug availability:
  parameters, captures, and locals visible at debugger source/step points
```

Every allocating/calling/GC-capable operation owns one unique safepoint program
point. Safepoint IDs are assigned during finalization or verification, cannot be
reused, and map one-to-one to root-live-before data. The verifier recomputes and
compares the map.

Debug availability extends through lexical scope even after the last ordinary
use. Physical backends project it to registers, spills, constants, or explicit
unavailable ranges; they must not silently substitute value liveness.

### 4.5 Backend Independence

After this contract is active, a physical backend must not depend on:

- basic-block arena order;
- a mutable function-global register fact table;
- adjacency of `IsMissing` and a branch;
- single-predecessor layout not proven by the verifier;
- skipped blocks selected by pattern-specific peepholes;
- HIR, AnalysisFacts, syntax, source text, or registry queries;
- a runtime fallback that changes a contract guard into an optimization guard.

Correct MIR operations materialize their semantic results. Backend-local
peepholes may remove or fuse them only after the verified semantics are fixed.

---

## 5. Linked Artifact And Cache/Profile Architecture

### 5.1 One Linker Output

The linker must produce one canonical result, conceptually:

```rust
pub struct LinkedArtifact {
    generation: ExecutableGenerationId,
    program: LinkedProgram,
    image: ProgramImage,
    executable_layout: ExecutableLayout,
    mir_function_handles: MirFunctionHandleMap,
}
```

`ExecutableLayout` owns the single immutable cache/profile/function layout. It
may be embedded in ProgramImage rather than stored as a separate field, but it
must not be independently reconstructed.

The linker performs, in one deterministic pass:

1. flatten every top-level function, method executable, and nested lambda;
2. assign every `ScriptFunctionHandle`;
3. map MIR executable identity to linked handles;
4. allocate generation-global `CacheSiteId` values;
5. allocate immutable profile slots and source/offset metadata;
6. rewrite every cache-bearing linked instruction;
7. build ProgramImage name/stable-ID indexes as views over the same handle set;
8. verify function, closure, cache, profile, guard, and target operands;
9. publish the artifact only after verification succeeds.

RuntimeImage must not rebase linked instructions by debug name. ProgramImage
must not independently flatten another copy of unlinked code.

### 5.2 Cache-Site Identity

Before linking, cache sites use a structural local identity:

```text
local executable identity + local cache-site index
```

After linking, `CacheSiteId` is dense and unique within one LinkedArtifact.
Every descriptor records at least:

```text
CacheSiteId
ScriptFunctionHandle
InstructionOffset
CacheSiteKind
```

Instruction enums must expose cache operands through one exhaustive mechanism,
such as a visitor or `cache_site_mut`, so new cache-bearing families cannot be
omitted from rebasing and verification matches.

The verifier rejects:

- duplicate generation-global site IDs;
- a site outside the artifact layout;
- descriptor/function/offset disagreement;
- descriptor kind versus instruction-family disagreement;
- an instruction cache operand absent from its function layout;
- a cache descriptor with no owning instruction unless explicitly documented.

### 5.3 Profile Ownership

Split immutable layout from mutable counters:

```text
LinkedArtifact / ProgramVersion:
  ProfileLayout for all top-level and nested executables

RuntimeState:
  generation-keyed RuntimeGenerationSidecar values
  each sidecar owns counters, caches, hotness, and tier selection for one layout
```

Accepted reload allocates fresh runtime counters for the new generation. Old
frames and closures keep old executable ownership but must not write into new
generation counters. Retained old code selects or lazily creates the sidecar
matching its owner generation. Sidecars weakly reference that generation and
are pruned at safe points once it has no executable owner. Counters are never
indexed by an unrelated layout.

---

## 6. Executable Generation And Hot Reload

### 6.1 ProgramVersion Ownership

ProgramVersion becomes the generation owner:

```rust
pub struct ProgramVersion {
    id: ProgramVersionId,
    mir: Arc<OwnedVerifiedMirBundle>,
    linked: Arc<LinkedArtifact>,
    abi: HotReloadAbi,
    // M22 may add a generation-owned compiled-artifact store.
}
```

Do not retain parallel independently authoritative function maps, ProgramImage
copies, LinkedProgram copies, or profile layouts. Hot-reload diff/report APIs
must query stable metadata views from the canonical generation artifact.

Compile APIs should return a cohesive compiled-program artifact carrying both
unlinked bytecode and its same-generation verified MIR until linking consumes
it. Update internal callers atomically rather than retaining a bytecode-only
compatibility compile route that discards MIR.

### 6.2 Runtime Ownership

RuntimeImage references the accepted immutable ProgramVersion/LinkedArtifact.
RuntimeState owns:

- script heap and persistent roots;
- host and script globals;
- generation-keyed sidecars containing inline-cache entries, profile counters,
  hotness, and active interpreter/JIT tier selection for the matching immutable
  layout;
- runtime ID and other actor-local execution state.

An accepted safe-point reload swaps the current immutable generation and
activates a fresh runtime-local sidecar for its layouts. Retained old-generation
sidecars remain isolated and available to old frames/closures. Sidecars hold a
weak generation owner and are pruned at safe points after no frame, closure,
retained value, or current RuntimeImage can keep that generation executable.
Reload does not rewrite retained heap closures to point at new code.

### 6.3 Closure And Frame Lifetime

A linked closure stores both owner and dense handle, conceptually:

```rust
struct LinkedClosureCode {
    generation: ExecutableGenerationId,
    owner: Arc<LinkedArtifact>,
    function: ScriptFunctionHandle,
}
```

`LinkedArtifact` is a lower-layer bytecode/linker type already consumable by the
VM, so closure ownership does not introduce a VM-to-hot-reload dependency. It
contains the linked program and immutable layouts needed to resolve nested calls
and select the matching runtime generation sidecar.

Rules:

- closure calls use the closure owner, not RuntimeImage's current program;
- active frames pin the same owner for their entire execution;
- calls made from old code resolve through the old linked program;
- new top-level/event calls after safe-point reload enter the new generation;
- old closure capture layout and parameter ABI are never interpreted by new
  code automatically;
- a host-retained `VelaValue` may intentionally keep an old generation alive;
- generation resources drop after runtimes, frames, closures, retained values,
  and compiled frames release their Arcs.

If closure migration is ever required, it is a separate feature with explicit
capture ABI comparison and migration policy. It is not name/FunctionId lookup.

Define `ExecutableGenerationId` below the hot-reload crate and make
ProgramVersionId carry or map one-to-one to it. Do not use pointer identity as
the diagnostic or verification identity.

---

## 7. Backend-Neutral Execution Budget

Raw emitted-bytecode instruction count is not a sustainable semantic unit once
bytecode and JIT consume the same MIR. Replace it with explicit execution units
at MIR program points.

### 7.1 Budget Model

Define a versioned work-unit contract and explicit charge operation or
equivalent verified metadata:

```rust
struct MirBudgetPoint {
    origin: MirSourceOrigin,
    units: u32,
    class: BudgetClass,
}
```

Required charge boundaries include:

- loop backedges and iterator/range steps;
- script, closure, native, stdlib, dynamic, host, and reflection calls;
- allocations and collection work not already covered by memory/growth limits;
- guards, dynamic dispatch, and bounded scans whose cost is script-controlled;
- explicit host/reflection effect boundaries where trap order is observable.

The schedule is inserted deterministically from MIR semantics and verified.
Bytecode and future JIT consume the same points. Backends may combine charges
only across a region that is pure, non-trapping, non-allocating, and has no
host/reflection/call/debug/safepoint boundary. A low-budget slow path must trap
at the same semantic point.

Memory, collection-growth, and call-depth limits remain distinct counters at
their existing semantic boundaries.

### 7.2 Public Contract Migration

Because the project is pre-release, perform one explicit breaking migration:

```text
instruction limit/count -> execution-unit limit/count
bytecode-layout edge     -> MIR semantic charge edge
```

Update Engine options, runtime reports, diagnostics, examples, benchmark
metadata, C API naming if exposed, architecture docs, and tests atomically.
Do not keep both old and new counters or emulate old bytecode counts in the JIT
foundation.

Record the work-unit table and observable trap ordering in `docs/decisions.md`.
Host writes that occur before a later budget trap remain committed, so tests
must pin charge placement around HostAccess effects.

---

## 8. Diagnostics And Error Ownership

User-source failures require a structured diagnostic with source origin.

Required changes:

- replace stringly `MirBackend(String)` propagation with structured variants;
- preserve the current operation/source origin for register overflow, dynamic
  host argument overflow, missing physical target, and other backend limits;
- map register overflow to its stable compiler diagnostic family rather than a
  debug-formatted internal string;
- move unsupported unqualified record-pattern handling to HIR/analysis or
  semantic-input validation with code, primary span, and repair/candidate data;
- remove the legacy no-span `UnsupportedSyntax("match pattern")` exception;
- attach MirBuild/MirVerify errors to their authoritative origin where they
  cross the compile boundary;
- keep bytecode verification errors distinct from MIR verification and
  physical backend errors.

Internal invariant errors may describe malformed compiler input, but they must
still identify the executable and MIR/source origin whenever available.

---

## 9. Phase Status And Checkpoint Rules

Status notation:

```text
[ ] not started
[~] in progress inside the active batch; compilation/tests may be red
[x] complete and validated
```

Rules:

1. Start from the first incomplete execution batch and continue across all
   phases assigned to that batch.
2. Default commit granularity is one substantial commit per execution batch,
   not one commit per phase, checklist item, module, or passing test group.
3. Intermediate recovery commits are allowed when context, review safety, or
   change volume requires them. They may fail compilation or tests and may
   contain an incomplete caller migration.
4. Do not pause merely because the worktree is red. Restore compilation and
   tests at the batch boundary, not after every internal type/API change.
5. Temporary incomplete states may exist inside a batch, but no selectable
   compatibility backend, runtime mode, or dual semantic contract may survive
   the completed batch.
6. Phase validation commands are diagnostic during a batch. Their full union
   becomes mandatory only at the batch-completion checkpoint.
7. Update `docs/progress.md` at batch start, when the active batch genuinely
   changes, and at batch completion; do not append per-phase implementation
   narration.
8. Update `docs/decisions.md` when a durable rule is activated or revised.
9. Keep implementation/test files below 1200 lines or document a justified
   exception.

### 9.1 Execution Batches

The default execution shape is four large checkpoints:

```text
Batch A: MIR semantic contract
  Phases 0-3
  architecture/baseline + owned verified MIR + CFG facts + callable contracts

Batch B: linked executable generation
  Phases 4-6
  LinkedArtifact + ProgramVersion/runtime sidecars + closure/frame ownership

Batch C: backend-neutral runtime contract
  Phases 7-8
  execution units + structured diagnostics + retained M22 JIT input

Batch D: acceptance close-out
  Phase 9
  behavior matrix + audits + full validation + performance/memory comparison
```

Aim for roughly these four substantial commits. Split a batch only when needed
to preserve recoverability or reviewability; do not split it merely because one
crate or checklist subsection has become green. A red intermediate commit is
acceptable, but the final commit for each batch must pass all validations from
its included phases and leave one coherent production architecture.

---

## 10. Phase 0: Contract And Baseline Closure

Purpose: make the target and regression surface explicit before changing
ownership or execution semantics.

- [x] Update runtime, hot-reload, guard, performance, and testing architecture
  docs with the target ownership table and identity classes.
- [x] Mark obsolete future sections of the completed MIR and runtime-image
  plans as superseded by this plan without deleting their historical record.
- [x] Inventory compile APIs that currently discard MIR and every constructor
  of ProgramVersion, RuntimeImage, ProgramImage, and LinkedProgram.
- [x] Inventory all cache-bearing linked/unlinked instructions and compare them
  with every cache rewrite and verifier match.
- [x] Inventory mutable cache/profile/hotness data and classify layout versus
  runtime state ownership.
- [x] Inventory closure/frame execution paths that accept the current linked
  program separately from closure/frame code ownership.
- [x] Record a pre-change benchmark and memory baseline using the same release
  toolchain and tracked workloads used for final comparison.
- [x] Specify final regression fixtures for all confirmed review findings.

Required fixture definitions:

```text
CFG shape join:
  two reachable record layouts with the same field at different slots

CFG immediate join:
  true branch writes 2, false branch writes 100, loop uses the joined local

callable forwarding:
  dynamic parameter forwards a lambda into map/filter and preserves arity

nested cache sites:
  two lambdas read different globals and execute repeatedly with caches enabled

retained closure reload:
  host retains a closure, accepted reload changes top-level/lambda handle layout,
  old closure still executes old code and new entry calls execute new code

malformed MIR:
  Dynamic typed operand, duplicate safepoint, invalid guard refinement,
  backend-dependent peephole shape

diagnostics:
  register overflow/backend limit and unsupported pattern have stable spans
```

Validation:

```bash
cargo test -p vela_mir
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_hot_reload
cargo test -p vela_engine
cargo bench --workspace --no-run
```

Checkpoint: architecture documents, inventories, fixture designs, and baseline
records agree; no production behavior has two authorities.

---

## 11. Phase 1: Owned Verified MIR And Strict Verification

Purpose: make verification produce an immutable generation-retainable artifact
that is sufficient for any backend.

- [x] Introduce owned verified program/bundle types without exposing mutation
  after verification.
- [x] Preserve stable root FunctionId to generation-local MIR function mappings,
  including nested lambdas, methods, and parameter-default prologues.
- [x] Make bytecode compilation borrow the owned verified artifact.
- [x] Make typed operation verification require exact or explicitly refined
  facts; remove the blanket Dynamic acceptance.
- [x] Represent guard-success refinements in verifier CFG data flow.
- [x] Separate contract guards from specialization guards in the verified form.
- [x] Make safepoint identity one-to-one with a program point and reject reuse.
- [x] Derive and verify root-live-before data per safepoint.
- [x] Split value liveness, root liveness, and lexical debug availability.
- [x] Remove or verify every implicit backend peephole precondition.
- [x] Add negative verifier tests for every rejected malformed form.
- [x] Keep HostAccess/reflection operations explicit and outside ordinary MIR
  places.

Validation:

```bash
cargo test -p vela_mir mir_verifier
cargo test -p vela_mir mir_liveness
cargo test -p vela_mir mir_guards
cargo test -p vela_mir mir_debug_metadata
cargo test -p vela_bytecode mir_backend
```

Checkpoint: no verified MIR relies on builder adjacency, block order, duplicate
safepoints, unproven typed operands, or value-liveness debugger substitution.

---

## 12. Phase 2: CFG Facts And Backend Consumption

Purpose: replace emission-order physical fact mutation with one verified
backend-neutral data-flow result.

- [x] Implement fixed-point CFG facts for locals and temps across branches,
  switches, loops, default prologues, try regions, and unreachable edges.
- [x] Carry constant provenance and only expose eligible non-allocating
  immediates at proven program points.
- [x] Carry stable type/shape/callable/family facts without physical registers
  or bytecode slots.
- [x] Merge facts by reachable-predecessor intersection and prove convergence.
- [x] Make guard-success edges refine facts and slow edges retain the original
  dynamic fact.
- [x] Replace `FunctionBackend` function-global shape/immediate inference with
  lookups into verified facts.
- [x] Select slot and immediate instructions only from the fact at the exact MIR
  use point.
- [x] Materialize semantic Bool/predicate results before backend optimization.
- [x] Remove block-skipping and alias peepholes whose safety is not represented
  by verified MIR/data flow.
- [x] Add CFG join regressions for if, match, default parameters, loop backedges,
  try joins, unreachable predecessors, and nested branches.
- [x] Add wrong-guard and generic-slow-path tests proving Conflict/Unknown facts
  never specialize.

Validation:

```bash
cargo test -p vela_mir mir_dataflow
cargo test -p vela_bytecode mir_backend
cargo test -p vela_vm records_enums
cargo test -p vela_vm runtime_semantics
cargo test -p vela_engine control_flow
```

Checkpoint: the confirmed record-slot failure and silent immediate
miscompilation pass through the production pipeline, and backend emission is
independent of block arena order.

---

## 13. Phase 3: Callable Contract End-To-End

Purpose: make callable kind and arity one lossless contract from analysis to VM.

- [x] Introduce accepted callable-kind sets and optional positional arity in the
  authoritative contract model.
- [x] Update analysis outcomes and tests for Function/Closure directionality.
- [x] Preserve the contract through compile targets, MIR, unlinked guards,
  linker conversion, linked guards, runtime checks, reflection, and ABI.
- [x] Validate direct functions, closures, erased callable values, wrong kinds,
  exact arity, unknown arity, and arity mismatch.
- [x] Ensure runtime diagnostics report the declared parameter/context and the
  actual callable kind/arity.
- [x] Add forwarding tests for dynamic parameters into array/map/set/iterator
  callbacks and nested script calls.
- [x] Remove any StandardTypeGuard conversion that drops callable arity.

Validation:

```bash
cargo test -p vela_analysis callable_contracts
cargo test -p vela_mir callable
cargo test -p vela_bytecode guard
cargo test -p vela_vm type_guards
cargo test -p vela_engine callback
cargo test -p vela_hot_reload function_abi
```

Checkpoint: a lambda forwarded through a dynamic Function-expected parameter is
accepted with the right arity, while wrong kind/arity failures remain stable.

---

## 14. Phase 4: Atomic LinkedArtifact Hard Switch

Purpose: make the linker the only authority for executable, cache, profile, and
ProgramImage layout.

Batch B completion rule: RuntimeImage rebasing, independent ProgramImage
flattening, and the second linked-layout builder must all be gone. Intermediate
commits may be non-compiling while callers move; do not spend work preserving a
green dual production path.

- [x] Define the canonical LinkedArtifact and immutable executable layout.
- [x] Flatten all top-level and nested executables before allocating dense
  handles and cache/profile IDs.
- [x] Build ProgramImage and LinkedProgram from the same handle/index records.
- [x] Allocate cache IDs generation-globally for every function and lambda.
- [x] Replace manual partial cache rewrite lists with one exhaustive mechanism.
- [x] Include native, global, record, method, host, and future cache-bearing
  instruction families in verification.
- [x] Generate profile layout for lambdas as well as top-level functions.
- [x] Add linked verification for cache descriptor kind/function/offset.
- [x] Delete RuntimeImage name-based cache rebasing.
- [x] Delete independent ProgramImage flatten/cache rewriting.
- [x] Update benchmark and test builders to consume LinkedArtifact rather than
  manually pairing separately built images/programs.
- [x] Add deterministic nested-lambda cache collision regressions and reload
  layout tests.

Validation:

```bash
cargo test -p vela_bytecode linker
cargo test -p vela_bytecode program_image
cargo test -p vela_vm linked
cargo test -p vela_engine inline_cache
cargo test -p vela_engine runtime_bytecode_profile
cargo test -p vela_hot_reload runtime_reports
```

Checkpoint: one linker output owns all executable layouts; two lambdas using
local site zero receive distinct linked IDs and cannot read each other's cache.

---

## 15. Phase 5: ProgramVersion And Runtime Ownership Hard Switch

Purpose: retain same-generation MIR and make layout/state ownership unambiguous.

- [x] Introduce a cohesive compiled-program artifact carrying unlinked bytecode
  and its owned verified MIR bundle.
- [x] Route every source/module/registry/engine/hot-reload compile API through
  that artifact without a bytecode-only MIR-dropping path.
- [x] Make ProgramVersion own `Arc<OwnedVerifiedMirBundle>` and
  `Arc<LinkedArtifact>`.
- [x] Remove independently authoritative ProgramVersion function/image/profile
  copies and transitional reconstruction paths.
- [x] Split immutable CacheLayout/ProfileLayout from mutable runtime entries and
  counters in types and documentation.
- [x] Make RuntimeImage reference the accepted generation rather than clone
  linked/image data.
- [x] Store cache/profile/hotness/tier state in generation-keyed RuntimeState
  sidecars, activate a fresh sidecar atomically on reload, and prune dead weakly
  owned sidecars at safe points.
- [x] Keep multiple runtimes over a shared immutable generation isolated in
  heap, globals, caches, counters, and hotness.
- [x] Add ProgramVersion/MIR memory-size and shared-runtime ownership tests.
- [x] Add accepted and rejected reload tests proving counters and caches never
  mix generations.

Validation:

```bash
cargo test -p vela_bytecode compiler
cargo test -p vela_hot_reload
cargo test -p vela_engine shared_runtime
cargo test -p vela_engine source_reload
cargo test -p vela_engine runtime_bytecode_profile
```

Checkpoint: a ProgramVersion is a complete immutable executable generation with
same-generation verified MIR, while all actor-local mutable state remains in
RuntimeState.

---

## 16. Phase 6: Closure And Frame Generation Lifetime

Purpose: make all callable executable handles owner-qualified.

- [x] Add lower-layer ExecutableGenerationId and map ProgramVersionId one-to-one
  to it.
- [x] Store immutable linked owner plus dense handle in linked closure code.
- [x] Make active frames pin the linked owner used at call entry.
- [x] Resolve closure calls and nested calls against their pinned owner.
- [x] Make profiler/cache/tier lookup owner-qualified and lazily create the
  matching runtime generation sidecar when retained old code executes.
- [x] Preserve protected heap roots and call-site/source diagnostics while
  changing code ownership.
- [x] Test host-retained VelaValue closures across accepted reloads that add,
  remove, or reorder private helpers and lambdas.
- [x] Test old closure -> old nested call, old closure -> host/native call, and
  new entry -> new code in the same runtime after reload.
- [x] Test that rejected reload leaves current and retained closure ownership
  unchanged.
- [x] Test old generation release with Weak/ownership probes after frames,
  closures, retained values, and generation sidecars are dropped/pruned.
- [x] Document that retained closures intentionally retain old executable
  generations and are not automatically migrated.

Validation:

```bash
cargo test -p vela_vm closure
cargo test -p vela_hot_reload old_version
cargo test -p vela_engine source_reload
cargo test -p vela_engine hot_reload
cargo test -p vela_engine args
```

Checkpoint: no dense function handle is resolved without its owner generation,
and old closures cannot misdispatch into a new LinkedProgram.

---

## 17. Phase 7: Backend-Neutral Budget Hard Switch

Purpose: make termination and charge ordering identical for interpreter and
future JIT without bytecode-layout coupling.

Batch C completion rule: the public/runtime counter rename, MIR schedule,
bytecode execution, diagnostics, examples, and tests move together. Intermediate
commits may be red, but the completed batch must not keep old instruction-count
and new execution-unit modes selectable in production.

- [x] Record the execution-unit table and trap-order contract in decisions and
  runtime architecture docs.
- [x] Add explicit MIR budget points and verifier coverage.
- [x] Insert deterministic charges at loops, calls, dynamic work, allocation,
  HostAccess, reflection, and other specified boundaries.
- [x] Lower the schedule to bytecode without deriving units from emitted
  instruction count.
- [x] Rename public instruction-limit/counter terminology to execution units
  across Rust APIs, C API if present, diagnostics, examples, reports, and docs.
- [x] Preserve separate memory, collection, and call-depth counters.
- [x] Remove per-dispatch implicit bytecode instruction charging.
- [x] Add exact threshold tests for loops, calls, guards, try, allocation,
  callbacks, host writes, reflection, and nested old-generation closures.
- [x] Prove successful HostAccess writes before a later charge trap remain
  committed and later effects do not execute.
- [x] Add backend conformance helpers that future JIT tests can reuse.

Validation:

```bash
cargo test -p vela_mir budget
cargo test -p vela_bytecode budget
cargo test -p vela_vm runtime_semantics
cargo test -p vela_host write_through
cargo test -p vela_reflect budget
cargo test -p vela_engine budget
cargo test -p vela_c_api
```

Checkpoint: budget observability is defined by verified MIR program points, not
bytecode layout, and all runtime boundaries consume the same execution units.

---

## 18. Phase 8: Structured Diagnostics And JIT Input Closure

Purpose: close remaining user-facing and M22-input contract gaps.

- [x] Replace stringly backend errors with structured variants carrying source
  origin and executable identity.
- [x] Restore stable register-overflow and physical-limit diagnostics.
- [x] Move unsupported record-pattern validation to its semantic owner with a
  source-spanned diagnostic and delete the legacy no-span exception.
- [x] Ensure MirBuild/MirVerify/backend/bytecode-verification errors remain
  distinct through CLI and Engine rendering.
- [x] Expose read-only ProgramVersion access to verified MIR for an internal
  future JIT backend without exposing mutable/public script APIs.
- [x] Define restricted-function JIT eligibility queries over verified MIR,
  effects, safepoints, budget points, and linked identity without compiling
  machine code.
- [x] Verify that eligibility never reruns HIR/analysis or queries current
  registry state from a different generation.
- [x] Document future compiled-artifact publication, generation invalidation,
  runtime-local tier selection, GC-root reporting, and debugger side-exit
  requirements.
- [x] Do not add Cranelift, machine code, JIT runtime options, or placeholder
  compiled artifact stores in this phase.

Validation:

```bash
cargo test -p vela_mir
cargo test -p vela_bytecode diagnostic
cargo test -p vela_cli diagnostic
cargo test -p vela_engine diagnostic
cargo test -p vela_hot_reload
```

Checkpoint: every user-source failure in scope has a structured span, and M22
can obtain complete same-generation verified input without changing ownership.

---

## 19. Phase 9: Final Acceptance And Performance Gate

Purpose: prove the new architecture across semantics, ownership, performance,
and documentation.

### 19.1 Required Behavior Matrix

- [ ] CFG: if/match/default/try/loop joins with agreeing and conflicting facts.
- [ ] Typed ops: exact facts, guard refinement, generic slow path, malformed MIR
  rejection.
- [ ] Callables: Function/Closure direction, exact/unknown/wrong arity, dynamic
  forwarding.
- [ ] Caches: every family hit/miss/wrong guard/fallback, nested lambdas,
  accepted reload, schema epoch, multi-runtime isolation.
- [ ] Profiles: top-level and lambda slots, generation reset, old-code policy,
  multi-runtime isolation.
- [ ] Hot reload: old frames, old retained closures, new calls, rejected updates,
  generation release.
- [ ] Budgets: exact semantic charge edges around loops, calls, allocation,
  HostAccess, reflection, try, callbacks, and retained closures.
- [ ] GC/debug: unique safepoints, root-live-before, lexical locals after last
  use, captures, parameter defaults, old-generation frames.
- [ ] Diagnostics: structured MIR/backend errors, spans, call stacks, repair
  information, unsupported patterns, physical limits.
- [ ] Host/reflection: no Rust reference exposure, all mutation through
  HostAccess, permissions and stale-reference behavior unchanged.

### 19.2 Performance And Memory Gate

Run pre/post measurements using the same release toolchain, machine, workload
parameters, and checksum validation. Record durable conclusions in
`docs/performance.md`, not raw logs in `docs/progress.md`.

Required comparisons:

- compile time and peak memory with retained verified MIR;
- ProgramVersion memory for top-level-heavy and lambda-heavy programs;
- one shared ProgramVersion across multiple Runtime values;
- scalar branch/loop interpreter throughput;
- function and closure call throughput;
- record slot/dynamic-field throughput;
- cache-enabled global/native/method/record workloads;
- hot-reload compile, link, safe-point swap, and retained-generation lifetime;
- budget-enabled and unbounded execution paths.

Investigate any repeatable regression over 5%. A regression over 10% in a
representative workload requires an explicit accepted decision and named
follow-up before completion. Correctness and ownership must not be weakened to
meet the threshold.

### 19.3 Final Validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
cargo bench --workspace --no-run
```

Run the tracked focused benchmark commands selected in Phase 0 and record the
final same-toolchain comparison.

### 19.4 Final Audits

The exact searches may be refined as names land, but completion must prove zero
production hits for these architectural remnants:

```bash
rg -n "rebase_linked_cache_sites|function_by_name\(&function_name\)" crates/vela_engine crates/vela_bytecode
rg -n "ClosureCode::Linked\(ScriptFunctionHandle|Linked\(ScriptFunctionHandle" crates/vela_vm
rg -n "MirBackend\(String|format!\(\"\{error:\?\}\"" crates/vela_bytecode crates/vela_cli
rg -n "actual == MirValueType::Dynamic" crates/vela_mir
rg -n "UnsupportedSyntax\(\"match pattern\"" crates
rg -n "instruction_limit|instructions_executed|charge_instruction" crates examples
```

Also audit:

- no physical register/slot/cache type appears in `vela_mir`;
- no HIR/syntax/analysis query appears in the physical backend;
- every cache-bearing instruction participates in one exhaustive verifier path;
- no ProgramVersion/RuntimeImage constructor independently rebuilds linked
  layouts;
- no mutable cache/profile counter is stored in a shared generation artifact;
- no old dense executable handle is resolved against the current generation
  without its owner;
- all active implementation and test files satisfy the 1200-line rule.

### 19.5 Completion Criteria

- [ ] Owned verified MIR is retained by every production ProgramVersion.
- [ ] Bytecode and future JIT share the same verified facts, guards, safepoints,
  debug availability, and budget schedule.
- [ ] The bytecode backend has no emission-order semantic fact inference.
- [ ] Callable kind and arity are lossless end-to-end.
- [ ] Linker output is the only executable/cache/profile layout authority.
- [ ] Cache and profile mutable state are runtime-local and generation-correct.
- [ ] Closures and frames pin their creation/entry generation.
- [ ] Old closures execute old code; new calls after reload execute new code.
- [ ] Execution budget semantics are backend-neutral and documented.
- [ ] MIR/backend user-source failures are structured and source-spanned.
- [ ] M22 can consume same-generation verified MIR without rerunning analysis.
- [ ] Full validation, audits, examples, benchmarks, and memory gates pass.
- [ ] `docs/progress.md` marks this track complete only after every item above.
