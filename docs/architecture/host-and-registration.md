## Host State Bridge

The host state bridge is the central differentiator. Scripts must not receive real mutable Rust references.

Wrong direction:

```rust
&mut Account
```

Correct direction:

```rust
HostRef<Account>
HostTargetPlan<Account.balance>
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
HostMutate(target=account.balance, op=Add, rhs=1)
HostWrite(target=account.status, value="preferred")
HostCall(target=account.ledger, method=add, args=["credit", 100])
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

### Host Targets

```rust
pub struct HostTargetPlan {
    pub root_type: HostTypeId,
    pub parts: HostPathParts,
}

pub enum HostPathPart {
    Field(FieldId),
    VariantField(FieldId),
    ConstIndex(u32),
    ConstKey(String),
    DynIndex { arg: u8 },
    DynKey { arg: u8 },
}

pub struct HostTargetInstance<'a> {
    pub root: HostRef,
    pub plan: &'a HostTargetPlan,
    pub args: &'a [HostPathArg<'a>],
}
```

Compiled bytecode stores interned `HostTargetPlan` values. Runtime execution
combines a plan with the current root `HostRef` and any dynamic index/key
arguments to form a `HostTargetInstance`.

`HostPath` remains available for diagnostics, reflection inspection, mock
fixture setup, and embedding APIs that intentionally materialize a readable
path. It is not the hot adapter API.

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
objects stay behind `HostRef` roots and compiled `HostTargetPlan` shapes; child
field access extends the target plan instead of cloning parent structures. Host
field reads and writes use scalar `HostValue` conversion at the boundary: unit,
bool, char, explicit scalar primitives such as `i64`, `u32`, `f32`, and `f64`,
string, bytes, and handles. Complex script-owned records, arrays, maps, and
enums cross via the explicit owned-value serialization path, not the
high-frequency host handle path.

Scripts observe writes made earlier in the same call because writes mutate the
adapter immediately:

```rust
account.balance = 10
print(account.balance) // prints 10
```

Read logic:

```text
read(target):
    resolve HostAccessSpec(Read, target.plan) to ResolvedHostAccess
    validate generation and read permission
    return current adapter value
```

Write logic:

```text
write(target, value):
    resolve HostAccessSpec(Write, target.plan) to ResolvedHostAccess
    validate access
    write adapter immediately
```

If a later script operation traps, previous host writes are retained.

### Read-Modify-Write

`account.balance += 1` reads the current adapter value, computes the scalar
result, and writes the adapter. This keeps permissions and source-spanned
diagnostics in one host access boundary without retaining a growing journal.

Dynamic host indexes and keys are passed as explicit `HostPathArg` values, so
adapters can resolve them without depending on VM-internal symbol tables.
Collection-shaped host mutations must be adapter-defined write-through
operations. The default host boundary must not read a complex host collection,
clone it into `HostValue`, modify the clone, and write it back. Scalar-only
`HostValue` conversion cannot synthesize collection mutation by copying arrays
or maps.

### Host State Adapter

```rust
pub trait ScriptStateAdapter {
    fn host_schema_epoch(&self) -> HostSchemaEpoch;

    fn extern_state_ref(&self, state: ExternStateBinding<'_>) -> HostResult<HostRef>;

    fn resolve_host_access(&self, spec: HostAccessSpec<'_>) -> HostResult<ResolvedHostAccess>;

    fn read_host(&self, access: ResolvedHostAccess, target: HostTargetInstance<'_>)
        -> HostResult<HostValue>;

    fn query_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        query: HostCollectionQuery,
    ) -> HostResult<HostValue>;

    fn write_host(&mut self, access: ResolvedHostAccess, target: HostTargetInstance<'_>, value: HostValue)
        -> HostResult<()>;

    fn mutate_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        op: HostMutationOp,
        rhs: HostValue,
    ) -> HostResult<()>;

    fn remove_host(&mut self, access: ResolvedHostAccess, target: HostTargetInstance<'_>)
        -> HostResult<()>;

    fn call_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
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

### Runtime State

Persistent cross-call state has two explicit declaration forms:

```vela
state cache: Cache = Cache { hits: 0 };
pub extern state server: ServerState;
```

`state` creates one VM-owned cell per `Runtime`. Its explicit initializer runs
once during construction, or when that state is first added by hot reload.
The restricted initializer may construct script-managed values but cannot read
state or extern state and cannot call native, host, provider, reflection,
capability, IO, event, time, random, or async surfaces. Construction publishes
no cells unless every initializer succeeds. Rust may inspect or replace a VM
cell through `state`, `state_as`, `set_state`, and `update_state`; values remain
script-GC roots owned by that Runtime.

Rust replacement resolves exact canonical qualified type names and performs
linked-aware recursive canonicalization before insertion. Qualified names do
not fall back to leaf matching; an unqualified name is valid only when it has
one permitted linked candidate. Record fields and enum variants/payloads must
match their linked descriptors, and accepted values receive canonical runtime
identities so Vela field access, guards, and pattern matching remain nominal.

`extern state` declares a host-owned root and never allocates a script value.
The host binds it with `RuntimeBuilder::bind_extern_state` before construction,
or replaces/stages a binding through `replace_extern_state` and
`stage_extern_state`. Reads produce a `HostRef`; nested reads, writes, methods,
and keyed paths use `HostPath`, `PathProxy`, and write-through `HostAccess`.
Vela cannot replace the extern root, and Rust state never enters the script GC.
Persistent bindings must be `Send` because a Runtime may move between worker
threads.

Both forms use a stable package/module/name-derived `StateId`; dense
`StateSlot` operands belong to one executable generation. Exact-compatible
reload preserves existing VM cells and extern bindings without rerunning an
initializer. Storage or type changes reject, rename is remove plus add, and an
old generation retains removed cells until its final frame, closure, value, or
suspended execution owner is gone.

For ordinary returned script values, `Runtime::call` yields a runtime-managed
`VelaValue`. It can be passed back to the same runtime without materialization;
`value_to_owned` creates a detached boundary copy, and `from_value` performs
typed deserialization when serde support is enabled.

Direct call-boundary objects implement the same method shape through
`ScriptHostObject::call_resolved_host(ResolvedHostAccess, HostTargetInstance,
HostMethodId, &[HostValue])`. Passing the receiver target instance is required
for child methods such as `player.inventory.add("gold", 10)` and trait-object
fields whose callable surface lives behind a nested host target.

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
receiver: HostTargetInstance
access: ResolvedHostAccess
method_id: HostMethodId
args: scalar HostValue values or typed script-owned arguments
```

The VM does not special-case a concrete Rust collection family. Standard
collection calls are lowered to semantic host protocols such as
`HostCollectionQuery`, and the adapter or direct host object implements that
protocol for its concrete type. User-defined collections enter through the
same protocol surface. Non-protocol host methods continue through the
registered method thunk.

Indexing is a capability of the receiver type, not a map-only API. `obj[key]`
is represented as a keyed host path segment or by an adapter-defined index
operation when the type schema declares index support. Missing support should
be diagnosed as unsupported index access once the compiler has enough receiver
type facts; dynamic fallback remains a runtime adapter error.

Borrowed collection facts use the ordinary index syntax and bytecode shape.
When the receiver value is a `HostRef`, the VM routes the operation through a
root-local `HostTargetPlan` and HostAccess instead of treating the handle as a
script-owned Array or Map. Shared roots may read but cannot write; exclusive
roots write through immediately. Prepared plans and the broader typed map-key
protocol replace the initial per-operation root plan as S3 advances.

Low-level Rust native methods may use typed handles such as `HostRef` and
`PathProxy`. The approved ordinary interop target instead permits authored
`&T`/`&mut T` parameters when generated registration code can prove the exact
direct host object, acquire the complete shared/exclusive lease set atomically,
and keep every reference invocation-scoped. Vela still receives only host
handles; the Rust references exist solely inside the trusted native call. The
active implementation contract is defined in
[the Rust/Vela interop guide](../rust-vela-interop.md).

### Unified Callable Boundary

Explicit Rust exports and public Vela bindings share one `CallableContract`.
Its ABI contains stable semantic identity, ordered parameter names and boundary
modes, return/error mode, sync versus async shape, normalized effective
effects, and semantic visibility. Its deterministic fingerprint excludes docs,
source positions, active capability grants, execution profiles, callable and
host-type allowlists, reflection permissions, budgets, filesystem policy, and
arbitrary business permission strings. Hot reload and generated bindings use a
field-level ABI diff when fingerprints disagree.

Rust signatures infer their base effects from the shared parameter classifier:
value-only and read-only value-borrow parameters are `pure`, `&T`/`&self` host
parameters infer `host_read`, and any `&mut T`/`&mut self` infers `host_write`.
`effects(...)` may add time, random, event, IO, reflection, or other host
effects but cannot remove the inferred base. One canonical fixed-bit mapping
derives the coarse `CapabilitySet`; callers do not author a second capability
list.

An exported Rust body is trusted native code. Before it starts, the runtime
must validate callable registration and visibility, the effect-derived coarse
capabilities, budget entry, argument ABI, exact concrete host identity, and the
complete atomic shared/exclusive lease set. Once an exclusive lease safely
creates invocation-scoped `&mut T`, ordinary Rust field authority applies for
that call. Script field metadata does not sandbox statements inside the Rust
body. Deployments needing a narrower native sandbox should expose fewer
callables or opt a specific advanced callable into low-level `HostAccess`; the
ordinary reference surface does not become a proxy API.

Rust traits enter Vela only through an explicit protocol export with its own
stable Vela public path. Rust trait paths and `TypeId` values are not public
protocol identity, and implementing a Rust trait alone exposes no methods.
Inherent and selected trait methods use the same parameter classifier,
effects, leases, callable ABI, and normal Vela method syntax.

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
`HostTargetInstance` scalar fields. Scripts can copy handles, pass them to
closures, and mutate aliases inside the same call; they still never receive
real `&T` or `&mut T`.

`with_host_ref` creates a read-only handle. `with_host_mut` creates a writable
`Send + Sync` binding whose mutations write through immediately through
`HostAccess`. Its direct lease slot has exact `available`, `shared(n)`, and
`exclusive` states: shared async methods may coexist and leave parent reads
available, while mutation conflicts until every shared lease drops. Exclusive
leases block both reads and writes through the parent handle. Non-`Sync` and
opaque mutable origins fail closed instead of being represented by a stronger
lease kind. Hosts that
already store state behind their own adapter should pass existing handles with
`with_host_handle` and attach that adapter to the same `CallArgs` with
`with_fallback_adapter`. Runtime consumes the arguments into one
execution-owned `ExecutionHost`; there is no adapter-specific execution API.
The high-level direct call result is a runtime-managed `VelaValue`; hosts
materialize it only when they need a detached Rust boundary value. Hosts that
need diagnostics should derive them from their own adapter or domain-level
instrumentation.

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
            HostValue::Scalar(ScalarValue::I64(amount)),
            None,
        )
    }
}
```

Host method implementations mutate real Rust state through the adapter
immediately. `NativeCallContext` path helpers are embedding conveniences that
materialize a diagnostic path before routing through `HostAccess` and the
resolved target adapter API. The VM-facing callable receives `HostRef`,
`PathProxy`, target instances, or copied scalar values rather than `&mut self`.

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
module function       log("message")
module function       math::clamp(value, min, max)
host type method      account.ledger.add(code, amount)
```

All three shapes must become registry entries with stable IDs, signatures,
effects, access metadata, docs, and conversion rules. Scripts call them
normally, but the VM dispatches them through a native function table and checks
effective effects against the engine capability profile.

An ordinary Rust export publishes one domain-neutral `EffectSet` formed from
its signature-inferred base plus explicit additional effects. `&T`/`&self`
host borrows infer `host_read`; `&mut T`/`&mut self` infer `host_write`;
value-only signatures infer `pure`. Its required `CapabilitySet` is derived by
the canonical effect-to-capability mapping. Native export attributes and
descriptors do not accept arbitrary business permission strings.
`FunctionAccess` records semantic public/reflection access, not active
deployment grants or a callable ACL.

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
            .returns(TypeHint::unit())
            .effects(EffectSet::pure_host_log())
            .docs("Writes to the host audit log."),
        audit_log,
    )
    .register_native_fn(
        NativeFunctionDesc::new("math::clamp", NativeFunctionId(20_001))
            .param("value", TypeHint::Primitive(PrimitiveTag::F64))
            .param("min", TypeHint::Primitive(PrimitiveTag::F64))
            .param("max", TypeHint::Primitive(PrimitiveTag::F64))
            .returns(TypeHint::Primitive(PrimitiveTag::F64))
            .effects(EffectSet::pure()),
        math_clamp,
    )
    .build()?;
```

Rust type constructors are authored on the same `TypeBinding<T>` that owns the
type identity. `TypeBinding::constructor_fn` associates a constructor with the
type, while execution, capability checks, reflection metadata, and compiler
resolution reuse the ordinary native-function registry. Constructor names are
direct children of the type path such as `host::Widget::new`, and their exact
function IDs are projected through the sealed type-binding facts. Registering
one derives the binding's `construct` capability; hosts do not set that bit
independently.

Value constructors return the exact registered record or enum representation.
Their IDs, names, parameters, result, asyncness, effects, and access shape
participate in `TypeAbiFingerprint`; documentation, source spans, and Rust
closure code do not. Host-object construction additionally requires a
host-owned factory/arena and must never place the Rust object under script GC.
`TypeBinding::host_constructor_fn` transfers the exact Rust result into the
calling Runtime's owned-host arena and returns only a `HostRef`. The arena is
actor-local Runtime state, supports the same shared/exclusive lease boundary as
other host roots, and currently retains constructed objects until Runtime drop;
script values may keep or pass handles but never own the Rust allocation.

For structural DTOs, `#[derive(Value)]` generates the exact `ScriptStruct` or
`ScriptEnum` descriptor, stable field and variant IDs, direct
`IntoScriptArg`/`FromScriptArg` lowering, and `vela_type_binding()`. Named Rust
struct fields and enum variants participate by default; enums support unit and
named-field variants, while tuple variants are rejected until they have one
explicit structural ABI. `#[script(name = "...")]` changes a public name and
`alias` preserves stable identity. Fields and variants cannot be skipped
because decoding and encoding must cover the exact Rust value; hosts with
partial/private representations use a manual `ValueCodec`. The generated
binding still enters the ordinary `TypeBinding` registry and does not create a
macro-specific registry.

`RustValueType` is the generated registration-closure contract for owned
values. `EngineBuilder::register_rust_value_closure::<T>()` recursively installs
the exact concrete standard containers and derived Value field/variant types
reachable from `T`, then installs `T` itself. Shared dependencies are
idempotent only when their Rust `TypeId`, stable binding key, and complete
pending ABI fingerprint agree; a different manual binding for the same Rust
type remains a sealing error. This is Rust-side monomorphized registration of
concrete ABI entries, not Vela
generics. Manual `register_rust_type::<T>(binding)` remains the escape hatch for
external types and custom codecs; generated service bundles will combine those
explicit leaves with the same recursive owned-Value closure.

Registered structural types used by a linked program are emitted into its
nominal descriptor table. Every Rust-owned argument and every sync or async
native result is materialized against that table before script execution, so
record shape checks and enum `match` use the same generation-local
`TypeId`/`VariantId` identity as script constructors.

For host-owned objects, `#[derive(ScriptHost)]` emits
`vela_type_binding()` through `ScriptHostSchema::script_host_binding()`. The
binding carries the generated Host descriptor and Host storage/capabilities
into the same `register_rust_type::<T>` path as Value types; the object itself
still enters execution only through a call-scoped or Runtime-owned `HostRef`.
Generated method thunks remain separate macro output until the service bundle
composition slice folds them into the same registration transaction.

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

The unified Rust/Vela interop hard switch replaces those shape-specific
authoring macros with item-level `#[vela::export]`, explicit
`#[vela::export_module]` groups for many free functions, and
`#[vela::methods]` groups for inherent methods. An export module treats its
supported immediate public functions as the approved surface, derives paths
from one configured prefix, and generates one deterministic `vela_exports()`
bundle. Engine registers that value once through `register_exports`; there is
no ambient inventory, linker-section discovery, or runtime source scan.
An unsupported public item inside an explicit export group is a declaration
error rather than a silently omitted export; private helpers remain Rust-only.

Signature inference supplies ordinary `pure`/`host_read`/`host_write` cases.
Only exceptional effects use an additive identifier list such as
`#[vela::export(effects(random, event_emit))]`. Module-wide default effects are
not supported because they silently overgrant unrelated functions. The final
normalized set, rather than annotation spelling, is callable ABI.
The unified export path does not reuse the older shape-specific macro fallback
that interprets every omitted effect as `pure`; omission means use the inferred
signature base.

### Rust Signature Mapping

Native functions should use narrow conversion rules:

```text
Rust bool/char/i8..i64/u8..u64/f32/f64/String/Vec<u8>
                                     <-> Vela bool/char/scalars/string/bytes
Option<T> in Rust API             <-> Vela Option::Some(value) or Option::None
Vec<T> / HashMap<K, V> copies      <-> script array/map values
HostRef<T>                         <-> host object reference
&T / &mut T in generated exports  <-> invocation-scoped shared/exclusive host lease
&mut NativeCallContext             -> explicit host service and HostAccess access
HostResult<T>                      -> Vela call success or diagnostic error
```

Do not represent these Rust implementation types as Vela values:

```text
&T
&mut T
Arc<Mutex<T>>
database connection handles
network connection handles
runtime-owned service pointers
```

`&T` and `&mut T` may appear in an ordinary exported Rust signature only as
generated adapter inputs. The adapter receives a Vela `HostRef`, validates
exact type and canonical identity through the host boundary, atomically
acquires an invocation-scoped lease, and calls trusted Rust. The reference is
never visible to Vela, reflection, GC state, or persistent script storage.

If a native function needs to mutate host state, it should either:

```text
record HostAccess operations through NativeCallContext
call ScriptStateAdapter resolved target methods
receive a generated invocation-scoped &mut T after HostAccess callable and lease checks
return a value that script code later writes through normal HostAccess paths
```

The generated `&mut T` path is callable-grained trusted native authority. It
does not promise field-level sandboxing inside the Rust body. Direct Vela
field/index/path mutations retain their fine-grained `HostAccess` policy; a
future stronger native sandbox may opt specific functions into the low-level
HostAccess API without changing the default ordinary-signature model.
Reflection member `required_permissions` remain reflection tooling/policy
metadata and must not be reused as authorization for this ordinary native-call
path.

### Method Registration

Host type methods are registered through `#[script_methods]` and become
`MethodDesc { kind: MethodKind::HostNative(...) }`. Method calls receive the
receiver as a `HostTargetInstance`, `PathProxy`, or host ref, not as `&mut T`
in the VM.

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
            vec![
                HostValue::String(code),
                HostValue::Scalar(ScalarValue::I64(amount)),
            ],
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
signature classification infers pure/host_read/host_write base effects
explicit effect lists may add to but never remove the inferred base
the normalized effective effect set is fixed before registration
coarse capability requirements are derived from effects
active ExecutionProfile grants and allowlists are deployment policy, not callable ABI
capability checks happen before effectful native call dispatch
context operations and nested bindings cannot exceed the current callable effect ceiling
native-call authorization does not perform arbitrary business-string lookups
native calls consume execution budget
native functions cannot store Value or HostRef beyond the call unless explicitly allowed
native functions cannot mutate TypeRegistry at runtime
reflection can call only reflect_callable native functions
hot reload can replace script functions, but host native function ABI is fixed for the engine version
```

## Provider Runtime Boundary

Provider discovery is an optional metadata projection over the same sealed
package snapshot used by ordinary compilation. Engine selects full stable
`ProviderKey` values and the linker seals only those selections into the linked
artifact. Runtime provider calls resolve `MethodId` to linked script dispatch,
construct a fresh zero-field script receiver, and use the ordinary VM,
HostAccess, capability, budget, GC-root, and profiling boundaries. Provider
lookup policy does not belong in the core VM API.
