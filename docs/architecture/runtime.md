## Executable Generation Ownership

Verified MIR is an immutable generation-owned backend contract. A
`ProgramVersion` retains the exact owned verification seal used by bytecode
emission; future JIT work consumes that seal without rebuilding HIR, analysis,
or MIR from bytecode. The seal contains program-point facts, guard-success
refinements, value liveness, root-live-before safepoints, lexical debug
availability, and the backend-neutral execution schedule.

Generation-owned immutable data consists of verified MIR, linked code,
`ProgramImage` indexes, executable handles, cache/profile layouts, source maps,
and future compiled artifacts. Actor-local mutable data consists of the script
heap, roots, persistent Vela `state`, extern-state bindings, active/suspended
execution state, HostRef leases, and the actor's adopted generation. Per-call
budget counters, capabilities, cancellation, and tracing belong to the active
`ExecutionSession`.

Cache entries, profile counters, hotness, and tier selection are execution
metadata rather than actor semantics. Their ownership is selected by what each
entry depends on:

```text
statically resolvable fact       -> immutable linked artifact
generation-stable shared fact   -> generation-shared synchronized slot
measured polymorphic hot site    -> optional execution-lane sidecar
actor-identity-dependent fact   -> actor-local sparse or lazy state
```

Every mutable execution-metadata slot is qualified by its executable
generation. A default actor Runtime must not eagerly allocate arrays sized to
all cache sites or every bytecode instruction merely because it references a
large shared program. Full instruction profiling is opt-in and is aggregated
per generation or execution lane unless a concrete diagnostic explicitly
requires actor-local counters.

The linker emits one `LinkedArtifact`. Its canonical flattened records allocate
`ProgramImage` indexes, linked function handles, generation-global cache IDs,
and profile layout together. `RuntimeImage` references that artifact; it does
not rebuild or rebase cache operands by function name. Multiple actor runtimes
share the immutable artifact while retaining isolated heap, roots, Vela state,
extern bindings, leases, and active executions. Sharing an artifact does not
imply sharing script-visible mutable state.

Unlinked bytecode is a compiler, verifier, linker, and test-fixture format. It
is never interpreted. Every runtime entry links first, then executes through
the single linked instruction loop; frames and closures retain an
`Arc<LinkedArtifact>` so nested calls and retained closures cannot resolve
generation-local handles against a different program version.

Stable semantic IDs (`FunctionId`, `MethodId`, `TypeId`, `FieldId`,
`VariantId`, and schema/shape identities) may be compared across generations.
Dense executable handles, MIR IDs, cache sites, profile slots, bytecode
offsets, and compiled-entry indexes are generation-local and are valid only
with their immutable owner.

## Struct, Record, And Enum Memory Model

### Record

Script structs are dynamic values with stable shapes:

```rust
struct Position {
    x
    y
}
```

Runtime:

```rust
ObjRecord {
    shape_id: ShapeId,
    fields: Vec<Value>,
}
```

Field access:

```rust
pos.x
```

Compiles to:

```text
GET_FIELD_CONST r_dst, r_obj, shape=Position, field=x, slot=0
```

If the shape matches, the VM reads the slot directly. Otherwise it falls back to the slow path.

### Enum

```rust
enum Damage {
    Physical { amount }
    Magical { amount, element }
    True { amount }
}
```

Runtime:

```rust
ObjEnum {
    enum_id: TypeKey,
    variant_id: VariantId,
    fields: Vec<Value>,
}
```

`match` compiles into tag checks and field bindings.

## VM And Bytecode

Use register-based bytecode:

```text
LOAD_CONST      r0, const#10
GET_HOST_FIELD  r1, account, FieldId(balance)
ADD             r2, r1, r0
SET_HOST_FIELD  account, FieldId(balance), r2
RETURN          ()
```

Benefits:

```text
fewer instructions
local optimization is easier
field and method access can be specialized
good fit for later inline caches
```

Method calls have two linked bytecode shapes. Statically known receivers keep
the resolved `CallMethodId`/`MethodDispatchHandle` fast path. Unknown receivers
with a source-static method name link as `CallDynamicMethod`, then resolve at
runtime through guarded standard-value, script-method, or host-method targets.
Dynamic method failures are runtime errors with the original call span, not
link-time rejection of ordinary source code.

### Value Layout

Runtime execution uses four explicit value layers:

```text
Value       VM runtime slot; Copy; scalars or handles only
OwnedValue  heap-detached Rust boundary/materialized value
HeapValue   non-moving script heap object referenced by GcRef
HostValue   host-adapter boundary value copied across ScriptStateAdapter
```

The engine embedding layer also exposes `VelaValue`, a runtime-managed handle
to a `Value` pinned in a specific `Runtime`'s persistent heap roots. Hosts use
it when a script return value should be passed back to later script calls
without materializing an `OwnedValue`. `VelaValue` cannot cross runtime
instances; Rust must explicitly materialize through `value_to_owned` when it
needs a heap-detached copy.

The runtime slot stays compact and is guarded by tests to remain at or below
32 bytes on 64-bit targets:

```rust
pub enum ScalarValue {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
}

pub enum Value {
    Missing,
    Unit,
    Bool(bool),
    Scalar(ScalarValue),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
}
```

`OwnedValue`, `HostValue`, and bytecode constants use the same `ScalarValue`
model at their boundaries. All non-scalar script objects live in `HeapValue`:
`Value::Missing` is not a language value and must not cross public boundaries;
it is reserved for VM and bytecode call/default plumbing such as omitted
arguments before default substitution. Owned values, host values, C ABI values,
serde conversion, playground JSON, reflection data, and user-visible results use
`()`, `Option`, `Result`, or a structured value instead.

```rust
pub enum HeapValue {
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Map(ScriptMap),
    Set(ScriptSet),
    Record { type_name: String, fields: ScriptFields<Value> },
    Enum { enum_name: String, variant: String, fields: ScriptFields<Value> },
    Closure(ClosureValue),
    Iterator(IteratorState),
    PathProxy(PathProxy),
}
```

`ScriptMap` and `ScriptSet` preserve original runtime key/element `Value`s,
but lookup, uniqueness, and removal are driven by `ValueKey`. Immutable leaf
keys compare by value, script heap objects and host refs compare by identity,
and transient non-data values are rejected before mutation. Map and set keying
does not call user comparison traits or any script-visible hash hook.

`HeapValue::Iterator` stores one-shot cursor state. Iterator state may point at
script heap sources such as arrays, sets, maps, and strings by `GcRef`, may own
copied `Value` snapshots from safe boundaries, and may wrap lazy adapter state
such as `map`, `filter`, `take`, and `skip`. It must trace script heap
references that it stores or protects, including callback closures and source
containers, but it must not trace Rust host state as script-owned memory.

Collection-backed iterator sources read current heap slots lazily while keeping
the source identity and initial traversal extent. Map views snapshot key order
for traversal and read values from the source map when each item is produced.
Iterator adapters are also one-shot: creating an adapter takes the source
iterator state, and terminal methods consume it.

Iterator object allocation charges heap memory like other script heap values.
Stepping an iterator and invoking callbacks still runs through normal VM budget
and call-depth checks. Final collection materialization, such as
`collect_array()`, charges output heap allocation and collection growth at the
allocation or mutation boundary; lazy adapters do not charge collection growth
for intermediate values they do not materialize.

Only consider the following after profiling proves `Value` overhead is too high:

```text
16-byte tagged value
NaN boxing
pointer tagging
specialized arrays
```

### Execution Session And Frames

Every Runtime call constructs one execution-owned host boundary and drives one
`ExecutionSession`. The session owns the explicit linked VM frame stack, return
continuations, pending comparison/callback/iterator operations, execution
budget, heap/GC context, and generation roots for the outer call. Function,
closure, script-method, provider, guard, comparison, collection-callback, and
iterator calls push frames and resume their parent operation after return.

VM ownership follows those semantic boundaries. `execution_session.rs` owns
session/frame/continuation definitions and start policy, `async_resume.rs` owns
prepared async boundaries and result resumption, and `execution_reentry.rs`
owns child push/abort policy. `linked_execution.rs` retains the single frame
driver, exhaustive linked-opcode dispatch, frame preparation, and root glue; it
does not duplicate session, resume, or reentry policy.

Production linked execution does not recursively invoke the interpreter. The
remaining `execute_linked_call` name is a non-recursive root driver shim over
the session loop. Call depth remains a logical frame budget rather than a Rust
stack limit, so deeply nested script and callback calls fail only at the
configured language budget.

Await is preserved as explicit verified MIR and linked resume control flow.
Known async calls require await, sync entry calls reject declared async entries,
and non-awaited dynamic or reflected async targets trap before invocation.
`call` and `call_async` enter this same session driver. `call_async` returns a
scoped `Send` future that may borrow Runtime, call arguments, and a fallback
adapter for the invocation lifetime; it does not require Runtime ownership or
`'static` inputs. The driver returns `Pending` immediately when a registered
Rust future does, and resumes only when its caller polls again. Core runtime
crates contain no executor or Tokio dependency.

Before yielding an async boundary, the session recursively checks every live
frame value and reachable aggregate for call-scoped HostRefs. A live borrowed
child cannot cross suspension; it must reach a proven last use or an explicit
`host::release` first. The same recursive check rejects call-scoped children
inside state values, closures, aggregates, PathProxy roots, and the final root
result. Dynamic and reflection calls do not form alternate lifetime domains:
they return through the same frame, heap, host adapter, and root-boundary
checks.

Restricted JIT input marks declared async and await-containing MIR functions
with `MirJitIneligibility::Async`. The future backend boundary remains the same
verified MIR plus linked artifact, including the await operation, safepoint,
and resume edge; there is no compiled async execution path in the MVP.

Engine registration has one async family beside each supported native boundary.
Their factories are `Send + Sync + 'static`, while the returned
`NativeCallFuture<'call>` may borrow invocation arguments and host execution
state for `'call`. `#[vela_macros::export]`, `export_module`, and `methods`
emit the same contract for Rust `async fn`.

`#[vela_macros::methods]` also supports async `&self`/`&mut self` methods. Runtime
atomically acquires Rust-only `HostLeaseRef`/`HostLeaseMut` scopes for the
receiver and any typed `&T`/`&mut T` host parameters. Mutable-origin call
bindings require `Send` and use one owned exclusive root guard. A shared
receiver is a temporary `&T` view reborrowed from that guard, not a concurrent
shared root lease. The same mutable origin therefore cannot enter two Rust
method calls at once, even when both methods have shared receivers. RAII
restores the root on return, error, cancellation, unwind, or failed
multi-acquisition. Async Host methods may hold the guard across suspension only
through a scoped `Send` future. Runtime-owned VM state and adapters without the
requested erased receiver capability fail closed with `HostLeaseUnsupported`.
Neither lease wrapper is a Vela value or reflection type.

An active `NativeCallContext` can call a sync child with `call`, call a sync or
async child with `call_async`, and bind a script method target. Reentry pushes a
marker and child frame on the same session, so it inherits the pinned artifact,
heap, VM/extern state, host boundary, exact generation execution-data view,
remaining budgets, capabilities, and cancellation state. Child `CallArgs`
receive new HostRefs from the execution's
single allocator and are invalid after the child scope exits. A mutable method
must explicitly reborrow its lease into child arguments; the raw parent HostRef
remains busy while the exclusive lease is live.

A heap value returned by reentry is admitted to the active `HeapExecution`
before child frame roots are truncated. The VM creates its dynamic-root
registry lazily on the first such return and marks the admitted roots
immediately if incremental collection is already active. Runtime cross-call
retention stores the weak active-root guard in a sparse sidecar keyed by the
existing `VelaValue` root ID; dropping the last shared handle removes the guard,
and session teardown invalidates the registry. Ordinary calls allocate neither
the registry nor a per-value token.

Async suspension does not change hot-reload ownership. The outer execution and
all nested frames, providers, closures, and reentry retain one
`Arc<LinkedArtifact>`. A cloneable `HotReloadStagingHandle` may replace only the
pending-update slot while `call_async` holds its scoped mutable Runtime borrow;
it cannot activate an update or mutate the active image. After the outer future
completes or is dropped, the embedding host calls `Runtime::activate_reload` at
its existing explicit safe point. No suspended frame, register file, native
future, or host lease migrates generations.

Provider calls use the same target and driver. One pure resolver validates the
Runtime-bound provider handle and reads method dispatch, callable asyncness,
receiver shape, parameter names, and defaults from the pinned
`LinkedArtifact`. Outer and reentry callers allocate and root the fresh
receiver only after that shared resolution, so both paths report identical
metadata and validation failures.

### Execution Budget

The VM charges backend-neutral execution units at explicit MIR semantic points:

```rust
pub struct ExecutionLimits {
    pub execution_unit_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    pub collection_limits: CollectionLimits,
}

pub struct ExecutionBudget {
    limits: ExecutionLimits,
    counters: ExecutionCounters,
    flags: BudgetFlags,
}
```

Verified MIR records positive execution-unit points for CFG backedges,
iterator steps, calls, dynamic work, allocations, HostAccess, and reflection.
The bytecode backend attaches those points to semantic operations as immutable
execution-unit metadata; the VM does not charge implicitly per dispatched
opcode. Runtime-sized callback, iterator,
and container-guard work adds units through the same counter. A charge traps
before its associated effect, while effects completed before a later charge
trap remain committed.

Budgets prevent:

```text
infinite loops
unbounded memory growth
recursive stack overflow
unbounded array/map/set growth
too many state writes in a single event
```

Execution limits are immutable configuration for a run, counters are mutable
runtime state, and budget flags are precomputed from the limits. Hot paths test
the flags instead of reinterpreting sentinel limit values. `usize::MAX` /
`u64::MAX` still mean "disabled" at the public constructor boundary.

Heap allocation and in-place heap collection growth charge the memory budget
when `memory_limit_bytes` is finite. `ExecutionBudget::unbounded()` disables
instruction, memory, call-depth, and collection-growth bookkeeping, so hot
paths can run without budget bookkeeping when the host intentionally chooses
that mode. Arrays and sets charge collection memory by script-visible element
count, and maps charge by script-visible entry keys plus stored values. Hosts
can set collection length limits in addition to, or independently from, the
byte budget when a script should not be allowed to build arbitrarily large
arrays, maps, or sets.

## Threading Model

Vela is a single-threaded scripting language from the script author's point of
view. The primary embedding contract is one logical Vela `Runtime` owned by one
actor. That Runtime contains the actor's Vela state and heap and executes at
most one actor turn at a time. The actor mailbox already supplies exclusive
ownership, so ordinary execution uses `&mut Runtime` directly and must not put
the Runtime behind `Arc<Mutex<Runtime>>`.

An actor Runtime is not an OS-thread-local Runtime. `Runtime` is `Send`, so the
host scheduler may move the actor between workers, and a scoped
`RuntimeCallFuture` may be polled by different executor workers while the actor
turn remains exclusively borrowed. Correctness must therefore never depend on
OS thread-local cache state or a stable worker assignment.

The language does not expose:

```text
thread creation
shared-memory concurrency
locks or atomics
script-visible task/coroutine handles or manual resume
channels
parallel iterators
```

If a host application needs concurrency, the Rust host owns it. Many actors and
therefore many independent actor Runtimes may execute concurrently on different
workers. Each actor invocation still observes a single-threaded VM boundary.
Vela `async fn` and `.await` preserve sequential script semantics; suspension
keeps the actor turn exclusively owned and does not make its Runtime
concurrently callable or expose threads or shared memory to scripts.

Allowed host-level concurrency models:

```text
one logical Runtime per actor
many actor Runtimes scheduled on one worker or execution lane
an actor Runtime moved between workers only under exclusive actor ownership
host async tasks that call into Vela only at explicit scheduling points
background IO that returns copied data or HostRef handles to later script calls
```

The shared deployment generation contains immutable code, metadata, schemas,
source maps, cache/profile layouts, and the shared hot-reload ABI. Each exact
Engine deployment weakly registers generation-qualified execution data. That
data owns one typed synchronized cache slot per linked cache site and optional
aggregate atomic instruction counters; Actor Runtimes retain only handles for
generations they can still execute. A host may additionally use measured
execution-lane sidecars.
`WorkerExecutionSidecars` is an optional performance implementation, not a
semantic layer and not a requirement that an actor remain on one worker. It is
valid only when benchmarks show that generation-shared slots contend and the
host can provide an explicit stable execution-lane identity. Reload publishes
a new immutable generation and new generation-qualified metadata instead of
clearing or rebasing old slots in place.

Required boundaries:

```text
do not call the same Runtime concurrently from multiple threads
do not share script GC objects across runtimes or threads
do not let native functions store borrowed Value references after a call
do not expose host locks, atomics, or thread handles to scripts
do not mutate the same host object set concurrently through multiple runtimes
```

Runtime-managed `VelaValue` handles are also `Send` and may be moved with host
messages, but they remain bound to the runtime that created them. Passing a
`VelaValue` to another runtime is a runtime type error.

Data crossing host threads must be copied, serialized, or represented by stable
host handles such as `HostRef`. Cross-thread conflict resolution, ordering,
locking, database transactions, actor mailboxes, and network IO are host
responsibilities, not Vela language features.

Hot reload follows the same rule: the host publishes a deployment generation,
and each actor Runtime adopts it only at its own safe point. Existing actor
turns and suspended sessions retain their prior generation.

## GC

GC manages:

```text
string objects
arrays
maps
sets
records
enums
closures
upvalues
iterators
call frame objects
```

GC does not manage:

```text
Rust Player
Rust World
Rust Inventory
database objects
network connections
```

Scripts hold only `HostRef` values for host state.

First-version GC:

```text
non-moving mark-sweep
arena allocation
explicit root stack
event/tick boundary step_gc
configurable GC budget
```

API:

```rust
runtime.step_gc(GcBudget::micros(200));
runtime.collect_full_gc();
runtime.set_gc_config(GcConfig {
    max_pause_micros: 500,
    heap_growth_factor: 1.5,
});
```

Moving GC is deferred because it complicates:

```text
GcRef stability
host bridge
debugger
reflection objects
call frames
FFI/native functions
```
