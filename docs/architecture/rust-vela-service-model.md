# Rust/Vela Unified Service Model

This document is the normative technical contract for Rust/Vela service
authoring, immutable service generations, unified Rust type interop, generated
macros, and the callable-replacement deletion boundary.

Execution order, phase gates, current status, and validation cadence live in
[the hard-switch plan](../rust-vela-service-hard-switch-plan.md). Current
implementation truth and remaining gaps live in [progress.md](../progress.md).

## 1. Non-Negotiable Model

### 1.1 One service set, one published generation

The host owns one generated service set for a deployment domain:

```text
ArcSwap<GameServiceGeneration>
               |
               +-- inventory: Arc<dyn InventoryServiceDispatch>
               +-- reward:    Arc<dyn RewardServiceDispatch>
               +-- combat:    Arc<dyn CombatServiceDispatch>
               +-- handlers:  Arc<dyn HandlerServiceDispatch>
```

The exact generated Rust representation may use hidden object-safe dispatch
traits, especially for authored `async fn`, but the semantic unit is always a
whole immutable `ServiceGeneration`. Individual fields are never published
independently. Staging may change one method, but activation publishes the full
generation in one atomic operation. This prevents a call chain from observing
half of a multi-service patch.

The Vela core provides immutable generation construction, validation, pinning,
and rollback facts. The generated host integration publishes the generation
with `ArcSwap` or an equivalent single-pointer atomic primitive. There is no
global mutable Runtime inside the service set.

### 1.2 Pin once at the host safe point

An actor mailbox turn, request, tick, or equivalent host operation pins one
`Arc<GameServiceGeneration>` before invoking business logic. Every Rust-to-Rust,
Rust-to-Vela, Vela-to-Rust, and Vela-to-Vela service call underneath that root
uses the same generation.

```text
actor/request safe point
        |
        +-- pin generation G17
        +-- inventory.grant  -> Vela in G17
        |      +-- reward.apply -> Vela in G17
        |      +-- audit.write  -> Rust fallback in G17
        +-- root completes
        +-- release G17
```

Activation affects only roots pinned afterward. Active sync calls, suspended
async calls, nested service calls, closures, and borrowed-return lifetimes keep
their old linked artifact and service generation. Rollback republishes a prior
validated generation; it never retries or rewinds an in-flight call.

### 1.3 No per-callable interception

The generated service object is the dispatch boundary. Authored Rust method
bodies are not moved into private fallbacks and their public entries are not
instrumented. The Rust default implementation remains an ordinary trait
implementation. A generated composite service selects either:

- the staged Vela method for the pinned generation; or
- the Rust default implementation held by that generation.

Selection is prelinked by stable `ServiceId` and `ServiceMethodId`; it performs
no runtime string lookup, source parsing, reflection search, or mutable global
registry access. Missing Vela methods are resolved to Rust during staging, not
interpreted as missing-method errors during a production call.

### 1.4 Partial Vela implementation is the default

A Vela patch implements only the methods it intends to replace. Repeating
unmodified methods is forbidden as a deployment requirement because copied
fallback logic increases patch risk. Completeness is obtained by composing the
sparse Vela method table over the Rust default implementation.

A Vela error from a selected method propagates to its caller. The runtime must
not catch it and execute the Rust fallback, because the Vela body may already
have performed irreversible host writes or nested calls.

### 1.5 Explicit base and cross-service calls

Inside a Vela service method, two compiler-provided lexical bindings are
available:

- `base` calls the Rust default implementation of the current service and
  bypasses the current Vela method selection.
- `services` calls any service from the same pinned generation, including
  another Vela-patched method.

Neither binding is a global singleton, a script-storable value, or part of the
business method ABI. They are scoped capabilities created by the generated
service invocation. `base` prevents accidental recursion when a patch wants to
wrap the original Rust behavior. `services` preserves the original service call
chain and generation coherence.

### 1.6 One Rust type interaction model

Vela uses one `TypeBinding` contract for standard-library and user-defined Rust
types. `T`, `&T`, and `&mut T` share one stable `InteropTypeId` and method
catalog; only their storage and receiver capabilities differ. The binding also
owns value conversion, HostRef views, constructors, fields, indexes,
iteration, standard protocols, escape rules, effects, and ABI fingerprint.

A Rust value does not become an untyped serialized blob. A registered value
policy lowers it directly into typed Vela values, while a registered host
policy keeps the exact Rust object in a host-owned arena and exposes only a
typed handle. Rust references always become scoped shared or exclusive views.
No real Rust reference or Rust-owned object enters the script GC.

## 2. Authoring Shape

### 2.1 Rust business authoring

The intended Rust surface is:

```rust,ignore
#[vela::service(path = "game::inventory")]
pub trait InventoryService: Send + Sync {
    fn grant(
        &self,
        turn: &mut GameTurn,
        player: &mut Player,
        items: &[ItemGrant],
    ) -> GameResult<Vec<DisplayItem>>;

    fn remove(
        &self,
        turn: &mut GameTurn,
        player: &mut Player,
        item_ids: &[i64],
    ) -> GameResult<()>;
}

pub struct RustInventoryService;

impl InventoryService for RustInventoryService {
    fn grant(
        &self,
        turn: &mut GameTurn,
        player: &mut Player,
        items: &[ItemGrant],
    ) -> GameResult<Vec<DisplayItem>> {
        // Ordinary Rust business logic. No patch branch or Vela wrapper.
        todo!()
    }

    fn remove(
        &self,
        turn: &mut GameTurn,
        player: &mut Player,
        item_ids: &[i64],
    ) -> GameResult<()> {
        todo!()
    }
}

#[vela::service_set(context = GameTurn)]
pub struct GameServices {
    #[vela::default(RustInventoryService)]
    pub inventory: dyn InventoryService,

    #[vela::default(RustRewardService)]
    pub reward: dyn RewardService,
}
```

The spelling is normative for the initial implementation. A later ergonomic
alias may shorten it only if it generates the same schema and does not create a
second runtime path.

The business author supplies only:

1. a service trait;
2. its Rust default implementation;
3. one service-set declaration; and
4. ordinary boundary derives for business types when required.

The macro supplies service IDs, method IDs, ABI descriptors, type-closure
registration, Rust fallback thunks, Vela entry thunks, partial-composition
logic, the hidden object-safe async dispatch surface, registration bundles,
staging validation, and generation accessors. No Vela implementation is
written until a real patch is needed.

The service set declares its execution-authority carrier once. In an actor
server this is usually the normal `&mut GameTurn` or actor context already
present in business signatures. Generated code borrows that actor's Runtime;
it never finds one through ambient thread-local or process-global state. A
service contract that cannot reach an explicit Runtime authority is callable
only through its Rust default until the host gives it one.

### 2.2 Rust call sites

The actor or request framework pins the generation at its safe point and
exposes it as ordinary dependency injection:

```rust,ignore
fn handle_grant(turn: &mut GameTurn, command: GrantCommand) -> GameResult<()> {
    let services = turn.services().clone(); // already pinned for this turn
    services.inventory().grant(
        turn,
        &mut command.player,
        &command.items,
    )?;
    Ok(())
}
```

The caller does not test whether `grant` is patched. It does not hold a
`DispatchRoot`, choose a slot, construct a proxy, or call a Vela-specific API.
Existing frameworks may generate the `services` accessor or inject the pinned
handle, but they must preserve the same explicit root authority and generation
semantics.

Calls made directly on `RustInventoryService` intentionally bypass Vela. Code
that needs hotfix behavior must depend on the generated service contract, not a
concrete implementation. This is the one unavoidable architectural opt-in: a
call cannot be replaceable without crossing a stable dispatch boundary.

### 2.3 Vela patch authoring

The Vela patch surface is a partial implementation of the imported Rust
contract:

```vela
#[service_impl(game::inventory::InventoryService)]
impl InventoryHotfix {
    fn grant(turn, player, items) {
        let grouped = items.group_by(|item| item.template_id);
        let rewards = services.reward.apply(turn, player, grouped)?;

        if rewards.is_empty() {
            return base.grant(turn, player, items);
        }

        player.last_reward_count += rewards.len();
        return Ok(rewards);
    }
}
```

`remove` is absent and therefore remains Rust. The compiler imports parameter
and return facts from the Rust service schema, so the Vela author does not
repeat types unless an optional local annotation improves readability. Method
matching is positional; Rust and Vela parameter names are not ABI.

`#[service_impl(...)]` is a declaration-level contract, not monkey patching.
It is legal only for a registered Rust service trait and only at compile/link
time. It cannot add fields, methods, implementations, or type structure at
runtime. Two Vela implementations of the same service method in one candidate
are a link error.

### 2.4 Handler, rule, and event shape

There is no `HandlerSlot`, `RuleSlot`, or `EventOverride` API. Framework-facing
traits are service contracts:

```rust,ignore
#[vela::service(path = "game::handlers::login")]
pub trait LoginHandlerService: Send + Sync {
    async fn handle(
        &self,
        turn: &mut GameTurn,
        request: LoginRequest,
    ) -> GameResult<LoginResponse>;
}
```

Routing, mailbox decoding, protocol framing, and transport ownership remain in
Rust. The routed business operation crosses the generated service boundary.
The same rule applies to scheduled jobs, combat rules, event consumers, and
administrative commands.

## 3. Generated Contract And Runtime Objects

### 3.1 Stable identity and ABI

Each service schema contains:

```text
ServiceId
stable service path
ServiceAbiFingerprint
ordered ServiceMethodDescriptor[]
  ServiceMethodId
  stable method path
  sync/async shape
  receiver mode
  positional parameter modes and type descriptors
  InteropTypeId, representation capability, and TypeAbiFingerprint
  return/error family
  borrowed-return origin and freeze rules
  normalized effect ceiling
  derived capability requirements
  source origin and diagnostics metadata
transitive boundary type closure
```

`ServiceId` and `ServiceMethodId` are semantic stable IDs, not dense authored
indices. A generation may build dense internal tables after validation, but no
index enters macro input, source syntax, deployment manifests, or public APIs.

Parameter names, docs, source positions, Runtime grants, allowlists, budgets,
and active policy are not ABI. Parameter order/mode, supported type shape,
return/error family, asyncness, borrowed-return provenance, and effect ceiling
are ABI. A patch with a strict effect subset is valid; a patch exceeding the
Rust contract is rejected before publication.

Changing the Rust trait schema requires a new server build unless an explicit
future service-schema migration plan says otherwise. The initial model does not
attempt to hot-add Rust trait methods, migrate concrete Rust types, or bridge
two incompatible service manifests.

### 3.2 Service candidate

Compiling or loading a deployment update produces an immutable
`ServiceUpdateBundle`:

```text
ServiceUpdateBundle
  UpdateMode
    Snapshot
    Delta { base_generation_id, base_artifact_checksum }
  package identity and checksum
  VelaProgramUpdate
    SnapshotArtifact
    DeltaArtifact { FunctionId -> Replace | Remove }
  service manifest checksum
  sealed TypeBinding registry checksum
  ServiceMethodId -> Replace(linked Vela callable) | RustDefault
  Vela state/schema reload facts
```

The two modes have different omission semantics:

| Update mode | Unmentioned service method | Explicit `RustDefault` |
|---|---|---|
| `Snapshot` | select the registered Rust default | also select Rust default |
| `Delta` | inherit the exact base generation selection | replace any inherited Vela implementation with Rust default |

The same distinction applies to ordinary Vela modules and functions. A
Snapshot contains the complete desired program graph. A Delta inherits
unmentioned functions from its exact base and applies explicit replacements or
validated removals. This covers both Vela-authored baseline logic and emergency
service patches without publishing their code maps separately.

A normal release should prefer `Snapshot`: the bundle describes the complete
desired Vela service state and does not depend on deployment history. An
emergency `Delta` may contain only changed Vela functions and service methods,
but it must name the exact generation and linked-artifact checksum it was built
against. Delta inheritance is a staging rule, never a runtime fallback chain.

Staging a Snapshot composes its declarations over every registered Rust
default. Staging a Delta composes its operations over the named complete base
generation. Both produce one flattened `ServiceGenerationCandidate` and one
coherent linked artifact; unchanged immutable CodeObjects may be Arc-shared
from the base, but runtime target lookup never walks prior generations. Before
publication staging validates:

- exact host service-set identity;
- every service and method ID;
- complete callable ABI and effect compatibility;
- transitive TypeBinding availability, representation capability, and ABI;
- host-reference parameter and borrowed-return provenance;
- collection element/key/value descriptors and mutation mode;
- sync/async form and cancellation-safe adapter shape;
- linked artifact, state, and hot-reload compatibility;
- duplicate method claims and unknown service declarations; and
- deployment capabilities and policy.

A failed candidate changes no active state. Validation performs no business
method invocation and no host mutation. Vela module/function changes, service
method selections, and state/schema reload facts belong to the same candidate;
they cannot be staged or published as independently visible updates.

### 3.3 Service generation and publication

The accepted candidate becomes an immutable `Arc<ServiceGeneration>`. The
generated `GameServiceGeneration` holds the complete set of generated composite
services plus its linked artifact and exact generation identity. A candidate
records its expected current generation. Publication uses one conditional
ArcSwap/CAS operation and fails with `StaleBaseGeneration` if the active
generation changed after staging; it never silently rebases or overwrites a
concurrent release.

Successful activation returns a controller/schema-bound rollback token holding
the replaced generation and the generation that replaced it. Rollback is also
conditional: it republishes the prior complete generation only while the
expected replacement is still current. A later deployment makes the old token
stale rather than allowing it to overwrite newer code.

The runtime must not use one `ArcSwap` per method or per service. Such a layout
would allow a nested call to mix generations and would reintroduce slot-like
deployment semantics.

The intended deployment API is conceptually:

```rust,ignore
let base = services.current_generation();
let update = engine.load_service_update(bundle_bytes)?;
let candidate = services.stage_delta(&base, update, StageOptions::default())?;
let rollback = services.activate_if_current(candidate)?;

// Only succeeds if the generation installed above is still current.
services.rollback_if_current(rollback)?;
```

Snapshot staging has the corresponding `stage_snapshot` entry and does not
inherit Vela method selections from `base`. Production control planes should
compile or load and validate immutable bundles away from request execution;
activation itself is the bounded pointer publication step.

### 3.4 Execution ownership

Service generations own immutable selection and code metadata only. One logical
Runtime per actor continues to own persistent Vela state, heap, roots, extern
bindings, HostRef allocator and leases, suspended sessions, and adopted code
generation. A generated service invocation borrows the current actor turn's
`&mut Runtime`.

A selected Vela method enters the existing `Runtime::call` /
`Runtime::call_async` and `ExecutionSession` driver. A nested call made through
`services` or a generated Rust binding uses the active `NativeCallContext` and
pushes onto the same session. It inherits:

- pinned service and linked-artifact generations;
- heap, script state, and extern state view;
- `HostAccess`, host identities, leases, and reborrow provenance;
- remaining execution, memory, collection, and call-depth budgets;
- effect ceiling, capabilities, and allowlists;
- tracing and diagnostics context; and
- cancellation and async suspension ownership.

There is no target-owned `Runtime`, Runtime mutex, reentrant global lock,
thread-local Runtime, target-local default budget, or fresh nested session.

### 3.5 Successive Vela updates

Suppose generation G12 selects Vela implementations for two methods and Rust
for a third. A Delta built against G12 that replaces only the first method
produces G13 with the new first implementation, the inherited second Vela
implementation, and the inherited Rust third implementation. Old roots retain
G12; new roots pin G13. An explicit `RustDefault` operation is required to
remove the second Vela implementation in a later Delta.

`base.method(...)` always names the registered Rust default, never the prior
Vela implementation. The runtime therefore does not build `patch v3 -> patch
v2 -> patch v1 -> Rust` call chains. Shared Vela behavior that a later patch
needs to reuse must remain an ordinary named Vela function and enter the same
module/function update artifact. Operators should periodically fold accepted
Deltas into a new Snapshot so deployment history is not a permanent source
dependency even though each runtime generation is already flattened.

## 4. Unified Rust Type Interop Model

### 4.1 `TypeBinding` is the single registration unit

Every reachable Rust type has one deterministic binding:

```text
TypeBinding
  InteropTypeId and stable Vela path
  StoragePolicy: Value | Host
  owned/shared/exclusive representations
  constructors and static methods
  instance methods with receiver capability
  fields, indexes, iteration, and standard protocols
  value codec or host-object adapter
  lifetime, escape, effects, and capabilities
  TypeAbiFingerprint and diagnostic origin
```

`T`, `&T`, and `&mut T` never create unrelated method registries. They share
the same nominal type and method IDs. The receiver capability decides which
methods are callable:

| Rust receiver | Capability | Legal method families |
|---|---|---|
| owned `T` | owned + shared + exclusive | consuming, shared, and mutating |
| `&T` | shared | shared only |
| `&mut T` | exclusive + shared reborrow | shared and mutating |
| type/static | construct | registered constructors and static methods |

Known capability violations are compile errors; dynamic paths repeat the check
before entering Rust. Registration rejects duplicate IDs, unsupported
signatures, ambiguous storage policy, and methods whose declared receiver does
not match their Rust receiver.

### 4.2 Value and host storage policies

Every binding selects an explicit storage policy:

| Policy | Rust examples | Vela representation | Ownership |
|---|---|---|---|
| `Value` | scalar, String, enum, DTO, owned collection | scalar, record, enum, Array/Map/Set | script-owned after direct typed lowering |
| `Host` | identity-bearing or opaque Rust object | typed `OwnedHost<T>` / HostRef handle | external host arena owns Rust state |
| borrowed view | `&T`, `&mut T` under either policy | typed View/MutView backed by HostRef | Rust-owned, invocation-scoped capability |

Value lowering is generated field/element conversion, not JSON, bincode, or
runtime serde reflection. A host-owned value moved into Vela remains an exact
Rust object and may call its registered Rust methods; construction uses a
registered host factory. The script GC may trace the handle but never owns or
traces the Rust object. Promotion beyond a root call is an explicit host policy,
not an accidental consequence of storing a handle.

`Arc<T>`, `Rc<T>`, boxes, engine handles, ECS handles, database sessions, and
other Rust ownership wrappers do not infer a policy automatically. Their
binding must say whether they lower to a value or resolve to a host object.

### 4.3 Value objects

An owned boundary DTO derives one generated structural contract:

```rust,ignore
#[derive(vela::Value)]
pub struct ItemGrant {
    pub template_id: i32,
    pub count: i32,
    pub metadata: HashMap<String, String>,
}
```

The derive emits conversion and deterministic schema metadata. It does not use
runtime serde reflection as the linked call path. Nested records, enums,
Option, Result, tuples, and supported collections are recursively included in
the service type closure. Cycles that require identity must use a host object
or a future explicit graph-value contract; the initial DTO conversion does not
invent object identity.

### 4.4 Host objects and Rust references

An authored service signature may use ordinary call-scoped `&T` and `&mut T`.
Generated adapters represent them in Vela with typed HostRefs and acquire the
complete lease set atomically before creating any Rust reference:

```text
&T      -> shared HostRef capability
&mut T  -> exclusive HostRef capability
```

Vela never stores a real Rust reference. A nested service call derives a scoped
child reborrow from the active parent lease. It preserves canonical identity,
host type, path provenance, and shared/exclusive mode. A shared-to-exclusive
upgrade, overlapping exclusive alias, expired parent, or mismatched type fails
before the nested body executes.

Returned Rust borrows use the existing call-tree-scoped child HostRef and
parent-freeze rules. They may flow through Vela locals, temporary collections,
and nested service calls in the same root, but cannot escape to persistent
state, globals, native caches, unscoped tasks, or the root result. Early
compiler-proven release and `host::release` remain valid; GC timing is never a
correctness dependency.

The initial service return whitelist admits synchronous exact-parameter
borrows, exact borrowed collection parameters, `Option<&T>`, and
`Result<&T, E>` with one explicit Host-parameter origin. A successful return
must be the same direct HostRef as that parameter; the generated terminal sink
validates its type, object identity, generation, and envelope, then Rust
reuses the already-live authored borrow. `None` and `Err` retain no HostRef,
and `E` uses its sealed bidirectional owned Value codec. A service signature
that projects `Table -> &Row`, exclusive Option/Result envelopes, borrowed
children nested inside owned containers, and async borrowed returns is
rejected during macro expansion.

The S0-S7 hard switch deliberately keeps this accepted borrowed-return model
conservative. It requires an unambiguous retained origin, uses owner-level
freeze when a finer safe domain is not registered, and treats every returned
borrow as root-call-tree scoped. The following refinements are deferred until
the unified service path is accepted and a concrete workload demonstrates the
need:

- origin sets for signatures whose result may borrow from one of several
  declared parameters, with runtime provenance selecting the actual origin;
- registered projection metadata plus a safe Rust return representation for
  proving and restoring disjoint subobject borrows without fabricating a
  reference from an erased HostRef;
- finer owner-conflict checks where a `TypeBinding` can prove that operations
  touch independent lease domains; and
- a separate durable `HostHandle<T>` contract for identity that must survive a
  root call, rather than allowing `View<T>` or `MutView<T>` to escape.

These refinements must preserve the existing rules: scripts never receive real
Rust references, shared capability never upgrades to exclusive, conflicting
aliases fail before Rust references exist, nested service calls keep the same
pinned generation, and scoped borrows never become durable through GC or
implicit promotion. They are not prerequisites for returning the exact direct
`&T` or `&mut T` parameter and passing it through nested Rust/Vela services in
the same root. Projected Host children remain usable within Vela through
ordinary registered Host methods; they are not admitted as authored Service
return types.

#### Required HostRef hot-path contract

HostRef optimization is part of S0-S7, not a post-hard-switch follow-up. The
script-visible reference is a compact, copyable generational handle into the
current root execution's dense host-slot table. Canonical identity, type,
capability, owner, borrow-group state, provenance, prepared adapter, and pinned
generation live once in root-owned metadata rather than being copied into every
Vela alias. Copying a View/MutView must not allocate, clone an `Arc`, increment
an atomic reference count, or create another host lease.

The linked fast path follows these rules:

- statically resolved fields, indexes, and methods use dense IDs, prepared
  `HostTargetPlan` data, and generated typed thunks; a successful known access
  performs no string/hash/reflection lookup and materializes no owned
  `HostPath` or segment vector;
- one Rust invocation preflights its complete host-argument request set before
  creating any reference, using allocation-free inline storage for the common
  service arities and one canonical identity/conflict pass;
- a same-session nested call derives a child reborrow from the active root-local
  lease/provenance entry instead of globally reacquiring the owner, resolving a
  business ID, or allocating a second host object;
- all aliases of one scoped borrow share one `BorrowLeaseId`; early or explicit
  release invalidates the group without per-alias lifetime bookkeeping;
- a method selected to the Rust default passes ordinary Rust values and
  references directly and does not create HostRefs or enter the VM; and
- collection protocols expose prepared bulk operations so realistic filter,
  grouping, iteration, and mutation do not require avoidable per-element
  dynamic boundary setup.

These are representation and preparation optimizations, not weaker semantics.
Epoch/freshness, type, capability, provenance, escape, generation, HostAccess,
and complete alias-conflict checks remain mandatory. They may be linked,
hoisted, combined, or made O(1), but never skipped merely because a call site
was previously successful.

### 4.5 Collection views and standard protocols

Vela exposes one standard collection vocabulary with explicit borrowed
representations:

```text
Array<T> / ArrayView<T> / ArrayMut<T>
Map<K, V> / MapView<K, V> / MapMut<K, V>
Set<T> / SetView<T> / SetMut<T>
Iterator<T>
```

These are restricted builtin type hints, not general script-language generics.
Users cannot define generic types, generic functions, or arbitrary generic
implementations. The compiler and registry may carry element/key/value facts
internally because service ABI and method validation require them.

Rust mapping is:

| Rust input | Vela surface | Mutation semantics |
|---|---|---|
| `Vec<T>` | owned `Array<T>` | full Array mutation |
| `&[T]`, `&Vec<T>` | `ArrayView<T>` | read-only |
| `&mut [T]` | `ArrayMut<T>` | element replacement, fixed length |
| `&mut Vec<T>` | `ArrayMut<T>` | write-through element and length mutation |
| owned `HashMap<K,V>` / `BTreeMap<K,V>` | owned `Map<K,V>` | full Map mutation |
| shared borrowed map | `MapView<K,V>` | read-only |
| exclusive borrowed map | `MapMut<K,V>` | write-through mutation |
| owned `HashSet<T>` / `BTreeSet<T>` | owned `Set<T>` | full Set mutation |
| shared/exclusive borrowed set | `SetView<T>` / `SetMut<T>` | capability-derived |

The owned and view types share standard `Sequence`, `Iterable`, `MapLike`, or
`SetLike` protocols, so normal read, iteration, `filter`, `group_by`, `fold`,
and collection syntax is identical. The explicit View/Mut type facts make
borrow and mutation diagnostics honest. The compiler rejects a statically
known write through a shared view, and the runtime repeats the check for
dynamic paths.
`&mut [T]` rejects `push`, `remove`, and every length-changing operation even
though element writes are legal.

Host-backed methods call generated `HostAccess` container operations and mutate
Rust immediately. They do not use copy-in/copy-out. Iterators retain the
parent view lease and generation for their lifetime. Operations such as
`map`, `filter`, `group_by`, `collect`, and sorting produce script-owned
collections unless a method explicitly documents an in-place host mutation.

### 4.6 Passing collections across services

The bridge follows these rules:

1. A host-backed view passed to another service is reborrowed; it is not
   materialized. Rust and Vela callees therefore observe the same collection.
2. A script-owned Array/Map/Set passed to an owned Rust parameter is
   materialized with one checked conversion.
3. A script-owned value collection may back a temporary Rust shared borrow for
   the duration of one call when every element has a safe value conversion.
4. A script-owned collection cannot satisfy a Rust mutable borrow through
   implicit copy-in/copy-out. It must already be an exact mutable host-backed
   view, because failure, suspension, aliasing, and partial mutation make
   copy-back semantics unsafe and surprising.
5. A borrowed container or iterator cannot escape its root call tree. A
   collected script-owned result may escape subject to normal value rules.

The same rules apply whether the next method is implemented in Rust or Vela.
The caller never performs a language-direction-specific conversion.

### 4.7 Map and set keys

Map and Set identity uses Vela's deterministic `ValueKey` contract, not an
arbitrary Rust `Hash`/`Eq` implementation. Supported keys are key-safe scalars,
strings, bytes, enums, tuples, and explicitly derived stable value keys. Float
NaN and mutable host identity do not silently enter the key model. Rust map
registration fails at build time when `K` cannot supply the exact stable key
contract.

### 4.8 Standard and user-defined type registration

Vela supplies built-in binding families for supported Rust primitives,
`String`, `Vec`, slices, arrays, `BTreeMap`, `HashMap`, `BTreeSet`, `HashSet`,
Option, Result, tuples, and their view forms. These bindings register Rust-like
constructors and methods where meaningful and implement Vela standard
protocols once. `filter` and `group_by` belong to the shared protocols rather
than handwritten implementations for every Rust collection.

Hosts register custom types through the same low-level API used by standard
bindings:

```rust,ignore
Engine::builder().register_rust_type::<Inventory>(
    TypeBinding::host("host::Inventory")
        .constructor("new", Inventory::new)
        .shared_method("contains", Inventory::contains)
        .exclusive_method("grant", Inventory::grant)
        .iterator(Inventory::iter),
);
```

`#[derive(vela::Value)]`, `#[derive(vela::Host)]`, and `#[vela::methods]` are
generators for this contract, not parallel registries. Manual registration is
available for external types that cannot be annotated. Constructors are always
explicit; Vela does not infer that every Rust `Default` or inherent `new`
method is script-visible.

### 4.9 Registration closure, not per-instantiation boilerplate

Authors do not register `Vec<i32>`, `Vec<Item>`, `HashMap<i32, Item>`, and every
method separately. The service macro walks the transitive signature graph and
emits a deterministic type-registration bundle. Collection behavior is
implemented once by standard Array/Map/Set/Iterator protocols. Concrete
element/key/value descriptors specialize validation and conversion but do not
duplicate method implementations.

External generators, including protobuf, Luban, ECS, or game-schema tooling,
may emit `vela::Value`, `vela::Host`, stable-key, and service-registration
facts. They must feed the same registry and ABI model rather than introduce a
generator-specific runtime bridge.

## 5. Standard Library Requirements

The service hard switch is not useful until Vela can manipulate the business
containers it receives. The supported baseline is:

### Array

```text
len, is_empty, get, first, last, contains
iter, enumerate, windows, chunks
map, filter, filter_map, flat_map, fold, any, all, find, position
group_by, associate_by, count_by, collect
push, pop, insert, remove, clear, retain, sort, sort_by, dedup
```

Mutating methods are available only when the receiver capability permits them.
Order and allocation behavior must be deterministic and budgeted.

### Map

```text
len, is_empty, contains_key, get
keys, values, entries, iter
get_or_insert, insert, remove, clear, retain
map_values, filter, group_by, merge, collect
```

Indexing a missing key must use the language's documented Option/error
semantics rather than create an entry implicitly.

### Set

```text
len, is_empty, contains, iter
insert, remove, clear, retain
union, intersection, difference, symmetric_difference, collect
```

### Iteration and budgets

Every traversal, materialization, sort, grouping, hash operation, and growth
operation charges execution and memory/collection budgets. Host-backed
iteration validates generation and lease authority on every resumable boundary.
No iterator may create an unbudgeted infinite execution path.

## 6. Macro And Tooling Responsibilities

### 6.1 Rust type bindings and derives

The registry and macros must:

- expose one public `register_rust_type::<T>(TypeBinding)` path for standard
  and user-defined types;
- generate stable identity, storage policy, conversion/view adapters,
  constructors, receiver-qualified methods, protocols, effects, and ABI;
- provide manual bindings for unannotatable external Rust types;
- synthesize concrete standard collection bindings from internal element/key/
  value facts without adding script-language generics; and
- make the compiler, Runtime, reflection, LSP, and service schema consume the
  same sealed binding snapshot.

### 6.2 `#[vela::service]`

The trait macro must:

- reject generic service traits, generic methods, unsupported associated types,
  variadics, unsafe functions, and non-boundary-safe signatures;
- accept ordinary values, Result/Option/tuples, supported collections,
  call-scoped shared/exclusive host references, and supported async methods;
- generate stable service/method metadata and ABI fingerprints;
- generate type-checked Rust default thunks and hidden object-safe dispatch;
- generate Vela imported declarations and diagnostics origins;
- generate partial-composite dispatch without altering authored bodies; and
- expose one registration bundle, never one handwritten function per method.

### 6.3 `#[vela::service_set]`

The set macro must:

- validate unique service identities and one Rust default per service;
- declare the Runtime authority carrier once;
- generate the immutable service generation and controller;
- generate the `ArcSwap` publication owner and safe-point pin handle;
- generate Snapshot and exact-base Delta staging, conditional activate,
  conditional rollback, and current-generation APIs;
- provide same-generation access for Rust and Vela cross-service calls; and
- generate framework adapters without exposing Runtime or lease internals to
  business callers.

### 6.4 `#[service_impl]`

The Vela compiler must:

- resolve the imported service contract statically;
- accept a sparse method set;
- infer method parameter/return contracts from Rust metadata;
- expose lexical `base` and `services` capabilities;
- reject unknown, duplicate, incompatible, or over-effect methods;
- include exact source spans in stage diagnostics; and
- emit sparse `Replace` operations and never mutate a live service table. The
  deployment bundle builder/manifest owns explicit `RustDefault` operations
  and the enclosing Snapshot/Delta omission semantics; Vela does not gain
  another patch-control statement for deployment bookkeeping.

### 6.5 Diagnostics and reflection

Diagnostics name the service path, method, positional parameter, expected and
actual type/mode, source span, and generation when available. Reflection may
query service and method metadata and may perform controlled calls through the
same linked target. It cannot add a service, method, field, trait
implementation, or patch to a live generation.

The language service consumes the imported service schema for completion,
hover, signature help, navigation, diagnostics, and rename safety. It must not
reparse Rust or infer contracts from macro-expanded source text.

## 7. Hard-Switch Deletion Contract

The following concepts are removed from production source, public exports,
tests, examples, benchmarks, and active documentation:

```text
ReplaceableSlotDescriptor
ReplaceableSlotId
InterceptSlotIndex
DispatchController
DispatchGeneration as a callable-slot table
DispatchRoot
DispatchInvocation
DispatchAuthority
VelaOverrideTarget
#[vela::replaceable]
#[override(...)]
register_replaceable_slots
vela_replaceable_slot_*
vela_replaceable_slots
replaceable_handler example
replaceable_service_method example
```

Names may remain only in archived historical reports and in this deletion
checklist. There are no deprecated aliases, compatibility traits, feature
flags, dual reads/writes, slot-to-service adapters, old/new source syntaxes, or
fallback target strings.

The following general mechanisms are retained under service-neutral ownership
where they remain valid:

- `CallableContract` and boundary ABI comparison;
- `ProgramVersion`, `CodeObject`, and linked-artifact generation pinning;
- `NativeCallContext` same-session re-entry;
- host argument conversion, atomic lease acquisition, child reborrow, and
  borrowed-return provenance/freeze;
- actor-owned Runtime and scoped async execution;
- staging, validation, atomic activation, prior-generation rollback, and
  no-retry semantics; and
- budget, capability, effect, tracing, cancellation, and diagnostic plumbing.

Retained code must be renamed and relocated when its current owner or name is
slot-specific. Copying the old dispatch module under a new name without
changing its service-generation semantics does not satisfy the switch.
