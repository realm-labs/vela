## Host State Bridge

The host state bridge is the central differentiator. Scripts must not receive real mutable Rust references.

Wrong direction:

```rust
&mut Account
```

Correct direction:

```rust
HostRef<Account>
PathProxy<Account.balance>
HostAccess
```

Script code looks natural:

```rust
account.balance += 1
account.status = "preferred"
account.ledger.add("credit", 100)
```

Runtime operations are explicit:

```text
ReadModifyWrite(account.balance, Add(1))
Set(account.status, "preferred")
CallHostMethod(account.ledger, add, ["credit", 100])
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
    Key(String),
    VariantField(FieldId),
}
```

### HostAccess

```rust
pub struct HostAccess;
```

`HostAccess` is a call-scoped access context. It is not a transaction, journal,
or rollback container. It routes reads, writes, removals, compound scalar
writes, and method calls to the adapter immediately.

### Read And Write Semantics

Host handles are call-scope references to Rust-owned state. Complex Rust
objects stay behind `HostRef` and `HostPath`; child field access appends path
segments instead of cloning parent structures. Host field reads and writes use
scalar `HostValue` conversion at the boundary: null, bool, int, float, string,
and handles. Complex script-owned records, arrays, maps, and enums cross via
the explicit owned-value serialization path, not the high-frequency host
handle path.

Scripts observe writes made earlier in the same call because writes mutate the
adapter immediately:

```rust
account.balance = 10
print(account.balance) // prints 10
```

Read logic:

```text
read_path(path):
    validate generation and read permission
    return current adapter value
```

Write logic:

```text
write_path(path, value):
    validate access
    write adapter immediately
```

If a later script operation traps, previous host writes are retained.

### Read-Modify-Write

`account.balance += 1` reads the current adapter value, computes the scalar
result, and writes the adapter. This keeps permissions and source-spanned
diagnostics in one host access boundary without retaining a growing journal.

Map-like host paths keep script string keys in `PathSegment::Key(String)` so
directly injected Rust objects and generic adapters can resolve the key without
depending on VM-internal symbol tables. Collection-shaped host mutations must
be adapter-defined write-through operations. The default host boundary must not
read a complex host collection, clone it into `HostValue`, modify the clone,
and write it back. Scalar-only `HostValue` conversion cannot synthesize
collection mutation by copying arrays or maps.

### Host State Adapter

```rust
pub trait ScriptStateAdapter {
    fn read_path(&self, path: &HostPath) -> HostResult<HostValue>;

    fn write_path(&mut self, path: &HostPath, value: HostValue) -> HostResult<()>;

    fn remove_path(&mut self, path: &HostPath) -> HostResult<()>;

    fn call_method(
        &mut self,
        path: &HostPath,
        method: HostMethodId,
        args: &[HostValue],
    ) -> HostResult<HostValue>;
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

### Runtime Globals

Global state is declared as a module item and bound by the embedding host:

```vela
pub global state: ServerState
```

The declaration is ABI metadata. It does not allocate state, run an
initializer, or create top-level side effects. Rust inserts a matching runtime
global by its fully qualified declaration name, such as
`game::state::state`, before script calls that access it.

Rust-defined globals are represented as persistent host objects in the
runtime's global store. Loading a Rust-defined global produces a `HostRef`
root, and script field reads, writes, method calls, and keyed paths then use
the same `HostPath` and write-through `HostAccess` path as call-boundary host
handles. Because a `Runtime` may be moved to a worker thread, persistent host
global objects must be `Send`. Direct call-boundary `with_host_ref` and
`with_host_mut` bindings are scoped to the call and do not impose this
persistent-global requirement.

Vela-defined script-value globals use the same declaration surface but are
stored in the runtime's persistent script heap. Rust inserts or replaces them
through the single `Runtime::insert_global` / `set_global` API, which accepts
`OwnedValue`, serde values passed by reference when the `serde` feature is
enabled, and `VelaValue` handles from the same runtime. Scripts see ordinary
script records, arrays, maps, sets, enums, and scalars. These values are
VM-managed script objects, not Rust host state and not `HostRef` roots. Runtime
global roots are retained across calls and included in GC roots during calls.
Missing runtime instances are runtime errors.

For non-global script values returned from calls, Rust can choose
`Runtime::call_value` to keep the returned aggregate as a runtime-managed
`VelaValue`. That value can be passed back to later calls on the same runtime
without `OwnedValue` materialization; explicit `value_to_owned` creates a
detached Rust boundary copy when needed. With the `serde` feature enabled,
`from_value` and `global_as` deserialize runtime-managed script values directly
from the runtime heap without first constructing an `OwnedValue`.

Globals do not reintroduce module-level `let` or mutable static
initialization. A script function may construct a value and return it to Rust;
Rust can then insert that returned `VelaValue` or materialized `OwnedValue` as
the runtime instance for a declared global.

Direct call-boundary objects implement the same method shape through
`ScriptHostObject::call_host_method(&HostPath, HostMethodId, &[HostValue])`.
Passing the receiver path is required for child methods such as
`player.inventory.add("gold", 10)` and trait-object fields whose callable
surface lives behind a host path.

### Host Type Methods And Indexing

Host registration uses one concrete-type model. A host type schema contains its
script-visible type name, stable IDs, fields, methods, optional index
capability metadata, and adapter/native method thunks. `HashMap<i32, i32>`,
`HashMap<i32, Item>`, `Vec<Item>`, `HashSet<String>`, and trait-object fields
are registered as concrete named host types; scripts do not see Rust generics
and method lookup is always receiver type plus method name.

Rust-side helper functions or macros may generate repeated concrete specs for
generic Rust containers, but the generated result is still a normal host type
spec. There is no separate `host_map`, `host_set`, or `host_vec` script model.

Host method calls use a single runtime shape:

```text
receiver_path: HostPath
method_id: HostMethodId
args: scalar HostValue values or typed script-owned arguments
```

The VM does not special-case whether the receiver is a struct, map, set, vec,
or trait-object field. The adapter or direct host object resolves the receiver
path and executes the registered method thunk.

Indexing is a capability of the receiver type, not a map-only API. `obj[key]`
is represented as a keyed host path segment or by an adapter-defined index
operation when the type schema declares index support. Missing support should
be diagnosed as unsupported index access once the compiler has enough receiver
type facts; dynamic fallback remains a runtime adapter error.

Rust native methods that need another host object parameter should use typed
path wrappers such as `TypedHostRef<T>` or `TypedHostMut<T>`. These wrappers
store only `HostPath` and optional type metadata; they do not contain Rust
references. Except for receiver syntax sugar generated inside future typed
registration helpers, script-visible Rust parameters should not use bare
`&T` or `&mut T`.

### Direct Call Arguments

Embedding hosts may bind ordinary Rust values directly at the call boundary:

```rust
let args = CallArgs::new()
    .with_host_ref("config", &config)
    .with_host_mut("player", &mut player)
    .with_value("amount", 10);

let output = runtime.call("handle", args, options)?;
```

This is an embedding API convenience, not a different script value model.
`config` and `player` become call-scope `HostRef` handles inside the VM.
The Rust type implements the host object adapter surface that reads and writes
`HostPath` scalar fields. Scripts can copy handles, pass them to closures, and
mutate aliases inside the same call; they still never receive real `&T` or
`&mut T`.

`with_host_ref` creates a read-only handle. `with_host_mut` creates a writable
handle whose mutations write through immediately through `HostAccess`. Hosts that
already store state behind their own adapter should pass existing handles with
`with_host_handle` and use `runtime.call_with_adapter` with that adapter.
The high-level direct call result dereferences to the returned `OwnedValue`;
hosts that need diagnostics should derive them from their own adapter or
domain-level instrumentation.

## Rust Host Macros

### Type Exposure

```rust
#[derive(ScriptHost, ScriptReflect)]
#[script(path = "billing::account::Account")]
pub struct Account {
    #[script(get, set)]
    pub balance: i64,

    #[script(get, set)]
    pub status: String,

    #[script(get, set)]
    pub owner: String,

    #[script(get)]
    pub ledger: Ledger,
}
```

The public macro contract is the script-facing stable path plus optional
`alias` values for compatible Rust or script-facing renames. Numeric IDs remain
runtime handles, but host authors do not choose them in derive/function macros.

### Method Exposure

```rust
#[script_methods]
impl Account {
    #[script_method(effect = "write_host")]
    pub fn credit(
        ctx: &mut NativeCallContext,
        account: HostRef<Account>,
        amount: i64,
    ) -> HostResult<()> {
        ctx.add_path(
            HostPath::new(account).field(FieldId(1)),
            HostValue::Int(amount),
            None,
        )
    }
}
```

Host method implementations mutate real Rust state through the adapter
immediately. The VM-facing callable receives `HostRef`, `HostPath`, or copied
scalar values rather than `&mut self`.

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
should use explicit names such as `create_invoice` and `create_invoice_with_terms`
instead of registering multiple signatures under the same script-visible name.

There are three registration shapes:

```text
global function       log("message")
module function       math::clamp(value, min, max)
host type method      account.ledger.add(code, amount)
```

All three shapes must become registry entries with stable IDs, signatures,
effects, access metadata, docs, and conversion rules. Scripts call them
normally, but the VM dispatches them through a native function table and checks
declared effects against the engine capability profile.

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
    pub engine: &'a Engine,
    pub host: &'a mut HostExecution<'a>,
    pub access: &'a mut HostAccess,
    pub capabilities: CapabilitySet,
    pub budget: &'a mut ExecutionBudget,
}
```

`NativeCallContext` is the only native entry point that may touch host services
or `HostAccess`. A native function must not hand real Rust references back to the
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
        NativeFunctionDesc::new("audit::log", NativeFunctionId(10_001))
            .param("message", TypeHint::String)
            .returns(TypeHint::Null)
            .effects(EffectSet::pure_host_log())
            .docs("Writes to the host audit log."),
        audit_log,
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
        NativeFunctionDesc::new("audit::log", NativeFunctionId(10_001)),
        audit_log,
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
&mut NativeCallContext             -> explicit host service and HostAccess access
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
record HostAccess operations through NativeCallContext
call ScriptStateAdapter methods
return a value that script code later writes through normal HostAccess paths
```

### Method Registration

Host type methods are registered through `#[script_methods]` and become
`MethodDesc { kind: MethodKind::HostNative(...) }`. Method calls receive the
receiver as a host path or host ref, not as `&mut T` in the VM.

```rust
#[script_methods]
impl Ledger {
    #[script_method(
        name = "add",
        effect = "write_host",
        docs = "Adds an entry to this ledger."
    )]
    pub fn add(
        ctx: &mut NativeCallContext,
        ledger: HostRef<Ledger>,
        code: String,
        amount: i64,
    ) -> HostResult<()> {
        ctx.call_method(
            HostPath::new(ledger),
            HostMethodId(1),
            vec![HostValue::String(code), HostValue::Int(amount)],
            None,
        )?;
        Ok(())
    }
}
```

This keeps method syntax ergonomic:

```rust
account.ledger.add("credit", 100)
```

while preserving the host boundary:

```text
CallHostMethod(account.ledger, add, ["credit", 100])
```

### Registration Rules

```text
function module/name/stable_id must be unique
function overloading is unsupported; duplicate script-visible names are invalid
registered signatures must be deterministic and serializable into TypeRegistry
effects must be declared up front
capability checks happen before effectful native call dispatch
native calls consume execution budget
native functions cannot store Value or HostRef beyond the call unless explicitly allowed
native functions cannot mutate TypeRegistry at runtime
reflection can call only reflect_callable native functions
hot reload can replace script functions, but host native function ABI is fixed for the engine version
```
