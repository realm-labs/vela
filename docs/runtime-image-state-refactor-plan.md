# Runtime Image and State Refactor Plan

## Purpose

This document proposes a clean architecture refactor for Vela runtime ownership, shared immutable code, per-runtime inline caches, hot reload, and future JIT support.

The intended deployment model is a game server where many actors run the same Vela logic, but each actor owns its own runtime state:

```text
shared across actors:
  compiled code
  immutable metadata
  native function registry
  type registry
  hot-reload version image

owned by each actor/runtime:
  script heap
  globals
  retained runtime values
  inline caches
  call stack
  HostAccess
  player/actor state
```

The key design choices are:

```text
1. Runtime image/state split.
2. Stable default Runtime API while image storage evolves internally.
3. Owned runtime remains the default.
4. Shared runtime is opt-in.
5. Inline caches are per-runtime.
6. JIT-facing contracts are documented early, but JIT code stays later.
7. Shared JIT code can be added later, but mutable optimization state remains runtime-local.
```

This is intentionally **not** a minimal refactor. The goal is a clean long-term architecture.

---

## Current State Summary

Based on the currently inspected repository state:

`Runtime` currently mixes immutable and mutable data:

```rust
pub struct Runtime {
    id: u64,
    engine: Engine,
    program: Program,
    hot_reload: Option<HotReloadRuntime>,
    globals: RuntimeGlobalStore,
    script_globals: RuntimeScriptGlobalStore,
}
```

This means every runtime owns an `Engine` and a full `Program` by value.

`Program` currently owns code objects directly:

```rust
pub struct Program {
    pub functions: BTreeMap<String, CodeObject>,
    global_names: Vec<String>,
    global_slots: BTreeMap<String, GlobalSlot>,
    script_methods: ScriptMethodTable,
    script_metadata: Option<ModuleGraph>,
}
```

Each `CodeObject` owns parameters, constants, bytecode instructions, and debug frame info:

```rust
pub struct CodeObject {
    pub name: String,
    pub params: Vec<String>,
    pub param_defaults: Vec<bool>,
    pub capture_count: u16,
    pub register_count: u16,
    pub frame: FrameDebugInfo,
    pub constants: Vec<Constant>,
    pub instructions: Vec<Instruction>,
}
```

The hot reload layer is already closer to the desired architecture. `ProgramVersion` stores functions as `Arc<CodeObject>`:

```rust
pub struct ProgramVersion {
    pub id: ProgramVersionId,
    pub(crate) functions: BTreeMap<FunctionSymbolId, Arc<CodeObject>>,
    pub(crate) script_methods: ScriptMethodTable,
    pub(crate) script_metadata: Option<ModuleGraph>,
    pub(crate) abi: HotReloadAbi,
    pub(crate) profile: ProgramProfile,
}
```

However, `Runtime::from_hot_reload_version` converts a `ProgramVersion` back into an owned `Program`, and `ProgramVersion::to_program()` clones code objects back into a normal `Program`. That loses most of the sharing benefit at runtime creation time.

`RuntimeScriptGlobalStore` is already clearly per-runtime because it owns a `ScriptHeap`, script global values, and retained runtime roots:

```rust
pub struct RuntimeScriptGlobalStore {
    heap: ScriptHeap,
    values: ScriptGlobalValues,
    retained_values: Arc<Mutex<RuntimeValueRoots>>,
}
```

The current docs already define the correct high-level threading model: a `Runtime` executes one script call at a time, has one VM stack and one script heap/GC context, and may be moved into an actor or worker thread but must not be called concurrently.

---

## Architecture Goals

### Goal 1: Cleanly separate immutable image from mutable state

Introduce an explicit split:

```text
RuntimeImage:
  immutable, shareable, compile-time/runtime-code data

RuntimeState:
  mutable, actor-local execution state
```

A runtime should first become conceptually:

```rust
pub struct Runtime {
    image: RuntimeImage,
    state: RuntimeState,
}
```

After the split is stable, the implementation can become generic or add a
separate shared-runtime wrapper:

```rust
pub struct Runtime<I = OwnedImage>
where
    I: RuntimeImageStorage,
{
    image: I,
    state: RuntimeState,
}
```

### Goal 2: Default runtime API should stay simple

Normal users should keep writing:

```rust
let runtime = Runtime::new(engine, program);
```

This should use owned image storage internally. The first implementation should
not expose generic runtime storage in public APIs until the concrete
image/state split is stable.

### Goal 3: Game servers can opt into shared immutable code

Game server users should be able to compile once and instantiate many runtimes:

```rust
let image = RuntimeImage::new(engine, program).into_shared();

let runtime_a = SharedRuntime::from_shared_image(Arc::clone(&image));
let runtime_b = SharedRuntime::from_shared_image(Arc::clone(&image));
let runtime_c = SharedRuntime::from_shared_image(Arc::clone(&image));
```

### Goal 4: Per-runtime inline caches

Inline caches should never be stored inside shared bytecode instructions or shared code objects.

Instead:

```text
shared instruction:
  contains CacheSiteId

per-runtime state:
  contains InlineCaches[CacheSiteId]
```

This keeps actor isolation, avoids locks, and avoids cross-thread cache contention.

### Goal 5: JIT-compatible contracts from the beginning

Future generated machine code should be able to execute against any runtime
state using the same shared image:

```rust
fn call_compiled(
    compiled: &CompiledFunction,
    image: &RuntimeImage,
    state: &mut RuntimeState,
    args: &[Value],
    host: &mut HostExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<Value>;
```

The JIT must not bake per-actor heap addresses, global values, or `HostAccess` pointers into shared code.

The first refactor should document and preserve these boundaries, not add JIT
runtime types or machine-code storage before M22 work needs them.

---

## Non-Goals

This refactor should not try to solve everything at once.

Non-goals for the first clean architecture pass:

```text
1. Implementing the JIT immediately.
2. Implementing all inline caches immediately.
3. Rewriting the whole bytecode format in one patch.
4. Making one Runtime concurrently callable.
5. Sharing ScriptHeap, GcRef, HeapValue, VelaValue, or script globals across runtimes.
6. Sharing mutable inline caches between actors.
```

---

## Core Ownership Invariants

These invariants should be documented in code and tests.

### Shared safely

The following may be shared across runtimes:

```text
RuntimeImage
ProgramImage
FunctionImage
CodeObject, if immutable
constant pools
instruction arrays
script method tables
module metadata
source/debug metadata
hot-reload ABI/profile metadata
native function registry
reflection/type registry
JIT machine code, if compiled from image-level facts only
```

### Never shared between runtime instances

The following must remain per-runtime:

```text
RuntimeState
ScriptHeap
GcRef / HeapValue ownership domain
ScriptGlobalValues
RuntimeValueRoots
VelaValue roots
CallFrame stack
HostAccess
ScriptStateAdapter
per-runtime InlineCaches
actor/player-local state
```

### `VelaValue` remains runtime-local

`VelaValue` must remain bound to the runtime that created it. Passing a `VelaValue` into another runtime should continue to be a runtime type error.

This is especially important after shared code is introduced. Shared code does not imply shared values.

---

## Proposed Module Layout

Suggested new module structure inside `vela_engine` and related crates:

```text
crates/vela_engine/src/runtime/
  mod.rs
  runtime.rs              // Runtime / RuntimeImpl<I>
  image.rs                // RuntimeImage, OwnedImage, SharedImage
  state.rs                // RuntimeState
  storage.rs              // RuntimeImageStorage trait
  inline_cache.rs         // InlineCaches, CacheSiteId, cache entries
  handles.rs              // VelaValue, VelaFunction, VelaMethod, runtime-bound handles
  call_args.rs
  hot_reload.rs           // runtime-level image swap/reload integration

crates/vela_bytecode/src/image.rs
  ProgramImage
  FunctionImage
  GlobalLayout
  CacheSiteDesc

crates/vela_vm/src/execution_context.rs
  RuntimeExecutionContext
  ImageExecutionContext
```

The goal is to make ownership obvious from the file structure:

```text
image.rs:
  immutable things

state.rs:
  mutable per-runtime things

inline_cache.rs:
  mutable per-runtime optimization state

runtime.rs:
  orchestration API
```

---

## Target Type Design

### `RuntimeImage`

Initial version:

```rust
pub struct RuntimeImage {
    engine: Engine,
    program: Program,
    version_id: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
    hot_reload: Option<RuntimeImageHotReload>,
}

pub struct RuntimeImageLayout {
    global_names: Box<[String]>,
    // Later: function IDs, cache site layouts, method dispatch tables, etc.
}

pub struct RuntimeImageHotReload {
    abi: HotReloadAbi,
    profile: ProgramProfile,
}
```

Long-term version:

```rust
pub struct RuntimeImage {
    engine: Engine,
    program: ProgramImage,
    version: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
    jit: Option<JitImageState>,
}
```

Recommended methods:

```rust
impl RuntimeImage {
    pub fn new(engine: Engine, program: Program) -> Self;

    pub fn from_program_version(engine: Engine, version: ProgramVersion) -> Self;

    pub fn engine(&self) -> &Engine;

    pub fn program(&self) -> &Program;

    pub fn global_names(&self) -> &[String];

    pub fn version_id(&self) -> Option<ProgramVersionId>;

    pub fn hot_reload_profile(&self) -> Option<&ProgramProfile>;

    pub fn into_shared(self) -> Arc<Self>;
}
```

Later, when `ProgramImage` exists:

```rust
impl RuntimeImage {
    pub fn program_image(&self) -> &ProgramImage;
}
```

Important: runtime image construction must copy the complete execution layout
from the source artifact. Do not make `ProgramVersion::to_program()` the
canonical runtime path, because conversion back into `Program` can drop or
reconstruct layout details such as global slots.

---

### Generic image storage

Generic storage is a possible internal implementation once `RuntimeImage` and
`RuntimeState` are stable. Do not make it the first public API shape if that
would spread generic parameters through handles, call args, C API wrappers,
tests, and examples before the split has proven itself.

If generics are chosen, use generic storage instead of forcing the default
runtime path through `Arc<RuntimeImage>`:

```rust
use std::ops::Deref;
use std::sync::Arc;

pub trait RuntimeImageStorage: Deref<Target = RuntimeImage> {}

pub struct OwnedImage {
    image: RuntimeImage,
}

impl Deref for OwnedImage {
    type Target = RuntimeImage;

    fn deref(&self) -> &RuntimeImage {
        &self.image
    }
}

impl RuntimeImageStorage for OwnedImage {}

#[derive(Clone)]
pub struct SharedImage {
    image: Arc<RuntimeImage>,
}

impl Deref for SharedImage {
    type Target = RuntimeImage;

    fn deref(&self) -> &RuntimeImage {
        &self.image
    }
}

impl RuntimeImageStorage for SharedImage {}
```

Optional convenience trait:

```rust
pub trait RuntimeImageStorageExt: RuntimeImageStorage {
    fn as_image(&self) -> &RuntimeImage {
        self.deref()
    }
}

impl<T> RuntimeImageStorageExt for T where T: RuntimeImageStorage {}
```

---

### `RuntimeState`

```rust
pub struct RuntimeState {
    id: u64,
    globals: RuntimeGlobalStore,
    script_globals: RuntimeScriptGlobalStore,
    inline_caches: InlineCaches,
}
```

Constructor:

```rust
impl RuntimeState {
    pub fn new_for_image(image: &RuntimeImage) -> Self {
        let global_names = image.global_names();

        Self {
            id: next_runtime_id(),
            globals: RuntimeGlobalStore::with_global_layout(global_names),
            script_globals: RuntimeScriptGlobalStore::with_global_layout(global_names),
            inline_caches: InlineCaches::for_image(image),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}
```

When image layout changes after hot reload:

```rust
impl RuntimeState {
    pub fn rebind_to_image(&mut self, image: &RuntimeImage) {
        let names = image.global_names();
        self.globals.set_global_layout(names);
        self.script_globals.set_global_layout(names);
        self.inline_caches = InlineCaches::for_image(image);
    }
}
```

For the first implementation, clearing inline caches on image swap is correct and simple.

---

### `RuntimeImpl<I>`

This is the long-term target if generic storage proves worth the API cost. The
first implementation should keep the public `Runtime` concrete and use this
shape only after `RuntimeImage`, `RuntimeState`, and image-native hot reload
are already tested.

```rust
pub struct RuntimeImpl<I = OwnedImage>
where
    I: RuntimeImageStorage,
{
    image: I,
    state: RuntimeState,
    reload: Option<RuntimeReloadState<I>>,
}
```

Generic methods:

```rust
impl<I> RuntimeImpl<I>
where
    I: RuntimeImageStorage,
{
    pub fn image(&self) -> &RuntimeImage {
        &self.image
    }

    pub fn engine(&self) -> &Engine {
        self.image().engine()
    }

    pub fn program(&self) -> &Program {
        self.image().program()
    }

    pub fn id(&self) -> u64 {
        self.state.id()
    }

    pub fn call<T>(
        &mut self,
        entry: T,
        args: CallArgs<'_>,
        options: CallOptions,
    ) -> VmResult<VelaValue>
    where
        T: RuntimeCallTarget,
    {
        // Use self.image + self.state.
    }
}
```

Owned constructor:

```rust
pub type Runtime = RuntimeImpl<OwnedImage>;

impl RuntimeImpl<OwnedImage> {
    pub fn new(engine: Engine, program: Program) -> Self {
        Self::from_owned_image(RuntimeImage::new(engine, program))
    }

    pub fn from_owned_image(image: RuntimeImage) -> Self {
        let state = RuntimeState::new_for_image(&image);
        Self {
            image: OwnedImage { image },
            state,
            reload: None,
        }
    }
}
```

Shared constructor:

```rust
pub type SharedRuntime = RuntimeImpl<SharedImage>;

impl RuntimeImpl<SharedImage> {
    pub fn from_shared_image(image: Arc<RuntimeImage>) -> Self {
        let state = RuntimeState::new_for_image(&image);
        Self {
            image: SharedImage { image },
            state,
            reload: None,
        }
    }
}
```

---

## Program Image Design

The first milestone can keep using `Program` inside `RuntimeImage`.

However, for the clean long-term architecture, introduce `ProgramImage` as an immutable, index-friendly representation.

### `ProgramImage`

```rust
pub struct ProgramImage {
    functions: Box<[FunctionImage]>,
    function_by_name: BTreeMap<String, FunctionIndex>,
    global_layout: GlobalLayout,
    script_methods: ScriptMethodTable,
    script_metadata: Option<ModuleGraph>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct FunctionIndex(pub u32);
```

### `FunctionImage`

```rust
pub struct FunctionImage {
    id: FunctionIndex,
    name: String,
    params: Box<[String]>,
    param_defaults: Box<[bool]>,
    capture_count: u16,
    register_count: u16,
    frame: FrameDebugInfo,
    constants: Box<[Constant]>,
    instructions: Box<[Instruction]>,
    cache_sites: Box<[CacheSiteDesc]>,
}
```

This gives future JIT and inline cache passes stable indexes.

### Why not just `Arc<CodeObject>` everywhere?

`Arc<CodeObject>` is useful, and hot reload already uses it. But a clean architecture should move toward a true image model:

```text
Program:
  compiler output / compatibility wrapper

ProgramImage:
  immutable execution image
  indexed
  shareable
  JIT-friendly
  cache-site-aware
```

`Program` can remain public for compatibility while `RuntimeImage` internally lowers it to `ProgramImage`.

---

## Closure Representation

Current bytecode has a problematic shape for long-term sharing/JIT:

```rust
MakeClosure {
    dst: Register,
    code: Box<CodeObject>,
    captures: Vec<Register>,
}
```

For shared images and JIT, nested `Box<CodeObject>` should become a stable function/lambda reference:

```rust
MakeClosure {
    dst: Register,
    function: FunctionIndex,
    captures: Box<[Register]>,
}
```

Benefits:

```text
1. No nested code-object copying.
2. All executable code lives in ProgramImage.functions.
3. JIT can compile lambdas the same way as top-level functions.
4. Hot reload can reason about function identities consistently.
5. Cache-site IDs can be globally unique within FunctionImage.
```

---

## Inline Cache Architecture

### Do not put mutable caches in shared code

Avoid this:

```rust
pub struct Instruction {
    kind: InstructionKind,
    cache: UnsafeCell<InlineCacheEntry>,
}
```

This makes immutable code secretly mutable and creates contention when code is shared across actor runtimes.

### Put only cache-site IDs in bytecode

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct CacheSiteId(pub u32);

pub struct CacheSiteDesc {
    pub id: CacheSiteId,
    pub kind: CacheSiteKind,
    pub function: FunctionIndex,
    pub instruction_offset: InstructionOffset,
}

pub enum CacheSiteKind {
    GlobalRead,
    GlobalWrite,
    RecordFieldRead,
    RecordFieldWrite,
    MethodCall,
    HostPathRead,
    HostPathWrite,
    NativeCall,
}
```

Example instruction:

```rust
InstructionKind::GetRecordSlot {
    dst,
    record,
    field,
    slot,
    cache_site,
}
```

### Per-runtime cache storage

```rust
pub struct InlineCaches {
    entries: Box<[InlineCacheEntry]>,
}

pub enum InlineCacheEntry {
    Empty,

    GlobalRead {
        slot: GlobalSlot,
    },

    RecordFieldRead {
        type_id: ScriptTypeId,
        field_slot: usize,
    },

    MethodCall {
        receiver_type: TypeKey,
        method_id: MethodId,
        target: FunctionIndex,
    },

    HostPathRead {
        type_id: TypeKey,
        path_key: HostPathKey,
    },
}
```

Constructor:

```rust
impl InlineCaches {
    pub fn for_image(image: &RuntimeImage) -> Self {
        let count = image.cache_site_count();
        Self {
            entries: vec![InlineCacheEntry::Empty; count].into_boxed_slice(),
        }
    }

    pub fn get_mut(&mut self, site: CacheSiteId) -> &mut InlineCacheEntry {
        &mut self.entries[site.0 as usize]
    }
}
```

### Execution path

VM execution should receive both immutable image and mutable runtime state:

```rust
pub struct RuntimeExecution<'a, 'host> {
    pub image: &'a RuntimeImage,
    pub state: &'a mut RuntimeState,
    pub host: &'a mut HostExecution<'host>,
    pub budget: &'a mut ExecutionBudget,
}
```

Instruction execution:

```rust
fn execute_get_record_field(
    exec: &mut RuntimeExecution<'_, '_>,
    dst: Register,
    record: Register,
    field: &str,
    cache_site: CacheSiteId,
) -> VmResult<()> {
    let cache = exec.state.inline_caches.get_mut(cache_site);

    // Fast path if cache matches.
    // Slow path resolves and updates this runtime's cache only.

    Ok(())
}
```

### Cache invalidation

For now:

```text
on hot reload / image swap:
  clear all per-runtime inline caches
```

Later:

```text
if function unchanged:
  preserve function-local caches

if ABI/type layout unchanged:
  preserve compatible field/method caches

if type layout changed:
  clear affected cache kinds
```

---

## VM Execution Refactor

Current runtime execution often constructs a `Vm` from `Engine` and `Program` per call path. The clean architecture should eventually separate:

```text
EngineCore:
  native functions
  host native functions
  type registry
  reflection policies

RuntimeImage:
  program image
  resolved dispatch metadata

RuntimeState:
  heap/globals/caches

VmExecutor:
  stateless or mostly immutable execution engine
```

Long-term execution shape:

```rust
pub struct VmExecutor {
    // Ideally stateless or image-independent.
}

impl VmExecutor {
    pub fn run_function(
        &self,
        image: &RuntimeImage,
        state: &mut RuntimeState,
        function: FunctionIndex,
        args: &[Value],
        host: &mut HostExecution<'_>,
        budget: &mut ExecutionBudget,
    ) -> VmResult<Value> {
        // Interpret bytecode using image + state.
    }
}
```

Do not make this part too ambitious in the first PR. But the direction should be clear:

```text
Runtime owns state and image storage.
VmExecutor executes against borrowed image + borrowed mutable state.
```

---

## Hot Reload Architecture

### Current problem

Hot reload already has `ProgramVersion` with `Arc<CodeObject>`, but current runtime creation converts it back into `Program`, cloning code objects.

This should be fixed early in the refactor. If `RuntimeImage` initially wraps
`Program`, it still needs a direct `from_program_version` construction path
that preserves version id, ABI/profile metadata, script methods, script
metadata, and global layout. Rebuilding a runtime image by cloning a
`ProgramVersion` into a plain `Program` should become a compatibility helper,
not the execution path.

### Clean target

Hot reload should operate on immutable runtime images:

```text
old image: Arc<RuntimeImage>
new image: Arc<RuntimeImage>
```

The host compiles or receives the new image once, then distributes it to many actor runtimes.

```rust
let new_image = hot_reload_manager.compile_next_image()?;
for actor in actors {
    actor.runtime.stage_shared_image(Arc::clone(&new_image));
}
```

Each actor swaps at a safe point:

```rust
runtime.check_reload_at_tick_boundary()?;
```

### Runtime reload state

For shared runtime:

```rust
pub struct RuntimeReloadState<I> {
    pending_image: Option<I>,
    last_report: Option<HotReloadReport>,
}
```

For `SharedImage`, `pending_image` is another `Arc<RuntimeImage>`.

For `OwnedImage`, hot reload can either:

```text
1. Replace the owned image directly, or
2. Use a different owned pending image.
```

### Image swap

```rust
impl SharedRuntime {
    pub fn stage_shared_image(&mut self, image: Arc<RuntimeImage>) {
        self.reload
            .get_or_insert_with(RuntimeReloadState::default)
            .pending_image = Some(SharedImage { image });
    }

    pub fn check_reload_at_tick_boundary(&mut self) -> EngineResult<Option<HotReloadReport>> {
        let Some(reload) = &mut self.reload else {
            return Ok(None);
        };

        let Some(next_image) = reload.pending_image.take() else {
            return Ok(None);
        };

        self.image = next_image;
        self.state.rebind_to_image(&self.image);

        Ok(Some(/* report */))
    }
}
```

### Version lifetime

With `Arc<RuntimeImage>`, old code naturally stays alive while any runtime still references it:

```text
worker 1 runtime still executing old image -> old image alive
worker 2 runtime swapped to new image      -> new image alive
all old runtimes done                      -> old image drops
```

This is a strong fit for actor-based hot reload.

---

## JIT Architecture

### JIT is compatible with shared immutable code

Shared code does not make JIT harder if mutable optimization state is separated.

Use this split:

```text
RuntimeImage:
  immutable bytecode
  immutable metadata
  optional shared compiled machine code

RuntimeState:
  heap
  globals
  roots
  per-runtime inline caches
  per-runtime profiling/deopt counters, if desired
```

### JIT machine code storage

Possible future policies:

```rust
pub enum JitPolicy {
    None,
    PerRuntime,
    PerWorker,
    PerImage,
}
```

Recommended long-term defaults:

```text
normal embedding:
  JitPolicy::None or PerRuntime

game server shared runtime:
  JitPolicy::PerImage for machine code
  per-runtime inline caches
  optional per-runtime counters
```

### Shared compiled functions

```rust
pub struct JitImageState {
    functions: DashMap<FunctionIndex, Arc<CompiledFunction>>,
}
```

Or, if avoiding dependencies:

```rust
pub struct JitImageState {
    functions: Mutex<BTreeMap<FunctionIndex, Arc<CompiledFunction>>>,
}
```

For a first JIT implementation, do not over-optimize this. Compile at safe points or compile on a worker-local compiler thread.

### Compiled function ABI

A compiled function should take state as an explicit argument:

```rust
pub type CompiledEntry = unsafe extern "C" fn(
    image: *const RuntimeImage,
    state: *mut RuntimeState,
    args: *const Value,
    args_len: usize,
    host: *mut HostExecution<'_>,
    budget: *mut ExecutionBudget,
    out: *mut Value,
) -> JitStatus;
```

Conceptually, the safe wrapper is:

```rust
fn call_compiled(
    function: &CompiledFunction,
    image: &RuntimeImage,
    state: &mut RuntimeState,
    args: &[Value],
    host: &mut HostExecution<'_>,
    budget: &mut ExecutionBudget,
) -> VmResult<Value>;
```

### Do not bake runtime-local data into shared machine code

Shared JIT code may assume:

```text
function bytecode shape
constant pool indexes
register count
field IDs
method IDs
global slot IDs
native function IDs
ABI version
```

Shared JIT code must not assume:

```text
current actor heap address
current player object address
current global Value contents
HostAccess pointer
ScriptStateAdapter pointer
per-runtime cache contents
GcRef validity across runtimes
```

### Deoptimization

Deopt should return to interpreter with an explicit status:

```rust
pub enum JitStatus {
    Ok,
    Trap,
    BudgetExceeded,
    Deopt { reason: DeoptReason, pc: InstructionOffset },
}
```

The interpreter resumes using:

```text
image + state + function index + instruction offset
```

This requires stable instruction offsets and function indexes, another reason to move toward `ProgramImage`.

---

## Public API Shape

### Default usage

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .build()?;

let program = engine.compile_dir("scripts")?;
let mut runtime = Runtime::new(engine, program);
```

### Shared game-server usage

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .build()?;

let program = engine.compile_dir("scripts")?;
let image = RuntimeImage::new(engine, program).into_shared();

let mut runtime = SharedRuntime::from_shared_image(Arc::clone(&image));
```

### Actor creation

```rust
pub struct Actor {
    id: ActorId,
    runtime: SharedRuntime,
    state: PlayerState,
}

impl Actor {
    pub fn new(id: ActorId, image: Arc<RuntimeImage>, state: PlayerState) -> Self {
        Self {
            id,
            runtime: SharedRuntime::from_shared_image(image),
            state,
        }
    }
}
```

### Hot reload distribution

```rust
let new_image = reload_manager.compile_next_runtime_image()?;

for actor in actors.iter_mut() {
    actor.runtime.stage_shared_image(Arc::clone(&new_image));
}

// Later, at actor tick boundary:
actor.runtime.check_reload_at_tick_boundary()?;
```

---

## Migration Plan

This is a clean architecture migration, not a minimal patch.

### PR 1: Create runtime submodules

Move current runtime code into a directory:

```text
crates/vela_engine/src/runtime/
  mod.rs
  runtime.rs
  state.rs
  image.rs
  call_args.rs
  handles.rs
```

No semantic changes yet.

### PR 2: Extract `RuntimeState`

Move only mutable runtime-owned data first:

```rust
pub struct RuntimeState {
    id: u64,
    globals: RuntimeGlobalStore,
    script_globals: RuntimeScriptGlobalStore,
    inline_caches: InlineCaches,
}
```

Keep `Runtime` concrete:

```rust
pub struct Runtime {
    engine: Engine,
    program: Program,
    hot_reload: Option<HotReloadRuntime>,
    state: RuntimeState,
}
```

This PR should remove direct destructuring of `id`, `globals`, and
`script_globals` in call paths, but it should not change hot reload behavior
yet.

### PR 3: Extract concrete `RuntimeImage`

Introduce a concrete image without public generic storage:

```rust
pub struct RuntimeImage {
    engine: Engine,
    program: Program,
    version_id: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
    hot_reload: Option<RuntimeImageHotReload>,
}
```

Then make `Runtime`:

```rust
pub struct Runtime {
    image: RuntimeImage,
    hot_reload: Option<HotReloadRuntime>,
    state: RuntimeState,
}
```

Keep existing user API working:

```rust
Runtime::new(engine, program)
Runtime::from_hot_reload_version(engine, version)
```

This PR should make `RuntimeImage::new(engine, program)` and
`RuntimeImage::from_program_version(engine, version)` preserve global layout
and version metadata.

### PR 4: Make runtime hot reload image-native

Stop assigning `self.program = version.to_program()` inside runtime reload
paths. Instead, accepted updates should produce or be converted directly into a
new `RuntimeImage`, and `RuntimeState::rebind_to_image` should update
per-runtime global layouts and clear runtime-local caches.

Keep `ProgramVersion::to_program()` only for tests, diagnostics, or
compatibility callers that explicitly need a `Program`.

Tests should verify:

```text
1. Accepted reload swaps the runtime image and version id.
2. Rejected reload leaves the old image and profile unchanged.
3. Global layout survives initial hot-reload image construction.
4. Global layout rebinding preserves matching runtime-owned global values.
5. VelaFunction and VelaMethod handles resolve correctly across compatible reloads.
```

### PR 5: Decide image storage shape

After PR 4, choose one of two implementation shapes:

```text
Option A:
  keep public Runtime concrete and store image internally as an enum

Option B:
  introduce RuntimeImpl<I>, with public aliases Runtime and SharedRuntime
```

If choosing generics, avoid making downstream APIs generic by default:

```rust
pub type Runtime = RuntimeImpl<OwnedImage>;
pub type SharedRuntime = RuntimeImpl<SharedImage>;
```

### PR 6: Add shared image and shared runtime API

Add the opt-in shared API:

```rust
RuntimeImage::into_shared()
SharedRuntime::from_shared_image(...)
```

Add tests that verify:

```text
1. Multiple runtimes can share one RuntimeImage.
2. Script globals are isolated.
3. Heap values are isolated.
4. VelaValue from runtime A cannot be used in runtime B.
5. VelaFunction from runtime A cannot be used in runtime B.
6. VelaMethod from runtime A cannot be used in runtime B.
7. Host globals are isolated unless host intentionally shares external objects.
```

### PR 7: Create empty per-runtime `InlineCaches`

Add `InlineCaches` to `RuntimeState` but do not optimize anything yet.

Thread mutable IC access through execution APIs where needed.

### PR 8: Introduce `CacheSiteId` and cache-site layout

Add cache-site metadata to compiler output.

Start with metadata only:

```rust
pub struct CacheSiteDesc {
    id: CacheSiteId,
    kind: CacheSiteKind,
    function: FunctionIndex,
    instruction_offset: InstructionOffset,
}
```

Do not implement fast paths yet.

### PR 9: Implement first per-runtime cache

Recommended first cache: global slot or record field slot.

Global slot caching is likely easiest because Vela already has global slots.

Record field caching is more performance-visible but may touch more code.

### PR 10: Replace nested closure `Box<CodeObject>`

Change:

```rust
MakeClosure { code: Box<CodeObject>, ... }
```

into:

```rust
MakeClosure { function: FunctionIndex, ... }
```

This is a larger compiler/bytecode refactor but important for clean architecture.

### PR 11: Introduce `ProgramImage`

Lower `Program` into `ProgramImage` when constructing `RuntimeImage`.

Keep public `Program` around as compiler output if needed.

### PR 12: Make hot reload produce `ProgramImage` directly

After `ProgramImage` exists, stop constructing image-native reloads through
the transitional `Program` wrapper.

### Later: JIT ownership skeleton

Keep these as documentation until M22 or an immediate interpreter contract
requires code:

```rust
JitPolicy
JitImageState
CompiledFunction
JitStatus
```

The first clean runtime refactor should establish the ownership boundaries
without adding unused JIT runtime types.

---

## Testing Plan

### Unit tests

```text
RuntimeImage construction
RuntimeImage construction from ProgramVersion preserves version/profile/layout
OwnedImage deref behavior, if generic storage is chosen
SharedImage clone behavior, if shared storage is chosen
RuntimeState initialization from image
RuntimeState rebind after image swap
InlineCaches allocation count matches image cache-site count
VelaValue runtime mismatch still errors
VelaFunction runtime mismatch still errors
VelaMethod runtime mismatch still errors
```

### Integration tests

```text
1. Create one shared image.
2. Create two runtimes from it.
3. Set different globals in each runtime.
4. Call the same script function.
5. Verify results differ according to per-runtime state.
6. Verify no heap object crosses runtimes.
7. Verify no retained VelaValue crosses runtimes.
8. Verify cached VelaFunction and VelaMethod handles stay runtime-bound.
```

Example:

```rust
#[test]
fn shared_image_isolates_script_globals() {
    let image = compile_test_image();

    let mut a = SharedRuntime::from_shared_image(Arc::clone(&image));
    let mut b = SharedRuntime::from_shared_image(Arc::clone(&image));

    a.set_global("score", 10).unwrap();
    b.set_global("score", 99).unwrap();

    assert_eq!(a.global_as::<i64>("score").unwrap(), Some(10));
    assert_eq!(b.global_as::<i64>("score").unwrap(), Some(99));
}
```

### Hot reload tests

```text
1. Create image v1.
2. Create many shared runtimes.
3. Stage image v2.
4. Swap only one runtime.
5. Verify swapped runtime sees v2 behavior.
6. Verify unswapped runtime still sees v1 behavior.
7. Drop old runtime and verify old image can be released.
8. Verify rejected reload keeps the old image and profile metadata.
9. Verify global slot/layout metadata survives image construction and rebinding.
```

### Concurrency tests

```text
1. Move SharedRuntime to worker thread.
2. Ensure Runtime remains Send if intended.
3. Ensure same Runtime cannot be called concurrently through safe API.
4. Create many runtimes from one image and call them on multiple threads.
```

### Property tests

Useful invariants:

```text
for any two RuntimeState values from the same image:
  heaps are independent
  roots are independent
  globals are independent
  inline caches are independent
```

---

## Performance Measurement Plan

Measure before and after.

### Memory benchmarks

Create N runtimes from the same script:

```text
N = 1
N = 100
N = 1_000
N = 10_000
```

Compare:

```text
current Runtime::new(engine.clone(), program.clone())
new concrete Runtime with owned image storage
new SharedRuntime with shared image storage
```

Track:

```text
RSS
heap allocations
allocated bytes per runtime
Program/CodeObject duplication
ScriptHeap baseline cost
InlineCaches baseline cost
```

### CPU benchmarks

Measure:

```text
runtime creation time
call overhead
hot reload image distribution
image swap cost
cache clearing cost
global access before/after IC
record field access before/after IC
method call before/after IC
```

### Important expectation

Shared image mode should reduce memory dramatically when many runtimes run the same code.

It should not necessarily improve raw per-call speed by itself. Speed improvements come later from:

```text
ProgramImage indexed layout
per-runtime inline caches
ID-based dispatch
JIT
```

---

## Risks and Mitigations

### Risk: generic runtime type infects public APIs

Mitigation:

Do not introduce public generic runtime storage in the first image/state split.
First land a concrete `RuntimeImage` and `RuntimeState`, then choose either an
internal storage enum or a `RuntimeImpl<I>` with public aliases.

```rust
pub type Runtime = RuntimeImpl<OwnedImage>;
pub type SharedRuntime = RuntimeImpl<SharedImage>;
```

Most users should keep using `Runtime`.

### Risk: too much code becomes generic

Mitigation:

Keep most implementation methods concrete until the image/state boundary is
stable. If generic storage is introduced later, keep generic parameters behind
aliases and implement shared-only APIs only on the shared alias or storage
specialization.

### Risk: inline caches accidentally become shared

Mitigation:

Do not allow `InlineCaches` inside `RuntimeImage`, `ProgramImage`, `FunctionImage`, `CodeObject`, or `Instruction`.

Make the compiler produce cache-site IDs only.

### Risk: hot reload invalidates cache-site IDs

Mitigation:

First implementation clears all per-runtime inline caches on image swap.

Later implementations can preserve compatible caches only when function and layout identities match.

### Risk: runtime image construction keeps depending on `to_program()`

Mitigation:

Make `RuntimeImage::from_program_version` the runtime construction path for hot
reload images. Preserve version id, ABI/profile metadata, script method
metadata, module metadata, and global layout there. Keep
`ProgramVersion::to_program()` as an explicit compatibility helper rather than
the execution path.

### Risk: JIT bakes in runtime-local state

Mitigation:

Document the compiled function ABI early so generated code always receives
`RuntimeState` explicitly. Do not add unused JIT runtime types during the clean
runtime refactor.

### Risk: `ProgramImage` migration is too large

Mitigation:

Stage it:

```text
1. RuntimeImage wraps existing Program.
2. SharedImage shares RuntimeImage.
3. Later lower Program into ProgramImage.
```

The final architecture should still be the guide, but the migration can be safe and incremental.

---

## Recommended Final Shape

```rust
pub type Runtime = RuntimeImpl<OwnedImage>;
pub type SharedRuntime = RuntimeImpl<SharedImage>;

pub struct RuntimeImpl<I = OwnedImage>
where
    I: RuntimeImageStorage,
{
    image: I,
    state: RuntimeState,
    reload: Option<RuntimeReloadState<I>>,
}

pub struct RuntimeImage {
    engine: Engine,
    program: ProgramImage,
    version: Option<ProgramVersionId>,
    layout: RuntimeImageLayout,
    jit: Option<JitImageState>,
}

pub struct RuntimeState {
    id: u64,
    globals: RuntimeGlobalStore,
    script_globals: RuntimeScriptGlobalStore,
    inline_caches: InlineCaches,
}
```

The guiding rule:

```text
RuntimeImage is immutable and may be shared.
RuntimeState is mutable and is never shared across runtimes.
```

This gives Vela a clean path to:

```text
actor-per-runtime game server deployment
shared immutable bytecode
hot reload image distribution
per-runtime inline caches
future shared JIT machine code
strict runtime-local heap/value isolation
```

---

## Implementation Checklist

```text
[x] Move runtime code into runtime/ module directory.
[x] Add RuntimeState.
[x] Move id/globals/script_globals into RuntimeState.
[x] Add RuntimeImage.
[x] Move engine/program into RuntimeImage.
[x] Add version/profile/global-layout metadata to RuntimeImage.
[x] Make RuntimeImage::from_program_version preserve hot-reload metadata.
[x] Make runtime hot reload swap RuntimeImage instead of cloning through ProgramVersion::to_program.
[x] Add InlineCaches placeholder.
[x] Decide whether image storage is internal enum or RuntimeImpl<I> aliases.
[x] If using generics, make RuntimeImpl generic over RuntimeImageStorage.
[x] Add OwnedImage.
[x] Preserve Runtime::new.
[ ] Add SharedImage.
[ ] Add SharedRuntime type alias.
[ ] Add RuntimeImage::into_shared.
[ ] Add SharedRuntime::from_shared_image.
[ ] Add tests for shared image and isolated state.
[ ] Add tests for VelaValue/VelaFunction/VelaMethod runtime mismatch.
[x] Add tests for reload image swaps, rejected reloads, and global layout rebinding.
[x] Add RuntimeState::rebind_to_image.
[ ] Add cache-site ID types.
[ ] Add cache-site metadata to bytecode/compiler.
[ ] Thread InlineCaches through VM execution.
[ ] Implement first per-runtime IC.
[ ] Replace MakeClosure Box<CodeObject> with FunctionIndex.
[ ] Introduce ProgramImage.
[ ] Move hot reload to ProgramImage-native version swapping.
[ ] Document JIT ownership ABI; defer JIT runtime types until M22.
```

---

## Bottom Line

The clean architecture is:

```text
Runtime
  default
  simple embedding
  no Arc<RuntimeImage>

SharedRuntime
  opt-in
  Arc<RuntimeImage>
  ideal for actor-per-runtime game servers

RuntimeImage
  immutable code and metadata

RuntimeState
  per-actor mutable state, heap, globals, roots, and inline caches

JIT later
  may share machine code by image/version
  must keep mutable IC/deopt/profiling state per runtime unless explicitly choosing another policy
```

This design gives Vela a clean long-term foundation rather than only patching memory duplication.
