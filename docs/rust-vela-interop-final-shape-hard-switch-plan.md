# Rust/Vela Interop Final Shape And Explicit Release Hard Switch

Status: active normative target and implementation plan.

This document defines the final Rust/Vela interaction model for ordinary Vela
calls and generated Services. It supersedes two previously accepted behaviors:

- compiler-proven last-use and lexical-scope release of scoped Host borrows; and
- accepting a Service signature while generating a runtime failure for a
  `base` call involving a non-`'static` call-scoped Host parameter.

The hard switch has no source-compatibility mode, no Engine option that restores
implicit release, no legacy bytecode interpretation, and no second Service
dispatch path. Old source, examples, tests, and precompiled artifacts are
updated or rejected.

The stable product constraints remain unchanged:

- Vela never stores or observes a real Rust `&T` or `&mut T`;
- Rust-owned state remains outside the script GC;
- Host mutation crosses `HostRef`, `HostPath`, `PathProxy`, and `HostAccess`;
- reflection cannot mutate type structure;
- general script-language generics and monkey patching remain unsupported; and
- generated service generations remain the sole Rust hotfix model.

## 1. Completion Definition

The hard switch is complete only when both of these invariants hold.

### 1.1 Explicit scoped-resource lifetime

Any Host capability that remains live across Vela statements is released only
by:

1. an authored `host::release(value)` call;
2. an explicit terminal capability transfer defined by a generated Service
   return sink; or
3. unconditional root teardown after success, error, panic, cancellation, or
   future drop.

The compiler never inserts an early release based on liveness, lexical scope,
branch convergence, temporary death, register reuse, or iterator exhaustion.
GC timing never releases a Host capability.

### 1.2 Total Service admission

A generated Service method is admissible only when all required directions
have complete executable adapters:

```text
direct Rust default
Rust caller -> Vela selection
Vela selection -> Rust base
Vela selection -> pinned Rust/Vela service
sync or async completion
result restoration to the authored Rust caller
```

An admitted branch may not defer unsupported behavior to a runtime panic,
placeholder, `TypeMismatch`, or automatic fallback. Macro expansion or
service-domain sealing rejects an incomplete method before Engine construction.

## 2. One Boundary Model

Ordinary Rust exports and Services share one boundary model:

```text
CallableContract
TypeBinding
InteropTypeId
Value codec
Host identity and capability
atomic lease preflight
scoped-resource return contract
explicit release semantics
async and escape rules
```

Service dispatch adds only:

```text
RustDefault or Vela selection
lexical base
lexical pinned services
one immutable published generation
Snapshot and exact-base Delta deployment
```

Type conversion, Host access, borrowed-result lifetime, permissions, effects,
diagnostics, reflection checks, and tooling facts do not depend on whether a
callable is an ordinary export or a Service method.

## 3. Lifetime Vocabulary

The implementation and tooling distinguish the following categories. These
are compiler-owned restricted type facts, not user-definable generic types.

| Category | Example origin | Crosses statements | Authored release |
|---|---|---:|---:|
| Owned Value | `ItemGrant`, `Vec<i64>` | yes | no |
| Root Host | Rust `CallArgs`, injected Service argument | root lifetime | no |
| Runtime-owned Host | explicit registered constructor | yes | separate owner policy |
| Invocation lease | Rust parameter `&T` or `&mut T` | no | no |
| Scoped View | Rust returned `&T` | yes, within one root | yes |
| Scoped MutView | Rust returned `&mut T` | yes, within one root | yes |
| Scoped Host iterator | lazy Host traversal retaining a lease | yes, within one root | yes |
| Terminal Service borrow | exact Service borrow returned to Rust | consumed by sink | no |

Tooling should display the distinction even though all Host categories use
compact HostRef-backed runtime values:

```text
Inventory                       root Host
View<Item>                      scoped shared Host capability
MutView<Item>                   scoped exclusive Host capability
ScopedIterator<View<Item>>      scoped traversal capability
```

`View`, `MutView`, and `ScopedIterator` are restricted builtin type-hint
families. They do not enable user-defined generic functions, structs, traits,
or implementations.

## 4. Explicit Release Contract

### 4.1 What remains automatic

Rust invocation leases remain RAII-scoped because Vela never receives them.

Supported Rust:

```rust,ignore
#[vela_macros::export(path = "inventory::adjust")]
pub fn adjust(inventory: &mut Inventory, delta: i64) -> i64 {
    inventory.total += delta;
    inventory.total
}
```

Supported Vela:

```vela
fn apply(inventory: Inventory, delta: i64) -> i64 {
    return inventory::adjust(inventory, delta);
}
```

The generated adapter atomically validates the Host argument set, creates the
Rust `&mut Inventory`, calls `adjust`, and drops that Rust borrow when the call
returns. There is no Vela value representing this invocation lease and
therefore nothing for the author to release.

Async native calls retain their invocation leases until their future completes
or is dropped:

```rust,ignore
#[vela_macros::export(path = "inventory::flush")]
pub async fn flush(inventory: &mut Inventory) -> Result<Receipt, FlushError> {
    // The invocation-scoped Rust borrow is retained by this future.
}
```

```vela
let receipt = inventory::flush(inventory).await?;
```

Completion, error, cancellation, panic, and future drop release the invocation
lease through RAII. This behavior is mandatory for Rust soundness and is not
script-level early release.

### 4.2 What becomes explicitly managed

A Rust callable that returns a Host borrow creates a scoped capability:

```rust,ignore
#[vela_macros::methods(path = "inventory::Inventory")]
impl Inventory {
    pub fn item(&self, id: i64) -> Option<&Item> {
        self.items.get(&id)
    }

    pub fn item_mut(&mut self, id: i64) -> Option<&mut Item> {
        self.items.get_mut(&id)
    }
}
```

Supported Vela:

```vela
fn increase(inventory: Inventory, id: i64) -> Result<i64, String> {
    let item = inventory.item_mut(id)
        .ok_or("unknown item")?;
    item.count += 1;
    let count = item.count;
    host::release(item);
    return Result::Ok(count);
}
```

The capability remains live until `host::release(item)` executes. Merely
reading `item.count` for the last time does not release it.

### 4.3 Alias groups

Copying a scoped Host value copies only its compact handle. All aliases share
one `BorrowLeaseId`.

Supported Vela:

```vela
let item = inventory.item_mut(id)?;
let alias = item;
alias.count += 1;
host::release(item);
```

After release, every alias fails deterministically:

```vela
// Runtime error: ExpiredBorrowedHostRef.
return alias.count;
```

Release is not reference counting. It invalidates the complete alias group in
one operation.

### 4.4 Parent and child order

Nested scoped resources release from child to parent.

Supported Vela:

```vela
let table = catalog.table();
let row = table.row(id)?;
let value = row.value;

host::release(row);
host::release(table);

return value;
```

Unsupported Vela:

```vela
let table = catalog.table();
let row = table.row(id)?;

// Runtime error: BorrowStillInUse.
host::release(table);
host::release(row);
```

Distinct sibling borrow groups release independently:

```vela
let left = team.member_mut(left_id)?;
let right = team.member_mut(right_id)?;

left.score += 1;
right.score += 1;

host::release(left);
// The parent remains frozen by right.
host::release(right);
```

### 4.5 Control flow

Vela does not infer that all branches ended a borrow.

Supported Vela:

```vela
let item = inventory.item_mut(id)?;
if enabled {
    item.enable();
} else {
    item.disable();
}
host::release(item);
```

Also supported, with deliberately path-dependent lifetime:

```vela
let item = inventory.item_mut(id)?;
if release_now {
    host::release(item);
}

// This succeeds only on paths where the group remains live.
// If release_now was true, it fails as ExpiredBorrowedHostRef.
return item.count;
```

The runtime does not block and does not guess author intent.

### 4.6 Await boundaries

No live non-suspendable scoped resource may enter an await boundary.

Supported Vela:

```vela
let item = inventory.item_mut(id)?;
item.pending = true;
host::release(item);

timer::sleep(1).await;
```

Unsupported Vela:

```vela
let item = inventory.item_mut(id)?;
item.pending = true;

// Error before the awaited call starts:
// live scoped mutable Host capability created by item_mut;
// release it with host::release(item).
timer::sleep(1).await;
```

The check happens at every await boundary, before polling the target. Behavior
does not depend on whether the future would have returned `Ready` immediately.
The check reads the ExecutionHost's complete active scoped-resource table; it
does not use local-variable liveness. Losing the last Vela reference without
releasing it therefore still blocks await until root teardown.

Initial final-shape rules:

- root Host arguments may be used by awaited Rust, `base`, and `services`
  calls;
- invocation leases owned by the awaited native future may cross suspension;
- Vela-held scoped View, MutView, and scoped Host iterators may not cross
  suspension;
- async Rust callables may not return call-scoped borrows; and
- no ready-future exception exists.

### 4.7 Root teardown

Root teardown always invalidates remaining scoped resources and releases:

- direct HostRef slot registrations;
- retained borrowed-return children;
- call-scoped constructed Host objects;
- pending native/service invocation leases; and
- pinned code and service generations.

This applies to success, authored error, VM error, panic, cancellation, and
future drop. It is an unconditional safety boundary, not an authored early
release path.

Forgetting release at the end of a root does not leak into another root:

```vela
fn inspect(inventory: Inventory, id: i64) -> Option<i64> {
    let item = inventory.item(id)?;
    // No later conflicting call and no await.
    // Root teardown still cleans this lease.
    return Option::Some(item.count);
}
```

Authors should normally release explicitly for clarity, but root teardown
remains the final cleanup authority.

### 4.8 Double release and invalid targets

Unsupported Vela:

```vela
let item = inventory.item(id)?;
host::release(item);
host::release(item); // ExpiredBorrowedHostRef
```

Unsupported Vela:

```vela
// inventory is a root Host, not a scoped returned capability.
host::release(inventory); // NotScopedBorrow
```

`host::release` is the sole release spelling. No bare `release`, compatibility
alias, destructor protocol, or GC hook is added.

### 4.9 Scoped producer results must be nameable

Purely explicit release requires authors to retain a handle for every produced
scoped resource. A scoped producer result must therefore be:

- bound to a local;
- forwarded to another script function that takes responsibility for it;
- consumed by `host::release`; or
- transferred to an admitted generated Service return sink.

Unsupported discarded result:

```vela
inventory.item_mut(id); // rejected: discarded scoped Host result
```

Unsupported implicit borrowed-result chain:

```vela
// tables() and item() both return scoped Host borrows. The intermediate
// capabilities would not be available for explicit release.
return config.tables().item(id)?.value;
```

Supported explicit form:

```vela
let tables = config.tables();
let item = tables.item(id)?;
let value = item.value;

host::release(item);
host::release(tables);

return value;
```

Ordinary scalar and owned-value HostPath chaining remains supported because it
does not retain a new scoped capability:

```vela
account.ledger.entries[entry_id].amount += 1;
```

The compiler diagnoses discarded and unnameable scoped producer results. This
is a local result-category check, not a general borrow checker or last-use
analysis.

## 5. Supported Ordinary Rust/Vela Interop

### 5.1 Scalars, strings, bytes, and owned Values

Supported Rust:

```rust,ignore
#[derive(Clone, vela_macros::Value)]
#[vela(path = "inventory::Request")]
pub struct Request {
    pub item_id: i64,
    pub amount: i64,
    pub tags: Vec<String>,
}

#[derive(Clone, vela_macros::Value)]
#[vela(path = "inventory::Reply")]
pub enum Reply {
    Applied { total: i64 },
    Rejected { reason: String },
}

#[vela_macros::export(path = "inventory::validate")]
pub fn validate(request: Request) -> Result<Request, String> {
    if request.amount > 0 {
        Ok(request)
    } else {
        Err("amount must be positive".to_owned())
    }
}
```

Supported Vela:

```vela
fn validate_request(request: Request) -> Result<Request, String> {
    return inventory::validate(request);
}
```

The owned structural grammar includes supported scalars, String, Bytes,
registered Value records and enums, Option, Result, tuples, and recursively
supported owned Array, Map, and Set values. Conversion is generated structural
lowering, not JSON or runtime Serde reflection.

### 5.2 Shared Value borrows

Supported Rust:

```rust,ignore
#[vela_macros::export(path = "inventory::score")]
pub fn score(request: &Request) -> i64 {
    request.amount + request.tags.len() as i64
}
```

Supported Vela:

```vela
let score = inventory::score(request);
```

When `Request` uses Value storage, the adapter decodes one invocation-local
temporary and lends `&Request` only for the Rust call. No HostRef or authored
release is involved.

### 5.3 Root Host parameters

Supported Rust:

```rust,ignore
#[derive(vela_macros::ScriptHost)]
#[vela(path = "inventory::Inventory")]
pub struct Inventory {
    pub total: i64,
}

#[vela_macros::export(path = "inventory::read_total")]
pub fn read_total(inventory: &Inventory) -> i64 {
    inventory.total
}

#[vela_macros::export(path = "inventory::add")]
pub fn add(inventory: &mut Inventory, amount: i64) -> i64 {
    inventory.total += amount;
    inventory.total
}
```

Supported Vela:

```vela
let before = inventory::read_total(inventory);
let after = inventory::add(inventory, 5);
```

The call adapter validates all shared/exclusive aliases before any Rust
reference exists. The Rust reference remains invocation-scoped and releases
through RAII.

### 5.4 Host fields, paths, indexes, and methods

Supported Vela:

```vela
inventory.total += request.amount;
inventory.entries[request.item_id].count += 1;
inventory.record(request.item_id, request.amount);
```

Static access uses linked stable IDs and prepared HostTarget plans. Dynamic and
reflected access repeats type, capability, permission, generation, and lease
checks.

### 5.5 Direct, optional, and fallible borrowed returns

Supported Rust:

```rust,ignore
#[vela_macros::methods(path = "inventory::Inventory")]
impl Inventory {
    pub fn current(&self) -> &Item {
        &self.current
    }

    pub fn find(&self, id: i64) -> Option<&Item> {
        self.items.get(&id)
    }

    pub fn require(&self, id: i64) -> Result<&Item, LookupError> {
        self.items.get(&id).ok_or(LookupError::Missing)
    }
}
```

Supported Vela:

```vela
let current = inventory.current();
let current_id = current.id;
host::release(current);

let optional = inventory.find(id);
match optional {
    Option::Some(item) => {
        let count = item.count;
        host::release(item);
    }
    Option::None => {}
}

let item = inventory.require(id)?;
let count = item.count;
host::release(item);
```

`None` and `Err` create no HostRef and retain no Host lease.

### 5.6 Borrowed collection views

Supported Rust:

```rust,ignore
#[vela_macros::methods(path = "inventory::Inventory")]
impl Inventory {
    pub fn items(&self) -> &[Item] {
        self.items.as_slice()
    }

    pub fn items_mut(&mut self) -> &mut [Item] {
        self.items.as_mut_slice()
    }
}
```

Supported Vela:

```vela
let items = inventory.items();
let count = items.len();
host::release(items);

let items_mut = inventory.items_mut();
items_mut[0].count += 1;
host::release(items_mut);
```

Shared views reject mutation. Fixed mutable slices allow element replacement
but reject structural growth. Mutable Vec/Map/Set views may expose registered
write-through structural operations.

### 5.7 Owned and borrowed collection parameters

Supported Rust:

```rust,ignore
#[vela_macros::export(path = "inventory::sum")]
pub fn sum(values: &[i64]) -> i64 {
    values.iter().sum()
}

#[vela_macros::export(path = "inventory::replace")]
pub fn replace(values: &mut Vec<i64>, next: i64) {
    values.clear();
    values.push(next);
}
```

Supported Vela Value materialization:

```vela
let values = [1, 2, 3];
let total = inventory::sum(values);
```

Supported zero-copy Host view:

```vela
let values = actor.values_mut();
inventory::replace(values, 9);
host::release(values);
```

A script-owned Array cannot satisfy `&mut Vec<T>` through implicit copy-back.

### 5.8 Explicit Rust protocols and external types

Selected inherent methods and explicitly exported Rust trait methods are
supported. Implementing a Rust trait alone exposes nothing.

Supported Rust:

```rust,ignore
#[vela_macros::trait_export(path = "inventory::Counted")]
pub trait Counted {
    fn count(&self) -> i64;
}
```

Supported Vela:

```vela
fn count(value: Counted) -> i64 {
    return value.count();
}
```

External local Rust types may use centralized `external_host` companion
registration. They enter the same TypeBinding and callable registry rather
than a generator-specific bridge.

### 5.9 Rust calling Vela

Generated Rust bindings remain the normal typed Rust-to-Vela path:

```rust,ignore
let mut bindings = vela_bindings::bind(&mut runtime)?;
let mut module = bindings.inventory_module();
let reply = module.apply(&mut inventory, request)?;
```

Generated calls use stable callable identity and exact ABI fingerprints.
Compatible body reload re-resolves the target; incompatible parameter,
representation, return, effect, or async changes fail before invocation.

## 6. Supported Service Patchability

### 6.1 Rust Service authoring

Supported Rust:

```rust,ignore
#[vela_macros::service(path = "inventory::operation")]
pub trait OperationService: Send + Sync {
    fn apply(
        &self,
        inventory: &mut Inventory,
        request: Request,
    ) -> Result<Reply, ApplyError>;
}
```

The ordinary Rust caller always calls the generated Service API. It does not
inspect patch state, target strings, Runtime values, or HostRefs.

### 6.2 Sparse Vela patch with `base`

Supported Vela:

```vela
#[service_impl(inventory::operation)]
impl OperationPatch {
    fn apply(
        inventory: Inventory,
        request: Request,
    ) -> Result<Reply, ApplyError> {
        if request.amount <= 0 {
            return Result::Err(ApplyError::InvalidAmount);
        }

        return base.apply(inventory, request);
    }
}
```

`base` always means the registered Rust default, never the previous Vela body.
It is a compiler-owned lexical capability and cannot be stored, returned,
captured, reflected, or dynamically invoked.

### 6.3 Non-`'static` call-scoped Host with `base`

Supported final Rust:

```rust,ignore
pub struct RequestContext<'request> {
    pub request_id: &'request str,
    pub total: &'request mut i64,
}

#[vela_macros::service(path = "request::handler")]
pub trait HandlerService: Send + Sync {
    async fn handle(
        &self,
        context: &mut RequestContext<'_>,
        request: Request,
    ) -> Result<Reply, HandleError>;
}
```

Supported final Vela:

```vela
#[service_impl(request::handler)]
impl HandlerPatch {
    async fn handle(
        context: RequestContext,
        request: Request,
    ) -> Result<Reply, HandleError> {
        context.record_attempt(request.item_id);
        return base.handle(context, request).await;
    }
}
```

The generated outer-call thunk retains the concrete Rust type knowledge needed
to invoke the authored default. The root HostRef, stable InteropTypeId,
generation, and complete lease set are validated before the typed thunk
reborrows the call-scoped object. This path does not use `Any` and never stores
a Rust reference in a Vela Value.

### 6.4 Cross-Service pinned calls

Supported Vela:

```vela
#[service_impl(request::handler)]
impl HandlerPatch {
    async fn handle(
        context: RequestContext,
        request: Request,
    ) -> Result<Reply, HandleError> {
        services.audit.record(context, request).await?;
        return base.handle(context, request).await;
    }
}
```

If `audit.record` is Vela-selected, it runs the target patch. A `base` call
inside that target patch invokes the Audit Service's Rust default. Returning to
the caller and invoking `base.handle` uses the same root-pinned immutable
generation.

### 6.5 Service borrowed return terminal transfer

Supported Rust:

```rust,ignore
#[vela_macros::service(path = "inventory::selection")]
pub trait SelectionService: Send + Sync {
    fn choose<'a>(&self, item: &'a Item, enabled: bool) -> Option<&'a Item>;
}
```

Supported Vela:

```vela
#[service_impl(inventory::selection)]
impl SelectionPatch {
    fn choose(item: Item, enabled: bool) -> Option<Item> {
        if enabled {
            // The generated terminal sink consumes this exact scoped
            // capability and restores the authored Rust borrow.
            return Option::Some(item);
        }
        return Option::None;
    }
}
```

The terminal sink validates exact type, canonical object identity, generation,
borrow origin, capability, and envelope. This is an explicit capability
transfer to the original Rust caller, not `host::release` and not an ordinary
Vela root return.

If a patch obtains another scoped child and does not return it through the
terminal contract, it must release that child explicitly.

### 6.6 Async Service behavior

Supported Vela:

```vela
#[service_impl(request::handler)]
impl HandlerPatch {
    async fn handle(
        context: RequestContext,
        request: Request,
    ) -> Result<Reply, HandleError> {
        let current = context.current_item();
        let allowed = current.enabled;
        host::release(current);

        if !allowed {
            return Result::Err(HandleError::Disabled);
        }

        return base.handle(context, request).await;
    }
}
```

The root Host arguments and pinned generation survive suspension. Scoped
borrowed children must be released before the await.

### 6.7 Service publication

Snapshot, exact-base Delta, and rollback semantics remain unchanged:

- Snapshot omission selects the Rust default;
- Delta omission inherits the exact base selection;
- explicit `RustDefault` removes an inherited Vela selection;
- old roots continue on their pinned generation;
- new roots enter the newly published generation;
- activation does not execute business logic;
- errors do not retry through Rust fallback; and
- rollback republishes a prior generation without reversing Host effects.

## 7. Supported Boundary Matrix

| Rust boundary shape | Ordinary export | Service | Vela representation |
|---|---:|---:|---|
| scalar, String, Bytes | yes | yes | owned Value |
| registered Value record/enum | yes | yes | owned structural Value |
| Option/Result/tuple of owned Values | yes | yes | owned structural Value |
| owned Vec/Map/Set of Values | yes | yes | owned Array/Map/Set |
| `&str` | yes | yes | invocation-local String borrow |
| `&ValueT` | yes | yes | temporary decoded Value borrow |
| `&HostT` | yes | yes | shared Host capability |
| `&mut HostT` | yes | yes | exclusive Host capability |
| `&[ValueT]` | yes | yes | temporary Vec/slice at Rust target |
| Host collection shared view | yes | yes | zero-copy View |
| Host collection mutable view | yes | yes | zero-copy MutView |
| direct synchronous `&HostT` return | yes | exact admitted origin | scoped View |
| direct synchronous `&mut HostT` return | yes | exact admitted origin | scoped MutView |
| `Option<&HostT>` | yes | exact admitted origin | optional scoped View |
| `Result<&HostT, E>` | yes | exact origin and owned `E` | scoped View or owned error |
| non-`'static` call-scoped Host parameter | yes | yes, including `base` | root Host capability |
| async owned return | yes | yes | owned Value |
| generated Rust-to-Vela call | yes | generated Service API | typed binding |

“Exact admitted origin” means the current Service ABI can restore the authored
Rust borrow without guessing. Unsupported return shapes fail during macro
expansion.

## 8. Deliberately Unsupported Shapes

### 8.1 Real Rust references in Vela storage

Unsupported:

```rust,ignore
// Vela never receives or stores this pointer/reference representation.
pub fn expose_raw(value: &mut Inventory) -> *mut Inventory;
```

Vela receives HostRef-backed capabilities only.

### 8.2 Mutable Value borrow and copy-back

Unsupported Rust target:

```rust,ignore
pub fn mutate_request(request: &mut Request);
```

Unsupported Vela source:

```vela
let request = Request { item_id: 1, amount: 2, tags: [] };
inventory::mutate_request(request);
```

A script-owned Value cannot satisfy `&mut T` through implicit copy-in/copy-out.
Use an owned transform that returns a new Value, or register identity-bearing
mutable state as Host storage.

### 8.3 Mutable collection copy-back

Unsupported:

```vela
let values = [1, 2, 3];
inventory::replace(values, 9); // Rust expects &mut Vec<i64>
```

Use an exact Host-backed mutable collection view.

### 8.4 Owned Host movement

Unsupported until a separate consuming-host contract exists:

```rust,ignore
pub fn consume(inventory: Inventory) -> Receipt;
```

No implicit clone, move-out, Arc conversion, or arena ownership transfer is
inferred from Host storage.

### 8.5 Borrowed children across await

Unsupported:

```vela
let item = inventory.item(id)?;
timer::sleep(1).await;
return item.count;
```

Release the item before await, or convert the needed data to an owned Value.

### 8.6 Async borrowed returns

Unsupported Rust:

```rust,ignore
pub async fn find(inventory: &Inventory, id: i64) -> Option<&Item>;
```

Return an owned Value, a stable business identifier, or a future explicit
durable HostHandle contract.

### 8.7 Borrowed values in owned containers

Unsupported Rust:

```rust,ignore
pub fn all(inventory: &Inventory) -> Vec<&Item>;
pub fn grouped(inventory: &Inventory) -> HashMap<String, &Item>;
```

Use a registered Host collection view, an owned Value projection, or a future
explicit scoped-container ABI. The bridge does not pretend that an owned Vela
Array can own Rust references.

### 8.8 Projected Service return restored to Rust

Unsupported Service:

```rust,ignore
pub trait TableService: Send + Sync {
    fn row<'a>(&self, table: &'a Table, id: i64) -> &'a Row;
}
```

The returned `Row` is a projected child rather than the exact direct Host
parameter. Ordinary registered Host methods may expose this child to Vela, but
the Service terminal Rust-return sink does not restore it until a separate
projection contract is implemented.

### 8.9 Multi-origin Service borrowed return

Unsupported Service:

```rust,ignore
pub trait ChoiceService: Send + Sync {
    fn choose<'a>(&self, left: &'a Item, right: &'a Item, flag: bool)
        -> &'a Item;
}
```

The result may originate from more than one parameter. Add an owned result,
use one unambiguous Host origin, or implement a future runtime-selected origin
set contract before admitting this shape.

### 8.10 Nested mutable envelopes

Unsupported Service:

```rust,ignore
fn optional_mut<'a>(&self, value: &'a mut Item) -> Option<&'a mut Item>;
fn fallible_mut<'a>(
    &self,
    value: &'a mut Item,
) -> Result<&'a mut Item, Error>;
```

These remain rejected until every direction has an exact envelope, exclusive
capability, terminal restoration, dynamic validation, and negative-path proof.

### 8.11 Scoped producer chaining and discard

Unsupported:

```vela
inventory.current();
config.tables().item(id)?.value;
```

Bind each scoped result and release it explicitly.

### 8.12 Scoped state, root return, and escaping closure

Unsupported:

```vela
state CURRENT: Item;

fn store(inventory: Inventory) {
    let item = inventory.current();
    CURRENT = item;
}
```

Unsupported:

```vela
fn leak(inventory: Inventory) {
    let item = inventory.current();
    return || item.count;
}
```

Unsupported ordinary Vela root result:

```vela
fn leak(inventory: Inventory) {
    return inventory.current();
}
```

Only an admitted generated Service terminal sink may transfer an exact scoped
borrow back to its original Rust caller.

### 8.13 Raw pointers, extern ABI, and script-callable unsafe Rust

Unsupported in this hard switch:

```rust,ignore
pub unsafe extern "C" fn write_raw(
    data: *mut u8,
    len: usize,
);
```

The implementation may use quarantined unsafe code in a reviewed erased-borrow
or C ABI module. This plan does not expose arbitrary unsafe preconditions, raw
pointers, or extern ABI values to Vela. A future trusted-unsafe-native proposal
must be a separate explicit capability model.

### 8.14 General generics and overloading

Unsupported:

```rust,ignore
pub fn convert<T: Convert>(value: T) -> T;
pub fn apply(value: i64);
pub fn apply(value: String);
```

Rust generic exports require explicit generated monomorphizations if added in
a future plan. Vela does not resolve overloads by arity or type.

### 8.15 Dynamic `base` and `services`

Unsupported:

```vela
let callable = base.apply;
reflect::call(base, "apply", [inventory, request]);
let deferred = || services.operation.apply(inventory, request);
```

`base` and `services` are lexical compiler capabilities with non-escaping call
shapes, not ordinary values.

### 8.16 Patching direct concrete Rust calls

Unsupported Rust call site:

```rust,ignore
let service = RustOperationService;
let reply = service.apply(&mut inventory, request);
```

If this concrete call does not cross the generated Service domain, Vela cannot
patch it. All hotfixable Rust operations must be called through the generated
Service API.

## 9. Implementation And Acceptance

The typed `base` design, crate-level deletion map, E0-E5 phase gates, required
test matrix, structural audits, and final review checklist live in the
[hard-switch implementation plan](rust-vela-interop-hard-switch-implementation.md).
