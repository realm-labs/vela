## Host State Bridge

The host state bridge is the central differentiator. Scripts must not receive real mutable Rust references.

Wrong direction:

```rust
&mut Player
```

Correct direction:

```rust
HostRef<Player>
PathProxy<Player.level>
PatchTx
```

Script code looks natural:

```rust
player.level += 1
player.exp = 0
player.inventory.add("gold", 100)
```

Runtime operations are explicit:

```text
ReadModifyWrite(player.level, Add(1))
Set(player.exp, 0)
CallHostMethod(player.inventory, add, ["gold", 100])
```

### HostRef

```rust
pub struct HostRef {
    pub type_id: HostTypeId,
    pub object_id: HostObjectId,
    pub generation: u32,
}
```

`generation` prevents stale references from writing to a new object after ID reuse.

### HostPath

```rust
pub struct HostPath {
    pub root: HostRef,
    pub segments: Vec<PathSegment>,
}

pub enum PathSegment {
    Field(FieldId),
    Index(u32),
    Key(Symbol),
    VariantField(FieldId),
}
```

### PatchTx

```rust
pub struct PatchTx {
    pub patches: Vec<Patch>,
    pub overlay: PatchOverlay,
}

pub struct Patch {
    pub path: HostPath,
    pub op: PatchOp,
    pub expected_base: Option<HostValue>,
    pub source_span: Option<Span>,
}

pub enum PatchOp {
    Set(HostValue),
    Add(HostValue),
    Sub(HostValue),
    Mul(HostValue),
    Div(HostValue),
    Rem(HostValue),
    Remove,
    Push(HostValue),
    CallHostMethod {
        method: HostMethodId,
        args: Vec<HostValue>,
    },
}
```

### Read And Write Semantics

Scripts must observe writes made earlier in the same transaction:

```rust
player.level = 10
print(player.level) // prints 10
```

Read logic:

```text
read_path(path):
    if tx.overlay has path:
        return overlay value
    else:
        return host snapshot value
```

Write logic:

```text
write_path(path, value):
    validate access
    record patch
    update overlay
```

### Read-Modify-Write

`player.level += 1` should prefer an explicit operation:

```rust
PatchOp::Add(HostValue::Int(1))
```

This lets the host perform atomic validation, range checks, conflict handling, and logging during apply.

### Host State Adapter

```rust
pub trait ScriptStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue>;

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()>;

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;

    fn validate_patch(&self, patch: &Patch) -> HostResult<()>;

    fn apply_patch(&mut self, patch: Patch) -> HostResult<()>;
}
```

The same runtime can adapt to:

```text
plain Rust structs
ECS worlds
actor state
database entities
network-replicated state
test mock state
```

## Rust Host Macros

### Type Exposure

```rust
#[derive(ScriptHost, ScriptReflect)]
#[script(path = "game::player::Player")]
pub struct Player {
    #[script(get, set)]
    pub level: u32,

    #[script(get, set)]
    pub exp: u64,

    #[script(get, set)]
    pub title: String,

    #[script(get)]
    pub inventory: Inventory,
}
```

The public macro contract is the script-facing stable path plus optional
`alias` values for compatible Rust or script-facing renames. Numeric IDs remain
runtime handles, but host authors do not choose them in derive/function macros.

### Method Exposure

```rust
#[script_methods]
impl Player {
    #[script_method(effect = "write_host")]
    pub fn add_exp(
        ctx: &mut NativeCallContext,
        player: HostRef<Player>,
        amount: i64,
    ) -> HostResult<()> {
        ctx.tx.push_add(player.field(FieldId(2)), HostValue::Int(amount))
    }
}
```

Host method implementations may mutate real Rust state later inside the host
adapter apply path, but the VM-facing callable receives `HostRef`, `HostPath`,
or copied values rather than `&mut self`.

### Generated Items

Macros should generate at least:

```text
TypeDesc
FieldDesc list
MethodDesc list
read_field / write_field helpers
method dispatch helpers
schema_hash
path-derived stable ID validation
```

## Host Function Registration

Host functions are Rust functions registered into the Vela engine as native
callables. They are used for logging, deterministic utility APIs, event context
helpers, config access, controlled random, metrics, and host-provided services.

Native functions follow the same no-overload rule as script functions. Each
public native callable has one canonical module/name and one stable ID. Hosts
should use explicit names such as `spawn_monster` and `spawn_monster_at`
instead of registering multiple signatures under the same script-visible name.

There are three registration shapes:

```text
global function       log("message")
module function       math::clamp(value, min, max)
host type method      player.inventory.add(item_id, count)
```

All three shapes must become registry entries with stable IDs, signatures,
effects, permissions, docs, and conversion rules. Scripts call them normally,
but the VM dispatches them through a native function table.

### Native Function Descriptor

```rust
pub struct NativeFunctionDesc {
    pub id: NativeFunctionId,
    pub module: Symbol,
    pub name: Symbol,
    pub params: Vec<ParamDesc>,
    pub returns: TypeHint,
    pub effects: EffectSet,
    pub access: FunctionAccess,
    pub attrs: AttrMap,
    pub origin: DeclOrigin,
    pub docs: Option<DocString>,
}

pub struct NativeFunctionId(pub u64);

pub struct FunctionAccess {
    pub public: bool,
    pub reflect_visible: bool,
    pub reflect_callable: bool,
    pub required_permissions: PermissionSet,
}
```

Native functions are also exposed through `FunctionDesc` so reflection, hot
reload ABI checks, diagnostics, and future LSP tooling see the same function
surface as the VM.

```rust
pub enum FunctionKind {
    Script(CodeObjectId),
    HostNative(NativeFunctionId),
}

pub struct FunctionDesc {
    pub key: FunctionKey,
    pub name: Symbol,
    pub module: Symbol,
    pub params: Vec<ParamDesc>,
    pub returns: TypeHint,
    pub kind: FunctionKind,
    pub effects: EffectSet,
    pub access: FunctionAccess,
    pub attrs: AttrMap,
    pub origin: DeclOrigin,
    pub docs: Option<DocString>,
}
```

### Native Function Trait

The VM should call host functions through a small erased trait:

```rust
pub type NativeFunction =
    Arc<dyn Fn(&[OwnedValue]) -> VmResult<OwnedValue> + Send + Sync + 'static>;

pub struct NativeCallContext<'a> {
    pub runtime: &'a mut Runtime,
    pub state: &'a mut dyn ScriptStateAdapter,
    pub tx: &'a mut PatchTx,
    pub permissions: &'a PermissionSet,
    pub budget: &'a mut ExecutionBudget,
}
```

`NativeCallContext` is the only native entry point that may touch host services
or `PatchTx`. A native function must not hand real Rust references back to the
script. Returned host objects must be represented as `HostRef`, copied
host-value data, or script-owned `OwnedValue`.

The engine owns the executable native function table separately from the
reflectable descriptors:

```rust
pub struct Engine {
    pub registry: Arc<TypeRegistry>,
    pub native_functions: HashMap<NativeFunctionId, Arc<dyn NativeFunction>>,
    pub native_methods: HashMap<HostMethodId, Arc<dyn NativeFunction>>,
}
```

### Builder API

The engine builder should support explicit descriptors for stable schemas:

```rust
let engine = Engine::builder()
    .register_native_fn(
        NativeFunctionDesc::new("game::log", NativeFunctionId(10_001))
            .param("message", TypeHint::String)
            .returns(TypeHint::Null)
            .effects(EffectSet::pure_host_log())
            .docs("Writes to the game log."),
        game_log,
    )
    .register_native_fn(
        NativeFunctionDesc::new("math::clamp", NativeFunctionId(20_001))
            .param("value", TypeHint::Float)
            .param("min", TypeHint::Float)
            .param("max", TypeHint::Float)
            .returns(TypeHint::Float)
            .effects(EffectSet::pure()),
        math_clamp,
    )
    .build()?;
```

For macro-exposed functions, `#[script_function]`,
`#[script_context_function]`, and `#[script_host_function]` derive the native
function ID from the public `::` qualified function name and optional `alias`.
They also expose descriptor access metadata such as `public`,
`reflect_visible`, and `reflect` / `reflect_callable`, so hosts can publish
private reflection-visible admin/debug functions without making them public
script APIs or reflective call targets.
Low-level descriptor constructors remain available for engine internals and
tests that need explicit IDs:

```rust
let engine = Engine::builder()
    .register_native_fn(
        NativeFunctionDesc::new("game::log", NativeFunctionId(10_001)),
        game_log,
    )?
    .build()?;
```

### Rust Signature Mapping

Native functions should use narrow conversion rules:

```text
Rust bool/i64/f64/String          <-> Vela bool/int/float/string
Option<T> in Rust API             <-> nullable argument or return value
Vec<T> / HashMap<K, V> copies      <-> script array/map values
HostRef<T>                         <-> host object reference
&mut NativeCallContext             -> explicit host access and PatchTx access
HostResult<T>                      -> Vela call success or diagnostic error
```

Do not expose these Rust types directly to scripts:

```text
&T
&mut T
Arc<Mutex<T>>
database connection handles
network connection handles
runtime-owned service pointers
```

If a native function needs to mutate host state, it should either:

```text
record PatchTx operations through NativeCallContext
call ScriptStateAdapter methods
return a value that script code later writes through normal PatchTx paths
```

### Method Registration

Host type methods are registered through `#[script_methods]` and become
`MethodDesc { kind: MethodKind::HostNative(...) }`. Method calls receive the
receiver as a host path or host ref, not as `&mut T` in the VM.

```rust
#[script_methods]
impl Inventory {
    #[script_method(
        name = "add",
        effect = "write_host",
        docs = "Adds an item stack to this inventory."
    )]
    pub fn add(
        ctx: &mut NativeCallContext,
        inventory: HostRef<Inventory>,
        item_id: String,
        count: i64,
    ) -> HostResult<()> {
        ctx.tx.push_method_call(inventory, HostMethodId(1), vec![
            HostValue::String(item_id.into()),
            HostValue::Int(count),
        ])?;
        Ok(())
    }
}
```

This keeps method syntax ergonomic:

```rust
player.inventory.add("gold", 100)
```

while preserving the host boundary:

```text
CallHostMethod(player.inventory, add, ["gold", 100])
```

### Registration Rules

```text
function module/name/stable_id must be unique
function overloading is unsupported; duplicate script-visible names are invalid
registered signatures must be deterministic and serializable into TypeRegistry
effects must be declared up front
permission checks happen before native call dispatch
native calls consume execution budget
native functions cannot store Value or HostRef beyond the call unless explicitly allowed
native functions cannot mutate TypeRegistry at runtime
reflection can call only reflect_callable native functions
hot reload can replace script functions, but host native function ABI is fixed for the engine version
```

