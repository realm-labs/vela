# Rust/Vela Service Patchability Completion Plan

> Status: Active plan
>
> Scope: make every admitted generated service method completely executable
> through Rust defaults and Vela-selected implementations
>
> Primary acceptance artifact:
> `examples/src/bin/service_hotfix_coverage`
>
> Relationship to the hard switch: the generation, publication, and unified
> service model in
> [rust-vela-service-hard-switch-plan.md](rust-vela-service-hard-switch-plan.md)
> remains authoritative. This plan closes the signature-totality and
> borrowed-return gaps found after that switch.
>
> Target authoring surface:
> [rust-vela-service-patchability-usage.md](rust-vela-service-patchability-usage.md).

## 0. Objective

Vela's Rust integration is primarily a hotfix orchestration boundary. It does
not need to model every Rust lifetime or make arbitrary Rust objects durable in
script storage. It must let one Vela-selected service method orchestrate the
already registered Rust service vocabulary inside one root call tree:

```text
unchanged Rust caller
        |
        v
generated service generation
        |
        +-- Rust default
        |
        `-- Vela-selected method
                |
                +-- base.<method>
                +-- services.<service>.<method>
                +-- owned Value arguments and results
                +-- shared/exclusive call-scoped HostRefs
                `-- borrowed returns passed to later Rust/Vela calls
```

The completion invariant is:

```text
#[service] compiles
+ service-set TypeBinding closure seals
+ a Vela candidate passes staging
------------------------------------------------
= every selected method has a non-panicking executable path
  for Rust callers and same-generation Vela/Rust nested callers
```

An unsupported Rust signature must fail during macro expansion. A missing or
incompatible runtime binding must fail while the Engine or service set is
sealed. An incompatible Vela implementation must fail compilation or staging.
No unsupported shape may survive until invocation.

## 1. Product Scope

### 1.1 Required use case

The required use case is a small emergency patch that changes orchestration or
policy while keeping authoritative objects and side effects in Rust:

```rust,ignore
#[vela::service(path = "coverage::lookup")]
pub trait LookupService: Send + Sync {
    fn get<'a>(
        &self,
        table: &'a Table,
        key: i64,
    ) -> Option<&'a Row>;
}

#[vela::service(path = "coverage::apply")]
pub trait ApplyService: Send + Sync {
    fn apply(
        &self,
        state: &mut RequestState,
        row: &Row,
    ) -> Result<i64, ServiceError>;
}
```

A Vela implementation must be able to call `lookup.get`, branch on `Some` or
`None`, read the returned `Row`, pass it to `apply` together with
`&mut RequestState`, call Rust `base`, call another patched service through
`services`, and return either an owned result or the borrowed result declared
by the service method.

### 1.2 Explicit non-goals

This plan does not add:

- durable typed host handles;
- cross-root borrowed references;
- a borrowed value live across async suspension;
- arbitrary nested containers of Rust references;
- multi-origin borrowed returns;
- script-language generics;
- hot mutation of a Rust service trait or TypeBinding schema;
- interception of calls made directly on concrete Rust implementations;
- automatic rollback of Rust host effects; or
- a second Rust hotfix mechanism beside generated services.

If a patch needs a borrowed child after `await`, it must copy the required
Value data before suspension, re-resolve by an owned identifier afterward, or
call a coarser Rust async service. If an existing Rust API returns an
unsupported nested borrowed container, the host supplies a thin service
adapter with one of the admitted boundary shapes below.

## 2. Admitted Service Signature Grammar

Admission is a whitelist. The classifier must never accept a type merely
because it does not recognize a reason to reject it.

### 2.1 Owned values

The recursive owned grammar is:

```text
Owned :=
    Unit
  | supported scalar
  | String
  | Bytes
  | registered Value
  | Option<Owned>
  | Result<Owned, Owned>
  | Tuple<Owned...>
  | Array<Owned>
  | Map<OwnedKey, Owned>
  | Set<OwnedKey>
```

An `Owned` subtree cannot contain a Rust reference, HostRef wrapper,
PathProxy, erased iterator, function, trait object, runtime context, or another
boundary-only implementation type. A service parameter spelled as owned `T`
is admitted only when `T` has `StoragePolicy::Value`; moving an object out of
Host storage through an owned parameter remains rejected until an explicit
consuming-host contract exists.

### 2.2 Representation-directed parameters

`T`, `&T`, and `&mut T` share nominal type and method identity, but the target
parameter mode and registered storage policy choose one boundary lowering:

```text
Rust T, Value storage       <- consume checked Vela Value<T>
Rust &T, Value storage      <- decode one temporary T and lend it for one sync call
Rust &T, Host storage       <- acquire a shared HostRef lease
Rust &mut T, Host storage   <- acquire an exclusive HostRef lease
Rust T, Host storage        <- reject
Rust &mut T, Value storage  <- reject implicit copy-in/copy-out
```

The same rule specializes collection parameters:

```text
&[T] / &Vec<T>      shared host view or temporary decoded Value collection
&mut [T]             fixed exclusive Host collection view only
&mut Vec<T>          growable exclusive Host collection view only
borrowed Map/Set     capability-qualified Host collection view only
```

`T` and every collection element/key/value type must have the exact storage,
codec, and representation capability required by the parameter. Complete
alias preflight remains atomic before generated code creates any Rust
reference.

Vela never constructs a Rust reference. It constructs or receives a typed
Value or Host object, and the generated call boundary creates the temporary
Rust borrow. Host availability has three explicit origins:

```text
Injected         supplied by the Rust root caller
Constructible    created through a registered Host constructor
ProducedBorrow   returned by a registered method or service
```

Not every Host type is constructible. Sessions, external resources, actor
contexts, and authority objects may remain Injected-only. This is a capability
boundary, not incomplete patchability. Tooling must report the available
origins for each host parameter so a patch author can see whether a planned
service call is reachable from the current entrypoint.

A Host constructor declares `CallScoped` or `RuntimeOwned` lifetime.
Call-scoped construction is the required scratch-object form for hotfix
orchestration and releases the object at root teardown. Runtime-owned
construction remains explicit and must not be inferred merely because a
constructor exists.

### 2.3 Scoped borrowed returns

The initial complete whitelist is deliberately small:

```text
&T
&mut T
shared/exclusive direct borrowed collection view
Option<&T>
```

`&mut T` is represented in Vela only as an exclusive call-scoped HostRef. Vela
never receives a real Rust mutable reference. `Option<&mut T>`,
`Result<&T, E>`, grouped borrowed tuples, and other envelopes remain rejected
until a later workload justifies adding one and supplies the complete
acceptance rows required by this plan.

For service methods, a borrowed return must have one explicit parameter
origin. The service `&self` receiver is dispatch infrastructure, not a
host-backed business owner, and cannot be the provenance source.

### 2.4 Required compile-time rejection

The macro must recursively reject at least:

```rust,compile_fail
fn rows(&self, table: &Table) -> Vec<&Row>;
fn rows(&self, table: &Table) -> Vec<Option<&Row>>;
fn rows(&self, table: &Table) -> BTreeMap<Key, Vec<Option<&Row>>>;
fn rows(&self, table: &Table) -> Option<Result<Vec<&Row>, Error>>;
fn row(&self, left: &Table, right: &Table) -> &Row;
async fn row(&self, table: &Table) -> &Row;
fn row(&self, table: &Table) -> Option<&mut Row>;
```

Diagnostics must name the full normalized return path where possible:

```text
service return contains a call-scoped host borrow inside an owned container
return path: Option::Some -> Result::Ok -> Array::element -> &Row
```

The diagnostic should recommend a direct borrowed collection view, owned
identifier collection, or a coarser service adapter. It must not recommend
copying a Host value into a script record.

### 2.5 Target-directed collection lowering

Collection conversion is automatic only at a statically known service
boundary and is driven by the target parameter:

```text
Vela Array<T>       -> Rust Vec<T>       one checked materialization
Vela Array<T>       -> Rust &[T]         temporary Vec<T> plus shared borrow
Host ArrayView<T>   -> Rust &[T]         zero-copy shared reborrow
Host ArrayMut<T>    -> Rust &mut [T]     zero-copy fixed write-through
Host ArrayMut<T>    -> Rust &mut Vec<T>  zero-copy growable write-through
Vela Array<T>       -> Rust &mut Vec<T>  reject
```

The same rule applies recursively to owned Map and Set parameters. It is
boundary lowering, not a general implicit cast in Vela. `filter`, `map`,
`group_by`, and `collect` may turn a Host view into a script-owned collection;
that result may later materialize into an owned Rust collection or a temporary
shared Value borrow, but it no longer has Host identity and cannot satisfy a
mutable Rust borrow.

Nominal element facts remain exact. `Array<T>` cannot satisfy `Vec<U>`, an
anonymous same-shaped record cannot impersonate a registered Value, and an
erased or dynamic element fact requires a recursive runtime guard before Rust
executes. No collection lowering uses JSON, Serde reflection, or implicit
mutable copy-back.

## 3. Admission Gates

### 3.1 Rust macro expansion

`#[service]` owns syntactic and lifetime-shape admission:

- recursively normalize every parameter and return type;
- distinguish owned values, direct Host borrows, borrowed collection views,
  and rejected nested borrow paths;
- require exactly one borrowed-return parameter origin;
- reject shared-to-exclusive promotion;
- reject async borrowed returns;
- reject every non-whitelisted borrowed envelope;
- classify owned `T`, shared `&T`, and exclusive `&mut T` against the required
  Value/Host representation rather than Rust spelling alone;
- emit target-directed collection lowering only for the admitted owned and
  shared cases;
- reject generic methods, unsupported associated types, trait objects,
  variadics, unsafe functions, and runtime result wrappers; and
- emit only methods for which all generated dispatch directions exist.

An accepted method cannot contain a generated `panic!`, `todo!`,
`unimplemented!`, placeholder error, or Rust-default-only branch for a
Vela-selected case.

### 3.2 TypeBinding and service-set sealing

The generated service requirements and Engine builder own semantic admission:

- every transitive `Owned` type has an owned conversion;
- every Host parameter and return has the exact shared/exclusive
  representation capability;
- every borrowed collection has a registered family and element/key/value
  contract;
- Value/shared-temporary/exclusive-Host lowering capabilities match every
  service parameter;
- every Host constructor declares call-scoped or Runtime-owned lifetime;
- constructor, injected-root, and produced-borrow origins remain distinct
  sealed facts;
- stable type IDs and ABI fingerprints are present;
- no required constructor, method, field, protocol, or conversion is missing;
- duplicate or incompatible registrations reject the service set; and
- the sealed registry checksum enters the service schema.

The Rust compiler cannot prove which bindings an application will register at
runtime. Therefore "compile-time guarantee" means Rust macro errors for
signature shape plus deterministic Engine/service-set construction failure for
missing registration. Neither may be deferred to the first request.

### 3.3 Vela compilation and deployment staging

The compiler and staging path own implementation admission:

- exact service and method identity;
- exact parameter order, mode, and TypeHint;
- exact return family and scoped-return metadata;
- effect subset and capability requirements;
- complete linked target resolution;
- no borrowed escape to state, closure, ordinary root result, or async
  suspension;
- exact base generation and artifact checksum for Delta; and
- one coherent linked artifact and service generation.

A generated service return adapter is a sealed egress sink. It may consume a
validated scoped HostRef and restore the Rust reference declared by the
service signature. Ordinary Vela root results remain forbidden from exporting
that HostRef.

## 4. Current Gap Inventory

### G0 — Service admission is not total

`crates/vela_macros/src/service.rs` currently emits a Vela-selected borrowed
return branch that panics when called through the ordinary Rust service
adapter. Macro expansion success therefore does not yet imply executable
patchability.

Required outcome:

- remove the runtime placeholder;
- generate a complete adapter or reject the signature; and
- add a production-source audit for non-executable service placeholders.

### G1 — Service borrowed envelopes are direct-only

`crates/vela_macros/src/service/dispatch.rs` accepts only
`ScopedReturnContainer::Direct`. `Option<&T>` works for ordinary synchronous
exports and inherent methods but is not complete in the service path.

Required outcome:

- reuse the existing `ScopedHost` return mode and optional envelope;
- preserve `Some` provenance and lease behavior;
- make `None` create no child HostRef and no lease; and
- support both nested Vela consumption and Rust caller restoration.

### G2 — Nested borrowed-container diagnostics are incidental

Owned container classification currently reaches an unsupported inner
reference and generally fails, but service-specific recursive-path
diagnostics and explicit compile-fail coverage do not exist.

Required outcome:

- one recursive return-shape validator after normalization;
- stable diagnostic codes/messages for nested call-scoped borrows;
- compile-fail fixtures for every required rejected family; and
- no fallback classification as an opaque registered Value.

### G3 — Rust caller borrowed-return restoration is missing

A Vela-selected service may create and consume a scoped child in the current
session, but the generated outer Rust adapter cannot yet return the declared
borrow to its Rust caller.

Required outcome:

- validate the returned compact HostRef against the declared origin, child
  type, access mode, root, generation, owner, and borrow group;
- prove the child descends from the original Rust parameter;
- close every Vela alias and root-local lease before leaving the session;
- restore a Rust reference whose lifetime is tied to the original parameter;
- rely on Rust's borrow checker after the generated call returns; and
- ensure no Runtime-owned or unrelated HostRef can be converted this way.

This handoff must be isolated in a narrowly audited generated/host adapter. It
must not create a public "HostRef to Rust reference" conversion.

### G4 — Controlled service egress needs a distinct escape rule

Ordinary scoped HostRefs cannot be root results. The service adapter needs one
controlled terminal sink without weakening general escape analysis.

Required outcome:

- represent the generated service return sink explicitly in MIR/bytecode or
  invocation metadata;
- allow only the descriptor-declared origin/type/access;
- reject the same value through an ordinary exported Vela function;
- reject nested placement in a non-whitelisted container; and
- close the borrow deterministically during the adapter handoff.

### G5 — Dynamic, reflected, and generated calls need parity proof

Static service calls already carry linked schema facts, but dynamic and
reflection paths must not bypass scoped-return and escape enforcement.

Required outcome:

- reuse the same callable descriptor and runtime return validator;
- preserve effect, capability, generation, and provenance checks;
- reject an erased or ambiguous borrowed result; and
- expose identical normalized type descriptions to analysis, reflection, CLI,
  generated bindings, and LSP.

### G6 — Patchability is not represented as a sealed schema fact

The schema records return mode and provenance, but it does not currently prove
that every generated call direction exists.

Required outcome:

- derive patchability from the normalized signature and supported adapter
  family;
- validate it while building the service schema;
- include behavior-affecting return/envelope/provenance facts in ABI and
  checksums; and
- refuse schema construction when a method lacks a complete adapter.

No public re-export facade or second registry is added.

### G7 — Acceptance fixtures do not call the missing outer path

The current interop fixture proves a Vela-selected borrowed return can feed a
nested service call. It does not call that borrowed-return method directly
from ordinary Rust after Vela selection, so the generated panic is unobserved.

Required outcome:

- add direct Rust caller tests for direct and optional borrowed returns;
- retain the nested-chain fixture;
- assert pointer identity and mutation visibility; and
- assert default and selected calls use the same authored Rust signature.

### G8 — The runnable demo does not cover borrowed return patchability

`service_hard_switch_fixture` proves the generation/deployment model, custom
Values, collections, async handling, `base`, `services`, Snapshot, successive
Delta, old roots, folded Snapshot, and rollback. It does not demonstrate
`Option<&T>` or a borrowed service result restored to Rust.

Required outcome:

- keep that fixture as the hard-switch regression;
- add the focused `service_hotfix_coverage` demo specified in section 7; and
- make the example test assert stable, human-readable output.

### G9 — Current status documentation overstates completion

`docs/progress.md` marks Rust/Vela service interop complete. That remains true
for the generation hard switch, but not for the stronger invariant "every
admitted service method is completely patchable."

Required outcome:

- track this plan as the active service closure;
- distinguish the accepted generation model from open signature totality; and
- restore Complete status only after all gates below pass.

### G10 — Parameter construction and origin closure are incomplete

TypeBinding has explicit Value and Host constructors, and a Host factory can
produce a Runtime-owned HostRef. The service model does not yet provide one
uniform rule for Value `T` temporarily satisfying `&T`, call-scoped scratch
Host construction, or tooling that distinguishes injected, constructible, and
service-produced Host origins. Current Host factories retain constructed
objects until Runtime drop, which is unsuitable for frequent patch-local
scratch objects.

Required outcome:

- implement the representation-directed parameter table in section 2.2;
- add call-scoped Host construction and deterministic root teardown;
- keep Runtime-owned construction explicit;
- reject owned Host `T` and mutable Value `&mut T`;
- expose construct lifetime and available Host origins through sealed
  TypeBinding/service/tooling facts; and
- report an unavailable argument origin before a candidate can be activated.

### G11 — Collection lowering is shape-specific instead of uniform

Owned collection materialization and a Value-slice temporary borrow exist, but
the service dispatcher still contains shape-specific paths rather than one
target-directed lowering contract. A Vela `filter`/`map`/`collect` result must
have predictable behavior when the next Rust service accepts `Vec<T>`,
`&[T]`, or `&mut Vec<T>`.

Required outcome:

- centralize owned collection materialization and temporary shared Value
  borrows;
- reborrow Host collection views without materialization;
- reject script-owned collections for mutable Rust borrows;
- preserve exact nested element/key/value facts and nominal Value identity;
- run a recursive guard for dynamic facts before authored Rust executes; and
- prove that lowering uses no JSON, Serde reflection, or mutable copy-back.

## 5. Phased Execution

Each phase produces one coherent verified checkpoint. Do not broaden the
signature whitelist during implementation unless an actual registered service
requires it.

### P0 — Freeze the patchability matrix

Deliverables:

- encode the admitted grammar in classifier unit tests;
- add service UI compile-pass fixtures for every admitted borrowed form;
- add service UI compile-fail fixtures for every rejected family;
- add a source audit for accepted-but-non-executable generated service
  branches; and
- record the existing failing direct-Rust selected-return scenario as a
  regression test before changing production code.

Gate:

```text
the regression test reaches the current missing outer adapter
every admitted and rejected signature has one named fixture
no test relies only on generated-token substring absence
```

### P1 — Make macro and schema admission fail closed

Deliverables:

- add one recursive service-boundary validator over normalized `TypeShape`;
- distinguish owned nesting from approved top-level scoped envelopes;
- produce return-path diagnostics for nested borrows;
- reject every scoped shape that has no complete adapter;
- make service requirements consume the same normalized decision; and
- add patchability validation to service schema construction.

Gate:

```text
all nested borrowed-container examples fail during cargo check
accepted direct borrowed methods still compile
Option<&T> is the only admitted optional borrowed form
schema construction cannot contain an adapter-incomplete method
```

### P2 — Complete service `Option<&T>` metadata and Vela conversion

Deliverables:

- admit exact synchronous `Option<&T>` with one host-parameter origin;
- emit `Option<Host<T>>` plus `ScopedHost` return metadata;
- reuse `ScopedHostNativeOutcome::OptionSome` and the existing child-retention
  model;
- return ordinary Vela `None` without creating a HostRef;
- preserve type, root, owner, generation, access, provenance, and lease for
  `Some`; and
- update compiler, reflection, analysis, CLI schema, generated bindings, and
  LSP displays from the same descriptor.

Gate:

```text
Some is readable and passable to another service without copying T
None changes no HostRef or lease count
dynamic and reflected calls observe the same Option<Host<T>> contract
schema and checksum tests distinguish owned Option<T> from scoped Option<&T>
```

### P3 — Complete Vela-selected borrowed returns to Rust callers

Deliverables:

- replace the generated panic with a typed scoped-return handoff;
- implement direct `&T`, direct `&mut T`, direct borrowed collection views,
  and `Option<&T>` according to the whitelist;
- verify provenance against the original Rust parameter lease;
- permit only the generated service-return sink;
- release all script aliases before returning to Rust;
- preserve pointer identity without clone, Serde, JSON, or script-record
  allocation; and
- make unwind and conversion failure release every lease.

Gate:

```text
unchanged Rust caller receives the declared reference from a Vela selection
Some and None return through the same authored Rust method
returned references point into the original Rust owner
unrelated, stale, wrong-type, and wrong-generation HostRefs fail closed
no generated selected branch contains panic/todo/unimplemented
```

### P4 — Complete construction and target-directed lowering

Deliverables:

- make owned `T`, shared `&T`, and exclusive `&mut T` dispatch by sealed
  storage/representation capability;
- allow a registered Value to back one invocation-scoped shared Rust borrow;
- add call-scoped Host construction with root-owned reclamation;
- retain explicit Runtime-owned Host construction for intentional durable
  runtime objects;
- surface Injected, Constructible, and ProducedBorrow origin facts;
- centralize Array/Map/Set owned materialization and temporary shared-borrow
  lowering;
- keep Host collection view reborrow zero-copy; and
- reject mutable Value and script-owned collection copy-back.

Gate:

```text
Vela constructs Value<T> and passes it as Rust T and &T
Vela constructs one call-scoped Host and passes shared/exclusive references
root teardown reclaims the scratch Host without Runtime-drop retention
transformed Array<T> passes to Vec<T> and &[T]
script-owned Array<T> cannot satisfy &mut Vec<T>
Host ArrayView/ArrayMut preserve zero-copy identity and write-through
```

### P5 — Close lifetime, permission, and dispatch parity

Deliverables:

- preserve shared-to-exclusive rejection through nested service calls;
- prove exclusive alias preflight before authored Rust executes;
- reject persistent-state writes, escaping closure captures, ordinary root
  returns, and async suspension of scoped children;
- retain direct host service arguments across async suspension where already
  supported;
- make static, dynamic, reflected, `base`, and `services` calls share the same
  validators; and
- preserve old-root generation behavior across activation.

Gate:

```text
no invocation path bypasses type/access/provenance/escape checks
child scoped borrows cannot cross await
root service arguments retain their existing async lease semantics
every failure path leaves owner and lease counts unchanged
```

### P6 — Add the runnable coverage demo

Deliverables:

- add the files in section 7;
- keep script sources beside the Rust entrypoint and load them with
  `include_str!`;
- use the same generated service caller before and after every activation;
- display each demonstrated capability with deterministic output;
- add the binary to `examples/tests/runnable_examples.rs`;
- document the command and coverage in `examples/README.md`; and
- retain `service_hard_switch_fixture` as an independent regression.

Gate:

```text
cargo run --manifest-path examples/Cargo.toml --bin service_hotfix_coverage
prints the exact accepted transcript
the runnable-examples test asserts that transcript
the demo contains no patch-aware branch in business caller code
```

### P7 — Final acceptance and documentation

Deliverables:

- update the normative service architecture with the final whitelist,
  controlled Rust-return sink, and total-admission invariant;
- update `docs/rust-vela-interop.md` with supported and rejected examples;
- mark the plan complete in `docs/progress.md`;
- archive detailed acceptance evidence if needed;
- run the focused, workspace, examples, docs, structural, and performance
  gates; and
- commit the final checkpoint with a concise validation summary.

Gate:

```text
registered-and-sealed service means fully Vela-patchable
all accepted borrowed forms work in both nested and outer Rust directions
all unsupported borrowed forms fail before Engine construction
the complete demo and repository validation are green
```

## 6. Complete Test Coverage

The rows below are mandatory. A phase may add more focused tests, but it may
not remove or collapse these behaviors into one broad assertion.

### 6.1 Rust macro compile-pass matrix

| ID | Signature | Required proof |
|---|---|---|
| MP-01 | owned scalar/Value/Option/Result/tuple | normalized owned ABI |
| MP-02 | nested owned Array/Map/Set | recursive owned closure |
| MP-03 | `&T` parameter | shared Host representation |
| MP-04 | `&mut T` parameter | exclusive Host representation |
| MP-05 | borrowed collection parameters | fixed/growable view facts |
| MP-06 | direct `&T` return from one parameter | shared origin metadata |
| MP-07 | direct `&mut T` return from one exclusive parameter | exclusive origin metadata |
| MP-08 | direct borrowed collection return | collection identity and origin |
| MP-09 | `Option<&T>` return | optional scoped envelope |
| MP-10 | sync service mixing owned, shared, and exclusive inputs | one complete descriptor |

### 6.2 Rust macro compile-fail matrix

| ID | Rejected shape | Required diagnostic fact |
|---|---|---|
| MF-01 | `Vec<&T>` | borrow under Array element |
| MF-02 | `Vec<Option<&T>>` | nested optional borrow under Array |
| MF-03 | `Map<K, Vec<Option<&T>>>` | complete return path |
| MF-04 | `Option<Result<Vec<&T>, E>>` | complete wrapper path |
| MF-05 | `Option<&mut T>` | unsupported optional exclusive envelope |
| MF-06 | `Result<&T, E>` | unsupported scoped Result envelope |
| MF-07 | mixed borrowed/owned tuple | unsupported grouped envelope |
| MF-08 | no host origin | missing provenance |
| MF-09 | two possible host origins | ambiguous provenance |
| MF-10 | shared origin returning exclusive | access upgrade |
| MF-11 | async borrowed return | suspend boundary |
| MF-12 | service receiver as borrowed origin | receiver is not host owner |
| MF-13 | generic host reference | exact concrete type required |
| MF-14 | erased Iterator/Fn/trait object | unsupported boundary |
| MF-15 | accepted shape with missing generated adapter | schema totality failure |

UI fixtures must assert stable diagnostics. Unit tests may additionally inspect
normalized shapes, but token-string tests alone are insufficient.

### 6.3 TypeBinding, schema, ABI, and tooling matrix

| ID | Behavior |
|---|---|
| TS-01 | missing host-backed binding rejects service-set construction |
| TS-02 | Value registration cannot satisfy Host return requirement |
| TS-03 | shared-only binding cannot satisfy exclusive parameter/return |
| TS-04 | transitive owned container type closure is complete |
| TS-05 | `Option<&T>` has `Option<Host<T>>` plus scoped return mode |
| TS-06 | owned `Option<T>` and scoped `Option<&T>` have different ABI facts |
| TS-07 | origin/access/envelope changes alter ABI fingerprint |
| TS-08 | equivalent schemas produce stable checksum |
| TS-09 | reflection exposes the exact normalized return |
| TS-10 | CLI schema, analysis, hover, completion, and signature help agree |
| TS-11 | generated Rust binding rejects unsupported iterator/borrowed shape |
| TS-12 | no public re-export facade is introduced |
| TS-13 | Value `T` exposes owned and temporary-shared lowering |
| TS-14 | Host `T` exposes shared/exclusive but not implicit owned move |
| TS-15 | Host constructor ABI records CallScoped or RuntimeOwned |
| TS-16 | tooling reports Injected, Constructible, and ProducedBorrow origins |

### 6.4 HostRef and conversion matrix

| ID | Behavior |
|---|---|
| HR-01 | direct shared child preserves root, owner, generation, and provenance |
| HR-02 | direct exclusive child preserves capability without exposing `&mut` |
| HR-03 | optional `Some` uses the same child path as direct `&T` |
| HR-04 | optional `None` creates no HostRef and acquires no lease |
| HR-05 | child retains owner while live |
| HR-06 | explicit release invalidates all aliases in the borrow group |
| HR-07 | sibling children retain the parent freeze independently |
| HR-08 | root cleanup releases every retained child |
| HR-09 | use after root release reports expired/stale borrow |
| HR-10 | stale generation fails |
| HR-11 | wrong host type fails |
| HR-12 | unrelated owner/provenance fails |
| HR-13 | shared-to-exclusive fails before Rust executes |
| HR-14 | duplicate exclusive aliases fail atomically |
| HR-15 | conversion failure and unwind restore exact lease counts |

### 6.5 Service call-direction matrix

| ID | Direction | Behavior |
|---|---|---|
| SD-01 | Rust caller -> Rust default | direct call, no VM or HostRef |
| SD-02 | Rust caller -> Vela selected | owned return |
| SD-03 | Rust caller -> Vela selected | direct `&T` return |
| SD-04 | Rust caller -> Vela selected | direct `&mut T` return |
| SD-05 | Rust caller -> Vela selected | `Option<&T>::Some` |
| SD-06 | Rust caller -> Vela selected | `Option<&T>::None` |
| SD-07 | Vela -> Rust default via `base` | borrow returned to Vela |
| SD-08 | Vela -> Rust service via `services` | borrow returned to Vela |
| SD-09 | Vela borrow -> later Rust service | exact reborrow |
| SD-10 | Vela borrow -> later Vela service | same generation and provenance |
| SD-11 | patched Vela -> patched Vela -> Rust | one session and budget |
| SD-12 | Vela failure | no Rust fallback retry |
| SD-13 | conversion failure | authored Rust body does not execute |

### 6.6 Escape and async matrix

| ID | Behavior |
|---|---|
| EA-01 | local synchronous use succeeds |
| EA-02 | temporary local collection use succeeds only within root |
| EA-03 | persistent state store rejects |
| EA-04 | extern/global/native cache escape rejects |
| EA-05 | escaping closure capture rejects |
| EA-06 | ordinary Vela root result rejects |
| EA-07 | generated service Rust-return sink succeeds |
| EA-08 | async service borrowed-return definition rejects |
| EA-09 | scoped child live at async suspension rejects |
| EA-10 | root direct service argument survives supported async suspension |
| EA-11 | cancellation/drop/unwind releases Runtime and leases |

### 6.7 Static, dynamic, reflection, and generated-call parity

| ID | Behavior |
|---|---|
| DP-01 | statically linked service call enforces scoped return |
| DP-02 | dynamic service call cannot erase lifetime/access |
| DP-03 | reflected service call cannot erase lifetime/access |
| DP-04 | reflected write through shared child rejects |
| DP-05 | generated Rust outer call uses the same descriptor |
| DP-06 | stale dynamic/reflection target cannot cross generation |
| DP-07 | effect/capability ceiling is identical on every path |

### 6.8 Hot-update and deployment matrix

| ID | Behavior |
|---|---|
| HU-01 | Rust-only generation uses all defaults |
| HU-02 | sparse Snapshot selects one Vela method |
| HU-03 | adjacent unmentioned method remains Rust |
| HU-04 | Delta inherits prior Vela selections |
| HU-05 | second Delta replaces another method |
| HU-06 | explicit `RustDefault` removes an inherited selection |
| HU-07 | pinned old sync root remains old |
| HU-08 | suspended old async root remains old |
| HU-09 | new root observes new complete generation |
| HU-10 | nested `services` calls cannot mix generations |
| HU-11 | stale-base Delta rejects without publication |
| HU-12 | ABI/effect/type mismatch rejects without publication |
| HU-13 | folded Snapshot equals accumulated Deltas |
| HU-14 | rollback republishes the prior generation |
| HU-15 | rollback does not retry or undo Host effects |
| HU-16 | borrowed return from old root cannot be used by new generation |

### 6.9 Allocation and regression matrix

| ID | Behavior |
|---|---|
| PR-01 | Rust-default selected service call performs no VM entry |
| PR-02 | Rust-default path creates no HostRef |
| PR-03 | HostRef alias copy creates no lease or refcount operation |
| PR-04 | `Some(&T)` performs no clone, Serde, JSON, or script-record allocation |
| PR-05 | `None` performs no HostRef/lease allocation |
| PR-06 | direct and optional borrowed-return hot path is independent of collection size |
| PR-07 | ordinary export direct `&T` behavior does not regress |
| PR-08 | ordinary export `Option<&T>` behavior does not regress |
| PR-09 | existing service nested borrowed chain does not regress |
| PR-10 | existing `service_hard_switch_fixture` output does not change |

### 6.10 Construction and collection-lowering matrix

| ID | Behavior |
|---|---|
| CL-01 | registered Value constructor result passes to Rust `T` |
| CL-02 | registered Value constructor result temporarily backs Rust `&T` |
| CL-03 | Value `T` cannot satisfy Rust `&mut T` |
| CL-04 | call-scoped Host constructor result passes to Rust `&T` |
| CL-05 | call-scoped Host constructor result passes to Rust `&mut T` |
| CL-06 | call-scoped Host is reclaimed at root teardown |
| CL-07 | Runtime-owned Host constructor remains explicitly durable |
| CL-08 | Host without Construct capability cannot be fabricated |
| CL-09 | unavailable Injected/Constructible/ProducedBorrow origin diagnoses before activation |
| CL-10 | transformed `Array<T>` materializes once into Rust `Vec<T>` |
| CL-11 | transformed `Array<T>` temporarily backs Rust `&[T]` |
| CL-12 | Host `ArrayView<T>` reborrows into Rust `&[T]` without materialization |
| CL-13 | Host `ArrayMut<T>` writes through Rust `&mut Vec<T>` |
| CL-14 | script-owned `Array<T>` cannot satisfy Rust `&mut Vec<T>` |
| CL-15 | nested owned Array/Map/Set lower recursively with exact facts |
| CL-16 | nominal or element-type mismatch rejects before Rust executes |
| CL-17 | dynamic/erased element facts run a recursive guard |
| CL-18 | conversion failure leaves Host state and lease counts unchanged |
| CL-19 | no lowering path uses JSON, Serde reflection, or mutable copy-back |

## 7. Runnable Demo Specification

### 7.1 Files

```text
examples/src/bin/service_hotfix_coverage/main.rs
examples/src/bin/service_hotfix_coverage/snapshot.vela
examples/src/bin/service_hotfix_coverage/delta_policy.vela
examples/src/bin/service_hotfix_coverage/delta_apply.vela
```

The Rust entrypoint uses `include_str!` for the scripts so the source remains
readable and independently inspectable. The example is domain-neutral and
uses only table, row, request, policy, apply, and audit terminology.

### 7.2 Host and Value types

```text
Row              host-backed, read-only fields visible to Vela
Table            host-backed owner of rows
RequestState     host-backed mutable state
Request          owned Value
Response         owned Value
ServiceError     owned Value
ValueRow         owned Value used after Vela collection transforms
PatchBuffer      call-scoped constructible Host scratch object
```

`Row` deliberately implements neither `Clone` nor `Serialize`. Adjacent
instrumentation tracks owned-codec entry so the demo can additionally assert
that lookup never serializes or materializes a script record.
`PatchBuffer` is constructed inside Vela, passed first as shared and then
exclusive to Rust, and reclaimed when the root ends. `ValueRow` is produced by
`filter`/`map`/`collect` and passed to Rust as both `Vec<ValueRow>` and
`&[ValueRow]`.

### 7.3 Services

```rust,ignore
#[service(path = "coverage::lookup")]
trait LookupService: Send + Sync {
    fn get<'a>(&self, table: &'a Table, key: i64) -> Option<&'a Row>;
    fn required<'a>(&self, table: &'a Table, key: i64) -> &'a Row;
    fn all<'a>(&self, table: &'a Table) -> &'a [Row];
}

#[service(path = "coverage::policy")]
trait PolicyService: Send + Sync {
    fn score(
        &self,
        state: &mut RequestState,
        row: &Row,
        adjustment: i64,
    ) -> Result<i64, ServiceError>;
}

#[service(path = "coverage::apply")]
trait ApplyService: Send + Sync {
    fn apply(
        &self,
        state: &mut RequestState,
        row: &Row,
        score: i64,
    ) -> Result<(), ServiceError>;
}

#[service(path = "coverage::audit")]
trait AuditService: Send + Sync {
    fn record(&self, state: &mut RequestState, code: i64);
}

#[service(path = "coverage::transform")]
trait TransformService: Send + Sync {
    fn consume(&self, values: Vec<ValueRow>) -> i64;
    fn inspect(&self, values: &[ValueRow]) -> i64;
    fn inspect_buffer(&self, buffer: &PatchBuffer) -> i64;
    fn update_buffer(&self, buffer: &mut PatchBuffer, delta: i64);
}

#[service(path = "coverage::handler")]
trait HandlerService: Send + Sync {
    async fn handle(
        &self,
        state: &mut RequestState,
        table: &Table,
        request: Request,
    ) -> Result<Response, ServiceError>;
}
```

The async handler awaits before creating a child borrowed return. This proves
that root host arguments survive supported suspension without implying that a
returned child may cross `await`.

### 7.4 Deployment sequence

The binary executes one unchanged Rust caller through:

1. Rust-default generation.
2. A sparse Snapshot patching `lookup.get` and `policy.score`.
3. `Some(&Row)` returned from Vela selection to ordinary Rust.
4. `None` returned from the same selected method.
5. A Vela handler chain that consumes `Some(&Row)`, reads fields, passes the
   child to `policy` and `apply`, and mutates `&mut RequestState`.
6. `base` from one patched method.
7. `services` calls spanning Rust and Vela selections in one generation.
8. A first exact-base Delta changing policy while inheriting lookup.
9. A second exact-base Delta adding apply/audit behavior while inheriting both.
10. An old pinned root after both activations.
11. A stale Delta and an incompatible candidate rejected without publication.
12. A folded Snapshot equivalent to the two Deltas.
13. Conditional rollback to the previous complete generation.
14. Verification that rollback did not undo already committed Host effects.
15. Construction of a call-scoped `PatchBuffer`, followed by shared and
    exclusive Rust service calls and root-end reclamation.
16. A Vela `filter`/`map`/`collect` chain whose `Array<ValueRow>` automatically
    supplies Rust `Vec<ValueRow>` and temporary `&[ValueRow]`.
17. A negative assertion that the same script-owned Array cannot supply
    `&mut Vec<ValueRow>`.

The demo also asserts:

- returned `Row` pointer identity equals the original table element;
- shared children cannot invoke exclusive operations;
- `None` changes no lease count;
- no Row clone/owned codec/Serde path runs;
- call-scoped Host construction leaves no Runtime-owned scratch object;
- owned collection lowering materializes once and shared lowering uses one
  invocation-scoped temporary;
- Host collection views retain zero-copy identity while mutable views write
  through;
- no borrowed child remains after each root; and
- the same caller contains no patch branch, Runtime target string, `CallArgs`,
  HostRef construction, or Vela-specific return conversion.

### 7.5 Required transcript

The exact numeric values may be chosen while implementing the fixture, but the
stable output must contain one line per capability:

```text
service_hotfix_coverage rust-default ...
service_hotfix_coverage snapshot some=... none=...
service_hotfix_coverage nested shared=... exclusive-write=...
service_hotfix_coverage delta-1 ...
service_hotfix_coverage delta-2 ...
service_hotfix_coverage old-root ...
service_hotfix_coverage rejected stale=true abi=true
service_hotfix_coverage folded ...
service_hotfix_coverage rollback ...
service_hotfix_coverage zero-copy clones=0 codecs=0
service_hotfix_coverage construct shared=... exclusive=... reclaimed=true
service_hotfix_coverage collections owned=... shared=... mutable-copyback=false
```

Assertions remain the primary correctness proof. The transcript exists so a
human can run the binary and see the complete hot-update sequence without
reading test internals.

## 8. Validation

### 8.1 Focused commands

Run the relevant subset after every independently verifiable change:

```bash
cargo test -p vela_macros --test ui
cargo test -p vela_macros --test service_contract
cargo test -p vela_engine --test service_interop
cargo test -p vela_engine --test service_selection
cargo test -p vela_engine --test service_activation
cargo test -p vela_engine --test service_async
cargo test -p vela_engine --test service_source
cargo test -p vela_engine optional_borrowed
cargo test -p vela_host scoped
cargo test -p vela_analysis service
cargo test -p vela_language_service schema
```

Command filters may be narrowed while developing, but each listed test target
must run before its owning phase closes.

### 8.2 Demo and examples

```bash
cargo run --manifest-path examples/Cargo.toml --bin service_hotfix_coverage
cargo run --manifest-path examples/Cargo.toml --bin service_hard_switch_fixture
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
```

### 8.3 Full repository gate

Use [validation.md](validation.md) as the source of truth:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo doc --workspace --all-features --no-deps
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
node editors/vscode/scripts/validate-package.js
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

### 8.4 Structural audits

```bash
rg -n 'Vela-selected borrowed service return.*not executable|panic!|todo!|unimplemented!' \
  crates/vela_macros/src/service.rs crates/vela_macros/src/service

rg -n 'Vec\\s*<\\s*&|Vec\\s*<\\s*Option\\s*<\\s*&|Option\\s*<\\s*Result\\s*<.*Vec\\s*<\\s*&' \
  crates/vela_macros/tests/ui/service
```

The first audit must have no non-test generated-path placeholder. The second
must find the intentional compile-fail fixtures.

## 9. Documentation Updates

At implementation completion:

- `docs/architecture/rust-vela-service-model.md` records total admission, the
  exact scoped-return whitelist, parameter-only provenance, controlled
  generated Rust-return egress, representation-directed parameters, Host
  origin/construction lifetime, target-directed collection lowering, async
  restrictions, and escape rules.
- `docs/rust-vela-interop.md` shows one admitted `Option<&T>` service and
  rejected nested borrowed-container examples.
- `docs/progress.md` marks this plan complete only after the demo and full gate.
- `docs/decisions.md` records the durable whitelist and total-admission rule.
- `examples/README.md` documents the runnable coverage demo.

Detailed command logs belong in the final commit/PR notes or an acceptance
report under `docs/archive/`, not in `docs/progress.md`.

## 10. Completion Definition

This plan is complete only when:

- every admitted service signature has Rust-default, Vela-selected, nested
  Rust/Vela, dynamic/reflection, and Rust caller coverage where applicable;
- direct and optional borrowed returns preserve identity and provenance in
  both directions;
- Value and Host parameters follow one storage-directed construction and
  borrowing model;
- every Host argument has an explicit Injected, Constructible, or
  ProducedBorrow origin;
- transformed Value collections automatically lower to owned or temporary
  shared Rust parameters while mutable Host views remain zero-copy;
- every unsupported borrowed shape fails before service registration;
- no selected generated branch contains a runtime non-executable placeholder;
- the runnable demo exercises the complete deployment sequence;
- the existing hard-switch and ordinary export fixtures do not regress; and
- focused tests, examples, the full workspace gate, docs, structural audits,
  and benchmark builds pass.

The resulting user-facing guarantee is:

```text
If a generated service set seals successfully, every admitted method can be
selected by Vela and called through its authored Rust signature. Values,
explicit Host origins, and target-directed owned/shared collection lowering
cover its admitted arguments. Unsupported borrowed or mutable-copy-back shapes
are compile-time errors, never production invocation errors.
```
