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
RETURN          null
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
    Null,
    Bool(bool),
    Scalar(ScalarValue),
    Range(RangeValue),
    HeapRef(GcRef),
    HostRef(HostRef),
}
```

`OwnedValue`, `HostValue`, and bytecode constants use the same `ScalarValue`
model at their boundaries. All non-scalar script objects live in `HeapValue`:

```rust
pub enum HeapValue {
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Set(Vec<Value>),
    Record { type_name: String, fields: ScriptFields<Value> },
    Enum { enum_name: String, variant: String, fields: ScriptFields<Value> },
    Closure(ClosureValue),
    Iterator(IteratorState),
    PathProxy(PathProxy),
}
```

Only consider the following after profiling proves `Value` overhead is too high:

```text
16-byte tagged value
NaN boxing
pointer tagging
specialized arrays
```

### Execution Budget

The VM charges an instruction budget while executing:

```rust
pub struct ExecutionBudget {
    pub instruction_limit: u64,
    pub memory_limit_bytes: usize,
    pub max_call_depth: usize,
    collection_limits: CollectionLimits,
}
```

Budgets prevent:

```text
infinite loops
unbounded memory growth
recursive stack overflow
unbounded array/map/set growth
too many state writes in a single event
```

Heap allocation and in-place heap collection growth charge the memory budget
when `memory_limit_bytes` is finite. `ExecutionBudget::unbounded()` disables
memory accounting the same way it disables instruction accounting, so hot
paths can run without budget bookkeeping when the host intentionally chooses
that mode. Arrays and sets charge collection memory by script-visible element
count, and maps charge by script-visible entry keys plus stored values. Hosts
can set collection length limits in addition to, or independently from, the
byte budget when a script should not be allowed to build arbitrarily large
arrays, maps, or sets.

## Threading Model

Vela is a single-threaded scripting language from the script author's point of
view. A single `Runtime` executes one script call at a time on one OS thread,
with one VM stack, one active `HostAccess`, and one script heap/GC context.
`Runtime` is `Send` so a host can move it into an actor or worker thread, but
the runtime API still requires mutable access for execution and does not make a
single runtime concurrently callable.

The language does not expose:

```text
thread creation
shared-memory concurrency
locks or atomics
async/await
coroutines
channels
parallel iterators
```

If a host application needs concurrency, the Rust host owns it. The host may run
multiple independent Vela runtimes on different threads, shard actors across
workers, schedule events on an async runtime, or perform IO in background tasks.
Each script invocation still observes a single-threaded VM boundary.

Allowed host-level concurrency models:

```text
one Runtime per actor worker
one Runtime per shard, tenant, or worker
runtime pool with no concurrent use of the same Runtime
host async tasks that call into Vela only at explicit scheduling points
background IO that returns copied data or HostRef handles to later script calls
```

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

Hot reload follows the same rule: the host may coordinate update distribution
across worker threads, but each runtime swaps `ProgramVersion` only at its own
safe points.

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
