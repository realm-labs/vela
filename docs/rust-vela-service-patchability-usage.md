# Rust/Vela Service Patchability — Final Usage Shape

> Status: implemented boundary contract; final demo and repository-wide
> acceptance remain in progress.
>
> The generation/deployment model, direct/optional/fallible borrowed service
> returns to ordinary Rust callers, explicit call-scoped Host construction,
> and storage-directed shared Value/Host parameter lowering are implemented.
> Origin reporting, the consolidated coverage demo, and the final validation
> gates remain tracked by
> [the completion plan](rust-vela-service-patchability-completion-plan.md).

## 1. User-Facing Guarantee

The final service model has one guarantee:

```text
If a generated service set seals successfully, every admitted method can be
selected by Vela and called through its authored Rust signature.
```

The same Rust caller works before and after a Vela patch. Vela may:

- call the current method's Rust default through `base`;
- call other methods from the same pinned generation through `services`;
- construct registered Value and permitted Host types;
- receive injected `&T` and `&mut T` parameters;
- pass shared/exclusive HostRefs to later Rust or Vela services;
- consume direct `&T`, direct `&mut T`, borrowed collection views, and
  `Option<&T>`/`Result<&T, E>` returns inside the current synchronous root
  call tree;
- return an admitted borrow through the generated service adapter to the
  ordinary Rust caller;
- transform typed Value collections and pass them to owned or shared Rust
  collection parameters;
- mutate exact Host-backed collection views with immediate write-through;
- call supported async services while root arguments and the pinned generation
  remain retained; and
- deploy sparse Snapshot/Delta updates, preserve old roots, fold a Snapshot,
  and conditionally roll back.

Vela never receives a real Rust reference, owns Rust Host state, or silently
gains a capability that was not registered.

## 2. Complete Example Model

The example uses only domain-neutral table, row, request, policy, transform,
apply, and audit concepts.

### 2.1 Owned Value types

Values are copied by typed field/element lowering. They are appropriate for
requests, responses, errors, commands, projections, and transformed
collections.

```rust,ignore
#[derive(Clone, Debug, vela_macros::Value)]
#[script(path = "example::Request")]
pub struct Request {
    pub key: i64,
    pub adjustment: i64,
}

#[derive(Clone, Debug, vela_macros::Value)]
#[script(path = "example::ValueRow")]
pub struct ValueRow {
    pub key: i64,
    pub score: i64,
}

#[derive(Clone, Debug, vela_macros::Value)]
#[script(path = "example::Response")]
pub struct Response {
    pub accepted: bool,
    pub score: i64,
    pub inspected: i64,
}

#[derive(Clone, Debug, vela_macros::Value)]
#[script(path = "example::ServiceError")]
pub struct ServiceError {
    pub message: String,
}
```

A Value may be created with a record literal or registered constructor. The
same `Value<T>` may satisfy Rust `T`, or temporarily back Rust `&T` for one
synchronous call. It may not satisfy Rust `&mut T`.

### 2.2 Injected Host types

Rows and authoritative request state stay in Rust:

```rust,ignore
#[derive(vela_macros::ScriptHost)]
#[script(path = "example::Row")]
pub struct Row {
    #[script(get)]
    pub key: i64,
    #[script(get)]
    pub base_score: i64,
}

#[derive(vela_macros::ScriptHost)]
#[script(path = "example::Table")]
pub struct Table {
    #[script(skip)]
    rows: Vec<Row>,
}

#[derive(vela_macros::ScriptHost)]
#[script(path = "example::RequestState")]
pub struct RequestState {
    #[script(get)]
    total: i64,
    #[script(skip)]
    services: ExampleServicesRoot,
    #[script(skip)]
    runtime: ServiceRuntimeSlot,
}
```

`Table` and `RequestState` are Injected Host types. Vela can use them only
because the Rust caller supplies them to a service root. Merely registering
their schemas does not grant construction authority.

### 2.3 Constructible scratch Host

A patch may need a new mutable Rust object that is not returned by another
service. That object uses an explicit call-scoped Host constructor:

```rust,ignore
#[derive(vela_macros::ScriptHost)]
#[script(path = "example::PatchBuffer")]
pub struct PatchBuffer {
    #[script(get)]
    value: i64,
}

#[vela_macros::methods(path = "example::PatchBuffer")]
impl PatchBuffer {
    pub fn value(&self) -> i64 {
        self.value
    }

    pub fn add(&mut self, delta: i64) {
        self.value += delta;
    }
}
```

Target registration shape:

```rust,ignore
let patch_buffer = PatchBuffer::vela_type_binding()
    .host_constructor_fn(
        HostConstructionLifetime::CallScoped,
        patch_buffer_constructor_desc(),
        |args, _host| Ok(PatchBuffer::from_args(args)?),
    );
```

The constructor returns only a typed HostRef. It may be borrowed as
`&PatchBuffer` or `&mut PatchBuffer`; the object is reclaimed when the root
ends. Runtime-owned construction uses an explicit `RuntimeOwned` lifetime and
is not inferred from the presence of a constructor.

## 3. Service Contracts

### 3.1 Ordinary exports used by a patch

Free functions and inherent methods use the same typed boundary as services.
They are useful building blocks inside a patch:

```rust,ignore
#[vela_macros::export(path = "example::normalize")]
pub fn normalize(value: i64) -> i64 {
    value.max(0)
}

#[vela_macros::methods(path = "example::Table")]
impl Table {
    pub fn get(&self, key: i64) -> Option<&Row> {
        self.rows.iter().find(|row| row.key == key)
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn checked(&self, key: i64) -> Result<&Row, ServiceError> {
        self.get(key).ok_or_else(|| ServiceError {
            message: format!("missing row {key}"),
        })
    }
}
```

The receiver is the unambiguous origin for both borrowed results. A free
function may instead use its unique Host-ref parameter as origin. Missing or
ambiguous origins, access upgrades, nested borrowed containers, and async
borrowed returns fail during macro expansion or binding construction:

```rust,ignore
fn missing_origin() -> &'static Row; // reject
fn ambiguous<'a>(left: &'a Table, right: &'a Table) -> &'a Row; // reject
fn nested<'a>(table: &'a Table) -> Vec<&'a Row>; // reject
async fn suspended<'a>(table: &'a Table) -> Option<&'a Row>; // reject
async fn fallible_async<'a>(
    table: &'a Table,
) -> Result<&'a Row, ServiceError>; // reject
```

Ordinary exports are callable from Vela but are not independently replaceable
Rust call sites. A Rust business call that must be patch-selectable crosses a
generated service-set method.

### 3.2 Service borrowed returns

```rust,ignore
#[vela_macros::service(path = "example::state")]
pub trait StateService: Send + Sync {
    fn identity<'a>(
        &self,
        state: &'a mut RequestState,
    ) -> &'a mut RequestState;

    fn optional<'a>(
        &self,
        state: &'a mut RequestState,
        present: bool,
    ) -> Option<&'a RequestState>;

    fn checked<'a>(
        &self,
        state: &'a mut RequestState,
        allowed: bool,
    ) -> Result<&'a RequestState, ServiceError>;
}
```

The successful result is the exact direct `state` parameter. The terminal
adapter validates the same Host type, object id, generation, access, and
envelope before reusing the authored Rust borrow. `None` and
`Err(ServiceError)` carry no HostRef. A projected signature such as
`fn get(&self, table: &Table) -> Option<&Row>` is rejected as a Service
method; expose it as the ordinary registered `Table::get` Host method shown
above and call it from the patch.

### 3.3 Value, shared Host, and exclusive Host parameters

```rust,ignore
#[vela_macros::service(path = "example::policy")]
pub trait PolicyService: Send + Sync {
    fn score(
        &self,
        state: &mut RequestState,
        row: &Row,
        request: &Request,
    ) -> Result<i64, ServiceError>;
}

#[vela_macros::service(path = "example::apply")]
pub trait ApplyService: Send + Sync {
    fn apply(
        &self,
        state: &mut RequestState,
        row: &Row,
        score: i64,
    ) -> Result<(), ServiceError>;
}
```

`Request` is a Value temporarily decoded for the shared `&Request` call.
`RequestState` is an exclusive HostRef with immediate Rust write-through.
`Row` is a shared child HostRef and cannot be upgraded to exclusive access.

### 3.4 Constructed Host and collection lowering

```rust,ignore
#[vela_macros::service(path = "example::transform")]
pub trait TransformService: Send + Sync {
    fn inspect_buffer(
        &self,
        buffer: &PatchBuffer,
    ) -> i64;

    fn update_buffer(
        &self,
        buffer: &mut PatchBuffer,
        delta: i64,
    );

    fn consume(
        &self,
        values: Vec<ValueRow>,
    ) -> i64;

    fn inspect(
        &self,
        values: &[ValueRow],
    ) -> i64;
}
```

A script-owned `Array<ValueRow>` materializes once for `Vec<ValueRow>` or
backs one invocation-scoped `&[ValueRow]`. It cannot satisfy
`&mut Vec<ValueRow>`. A real Host `ArrayMut<ValueRow>` may satisfy an admitted
mutable collection parameter with zero-copy write-through.

### 3.5 Async root and ordinary owned result

```rust,ignore
#[vela_macros::service(path = "example::handler")]
pub trait HandlerService: Send + Sync {
    async fn handle(
        &self,
        state: &mut RequestState,
        table: &Table,
        request: Request,
    ) -> Result<Response, ServiceError>;
}

#[vela_macros::service(path = "example::audit")]
pub trait AuditService: Send + Sync {
    fn record(
        &self,
        state: &mut RequestState,
        code: i64,
    );
}
```

Root Host arguments and their complete lease set survive supported async
suspension. A child returned by the ordinary `table.get` Host method may not
remain live across `.await`; the handler awaits before creating that child,
copies required Value facts before awaiting, or delegates the whole operation
to a Rust async service.

## 4. Rust Defaults And Service Set

Rust defaults are ordinary trait implementations:

```rust,ignore
pub struct RustStateService;

impl StateService for RustStateService {
    fn identity<'a>(&self, state: &'a mut RequestState) -> &'a mut RequestState {
        state
    }

    fn optional<'a>(
        &self,
        state: &'a mut RequestState,
        present: bool,
    ) -> Option<&'a RequestState> {
        present.then_some(&*state)
    }

    fn checked<'a>(
        &self,
        state: &'a mut RequestState,
        allowed: bool,
    ) -> Result<&'a RequestState, ServiceError> {
        allowed.then_some(&*state).ok_or_else(|| ServiceError {
            message: "blocked".to_owned(),
        })
    }
}
```

The service set declares one Rust default per service:

```rust,ignore
#[vela_macros::service_set(context = RequestState)]
pub struct ExampleServices {
    #[vela::default(RustStateService)]
    pub state: dyn StateService,
    #[vela::default(RustPolicyService)]
    pub policy: dyn PolicyService,
    #[vela::default(RustApplyService)]
    pub apply: dyn ApplyService,
    #[vela::default(RustTransformService)]
    pub transform: dyn TransformService,
    #[vela::default(RustAuditService)]
    pub audit: dyn AuditService,
    #[vela::default(RustHandlerService)]
    pub handler: dyn HandlerService,
}
```

Generated registration closes every transitive Value/Host/container
requirement:

```rust,ignore
let builder = ExampleServices::register_types(
    Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_rust_type::<Row>(Row::vela_type_binding())
        .register_rust_type::<Table>(Table::vela_type_binding())
        .register_rust_type::<RequestState>(RequestState::vela_type_binding())
        .register_rust_type::<PatchBuffer>(patch_buffer),
);

let engine = builder.build()?;
let services = ExampleServices::new(&engine.type_bindings())?;
```

Missing storage capability, constructor lifetime, codec, collection fact, or
adapter rejects construction before any request executes.

## 5. Unchanged Rust Caller

### 5.1 Generated service caller

The caller pins once and never branches on whether a method is patched:

```rust,ignore
async fn handle_request(
    services: &ExampleServices,
    state: &mut RequestState,
    table: &Table,
    request: Request,
) -> Result<Response, ServiceError> {
    let root = services.pin();
    root.handler().handle(state, table, request).await
}
```

An ordinary Rust caller may also receive a borrowed result through the same
authored signature:

```rust,ignore
let root = services.pin();

let same: &mut RequestState = root.state().identity(&mut state);
let some: Option<&RequestState> = root.state().optional(&mut state, true);
let none: Option<&RequestState> = root.state().optional(&mut state, false);
let ok: Result<&RequestState, ServiceError> =
    root.state().checked(&mut state, true);
let err: Result<&RequestState, ServiceError> =
    root.state().checked(&mut state, false);
```

This code is identical for Rust-default and Vela-selected generations. Calls
made directly on `RustStateService` intentionally bypass Vela.

### 5.2 Generated Rust-to-Vela caller

Public non-service Vela declarations remain available through generated typed
Rust bindings:

```rust,ignore
include!(concat!(env!("OUT_DIR"), "/vela_bindings.rs"));

let mut package = vela_bindings::bind(&mut runtime)?;
let mut module = package.example_module();
let result: Response = module.recompute(&mut state, request)?;
```

The binding records stable callable, parameter, return, effect, and async ABI
facts. A compatible Vela body reload is re-resolved by stable identity; an ABI
change is rejected. Nested Rust-to-Vela calls bind through the active native
call context so they inherit the current root, generation, leases, budgets,
capabilities, cancellation, and tracing.

This entry is appropriate for explicitly script-owned workflows. It does not
provide a Rust default, sparse service composition, `base`, or conditional
service-generation activation.

## 6. Sparse Vela Patch

The Vela patch implements only methods that need correction.

### 6.1 Borrowed return through `base`

```vela
#[service_impl(example::state)]
impl StatePatch {
    fn identity(state) {
        return base.identity(state);
    }

    fn optional(state, present) {
        if !present {
            return Option::None {};
        }

        return base.optional(state, present);
    }

    fn checked(state, allowed) {
        if !allowed {
            return Result::Err(example::ServiceError {
                message: "blocked",
            });
        }

        return base.checked(state, allowed);
    }
}
```

The successful branches return the same direct `state` HostRef. Returning it
targets the sealed generated Rust-return sink; it is not permission to return
a HostRef from an ordinary Vela root. `None` and `Err(ServiceError)` carry no
HostRef.

### 6.2 Full orchestration

```vela
#[service_impl(example::handler)]
impl HandlerPatch {
    async fn handle(state, table, request) {
        // Root arguments may cross this await. No returned child exists yet.
        let baseline = base.handle(state, table, request).await?;

        let buffer = example::PatchBuffer::new(request.adjustment);
        services.transform.update_buffer(buffer, 2);
        let inspected_buffer = services.transform.inspect_buffer(buffer);

        let projected = table
            .rows()
            .filter(|row| row.base_score > 0)
            .map(|row| example::ValueRow {
                key: row.key,
                score: row.base_score + inspected_buffer,
            })
            .collect();

        let owned_total = services.transform.consume(projected);
        let shared_total = services.transform.inspect(projected);

        match table.get(request.key) {
            Option::Some(row) => {
                let score = services.policy.score(state, row, request)?;
                services.apply.apply(state, row, score)?;
                services.audit.record(state, row.key);
                return Result::Ok(example::Response {
                    accepted: true,
                    score,
                    inspected: owned_total + shared_total,
                });
            }
            Option::None => {
                return Result::Ok(example::Response {
                    accepted: false,
                    score: baseline.score,
                    inspected: owned_total + shared_total,
                });
            }
        }
    }
}
```

The example exercises:

- async Rust `base`;
- a call-scoped constructed Host;
- exclusive and shared Rust calls on that Host;
- a direct borrowed collection view;
- `filter`/`map`/`collect` producing a script-owned typed Array;
- automatic `Array<ValueRow> -> Vec<ValueRow>`;
- automatic `Array<ValueRow> -> temporary &[ValueRow]`;
- projected `Option<&Row>` through an ordinary registered Host method;
- exact-parameter `Option<&RequestState>` and
  `Result<&RequestState, ServiceError>` through the state patch;
- shared child field reads;
- passing one child to multiple later services;
- `&mut RequestState` immediate write-through;
- business `Result`; and
- same-generation cross-service calls.

The child `row` is not stored, captured, returned from the async root, or kept
across an await.

## 7. Parameter And Conversion Rules

### 7.1 Single values

| Rust target | Vela source | Result |
|---|---|---|
| Value `T` | exact registered Value | checked owned lowering |
| Value `&T` | exact registered Value | one temporary decoded Rust value |
| Value `&mut T` | any script Value | rejected |
| Host `T` | HostRef | rejected without consuming-host contract |
| Host `&T` | exact HostRef | shared lease |
| Host `&mut T` | exact exclusive HostRef | exclusive lease/write-through |

### 7.2 Collections

| Rust target | Vela source | Result |
|---|---|---|
| `Vec<T>` | typed script Array | one checked materialization |
| `&[T]` / `&Vec<T>` | typed script Array of Values | temporary Vec plus shared borrow |
| `&[T]` / `&Vec<T>` | exact Host ArrayView | zero-copy shared reborrow |
| `&mut [T]` | exact fixed Host ArrayMut | zero-copy fixed write-through |
| `&mut Vec<T>` | exact growable Host ArrayMut | zero-copy growable write-through |
| mutable Rust collection | script-owned Array/Map/Set | rejected |

Owned Map and Set lower recursively by the exact target concrete type.
BTree/Hash bindings retain distinct ABI even though Vela uses the common
MapLike/SetLike protocols.

### 7.3 Dynamic facts

Static exact facts are checked by the compiler. If a dynamic path yields an
erased container fact, the runtime performs a recursive guard before invoking
Rust. A wrong element, key, value, nominal record, Host type, or capability
fails before authored Rust executes.

No boundary uses JSON, runtime Serde reflection, implicit Host cloning, or
mutable copy-back.

## 8. Borrow And Lifetime Rules

The following succeed inside the current synchronous root call tree:

- local reads and method calls through shared children;
- nested shared reborrow from shared access, and shared/exclusive reborrow from
  exclusive access;
- passing a child to later Rust or Vela services;
- temporary local containers that do not escape;
- explicit `host::release(value)`; and
- the generated terminal Rust-return sink declared by the service ABI.

The following always fail:

- shared-to-exclusive promotion;
- duplicate overlapping exclusive aliases;
- persistent state/global/native-cache storage;
- escaping closure capture;
- ordinary Vela root return;
- use after explicit release or root teardown;
- stale generation or owner lease;
- child borrow live at async suspension;
- cross-Runtime/cross-root use; and
- nested borrowed owned containers such as `Vec<&T>`.

For a top-level `Result<&T, E>`, `Ok` follows the same child rules and `Err`
uses the registered owned Value codec for `E`. A scoped Result does not make
borrowed Results inside Array, Map, tuple, Option, or another Result legal.

Compiler-proven last use may release a child early. Dynamic and reflected paths
repeat the same lifetime and permission checks.

## 9. Hot-Update Deployment

The control plane compiles and validates away from request execution:

```rust,ignore
let old_root = services.pin();

let snapshot = ServiceUpdateBundle::snapshot(
    services.schema(),
    snapshot_artifact,
    snapshot_manifest,
)?;

assert!(services.dry_run_bundle(&old_root, &snapshot).accepted());

let candidate = services.stage_bundle(
    &old_root,
    snapshot,
    ServiceRuntimeBinding::for_context::<RequestState>(),
    call_options,
)?;

let rollback = services.activate_if_current(candidate)?;
let snapshot_root = services.pin();
```

Successive fixes use exact-base Delta:

```rust,ignore
let delta = ServiceUpdateBundle::delta(
    services.schema(),
    snapshot_root.generation_id(),
    snapshot_root.artifact_checksum().expect("Vela artifact"),
    delta_artifact,
    delta_manifest,
)?;

let candidate = services.stage_bundle(
    &snapshot_root,
    delta,
    ServiceRuntimeBinding::for_context::<RequestState>(),
    call_options,
)?;

services.activate_if_current(candidate)?;
```

Semantics:

- Snapshot omission selects Rust default.
- Delta omission inherits the exact base selection.
- Explicit `RustDefault` removes an inherited Vela method.
- Old roots continue on their old complete generation.
- New roots enter the newly published generation.
- `base` always means the registered Rust default.
- `services` always means the current root's pinned generation.
- stale base, ABI, type, effect, or artifact mismatch changes no live state.
- rollback republishes a prior generation and never retries or reverses Host
  effects.

Accepted Deltas may later be folded into an equivalent Snapshot.

## 10. Errors, Effects, Reflection, And Tooling

- A missing Vela method is resolved to Rust during staging.
- `Result::Err(E)` is an authored business value, distinct from a VM,
  capability, cancellation, or conversion failure.
- An error from a selected Vela method propagates; there is no automatic Rust
  fallback retry.
- Host writes already performed before error, cancellation, or panic are not
  automatically rolled back.
- A Vela method cannot exceed the Rust service method's effect ceiling.
- Runtime capabilities and allowlists remain deployment policy.
- Static, dynamic, reflected, generated, `base`, and `services` calls use the
  same type, capability, generation, provenance, and escape validators.
- Reflection may inspect metadata and perform controlled calls; it cannot
  mutate the service or TypeBinding schema.
- CLI schema, analysis, hover, completion, signature help, and generated Rust
  bindings consume the same sealed descriptors.

Transactional business behavior must be exposed as an explicit Rust
transaction/batch service; the VM does not invent rollback for arbitrary Host
effects.

## 11. Capability Checklist

| Area | Final capability |
|---|---|
| Rust caller | one generated service API, unchanged across patch selection |
| Rust default | direct path with no VM entry or HostRef conversion |
| Sparse patch | implement only faulty methods |
| `base` | call current service's registered Rust default |
| `services` | call Rust/Vela methods from one pinned generation |
| Ordinary Rust export | typed free function callable from Vela |
| Ordinary Host method | typed receiver call with receiver provenance |
| Generated Rust binding | typed call to public non-service Vela declaration |
| Value construction | record/enum or registered constructor |
| Host construction | explicit CallScoped or RuntimeOwned constructor |
| Host origins | Injected, Constructible, ProducedBorrow |
| Host fields/paths | controlled read/write with immediate write-through |
| Host collections | typed index, iteration, and registered protocols |
| Owned `T` | Value storage only |
| Shared `&T` | temporary Value borrow or shared HostRef |
| Mutable `&mut T` | exclusive HostRef only |
| Direct borrowed return | `&T`, `&mut T`, direct collection view |
| Optional borrowed return | exact synchronous `Option<&T>` |
| Fallible borrowed return | exact synchronous `Result<&T, E>` |
| None/Err | no HostRef and no lease; `E` uses its owned Value codec |
| Owned collection | target-directed checked materialization |
| Shared Value collection | invocation-scoped temporary borrow |
| Host collection view | zero-copy shared/exclusive reborrow |
| Mutable collection | exact Host view with immediate write-through |
| Async | root arguments and generation survive suspension |
| Child borrow + await | rejected |
| Dynamic/reflection | same checks as static calls |
| Errors | propagate, no Rust fallback retry |
| Effects/capabilities | sealed ceiling plus deployment grants |
| Snapshot | complete desired Vela selection over Rust defaults |
| Delta | exact-base inheritance and replacement |
| Old roots | remain on old complete generation |
| Rollback | conditional publication only |
| ABI/schema | incompatible candidate rejects before activation |
| Performance | no JSON/Serde Host conversion; Rust default bypasses VM |

## 12. Deliberately Unsupported

The final shape intentionally does not include:

- automatic patching of direct concrete Rust calls;
- arbitrary nested borrowed containers such as `Vec<&T>`;
- `Option<&mut T>` or `Result<&mut T, E>`;
- multi-origin borrowed returns;
- durable typed Host handles;
- borrowed children across root calls or async suspension;
- implicit Value `&mut T` or mutable collection copy-back;
- implicit construction of protected Host resources;
- owned Host `T` movement without an explicit consuming-host contract;
- service ABI/schema mutation without a new Rust build;
- async frame migration between generations;
- automatic transaction rollback;
- arbitrary monkey patching; or
- a second hotfix model beside generated service generations.

When a real service signature needs another shape, it must either use a thin
domain-neutral adapter built from the admitted grammar or extend the whitelist
with ABI, conversion, lifetime, tooling, dynamic/reflection, performance, and
negative-path tests before registration accepts it.

## 13. Review Questions

Before the completion plan closes, verify that a representative host can
answer yes to all of these:

1. Can every hotfixable Rust call site be shown to cross the generated service
   contract rather than a concrete implementation?
2. Can a Vela patch express the Rust default's control flow using the same
   inputs, registered constructors, `base`, and `services`?
3. Does every required Host parameter have an Injected, Constructible, or
   ProducedBorrow origin?
4. Can the patch create scratch mutable Rust objects without retaining them
   until Runtime drop?
5. Can transformed Value collections pass to owned and shared Rust collection
   parameters without handwritten adapters?
6. Do mutable Rust parameters always refer to exact Host-backed state?
7. Can direct, optional, and fallible borrowed results flow through Vela and
   back to the ordinary Rust caller without copying or escaping?
8. Are all unsupported signatures rejected before Engine construction or
   candidate activation?
9. Do old roots, nested calls, async suspension, Delta inheritance, and
   rollback preserve one coherent generation?
10. Can dynamic/reflection calls, errors, cancellation, and permission denial
    be shown not to bypass the same safety model?
