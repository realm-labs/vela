# Runtime, Bytecode, And GC

Vela executes synchronously through one linked-bytecode interpreter. This is
an active post-D contract; [architecture.md](../architecture.md) owns the
cross-subsystem boundary. A–E removal implementation and software gates are
complete; see [current progress](../progress.md) and the
[acceptance report](../archive/runtime-simplification-acceptance.md).
Strict/native Input/Render coverage remains incomplete.

## Executable Generation Ownership

`ProgramVersion` owns an immutable linked artifact and its verified MIR.
Generation-owned data includes code, indexes, dense executable handles, source
maps, root/budget analysis, cache/profile layouts, and physical interpreter
plans. Stable semantic IDs compare declarations across generations; dense
handles, instruction offsets, MIR IDs, cache sites and plan offsets are valid
only with their exact owner.

A frame pins the generation selected at entry. A closure keeps its creation
owner, including nested script/native dispatch and state slots. New public
entry calls use the active generation. Reload must never rebind old closures
by name or apply a new generation's plan to an old handle.

Runtime-local mutable data owns the script heap, roots, VM `state`, retained
values, and active execution. Independent Runtimes can share one artifact
without sharing script-visible mutable state. Per-call limits, counters,
capabilities, cancellation and tracing belong to the execution session.

## Struct, Record, And Enum Memory Model

Script records have stable type/shape identity and field slots; enums have
stable type/variant identity and payload fields. Known field access lowers to
linked slots with validated shape assumptions; dynamic access uses script
metadata. Pattern matching checks variants and binds payloads. These are
script-owned aggregates, not transparent Rust objects.

See [language semantics](language.md) for equality and iteration and
[primitive contracts](primitives-type-hints-and-guards.md) for exact tags,
container hints and guards. Mutable heap values use runtime-qualified identity
for map/set keys; semantic comparison traits are a separate operation.

## VM And Bytecode

The compiler emits register-based unlinked bytecode. The verifier and linker
seal it into a `LinkedArtifact` before execution. Unlinked bytecode is an
intermediate/fixture format, never an alternate runtime path.

Values include exact scalar tags and references to script heap objects such as
strings, bytes, arrays, maps, sets, records, enums, closures and iterators. There
is no Rust object/borrow/path value variant. Detached `OwnedValue` conversion
is an explicit boundary, not a host write-through operation.

Known method receivers use stable method IDs and linked dispatch handles.
Unknown receivers with a source-static method name use guarded dynamic
standard-value/script-method resolution. Missing dynamic methods produce
source-spanned runtime errors; see [dynamic dispatch](dynamic_method_dispatch.md).

### Execution Session And Frames

One frame driver owns synchronous calls, returns, callback frames, nested calls,
roots and error unwinding. Existing synchronous native re-entry uses that
session, including its pinned artifact, heap, budgets and capabilities; this
pass adds no new re-entry model. There are no suspended frames, native futures,
script background roots or continuation queues.

### Physical Plans And Execution Metadata

Verified-MIR-selected superinstructions, scalar blocks and scalar range-loop
plans are immutable linked code. Every selected unit retains explicit coverage,
source points, exits and logical execution-unit charges. Ineligible operations
use ordinary instructions. The scalar executor returns to a verified ordinary
instruction boundary; it is not a second VM.

Scalar units preserve entry tag checks, checked arithmetic, partial writes,
trap ordering, backedge charges and root visibility. Heap work, native calls,
reflection and GC safepoints remain ordinary interpreter boundaries. Plan
selection is an interpreter optimization, not preparation for machine code.

Statically resolvable facts belong in the artifact. Generation-stable mutable
cache facts use synchronized generation-qualified execution data. Profiling is
opt-in; a default Runtime must not allocate a counter array proportional to
all instructions/cache sites. An execution-lane sidecar needs measurement and
explicit host ownership, not an assumed OS-thread identity. Accepted reloads
publish fresh execution data; old owners retain their old caches/counters/plans.

### Portable Formats

Portable program format is **8**, Rust-binding schema is **7**, and tooling
schema is **4**. Readers reject incompatible versions before payload use,
linking or activation. Physical-plan coverage and operands are verified even
when a payload checksum is valid. Do not reconstruct MIR, synthesize removed
facts or add a legacy interpreter to load old artifacts.

### Execution Budget

Verified MIR records logical execution-unit points for backedges, iterator
steps, calls, allocations and dynamic work. Bytecode carries these points as
immutable metadata; charging is not simply one unit per physical dispatch.
Runtime-sized callback, collection guard and iterator work uses the same
counter. A failing charge traps before its associated effect; effects already
completed before a later trap are not rolled back.

Execution limits cover units, memory, call depth and collection growth.
Allocation and in-place growth must be charged when limits are finite. Native
calls retain declared cost/effect accounting. Cancellation and deadlines are
host controls on synchronous execution, not script scheduling.

Explicit unbounded host options still exist; production embedders should select
finite limits. Removing obsolete machinery must not introduce a new unbudgeted
loop or bypass a budget on an optimized path.

## Threading Model

One mutable Runtime executes at most one call tree at a time. Rust hosts own
scheduling and may run independent Runtimes on different workers. Exclusive
ownership can move between workers; cache correctness must not depend on an
OS-thread-local owner. Scripts have no threads, locks, atomics, async/await,
tasks or shared-memory concurrency.

`VelaValue` is a retained script value tied to its creating Runtime, not a
Rust-object handle or a cross-Runtime sharing mechanism. Cross-Runtime data
exchange requires explicit owned conversion. Host work may finish externally
and supply owned inputs to a later synchronous call.

## GC

The current collector is non-moving mark-sweep over script objects with explicit
roots and bounded collection controls. Roots include frames, closures, retained
runtime values and state needed by live executable generations. Root refresh
and graph traversal must remain correct between collection steps and mutations.

Old-only state/code may be reclaimed only when no live owner can use it. A
removed closure-valued state root must not keep its own generation alive forever.
GC and generation liveness therefore share an ownership contract; see
[hot reload](hot-reload.md).

Rust host state, files, connections and application storage are outside script
GC. Neither moving GC nor structural state migration is supported. A production
incremental-collector redesign requires separate scoped work; historical plans
do not establish that it is implemented.
