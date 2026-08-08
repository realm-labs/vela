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
field access extends the target plan instead of cloning parent structures.
Ordinary field reads and writes use the narrow `HostValue` vocabulary at the
boundary: unit, bool, char, explicit scalar primitives such as `i64`, `u32`,
`f32`, and `f64`, string, bytes, and handles. A schema-declared owned-value
field may additionally carry one complete record, enum, tuple, or collection
replacement as `HostValue::Detached`. The VM serializes that script-owned value
without retaining a heap object, and the typed adapter validates and decodes
its declared shape. This explicit replacement path is distinct from the
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
clone it into `HostValue`, modify the clone, and write it back. Detached
whole-field replacement does not synthesize collection mutation by reading,
copying, and writing an existing host collection. Element and structural
collection mutations still require adapter-defined write-through operations.

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

    fn snapshot_collection_host(
        &self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        projection: HostCollectionProjection,
    ) -> HostResult<HostCollectionSnapshot>;

    fn mutate_collection_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        mutation: HostCollectionMutation<'_>,
    ) -> HostResult<()>;

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

Direct call-boundary objects implement erased methods through
`ScriptHostObject::call_resolved_host(ResolvedHostAccess,
HostTargetInstance, HostMethodId, &[HostCallValue])`. `HostCallValue` is a
detached structural boundary: it preserves scalars, HostRefs, tuples,
collections, records, and enums without storing a VM heap handle or requiring
the concrete Host object to be `'static`. The engine's typed helpers decode and
encode derived Rust Value types through the same `FromScriptArg` and
`IntoScriptArg` contracts used by registered native thunks. Field/path
operations continue to use `HostValue`; fields with an explicitly declared
owned-value shape wrap the same detached vocabulary in `HostValue::Detached`.
Passing the receiver target instance is required for child methods such as
`player.inventory.add("gold", 10)` and trait-object fields whose callable
surface lives behind a nested host target.

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
args: detached HostCallValue values decoded into typed Rust arguments
```

`HostCallValue` accepts tuples, arrays, maps, sets, records, enums, HostRefs,
and scalar values. Closures, runtime iterators, ranges, and PathProxies remain
runtime-managed capabilities and fail closed at this detached method boundary.

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
roots write through immediately. Dynamic Map keys cross as
`HostCollectionKey`, which preserves bool, char, exact-width signed/unsigned
integers, String, Bytes, and HostRef identity instead of formatting them into
path strings. Standard Rust map keys implement `ScriptHostKey` against that
exact representation; a user key type may implement the same conversion
contract. Arrays interpret an exact `i64` key as a checked position. Diagnostic
`HostPath` labels remain strings, but they are not the operational key format.
Read-only `MapView.has/get/get_or` and `SetView.has` derive their semantics from
the same resolved keyed HostAccess read. A missing map entry has the distinct
`MissingCollectionEntry` error kind; only this error becomes `false`,
`Option::None`, or the supplied fallback. Permission, stale-generation, value
projection, and adapter errors continue to propagate. No Rust adapter receives
or switches on Vela standard-library method IDs.
Growable `MapMut.set` uses the keyed HostAccess write and may construct a new
leaf through `ScriptHostFieldAccess::from_host_collection_value`; ordinary
index assignment uses the same insertion path. `MapMut.remove` first reads the
keyed value, removes through `HostAccessOp::Remove`, and returns the captured
value as `Option<V>`; an absent key returns `None`. `SetMut.add/remove` model
membership as keyed boolean reads and writes, preserving their changed/not
changed return value without materializing the set. Types that cannot be
constructed from a scalar `HostValue` fail closed instead of cloning a complex
Rust value through the script heap.
Borrowed Array `iter/values`, Map `keys/values/entries/iter`, and Set
`values/iter` use prepared call-scoped host iterators. Iterator construction
freezes the Array extent or deterministic Map/Set key order; each poll then
performs one prepared live read through HostAccess, revalidating the root and
lease. Later value replacement is visible, structural growth is outside the
frozen traversal, and removal of a pending indexed/keyed item fails instead of
silently substituting a stale value. Read-only collection callbacks, Array and
Map `group_by`, and `Iterator.fold` use the same resumable path and charge only
consumed items.
Host-backed iterators cannot escape their root call. Full bounded
`HostCollectionProjection` snapshots remain for operations whose contract
requires detached input, stable ordering, or transactional write-back, such as
sorting, collection transforms, algebra, merge, extend sources, and retain.
Growable borrowed collection `clear` uses one semantic
`HostCollectionMutation::Clear` write rather than Vela method IDs or
per-element boundary calls. The VM reads the collection length and charges its
execution cost before invoking the mutation, so a budget failure leaves the
host collection unchanged. Vec, Map, Set, and user-defined adapters share this
protocol while HostAccess still enforces write capability and immediate
write-through.
Growable `extend` uses the same boundary as one semantic request, never a VM
loop of dynamic host calls. `ExtendSequence`, `ExtendMap`, and `ExtendSet`
borrow exact boundary batches only for the duration of one HostAccess call.
The VM converts a script-owned source and charges one execution unit per input
before mutation. Standard Rust adapters then convert the complete batch to
the target element/key/value types before applying it, so conversion and
budget failures cannot partially change host state. Map replacement and Set
uniqueness follow the concrete Rust container semantics; the adapter cannot
retain the borrowed request.
Prepared plans replace the initial per-operation root plan as S3 advances.

Concrete Rust `[T; N]` uses one stable binding identity that includes `N`.
Borrowed `&[T; N]` and `&mut [T; N]` cross as shared and fixed mutable Array
views respectively. The fixed mutable view permits indexed replacement but
does not expose structural collection methods. Its HostRef always targets the
original Rust array; there is no mutable copy-in/copy-out. A separately owned
Vela Array remains growable, and the fixed-array value codec checks the exact
length when crossing back into Rust.

Concrete Rust `[T]` uses a separate borrowed-only TypeBinding identity and is
always represented as a HostRef-backed fixed Array view. `&[T]` and
`&mut [T]` are never converted to `Vec<T>`: HostAccess reads or replaces
elements against the original slice, and a generated Rust adapter obtains only
an invocation-scoped reborrow under the active shared/exclusive lease. Because
standard `Any` cannot erase a dynamically-sized slice with a non-`'static`
lifetime, one private `vela_host` module uses lifetime-aware erased shared and
exclusive slice tokens. Each token retains the data pointer, length, concrete
Rust `TypeId`, access mode, and borrow lifetime; checked typed reconstruction
is confined to that module, and the exclusive token is consumed by mutable
downcast. This is the only reviewed unsafe boundary in `vela_host`; every
other module forbids unsafe, and an architecture test audits Rust sources
against the explicit boundary-file allowlist. Stable `InteropTypeId`, not Rust
`TypeId`, remains the external ABI identity. Vela never receives a pointer or
Rust reference: script values, GC objects, reflection values, HostRef payloads,
and persistent state retain only HostRef aliases. Returned slices retain their
parent lease and may be passed to another synchronous Rust/Vela call in the
same root call tree. A live call-scoped child must be released before any
async suspension.
Structural growth remains unavailable; `[u8]` byte-view behavior is a separate
capability decision.

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

`with_host_ref` creates a read-only handle and therefore requires a `Sync`
origin. `with_host_mut` creates a writable binding, requires only `Send`, and
its mutations write through immediately through `HostAccess`. A mutable origin
has one exclusive root lease for the whole Rust method call. A method declared
with a shared receiver receives a temporary shared view through that exclusive
root guard; it does not create a second independently borrowable root. This is
the same ownership rule as reborrowing `&T` from an existing `&mut T`.
Consequently a non-`Sync`, non-`'static` call-scoped context is valid, while
concurrent method calls on the same mutable origin conflict until the active
lease drops. The guard may cross `await` only when the returned future is
`Send`, and RAII releases it on completion, error, cancellation, or unwind.
Hosts that
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
#[vela(path = "billing::account::Account", fields)]
pub struct Account {
    pub balance: i64,

    pub status: String,

    pub owner: String,

    #[vela(get)]
    pub ledger: Ledger,

    #[vela(deref)]
    pub inventory: Tracked<Inventory>,
}
```

The public macro contract is the script-facing stable path plus optional
`alias` values for compatible Rust or script-facing renames. Numeric IDs remain
runtime handles, but host authors do not choose them in derive/function macros.
`#[vela(fields)]` exposes every named field with read/write access by default;
field-level `get`, `set`, `skip`, naming, permission, and metadata attributes
override that default. `#[vela(deref)]` exposes a one-argument storage wrapper
as its `Deref::Target`: reads call `Deref::deref`, nested writes call
`DerefMut::deref_mut`, and replacing the wrapper itself is forbidden. This is
the persistence-wrapper contract used by `Tracked<T>`-style actor state.

### Method Exposure

```rust
#[vela_macros::methods]
impl Account {
    #[vela(effect = "write_host")]
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
calling Runtime's host arena and returns only a `HostRef`. Every Host
constructor declares `HostConstructionLifetime::CallScoped` or
`HostConstructionLifetime::RuntimeOwned`; the choice participates in the type
ABI and is exported through reflection, compile-view, analysis, and schema
facts. The former is reclaimed deterministically when the root call ends and
cannot cross root return, persistent-state, or async-suspension boundaries.
The latter remains available across Runtime calls until Runtime drop. The
arena is actor-local Runtime state and supports the same shared/exclusive lease
boundary as other host roots; script values may keep or pass permitted handles
but never own the Rust allocation.

For structural DTOs, `#[derive(Value)]` generates the exact `ScriptStruct` or
`ScriptEnum` descriptor, stable field and variant IDs, direct
`IntoScriptArg`/`FromScriptArg` lowering, and `vela_type_binding()`. Named Rust
struct fields and enum variants participate by default. Unit structs encode as
nominal zero-field Records; enums support unit and named-field variants, while
tuple structs and tuple variants are rejected until they have one explicit
structural ABI. `#[vela(name = "...")]` changes a public name and
`alias` preserves stable identity. Fields and variants cannot be skipped
because decoding and encoding must cover the exact Rust value; hosts with
partial/private representations use a manual `ValueCodec`. The generated
binding still enters the ordinary `TypeBinding` registry and does not create a
macro-specific registry. Unless a field has an explicit `#[vela(type =
"...")]` override, its reflected type hint comes from the field's
`VelaValueBoundary`; Rust import spelling therefore cannot leak an unresolved
short name into the sealed script schema.

`TypeRegistration<T>` is the single public type-registration object for both
owned Values and Rust-owned Hosts. `T::vela_type()` recursively installs
the exact concrete standard containers and derived Value field/variant types
reachable from `T`. Derived Hosts likewise register every exposed nested Host
type, peeling standard container wrappers and deref projections, so registering
an actor root installs its complete script-visible state graph. Shared dependencies are
idempotent only when their Rust `TypeId`, stable binding key, and complete
pending ABI fingerprint agree; a different manual binding for the same Rust
type remains a sealing error. This is Rust-side monomorphized registration of
concrete ABI entries, not Vela
generics. `TypeRegistration::binding(binding)` is the manual construction path
for external types and custom codecs; generated service bundles combine those
explicit leaves with the same recursive owned-Value closure.

An arbitrary concrete Rust type can also use the same registry without
implementing `ScriptHostObject`, deriving a Vela trait, or adding a business
newtype. `TypeRegistration::<T>::host("module::Type")` installs an opaque Host
identity, and `MethodRegistration::<T>::shared`/`exclusive` attach only the
methods selected by the embedding. A derived parent projects such a field with
`#[vela(host = "module::Type")]`; generated code stores an internal erased
call-scoped wrapper whose `Any` payload is the exact `T`, so method adapters
recover `&T` or `&mut T` after the ordinary lease checks. The wrapper exposes
no implicit fields, collection protocol, constructors, or methods. Generic
Rust containers are registered one concrete monomorphization at a time, while
Vela still sees a normal non-generic Host type.

An embedding may also expose one Rust generic operation through one
`MethodsRegistration<T>`. Its internal monomorphized family publishes exactly one ordinary Host
method with one `Any` parameter and installs one Rust-monomorphized adapter for
each accepted nominal `Record` or `Enum` type. Runtime selection uses the
argument's stable Vela type path and the selected adapter performs the ordinary
generated decode before calling Rust. Re-registering the same concrete type is
idempotent, so an application protocol registry can remain the only list of
accepted message types even when a type has several handlers. This is an
embedding registration facility, not script generics or overload resolution;
containers without retained nominal element identity are deliberately rejected.

Registered structural types used by a linked program are emitted into its
nominal descriptor table. Every Rust-owned argument and every sync or async
native result is materialized against that table before script execution, so
record shape checks and enum `match` use the same generation-local
`TypeId`/`VariantId` identity as script constructors.

For host-owned objects, `#[derive(ScriptHost)]` implements `VelaType` and the
Host object/field-access contracts. Registering the type therefore needs no
empty method impl or Host-specific builder method. The object itself still
enters execution only through a call-scoped or Runtime-owned `HostRef`.

Rust callables use `#[vela_macros::export]`, `#[vela_macros::export_module]`,
or `#[vela_macros::methods]`. These macros produce `ModuleRegistration` or
`MethodRegistration<T>`/`MethodsRegistration<T>` values. Applications collect them with their
`TypeRegistration<T>` values in one `VelaBindings`, then install that set once
through `EngineBuilder::register_bindings`. Low-level descriptor constructors
remain framework hooks rather than a parallel embedding API. Handwritten
opaque-Host methods use `registered_host_method_desc` to derive the same stable
owner and method identities as generated methods.
Engine internals and tests that need explicit IDs may still use those
low-level constructors:

```rust
let engine = Engine::builder()
    .register_native_fn(
        NativeFunctionDesc::new("audit::log", NativeFunctionId(10_001)),
        audit_log,
    )?
    .build()?;
```

The unified Rust/Vela interop hard switch replaces those shape-specific
authoring macros with item-level `#[vela_macros::export]`, explicit
`#[vela_macros::export_module]` groups for many free functions, and
`#[vela_macros::methods]` groups for inherent methods. An export module treats its
supported immediate public functions as the approved surface, derives paths
from one configured prefix, and generates one deterministic `vela_module()`
registration. Engine installs it as part of `VelaBindings`; there is
no ambient inventory, linker-section discovery, or runtime source scan.
An unsupported public item inside an explicit export group is a declaration
error rather than a silently omitted export; private helpers remain Rust-only.

Signature inference supplies ordinary `pure`/`host_read`/`host_write` cases.
Only exceptional effects use an additive identifier list such as
`#[vela_macros::export(effects(random, event_emit))]`. Module-wide default effects are
not supported because they silently overgrant unrelated functions. The final
normalized set, rather than annotation spelling, is callable ABI.
The unified export path does not reuse the older shape-specific macro fallback
that interprets every omitted effect as `pure`; omission means use the inferred
signature base.

An inherent `#[vela_macros::methods]` item may attach audited, non-ABI metadata with
`#[vela(attr = "key=value")]`. The generated `CallableContract` and
sealed `NativeMethodDesc` retain the same entry; duplicate keys are rejected.
These attributes support inventory and integration discovery only and never
grant effects, capabilities, or reflection access.

Applying `#[vela_macros::methods]` is the method-group opt-in. Its `pub`,
`pub(crate)`, and `pub(super)` methods are exported automatically. A private
method is exported only when it carries `#[vela(...)]`; `#[vela(skip)]` keeps
any method Rust-only.

### Rust Signature Mapping

Native functions should use narrow conversion rules:

```text
Rust bool/char/i8..i64/u8..u64/f32/f64/String/Vec<u8>
                                     <-> Vela bool/char/scalars/string/bytes
Option<T> in Rust API             <-> Vela Option::Some(value) or Option::None
Option<&T> in generated exports   <-> optional read-only call-scoped child HostRef
Result<&T, E> in generated exports <-> fallible read-only child or owned E
Vec<T> / HashMap<K, V> copies      <-> script array/map values
HostRef<T>                         <-> host object reference
&T / &mut T in generated exports  <-> invocation-scoped shared/exclusive host lease
&mut NativeCallContext             -> explicit host service and HostAccess access
HostResult<T>                      -> Vela call success or diagnostic error
```

`Vec<u8>` is the concrete byte-buffer exception to the general `Vec<T>` array
mapping. Generated `Value` and `ScriptHost` schemas therefore advertise it as
`Bytes`, including nested shapes such as `Vec<Vec<u8>>` as `Array<Bytes>`, so
compiler type checking and runtime encoding use the same representation.

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

A synchronous generated function or method may return `Option<&T>` or
`Result<&T, E>` when `T` has a registered host-backed binding and `E` has an
owned Value conversion. `Some`/`Ok` follows the existing direct
borrowed-return path: it creates one read-only child HostRef, retains the exact
owner/root lease, and preserves generation and borrow provenance without
copying or serializing `T`. `None`/`Err` create no HostRef or borrow lease;
`Err(E)` lowers only the owned error value.
Provenance must be statically unique: an inherent method may use its receiver,
and a free function may use exactly one borrowed host parameter. Missing or
ambiguous sources and shared-to-exclusive upgrades are macro errors.

These enveloped children are valid only inside the current synchronous root
call tree. The VM recursively rejects them in persistent state, closure
captures that escape through state or the root result, root returns, and live
frame values at an async suspension. Authored strict `host::release` and
idempotent `host::try_release` are the only early-release operations, and
unconditional root cleanup is the safety backstop; liveness and lexical scope
never release a child. `try_release` returns `false` only for a group already
released in the same root and preserves every other Host error.
Stale-generation checks use the same machinery as direct borrowed returns.
Async exported Rust functions and methods cannot declare call-scoped borrowed
returns. Dynamic and reflected dispatch invoke the same generated thunk and the
same boundary validators as static dispatch.

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

Host type methods are declared through `#[vela_macros::methods]`, collected in
`Type::vela_methods()`, attached to `Type::vela_type()` through a typed
`VelaBindings` handle, and installed with the rest of the application binding
set.
Authored methods use ordinary `&self` and `&mut self`; generated adapters
perform HostRef validation and lease acquisition before invoking them.

```rust
#[vela_macros::methods]
impl Ledger {
    pub fn add(&mut self, code: String, amount: i64) {
        self.entries.push((code, amount));
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
