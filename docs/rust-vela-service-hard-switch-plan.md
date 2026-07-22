# Rust/Vela Unified Service Model Hard-Switch Plan

> Track: one Rust/Vela service contract, generated Rust fallback and Vela
> partial implementation, generation-coherent publication, host-reference and
> collection interop, and deletion of callable-level replacement
>
> Status: approved design direction; implementation not started
>
> Switch policy: pre-release hard switch; no public compatibility layer and no
> second Rust hot-replacement model
>
> Supersedes: the optional `#[replaceable]` / `#[override]` slot model in
> [rust-vela-interop-model-plan.md](rust-vela-interop-model-plan.md) and
> [rust-vela-interop.md](rust-vela-interop.md)

## 0. Objective

Vela exists to supply the runtime dynamism that Rust game-server business
logic cannot provide by itself. The integration therefore optimizes for one
authoring experience:

```text
Rust defines an ordinary service trait and its default implementation.
The service framework macro generates all Vela boundary machinery.
Business callers always call the service contract.
A Vela package may implement only the faulty methods.
One validated service generation is atomically published for future calls.
```

Business code must not contain a per-method patch branch, dense replacement
slot, target string, runtime lookup, manual `CallArgs`, `HostRef`, or handwritten
Vela adapter. Whether a method currently runs Rust or Vela is a deployment
choice, not a caller concern.

This is also a simplification plan. Vela will not keep both service replacement
and callable-level replacement. The current `ReplaceableSlotId`,
`InterceptSlotIndex`, `DispatchController`, `DispatchRoot`, `#[replaceable]`,
and `#[override]` model is frozen immediately and removed by the hard-switch
batch. Handler, rule, event, and function hotfixes do not receive parallel slot
types; a host entry that must be hotfixable is expressed as a service method.

Vela's own `ProgramVersion` / `CodeObject` hot reload remains the mechanism for
changing Vela code safely. It is not a second Rust integration model. Ordinary
exported Rust functions and methods also remain callable from Vela, but they
are not independently replaceable. A Rust operation that must be hotfixable
must belong to a service contract.

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

Compiling a patch produces a sparse, immutable `ServicePatchSet`:

```text
ServicePatchSet
  package identity and checksum
  Arc<LinkedArtifact>
  service manifest checksum
  ServiceMethodId -> linked Vela callable
  Vela state/schema reload facts
```

Staging composes that sparse table with every registered Rust default to create
a complete `ServiceGenerationCandidate`. Before publication it validates:

- exact host service-set identity;
- every service and method ID;
- complete callable ABI and effect compatibility;
- transitive boundary type availability;
- host-reference parameter and borrowed-return provenance;
- collection element/key/value descriptors and mutation mode;
- sync/async form and cancellation-safe adapter shape;
- linked artifact, state, and hot-reload compatibility;
- duplicate method claims and unknown service declarations; and
- deployment capabilities and policy.

A failed candidate changes no active state. Validation performs no business
method invocation and no host mutation.

### 3.3 Service generation and publication

The accepted candidate becomes an immutable `Arc<ServiceGeneration>`. The
generated `GameServiceGeneration` holds the complete set of generated composite
services plus its linked artifact and exact generation identity. Publication
uses a single `ArcSwap` exchange. The returned prior `Arc` is the rollback
token; it is accepted only by the same service-set controller and schema.

The runtime must not use one `ArcSwap` per method or per service. Such a layout
would allow a nested call to mix generations and would reintroduce slot-like
deployment semantics.

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

## 4. Rust/Vela Type Model

### 4.1 Three boundary categories

Every reachable service type belongs to exactly one category:

| Category | Rust examples | Vela representation | Ownership |
|---|---|---|---|
| Scalar/value | `i32`, `i64`, `bool`, `String`, enum, owned DTO | scalar, record, enum | copied or moved into script-owned value |
| Script collection | owned `Vec<T>`, `HashMap<K,V>`, `HashSet<T>` | `Array<T>`, `Map<K,V>`, `Set<T>` | script-owned after conversion |
| Host-backed reference | `&T`, `&mut T`, borrowed collection | typed HostRef-backed value/view | Rust-owned, invocation-scoped capability |

`Arc<T>`, `Rc<T>`, boxes, engine handles, ECS handles, database sessions, and
other Rust ownership wrappers do not automatically become script-owned values.
They require an explicit value conversion or a registered host-reference
adapter. Rust host state is never placed under the Vela GC.

### 4.2 Value objects

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

### 4.3 Host objects and Rust references

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

### 4.4 Collection surface

Vela exposes one standard collection vocabulary:

```text
Array<T>
Map<K, V>
Set<T>
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
| `&[T]`, `&Vec<T>` | host-backed `Array<T>` view | read-only |
| `&mut [T]` | host-backed `Array<T>` view | element replacement, fixed length |
| `&mut Vec<T>` | host-backed `Array<T>` view | write-through element and length mutation |
| owned `HashMap<K,V>` / `BTreeMap<K,V>` | owned `Map<K,V>` | full Map mutation |
| shared borrowed map | host-backed `Map<K,V>` view | read-only |
| exclusive borrowed map | host-backed `Map<K,V>` view | write-through mutation |
| owned `HashSet<T>` / `BTreeSet<T>` | owned `Set<T>` | full Set mutation |
| borrowed set | host-backed `Set<T>` view | capability-derived |

The authored Vela type remains `Array`, `Map`, or `Set`; storage and mutation
capability are internal facts. The compiler rejects a statically known write
through a shared view, and the runtime repeats the check for dynamic paths.
`&mut [T]` rejects `push`, `remove`, and every length-changing operation even
though element writes are legal.

Host-backed methods call generated `HostAccess` container operations and mutate
Rust immediately. They do not use copy-in/copy-out. Iterators retain the
parent view lease and generation for their lifetime. Operations such as
`map`, `filter`, `group_by`, `collect`, and sorting produce script-owned
collections unless a method explicitly documents an in-place host mutation.

### 4.5 Passing collections across services

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

### 4.6 Map and set keys

Map and Set identity uses Vela's deterministic `ValueKey` contract, not an
arbitrary Rust `Hash`/`Eq` implementation. Supported keys are key-safe scalars,
strings, bytes, enums, tuples, and explicitly derived stable value keys. Float
NaN and mutable host identity do not silently enter the key model. Rust map
registration fails at build time when `K` cannot supply the exact stable key
contract.

### 4.7 Registration closure, not per-instantiation boilerplate

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

### 6.1 `#[vela::service]`

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

### 6.2 `#[vela::service_set]`

The set macro must:

- validate unique service identities and one Rust default per service;
- declare the Runtime authority carrier once;
- generate the immutable service generation and controller;
- generate the `ArcSwap` publication owner and safe-point pin handle;
- generate coherent stage, activate, rollback, and current-generation APIs;
- provide same-generation access for Rust and Vela cross-service calls; and
- generate framework adapters without exposing Runtime or lease internals to
  business callers.

### 6.3 `#[service_impl]`

The Vela compiler must:

- resolve the imported service contract statically;
- accept a sparse method set;
- infer method parameter/return contracts from Rust metadata;
- expose lexical `base` and `services` capabilities;
- reject unknown, duplicate, incompatible, or over-effect methods;
- include exact source spans in stage diagnostics; and
- emit a sparse `ServicePatchSet`, not mutate a live service table.

### 6.4 Diagnostics and reflection

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

## 8. Phased Execution

Each phase ends with focused tests, workspace formatting/lint/tests, active-doc
updates, and one small Conventional Commit checkpoint. The implementation may
use a short-lived internal construction sequence, but no accepted checkpoint
may advertise both callable replacement and service replacement as supported
authoring models.

### S0 — Freeze, inventory, and executable fixtures

Deliverables:

- mark callable-level replacement superseded in all active docs;
- inventory every old type, macro, parser attribute, linker field, Engine API,
  example, test, and benchmark;
- extract a representative domain-neutral fixture with at least two services,
  one handler service, a mutable host actor, value DTOs, Array/Map arguments,
  nested service calls, Result, and async coverage; and
- record direct Rust trait dispatch and current Vela boundary baselines.

Gate:

```text
the fixture runs entirely through Rust defaults
no new feature is added to #[replaceable] or #[override]
the deletion inventory has an owner and replacement for every hit
```

### S1 — Delete callable-level replacement

Deliverables:

- preserve general call/lease/reentry facts under neutral modules;
- remove the old IDs, dispatch module surface, Engine registration, macro,
  parser/linker override path, examples, tests, and benchmark rows;
- update ordinary interop docs so Rust exports are callable but not
  replaceable; and
- leave the workspace green before adding a new public service API.

Gate:

```text
production-source audit has zero old replacement identifiers
ordinary Rust/Vela calls and NativeCallContext re-entry remain green
there is no public Rust hot-replacement mechanism at this checkpoint
```

Deleting first is intentional. It prevents the new design from becoming an
optional layer beside an already accepted competing model.

### S2 — Service contract and Rust-only generation

Deliverables:

- implement `#[vela::service]` and `#[vela::service_set]` schema generation;
- generate object-safe sync/async dispatch and Rust default composites;
- implement whole-set generation identity, `ArcSwap` publication, safe-point
  pinning, and same-controller rollback validation; and
- migrate the fixture's callers to the generated service set.

Gate:

```text
business trait implementations and call sites contain no patch logic
all methods execute Rust defaults through one pinned generation
old and new roots retain their exact generations across activation/rollback
```

### S3 — Scalar partial Vela vertical slice

Deliverables:

- import service schemas into Vela;
- parse, resolve, compile, and link sparse `#[service_impl]` blocks;
- stage and activate one scalar sync method while adjacent methods stay Rust;
- implement `base` and same-service no-recursion behavior; and
- reject ABI/effect/identity/duplicate/unknown-method candidates atomically.

Gate:

```text
Rust default -> activate one Vela method -> adjacent Rust -> rollback
Vela failure propagates with no Rust fallback retry
the caller source is unchanged across the full sequence
```

This is the first runnable end-to-end service hotfix slice.

### S4 — Host references and cross-service re-entry

Deliverables:

- support shared/exclusive host arguments and borrowed returns through the
  existing HostRef/HostAccess boundary;
- implement `services.other.method(...)` on the same pinned generation and
  `base.method(...)` through the generated Rust default thunk;
- derive nested child reborrows instead of reacquiring unrelated leases; and
- preserve one execution session, budgets, effects, cancellation, tracing, and
  state view across Rust/Vela/Rust chains.

Gate:

```text
Vela service -> Rust service -> patched Vela service uses one generation
mutable host writes are immediately visible at every layer
alias conflicts fail before the nested callee body
borrowed values cannot escape the root call tree
```

### S5 — Collection and type-closure interop

Deliverables:

- implement owned and host-backed Array/Map/Set representations;
- implement read-only, fixed-length mutable, and growable mutable capabilities;
- synthesize recursive collection descriptors from service schemas;
- complete the required stdlib methods and iterator lease rules; and
- support safe cross-service pass-through without per-concrete registration.

Gate:

```text
Vec/HashMap/HashSet value and borrow matrix passes in both directions
group/filter/collect results are script-owned and budgeted
mutable host views write through immediately
implicit mutable copy-in/copy-out is rejected
no handwritten Vec<i32>/Map<K,V> method registration exists
```

### S6 — Async, handlers, deployment, and tooling

Deliverables:

- support authored async service methods with generated object-safe adapters;
- retain pinned service/artifact generations and leases across suspension;
- prove cancel, dropped-future, panic-unwind, and no-runtime-mutex behavior;
- model handlers/rules/events exclusively as service contracts;
- expose stage/activate/rollback diagnostics and service metadata to CLI/LSP;
  and
- replace old examples and benchmark rows with service-generation examples.

Gate:

```text
one actor may remain pending while another completes on shared code
actors keep isolated Runtime state
cancellation/drop/unwind releases Runtime borrows and host leases
there is no handler/rule/event-specific replacement API
```

### S7 — Host-framework integration and final acceptance

Deliverables:

- integrate the generated model into a representative domain-neutral
  service/handler call chain without patch-aware business code;
- patch one small method fragment, call Rust base, call another Rust service,
  and call another patched Vela service;
- exercise Player/actor mutable references, DTOs, nested Array/Map values,
  iteration and grouping, business Result, and async handling;
- measure Rust-default and active-Vela service paths; and
- complete structural, documentation, example, fuzz/build, and workspace gates.

Gate:

```text
the host pins once per actor/request safe point
the service caller is identical before and after activation
one atomic generation contains the complete multi-service patch
old in-flight roots finish on old code and new roots enter new code
rollback is publication only and never retries host effects
all old replacement production identifiers remain absent
```

## 9. Acceptance Matrix

### Authoring

- A Rust author writes no Vela adapter and no per-method patch annotation.
- A Vela author implements one method without copying adjacent Rust methods.
- A caller does not branch, use a target string, or construct a Runtime value.
- Direct concrete Rust calls are documented as intentional bypasses.
- Macro UI tests reject every unsupported service signature with a focused
  diagnostic.

### Dispatch and generation

- Rust-only generation, single partial patch, multiple methods in one service,
  and multiple services in one package all work.
- Activation and rollback publish exactly one whole-set pointer.
- A root pinned before activation never observes a new method selection.
- Nested calls cannot mix generations.
- Cross-controller and cross-schema candidates/rollback tokens are rejected.
- Missing Vela methods resolve to Rust at stage time.
- Vela failures never trigger automatic Rust execution.

### Values and collections

- Scalars, String/bytes, records, enums, Option, Result, tuples, and nested
  owned collections round-trip.
- Shared/exclusive host references preserve identity and alias rules.
- Borrowed returns preserve origin/freeze and cannot escape.
- Owned, shared, exclusive-fixed, and exclusive-growable Array cases pass.
- Owned/shared/exclusive Map and Set cases pass with stable keys.
- Iteration, grouping, collection, sorting, and mutation charge budgets.
- A host-backed collection passes through Vela to Rust and another Vela
  service without materialization or identity loss.

### Execution

- Sync and async methods use the same service model.
- Nested calls inherit artifact, state, heap, budgets, capabilities, effects,
  tracing, cancellation, and HostAccess.
- No service object, target, controller, or generation owns a mutable Runtime.
- Panic, error, cancellation, unpolled future drop, and rollback release every
  scoped borrow and lease.

### Structural

- No active production code contains the deleted callable-slot types/macros.
- No compatibility alias or dual syntax remains.
- No service method uses runtime string lookup on the hot path.
- No per-concrete standard collection implementation is handwritten.
- Rust host state remains outside the script GC.

## 10. Performance Contract

Correctness and authoring coherence come first, but the Rust-default path must
remain suitable for a high-frequency game server.

The accepted Rust-default service call performs:

```text
one already-pinned service-generation access
one generated service/dyn dispatch
one prelinked per-method Rust/Vela selection
ordinary Rust default call
```

It performs no allocation, serialization, Runtime lock, source lookup, string
lookup, hash lookup, reflection walk, collection materialization, or Vela VM
entry when the method resolves to Rust. Pinning is charged once per host root,
not once per nested service call.

Benchmarks compare:

- direct concrete Rust call;
- ordinary Rust trait-object call;
- generated Rust-default service call;
- generated Vela-active service call;
- nested same-generation Rust/Vela chains;
- owned versus host-backed collection calls; and
- activation/staging cost outside the request hot path.

No fixed percentage is accepted before the S0 baseline exists. S2 records a
budget justified by measured production-shaped calls. A regression cannot be
hidden by weakening generation coherence, lease safety, or error semantics.

## 11. Validation Commands

Every implementation checkpoint runs the focused crate/macro/UI/example tests
for its changed area plus the repository baseline:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Later phases also run:

```bash
cargo clippy --manifest-path examples/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path examples/Cargo.toml --no-fail-fast
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
```

The hard-switch audit excludes only `docs/archive/**` and this plan's deletion
history:

```bash
rg -n 'ReplaceableSlot|InterceptSlot|DispatchRoot|DispatchController|DispatchAuthority|register_replaceable_slots|vela_replaceable|#\[override' \
  crates examples tests fuzz docs \
  --glob '!docs/archive/**' \
  --glob '!docs/rust-vela-service-hard-switch-plan.md'
```

At S1 and every later accepted checkpoint, the command must return no
production/API/example matches. Active migration notes may name deleted terms
only when they clearly state that the model is unavailable.

## 12. Explicitly Deferred

The hard switch does not include:

- general script-language generics;
- arbitrary Rust trait reflection or automatic exposure of every trait;
- hot mutation of Rust service trait structure;
- monkey patching of existing Rust or Vela types;
- automatic hotfixing of direct concrete Rust calls;
- transparent mutable copy-in/copy-out for script collections;
- cross-root persistent borrowed container views;
- async frame migration between service generations;
- a service-owned or process-global Runtime;
- JIT, moving GC, or script-level shared-memory concurrency; or
- more replacement abstractions for handlers, rules, events, providers, or
  individual functions.

## 13. Completion Definition

This plan is complete only when S0-S7 and the full acceptance matrix are green,
the host-framework integration demonstrates the intended authoring form, and
the old callable-level replacement model is absent from all production paths.

The final architecture should be explainable in one sentence:

```text
Rust supplies a default service generation; Vela supplies sparse method
implementations; the host atomically publishes one complete generation, and
every call uses ordinary typed Rust/Vela values through the same safe boundary.
```
