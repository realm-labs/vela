# Rust/Vela Service Patchability — Final Usage Shape

> Status: accepted final usage contract after the explicit-release and typed
> Service namespace hard switch.
>
> The generation/deployment model, direct/optional/fallible borrowed service
> returns to ordinary Rust callers, explicit call-scoped Host construction,
> and storage-directed shared Value/Host parameter lowering are implemented.
> Host-origin reporting is exported through the sealed CLI/LSP schema. The
> consolidated coverage demo and repository-wide validation gates are accepted.
> The completed work is recorded in the
> [archived plan](archive/rust-vela-service-patchability-completion-plan.md)
> and
> [acceptance report](archive/rust-vela-service-patchability-acceptance-2026-07-25.md).
> E0-E5 interop acceptance is recorded in the
> [hard-switch report](archive/rust-vela-interop-hard-switch-acceptance-2026-07-31.md).
> The [final interop contract](rust-vela-interop-final-shape-hard-switch-plan.md)
> supersedes the former compiler-driven early-release rule and the incomplete
> non-`'static` Host `base` path.

## 1. User-Facing Guarantee

The final service model has one guarantee:

```text
If a generated service domain application builds successfully, every admitted method can be
selected by Vela and called through its authored Rust signature.
```

The same Rust caller works before and after a Vela patch. Vela may:

- call the current method's Rust default through `service::base`;
- call other methods from the same pinned generation through
  `service::pinned`;
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

Non-`'static` Host parameter types use a generated root-local typed thunk for
nested `service::base` and `service::pinned` Rust calls. The reviewed erased
reborrow is invocation-scoped and runs only after exact type, generation,
alias, capability, and lease validation. Vela still receives only HostRefs;
it never receives a real Rust reference or uses `Any` to recover one.

Erased Host-contract methods use the detached `HostCallValue` vocabulary.
Besides scalar `HostValue` data and HostRefs, it preserves tuples, arrays,
maps, sets, records, and enums. Generated or handwritten adapters use
`decode_host_call_arg` and `encode_host_call_return`, so a method can receive
and return the same derived Rust `Value` types as a statically registered
native thunk. Runtime-only values such as closures, iterators, ranges, and
PathProxies are not detached method arguments; they retain their dedicated
runtime or HostRef protocols.

## 2. Complete Example Model

The example uses only domain-neutral table, row, request, policy, transform,
apply, and audit concepts.

### 2.1 Owned Value types

Values are copied by typed field/element lowering. They are appropriate for
requests, responses, errors, commands, projections, and transformed
collections.

```rust,ignore
#[derive(Clone, Debug, vela_macros::Value)]
#[vela(path = "example::Request")]
pub struct Request {
    pub key: i64,
    pub adjustment: i64,
}

#[derive(Clone, Debug, vela_macros::Value)]
#[vela(path = "example::ValueRow")]
pub struct ValueRow {
    pub key: i64,
    pub score: i64,
}

#[derive(Clone, Debug, vela_macros::Value)]
#[vela(path = "example::Response")]
pub struct Response {
    pub accepted: bool,
    pub score: i64,
    pub inspected: i64,
}

#[derive(Clone, Debug, vela_macros::Value)]
#[vela(path = "example::ServiceError")]
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
#[vela(path = "example::Row")]
pub struct Row {
    #[vela(get)]
    pub key: i64,
    #[vela(get)]
    pub base_score: i64,
}

#[derive(vela_macros::ScriptHost)]
#[vela(path = "example::Table")]
pub struct Table {
    #[vela(skip)]
    rows: Vec<Row>,
}

#[derive(vela_macros::ScriptHost)]
#[vela(path = "example::RequestState")]
pub struct RequestState {
    #[vela(get)]
    total: i64,
    #[vela(skip)]
    services: ExampleServicesRoot,
}
```

`Table` and `RequestState` are Injected Host types. Vela can use them only
because the Rust caller supplies them to a service root. Merely registering
their schemas does not grant construction authority.

### 2.3 Borrowed call-scoped Host contexts

A business context may itself borrow state and therefore have no `'static`
Rust type:

```rust,ignore
pub struct RequestContext<'ctx, A> {
    actor: &'ctx mut A,
}

impl<A> ScriptHostSchema for RequestContext<'_, A> {
    fn script_host_type_desc() -> TypeDesc {
        request_context_contract()
    }
}

impl<A: Send> ScriptHostObject for RequestContext<'_, A> {
    fn call_resolved_host(
        &mut self,
        access: ResolvedHostAccess,
        target: HostTargetInstance<'_>,
        method: HostMethodId,
        args: &[HostCallValue],
    ) -> HostResult<HostCallValue> {
        match method {
            PROCESS => {
                let request: Request = decode_host_call_arg(&args[0])?;
                let response = self.process(request)?;
                encode_host_call_return(response)
            }
            _ => self.dispatch_generated_method(access, target, method, args),
        }
    }
}

#[vela_macros::service(path = "example::handler")]
pub trait HandlerService: Send + Sync {
    async fn handle(
        &self,
        context: &mut RequestContext<'_, OrderActor>,
        request: Request,
    ) -> Result<Response, ServiceError>;
}

#[vela_macros::service_domain]
pub struct ExampleServices {
    pub handler: Service<dyn HandlerService>,
}
```

The engine registers the static schema and erased method vtables, not the
borrowed Rust instantiation:

```rust,ignore
let app = ExampleServices::builder(
    Engine::builder().register_host_type(request_context_host_spec()),
)
.handler(RustHandlerService)
.build()?;
```

The normal generated service caller keeps its authored Rust signature. At the
low-level Runtime boundary the same instance can be supplied with
`CallArgs::new().with_host_mut("context", &mut context)`. No scoped variant of
that method exists, and the business context stores no Vela Runtime slot.

### 2.4 Constructible scratch Host

A patch may need a new mutable Rust object that is not returned by another
service. That object uses an explicit call-scoped Host constructor:

```rust,ignore
#[derive(vela_macros::ScriptHost)]
#[vela(path = "example::PatchBuffer")]
pub struct PatchBuffer {
    #[vela(get)]
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

## 4. Rust Defaults And Service Domain

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

The service domain declares its service schema explicitly:

```rust,ignore
#[vela_macros::service_domain(context = RequestState)]
pub struct ExampleServices {
    pub state: Service<dyn StateService>,
    pub policy: Service<dyn PolicyService>,
    pub apply: Service<dyn ApplyService>,
    pub transform: Service<dyn TransformService>,
    pub audit: Service<dyn AuditService>,
    pub handler: Service<dyn HandlerService>,
}
```

The application builder receives real default instances, closes every
transitive Value/Host/container requirement, seals the Engine, validates the
schema, and creates the initial Rust generation in one terminal operation:

```rust,ignore
let mut bindings = VelaBindings::new();
bindings.register_type(Row::vela_type());
bindings.register_type(Table::vela_type());
bindings.register_type(RequestState::vela_type());
bindings.register_type(TypeRegistration::binding(patch_buffer));
let app = ExampleServices::builder(
    Engine::builder()
        .capability(Capability::HostRead)
        .capability(Capability::HostWrite)
        .register_bindings(bindings),
)
.state(RustStateService)
.policy(RustPolicyService::new(policy_config.clone()))
.apply(RustApplyService::new(store.clone()))
.transform(RustTransformService)
.audit(RustAuditService::new(audit_sink.clone()))
.handler(RustHandlerService)
.call_options(call_options)
.build()?;
```

Missing storage capability, constructor lifetime, codec, collection fact, or
adapter rejects construction before any request executes. A missing default
also rejects construction. Published generations retain the exact supplied
instances behind `Arc`; staging never reconstructs defaults from types.

## 5. Unchanged Rust Caller

### 5.1 Generated service caller

The caller pins once and never branches on whether a method is patched:

```rust,ignore
async fn handle_request(
    app: &ExampleServicesApp,
    state: &mut RequestState,
    table: &Table,
    request: Request,
) -> Result<Response, ServiceError> {
    app.with_request_async(state, async |services, state| {
        services.handler().handle(state, table, request).await
    }).await
}
```

An ordinary Rust caller may also receive a borrowed result through the same
authored signature:

```rust,ignore
app.with_request(&mut state, |root, state| {
    let same: &mut RequestState = root.state().identity(state);
    let some: Option<&RequestState> = root.state().optional(state, true);
    let none: Option<&RequestState> = root.state().optional(state, false);
    let ok: Result<&RequestState, ServiceError> =
        root.state().checked(state, true);
    let err: Result<&RequestState, ServiceError> =
        root.state().checked(state, false);
});
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
provide a Rust default, sparse service composition, `service::base`, or
conditional service-generation activation.

## 6. Sparse Vela Patch

The Vela patch implements only methods that need correction.
The compiler-owned `service` namespace is available only inside these methods:
`service::base::method(...)` calls the current Rust default, while
`service::pinned::service_name::method(...)` calls through the root-pinned
generation. Neither path is a value, and the former bare `base` / `services`
receivers are rejected.

### 6.1 Borrowed return through `service::base`

```vela
#[service_impl(example::state)]
impl StatePatch {
    fn identity(state) {
        return service::base::identity(state);
    }

    fn optional(state, present) {
        if !present {
            return Option::None {};
        }

        return service::base::optional(state, present);
    }

    fn checked(state, allowed) {
        if !allowed {
            return Result::Err(example::ServiceError {
                message: "blocked",
            });
        }

        return service::base::checked(state, allowed);
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
        let baseline =
            service::base::handle(state, table, request).await?;

        let buffer = example::PatchBuffer::new(request.adjustment);
        service::pinned::transform::update_buffer(buffer, 2);
        let inspected_buffer =
            service::pinned::transform::inspect_buffer(buffer);

        let projected = table
            .rows()
            .filter(|row| row.base_score > 0)
            .map(|row| example::ValueRow {
                key: row.key,
                score: row.base_score + inspected_buffer,
            })
            .collect();

        let owned_total = service::pinned::transform::consume(projected);
        let shared_total = service::pinned::transform::inspect(projected);

        match table.get(request.key) {
            Option::Some(row) => {
                let score =
                    service::pinned::policy::score(state, row, request)?;
                service::pinned::apply::apply(state, row, score)?;
                service::pinned::audit::record(state, row.key);
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

- async Rust `service::base`;
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
- named local aliases that do not escape;
- explicit `host::release(value)` or `host::try_release(value)`; and
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

Only authored strict `host::release` or idempotent `host::try_release` releases
a retained child early. `try_release` returns `false` for an alias group already
released in the same root but preserves `NotScopedBorrow`, `BorrowStillInUse`,
and other Host errors. A generated terminal Service sink may transfer the exact
admitted borrow to the original Rust caller, and root teardown remains the
unconditional safety cleanup. Compiler liveness and lexical scope never release
a child. Dynamic and reflected paths repeat the same lifetime and permission
checks.

## 9. Hot-Update Deployment

The application patch facade compiles, validates, stages, and publishes away
from request execution:

```rust,ignore
let _grant_rollback = app
    .patches()
    .apply(PatchEdit::put("rules/grant.vela", grant_source))?;

let revision = app.patches().revision()?;
let staged = app.patches().stage(
    ServicePatch::against(&revision)
        .put("rules/audit.vela", audit_source)
        .remove("rules/obsolete.vela"),
)?;
let generation = staged.generation_id();
let rollback = staged.activate()?;
```

`PatchEdit` is the common one-file path. `ServicePatch::against` batches
several exact-base edits, while
`ServicePatch::replace(PatchSources::from_files(...))` installs a complete
workspace after a source-less deployment. Every form recompiles the complete
resulting Snapshot; omitted service methods select their Rust defaults.

The facade hides source ingestion, whole-workspace compilation, linking,
Runtime binding, call options, base pinning, revision publication, and
publication checks. A control plane can also compile a detached revision into
a portable bundle when `vela_engine`'s `artifact-codec` feature is enabled:

```rust,ignore
let revision = PatchRevision::empty().apply(
    PatchEdit::put("gm/operation.vela", operation_source),
)?;
let portable = engine.compile_portable_service_patch(
    service_schema,
    &revision,
    host_schema_hash,
)?;
let bytes = portable.encode()?;
```

Portable program, Service bundle, and detached Service metadata format version
2 are the first formats with explicit scoped-resource release semantics.
Version 1 input is rejected before staging or activation; there is no legacy
loader or compatibility interpretation.

The receiving Actor decodes and validates the portable bundle, loads it
against its sealed Engine/schema, and stages it through the same facade:

```rust,ignore
let portable = PortableServiceUpdateBundle::decode(&bytes)?;
let bundle = portable.load(app.engine(), app.domain().schema(), host_schema_hash)?;
assert!(app.patches().dry_run_bundle(&bundle).accepted());
let bundle_rollback = app.patches().stage_bundle(bundle)?.activate()?;

// Activation does not execute the implementation.
app.with_request(&mut actor, |services, actor| {
    services.operation().execute(actor, request)
})?;

app.patches().rollback(bundle_rollback)?;
```

Actor routing, authorization, parameters, invocation, and result delivery are
business responsibilities. A Service implementation has no implicit entry
instruction: compilation, distribution, loading, staging, and activation are
side-effect free with respect to business logic. The actor mailbox should
activate and invoke the intended generation in one serialized turn.

Semantics:

- Snapshot omission selects Rust default.
- Delta omission inherits the exact base selection.
- Explicit `RustDefault` removes an inherited Vela method.
- Old roots continue on their old complete generation.
- New roots enter the newly published generation.
- `service::base` always means the registered Rust default.
- `service::pinned` always means the current root's pinned generation.
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
- Static, dynamic, reflected, generated, `service::base`, and
  `service::pinned` calls use the same type, capability, generation,
  provenance, and escape validators.
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
| `service::base` | call current service's registered Rust default |
| `service::pinned` | call Rust/Vela methods from one pinned generation |
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
   inputs, registered constructors, `service::base`, and `service::pinned`?
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
