# Rust/Vela Service Interop And Hot Override Plan

> Track: ordinary Rust native signatures, generated service contracts, Rust/Vela provider dispatch, cross-service composition, and generation-safe hot override
>
> Status: design draft; implementation has not started
>
> Baseline: `master` at `414df408` on 2026-07-16
>
> Execution: coherent pre-release batches; reuse the existing VM call and provider paths
>
> Roadmap: queued work; this document does not replace the active checkpoint in `progress.md` until explicitly scheduled

This document defines the implementation goal for making Rust and Vela service
implementations interchangeable behind one stable contract. Rust authors should
write ordinary copied/owned parameters and call-scoped `&T`/`&mut T` parameters.
Generated boundary code must hide `HostRef`, `HostPath`, `PathProxy`,
`HostLeaseRef`, `HostLeaseMut`, and `HostAccess` from normal service code.

Vela providers must be able to call other services with the same qualified,
typed API regardless of whether the selected target is implemented in Rust or
Vela. Hot override must change future root calls atomically without changing the
implementation observed by an already-running call tree.

This is deliberately not arbitrary Rust ABI reflection. Every replaceable
operation crosses an explicitly generated service contract and a runtime-owned
service slot.

## 0. Codex Goal

Use the following command when this plan is approved and scheduled:

```text
/goal Execute docs/rust-vela-service-override-plan.md in full.
```

The execution goal is complete only after every required batch and acceptance
case in this document is implemented, verified, documented, and committed.
During execution, keep `docs/progress.md` aligned with the active checkpoint and
record accepted design decisions in `docs/decisions.md`.

### Fixed design constraints

1. A call is replaceable only when it goes through a generated service port and
   a stable `ServiceSlotId`. A direct call to a concrete Rust value remains an
   ordinary Rust call and is not intercepted.
2. The service trait and method ABI are stable. Rust versus Vela target kind and
   the target generation are implementation details behind the slot.
3. Rust `&T` parameters acquire shared call-scoped host leases. Rust `&mut T`
   parameters acquire exclusive call-scoped host leases.
4. All host parameter leases for one invocation are checked and acquired as one
   atomic request set before any Rust reference is created.
5. A nested service call may derive a scoped child reborrow from a currently
   active host lease. It must inherit canonical identity and provenance; it is
   not an unrelated second lease acquisition.
6. Passing the same host object to two exclusive parameters, or to shared and
   exclusive parameters in the same invocation, fails before entering Rust.
7. Opaque `HostAccess` adapters that cannot prove the exact concrete type and
   canonical lease identity must fail closed; type IDs alone are insufficient.
8. Service methods cannot return borrowed data to Vela or retain call-scoped
   references after the invocation finishes.
9. Vela service calls lower to statically resolved `ServiceSlotId` and
   `MethodId` values. Runtime strings are not the primary service locator.
10. `ServiceMethodTarget` joins the existing `Runtime::call` and
   `Runtime::call_async` target model. Do not create a parallel
   `call_service` execution family.
11. A root invocation pins an immutable service-dispatch generation. Nested
    service calls, callbacks, and permitted re-entry inherit it.
12. A provider may call any declared service. Calling its own public service
    name dispatches normally and is therefore recursive; delegation to the
    displaced implementation must be explicit.
13. Effects and capabilities are transitive across service calls and continue
    to use the normal runtime budget, host access, heap, cancellation, and
    tracing paths.
14. Once a target begins executing, its error is returned to the caller. The
    runtime must not automatically retry a fallback target after side effects
    may have occurred.
15. Provider-private Vela state is not migrated automatically. Persistent
    business state must live in compatible `state`, `extern state`, or host
    storage with explicit ownership.
16. No `unsafe` reference fabrication is allowed. Lease guards and RAII must
    restore availability on success, error, panic, cancellation, and dropped
    futures.

### Required implementation batches

- Batch A: contract and compile-time proof surface.
- Batch B: ordinary Rust reference parameters at native boundaries.
- Batch C: Rust service contracts, targets, and generated ports.
- Batch D: unified `ServiceSlot` dispatch and generation pinning.
- Batch E: Vela service namespace calls and cross-service composition.
- Batch F: Rust/Vela target switching, hot activation, and rollback.
- Batch G: end-to-end acceptance, tooling, documentation, and performance.

### Never-complete conditions

Do not declare the goal complete while any of the following remains true:

- ordinary supported Rust signatures require users to mention host-boundary
  wrapper types;
- conflicting host aliases can enter Rust as live references;
- a Rust service cannot pass an ordinary scoped reborrow to another service and
  resume using the parent reference afterward;
- any Rust/Vela direction uses a second execution, budget, or capability path;
- a running call tree can observe two service generations accidentally;
- service calls depend on runtime strings where static IDs are available;
- hot rollback retries an invocation that may already have mutated state;
- Vela provider composition cannot call another selected service;
- the examples demonstrate only isolated single-service replacement;
- focused, workspace, formatting, and lint validation are not green.

## 1. Target User Experience

The syntax below is the target experience, not a commitment to the final macro
spelling. Macro spelling is an open decision in Section 19.

### 1.1 Ordinary native Rust function

```rust
#[vela::native]
fn grant_exp(
    ctx: &mut NativeCallContext<'_, '_>,
    player: &mut Player,
    amount: i64,
) -> VmResult<()> {
    ctx.require_capability("player.write")?;
    player.exp += amount.max(0);
    Ok(())
}
```

The hidden context parameter is optional. A native function that does not need
runtime services, capabilities, state, or re-entry may omit it; it is never a
Vela-visible argument and does not force other business parameters to change.

User code sees an ordinary `&mut Player`. Generated registration and invocation
code validates the script argument, obtains its exact host identity, atomically
acquires the exclusive lease, creates the temporary Rust borrow, invokes the
function, and releases the lease.

The following call must fail before the Rust body runs:

```vela
grant_exp_pair(player, player, 10)
```

when both corresponding Rust parameters are `&mut Player`.

### 1.2 Replaceable Rust service

```rust
#[vela::service(name = "level")]
pub trait LevelService {
    fn grant_exp(
        &self,
        ctx: &mut NativeCallContext<'_, '_>,
        player: &mut Player,
        amount: i64,
    ) -> VmResult<()>;
}

pub struct DefaultLevelService;

#[vela::service_impl]
impl LevelService for DefaultLevelService {
    fn grant_exp(
        &self,
        ctx: &mut NativeCallContext<'_, '_>,
        player: &mut Player,
        amount: i64,
    ) -> VmResult<()> {
        ctx.services().reward().validate_exp(player, amount)?;
        ctx.services().inventory().grant_level_reward(player, amount)?;
        player.exp += amount.max(0);
        Ok(())
    }
}
```

The reward call receives a scoped child reborrow of the active `player` lease.
After it returns, Rust may use `player` again. This is distinct from supplying
the same object to two sibling mutable parameters in one invocation, which is
rejected atomically.

The generated service port is the replaceable call boundary:

```rust
services.level().grant_exp(&mut player, amount)?;
```

The concrete call below remains a direct Rust call and is intentionally not
replaceable:

```rust
DefaultLevelService.grant_exp(ctx, &mut player, amount)?;
```

### 1.3 Vela override with normal service composition

```vela
use game::services::{inventory, reward};

pub struct LevelHotfix {}

#[provider(id = "level_hotfix")]
impl LevelService for LevelHotfix {
    pub fn grant_exp(self, player: Player, amount: i64) {
        let actual = math::max(amount, 0);

        reward::validate_exp(player, actual);
        inventory::grant_level_reward(player, actual);

        player.exp += actual;
    }
}
```

`reward` and `inventory` are typed service namespaces. Each call resolves its
own selected slot target in the pinned dispatch generation, so either service
may independently be backed by Rust or Vela.

### 1.4 Required direction matrix

| Caller | Selected target | Required behavior |
| --- | --- | --- |
| Rust service port | Rust provider | Generated port dispatches through the normal runtime target path. |
| Rust service port | Vela provider | Values and host references cross the generated contract boundary. |
| Vela service call | Rust provider | Linked slot/method IDs invoke the generated Rust adapter. |
| Vela service call | Vela provider | Linked slot/method IDs invoke the selected script method. |

All four directions must share argument validation, budgets, capabilities,
effects, tracing, cancellation, and error semantics.

## 2. Core Model

### 2.1 `ServiceContract`

A service contract is generated from an approved Rust trait and contains:

- a stable `ServiceTraitId` and fully qualified contract path;
- stable `MethodId` values and method names;
- parameter types and boundary modes (`value`, `shared_host`,
  `exclusive_host`, or another explicitly supported mode);
- default values where the existing ABI permits them;
- return type and error mapping;
- sync/async shape;
- declared effect, capability, and host-access requirements;
- a contract schema version used for link and reload compatibility checks.

The contract is reflection metadata and a link-time/runtime validation input. It
does not expose Rust layout or permit runtime mutation of type structure.

### 2.2 `ServiceSlotId`

`ServiceSlotId` identifies a configured service dependency, not a concrete
implementation. Conceptually it is derived from:

```text
ServiceTraitId + stable slot name
```

The first implementation slice may support one default slot per trait. The ID
shape must still leave room for multiple named instances such as primary and
secondary inventory services without redesigning dispatch.

### 2.3 `ServiceTarget`

A service slot resolves to a validated target:

```rust
enum ServiceTarget {
    Rust(RustServiceTarget),
    Vela(VelaServiceTarget),
}
```

The runtime may later represent partial overrides as an immutable per-method
table, but the initial implementation should prefer one whole-service target.
Target values contain exact contract identity and generation-safe handles; they
must not rely on unvalidated names at invocation time.

### 2.4 `ServiceDispatchGeneration`

The runtime publishes immutable slot tables by generation. A root call captures
one generation token, and every nested service resolution uses it. Activation
publishes a new generation for future roots; old generations stay alive while
frames, futures, callbacks, or handles still reference them.

The generation owns selection, not business state. It must not duplicate VM
frame, heap, host access, budget, or capability ownership.

### 2.5 Generated `ServicePort`

Rust code receives an ergonomic generated port, for example
`services.level()`. The port carries runtime call authority and a stable slot
ID, but it does not expose the internal target kind. Its methods:

- perform compile-time Rust signature checking;
- package values and exact host identities using a root call-scoped binding or
  an authorized child reborrow, without leaking wrapper concepts to the service
  author;
- route through `Runtime::call` or `Runtime::call_async`;
- convert results into the declared Rust result type;
- inherit the current call context when invoked from another service.

## 3. Reviewed Baseline

The implementation must build on these existing capabilities rather than
replacing them:

1. `HostRef`, `HostPath`, `PathProxy`, and `HostAccess` already implement the
   controlled script-to-host boundary.
2. `CallArgs` already accepts call-scoped shared and mutable Rust host bindings,
   recognizes their temporary direct host references, and can acquire multiple
   leases atomically with rollback on conflict.
3. `HostLeaseRef<T>` and `HostLeaseMut<T>` already encode shared and exclusive
   call-scoped access.
4. Async direct-method macro expansion already hides Rust receiver and parameter
   references behind leases for supported methods.
5. Free/context native-function macro parsing currently rejects reference
   parameters; this is a real gap for the target ordinary-function experience.
6. `NativeCallContext` already carries runtime services such as budgets,
   capabilities, host access, state, and heap access; new service calls should
   extend this context rather than create a rival context.
7. Current Vela providers are public zero-field records annotated with
   `#[provider]` and validated against a script trait.
8. `ProviderKey` already combines package, service trait, and provider identity;
   `ProviderHandle` already re-resolves after compatible body reload.
9. The current provider runtime resolver accepts script method targets only.
   There is no unified Rust/Vela service slot yet.
10. Provider selection, selected target, and signature changes are currently
    treated as artifact ABI changes and incompatible reloads.
11. Provider calls already use the normal budget, `HostAccess`, capability, and
    VM execution paths.
12. Script traits and provider implementations exist, but Vela does not yet have
    the typed service namespace required for provider-to-provider composition.

The initial vertical slice must preserve all compatible provider behavior while
moving target selection behind the more general service-slot model.

## 4. Signature Boundary And ABI

### 4.1 Initial supported parameter mapping

| Rust declaration | Contract mode | Vela view | Boundary rule |
| --- | --- | --- | --- |
| `&mut NativeCallContext<'_, '_>` | hidden context | not an argument | Supplied by the runtime and never storable. |
| copied scalar or approved owned value | value | corresponding value | Use the existing value conversion and ABI schema. |
| `String`, bytes, and approved owned containers | value | corresponding value/container | Ownership is explicit; do not synthesize a borrow with a longer lifetime. |
| `&str` or `&[u8]` | read-only value borrow | string/bytes | Permit only when the generated adapter can prove invocation-scoped storage. |
| `&T` for a registered host type | shared host | host object value | Acquire a shared exact-object lease; initial bound is `T: ScriptHostObject + Sync + 'static`. |
| `&mut T` for a registered host type | exclusive host | host object value | Acquire an exclusive exact-object lease; initial call-scoped binding bound is `T: ScriptHostObject + Send + Sync + 'static`. |
| supported `Option<T>`/`Result<T, E>` | structured value | optional/result form | All nested types must be boundary-safe and schema-described. |

The first slice deliberately uses the stronger mutable-origin bound already
required by call-scoped `CallArgs`. Any later relaxation must be proved for sync
and async execution separately and must not be silently introduced by macro
expansion.

### 4.2 Unsupported first-slice signatures

Reject these declarations at compile time with a diagnostic on the exact trait
method or native function parameter:

- borrowed return values, including `&T`, `&mut T`, `&str`, and borrowed
  container views;
- raw pointers, pinned references, mutex guards, lease guards, task-local
  guards, and other lifetime-carrying implementation types;
- generic service methods or exposed associated types;
- user-defined higher-ranked or externally named lifetimes in the service ABI;
- variadic, `extern`, or `unsafe` service methods;
- overloaded script-visible method names;
- types without stable boundary metadata and conversion support.

Rust trait generics that do not appear in an exposed method ABI should also be
deferred initially. The script language must not gain generics as a consequence
of this feature.

### 4.3 Mutability is ABI

Changing a service parameter between value, `&T`, and `&mut T` is an ABI change.
So are changes to sync/async shape, parameter order, stable parameter identity,
return type, declared effects, or required capabilities. Compatibility checking
must report the first exact mismatch rather than accepting a layout-compatible
accident.

### 4.4 Trusted Rust boundary

Once a valid exclusive lease has produced `&mut T`, the Rust implementation may
mutate any field permitted by Rust. Script-visible field permission metadata does
not sandbox trusted Rust code. A deployment that needs field-level enforcement
inside native code must use `HostAccess` explicitly or place the operation in an
untrusted boundary; the generated ordinary-reference API must not pretend to
provide such isolation.

## 5. Lease And Alias Safety

### 5.1 Invocation preflight

Generated adapters first build a complete request set:

```text
HostParamLeaseRequest {
    argument_index,
    canonical_host_identity,
    expected_type,
    mode: Shared | Exclusive,
    source: RootBinding | Reborrow(LeaseProvenanceId),
}
```

The runtime validates types, canonical identities, and all pairwise conflicts,
then acquires the entire set atomically. No Rust reference may be created before
the set succeeds. If a later conversion fails, every acquired lease is rolled
back without invoking user code.

### 5.2 Required alias matrix

| Same canonical host object | Result |
| --- | --- |
| shared + shared | allowed |
| shared + exclusive | rejected |
| exclusive + shared | rejected |
| exclusive + exclusive | rejected |

Different canonical host objects may be acquired together, including multiple
values of the same Rust type.

The user-facing error should be a stable structured diagnostic such as
`AliasedMutableHostArguments`, including both parameter names or indices and the
service/method identity. It must never depend on a Rust panic from an attempted
borrow.

### 5.3 Exact-object requirement

The first implementation supports ordinary Rust references only for direct host
objects whose registered adapter can prove:

- the exact concrete Rust type;
- the canonical lease slot shared by every alias of that host object;
- the object remains pinned and alive for the whole call;
- the adapter can produce a temporary reference without changing identity.

A matching `TypeId` is not enough. Generic or opaque `HostAccess` adapters must
fail closed when they cannot provide this proof. A `PathProxy` to a nested field
does not become `&mut Field` in the first slice; nested mutations continue to use
`HostPath` and `HostAccess`.

### 5.4 Scoped reborrow provenance

Natural Rust service composition requires this call to work:

```rust
fn grant_exp(
    &self,
    ctx: &mut NativeCallContext<'_, '_>,
    player: &mut Player,
    amount: i64,
) -> VmResult<()> {
    ctx.services().reward().validate_exp(player, amount)?;
    player.exp += amount;
    Ok(())
}
```

The nested reward call receives a Rust reborrow of `player`; it is not the same
case as passing one player to two sibling `&mut Player` parameters. Generated
entry adapters must register hidden `LeaseProvenanceId` metadata for every live
host reference. A generated service port uses that provenance to create a child
binding with the same canonical host identity while Rust's borrow checker
suspends use of the parent reference for the child call.

Required reborrow rules are:

| Parent provenance | Child request | Result |
| --- | --- | --- |
| shared | shared | allowed scoped child reborrow |
| shared | exclusive | rejected |
| exclusive | shared | allowed scoped child reborrow |
| exclusive | exclusive | allowed scoped child reborrow |

The parent lease remains the ownership authority. The child cannot outlive it,
change canonical identity, be used as an unrelated alias, or be retained by a
spawned unscoped task. Access through another alias to the parent object remains
busy while the exclusive chain is active.

Existing call-scoped `CallArgs::push_host_ref` and `push_host_mut` bindings show
that borrowed Rust values can already enter one runtime call without requiring
`'static` ownership. Service ports should reuse and extend that scoped mechanism
rather than fabricate a `'static` reference or allocate a second independent
host object. Stored copies of a scoped `HostRef` must either be rejected at an
escape boundary or become deterministically invalid when the scope closes; they
must never keep the Rust borrow alive.

If a raw Rust reference passed to a port has neither active provenance nor a
valid new root-call scope, conversion fails before target dispatch. Pointer
address and `TypeId` alone do not authorize a reborrow.

### 5.5 Lifetime and cleanup

All generated references are invocation-scoped. They cannot be stored in Vela,
returned to Vela, cached in a service object, or moved into an unscoped task.
RAII guards must release leases on:

- normal return;
- returned error;
- Rust panic after the runtime's normal panic conversion;
- async cancellation;
- future drop;
- permitted callback or VM re-entry unwinding.

## 6. Unified Service Dispatch

### 6.1 One runtime call target

Add a sealed `ServiceMethodTarget` implementation to the existing runtime target
abstraction. Conceptually it contains:

```text
ServiceMethodTarget {
    slot_id: ServiceSlotId,
    method_id: MethodId,
    dispatch_generation: inherited or captured token,
}
```

Its preparation step resolves the slot in the pinned immutable generation and
produces either the existing script-method invocation or a generated Rust-native
invocation. Argument validation and lease acquisition occur through the selected
target's normal preparation path.

Do not add parallel public APIs with divergent semantics such as
`call_service`, `call_service_async`, `reload_service_call`, or provider-only
budget handling.

### 6.2 Nested-call context inheritance

Every nested service call inherits the caller's:

- linked artifact generation;
- service-dispatch generation;
- `state` and `extern state` view;
- scope stack and call-frame ownership;
- instruction, allocation, recursion, and host-call budgets;
- capability grants and effect policy;
- host access and heap handles;
- tracing correlation and cancellation token.

Only the service target changes. Creating a fresh runtime context for a nested
service call is incorrect because it can bypass limits or observe a newer
generation partway through a business operation.

### 6.3 Four-direction equivalence

Rust-to-Rust may optimize after resolution, but its observable contract must
match the other three directions. In particular it must still enforce the
service ABI, lease rules, capability policy, generation pin, tracing, and error
mapping. Any fast path must be proven equivalent by tests and benchmarks.

## 7. Vela Service Namespaces And Dependencies

### 7.1 Static qualified calls

Vela source uses imported typed service namespaces:

```vela
use game::services::{inventory, reward};

reward::validate_exp(player, actual);
inventory::grant_level_reward(player, actual);
```

Name resolution binds each namespace to a `ServiceSlotId`, and each method to a
`MethodId`. HIR, MIR, bytecode/link metadata, and diagnostics preserve those IDs.
The VM does not look up `"reward"` and `"validate_exp"` strings on every call.

### 7.2 Dependency and effect discovery

The linker records each provider's statically referenced service slots and
methods. The effect checker computes the transitive upper bound across those
dependencies. Staging rejects missing slots, missing methods, incompatible
contracts, disallowed dependency edges, and insufficient capabilities before
activation.

Dynamic runtime gates remain mandatory for host-dependent permissions even when
static analysis succeeds.

### 7.3 Multiple services and named slots

A provider can import and call multiple services normally. The service graph is
not restricted to one dependency or one global singleton. The first execution
slice may expose one default slot per service trait, but slot identity and link
metadata must support later host-configured named instances.

Vela code cannot install or replace a service slot by mutating reflection data.
Selection and activation remain host/runtime policy.

### 7.4 Self calls and cycles

Calling the provider's own public service namespace performs normal selected
dispatch and is recursive. It consumes ordinary recursion and instruction
budgets. Static analysis should diagnose obvious unconditional self cycles, but
legal conditional recursion remains supported within configured limits.

Delegation to the displaced target must use an explicit base/delegate handle;
the preferred source spelling remains an open decision. The runtime must never
guess that a recursive-looking call intended to invoke the old implementation.

## 8. Provider Integration

The existing provider model remains useful for Vela implementation identity and
artifact validation. Extend it rather than replacing it wholesale:

- preserve `ProviderKey` as the exact identity of a Vela provider declaration;
- preserve provider signature and trait validation;
- add generated Rust provider metadata with the same contract identity rules;
- let a `ServiceSlot` select a validated Rust or Vela target;
- adapt `ProviderHandle` to use the unified prepared invocation route;
- keep explicit provider handles for host-controlled direct targeting where
  needed, but do not make them the normal application dependency API.

`ServiceSlotId` is stable host-facing indirection. `ProviderKey` identifies one
specific Vela artifact provider. Conflating them would prevent clean Rust/Vela
switching and multiple configured instances.

Changing the selected target kind from Rust to Vela is compatible only when the
complete `ServiceContract` matches. Target identity and generation may change
behind a slot; contract identity may not.

## 9. Staging, Activation, Reload, And Rollback

### 9.1 Staging

Before publication, staging must validate the complete candidate slot table:

- every selected target exists and implements the exact contract;
- every provider method has the required sync/async and parameter ABI;
- all linked service dependencies resolve;
- transitive effects and capabilities fit deployment policy;
- state schemas and reload requirements remain compatible;
- no unresolved provider or service IDs remain.

A failed stage changes no live selection.

### 9.2 Atomic activation

At a safe point, the runtime publishes one new immutable dispatch generation.
New root calls capture it. Existing roots, nested service calls, callbacks, and
futures continue using their old pinned generation until completion.

The minimum vertical slice must demonstrate a Rust service port switching to a
Vela provider, not merely a provider body reload within an already selected
Vela target.

### 9.3 Rollback

Rollback publishes another validated generation, commonly selecting the prior
Rust target. It applies to future root calls only. It does not rewind host
mutation, restore external state, or retry calls that entered the failing Vela
target.

### 9.4 State ownership

Service implementation objects and Vela provider records are not an implicit
persistent-state migration boundary. Persistent values needed across target
switches belong in:

- compatible Vela `state` for VM-owned persistent values;
- `extern state` for host-provided persistent `HostRef` values;
- explicit Rust host storage reached through the service contract.

Any future provider-private state migration requires a separate design with an
explicit schema and migration protocol.

## 10. Whole-Service And Partial Override Policy

The first implementation should select one target for the whole service. This
makes compatibility, effects, activation, rollback, and observability easier to
reason about.

If partial method override is later accepted, staging must materialize a
complete immutable method table. Every method entry is resolved to exactly one
Rust or Vela target before activation. Runtime behavior must not be:

```text
try override -> catch missing/error -> call Rust fallback
```

That form is unsafe after partial side effects. Missing override declarations
must be resolved at stage time, and implementation errors must propagate from
the selected entry.

An explicit base/delegate target may call the displaced implementation, but it
must be generation-pinned, capability-checked, observable in tracing, and
invoked only because source code requested it.

## 11. Effects, Capabilities, And Trust

Each service method contract declares an upper bound for effects and required
capabilities. A Vela implementation must not widen that bound. A Rust
implementation registration must declare the same metadata and is treated as
trusted native code after runtime gates succeed.

For a provider method that calls other services, the effective static set is
the union of:

- the method's direct Vela effects;
- every statically linked service dependency's contract effects;
- host mutation and reflection permissions used by those paths.

Runtime capability checks remain authoritative because slot configuration and
host grants are deployment-specific. A static pass must not erase runtime
checks, and a runtime grant must not make an ABI-incompatible link valid.

Tracing should record stable service slot and method IDs plus the resolved target
kind and generation. Do not use target kind as a security decision after
resolution.

## 12. Async, Re-entry, And Cancellation

- Sync versus async is part of the service ABI.
- Vela must use explicit `await` for an async service call according to the
  language's existing async rules.
- A host lease may cross suspension only when the current scoped async lease
  model proves object lifetime, executor safety, and cancellation cleanup.
- A generated adapter must not turn a call-scoped `&mut T` into an unscoped
  `'static` task capture.
- VM re-entry while an exclusive host lease is held must observe the existing
  lease and reject conflicting aliases. Only a provenance-authorized descendant
  reborrow may continue the exclusive chain; it is not a fresh acquisition.
- Cancellation and dropped futures release all acquired leases and retain the
  pinned dispatch generation only as long as required for cleanup.
- A Rust panic uses the runtime's existing panic-to-error policy and must never
  trigger automatic fallback execution.

The first vertical slice may implement synchronous service methods only if the
contract and metadata already reserve async shape and the limitation is explicit
in diagnostics and progress tracking.

## 13. Reflection, Tooling, And Diagnostics

### 13.1 Reflection

Reflection may expose read-only service metadata:

- contract path and stable IDs;
- methods, parameters, return types, and sync/async shape;
- boundary modes without exposing raw Rust references;
- declared effects and capabilities;
- configured slot names;
- selected target kind and generation where host policy allows it.

Reflection cannot mutate contracts, install providers, change slot selection, or
rewrite type structure at runtime.

### 13.2 Editor tooling

The LSP/analysis path should provide:

- completion for imported service namespaces and methods;
- signature help using the Vela-facing contract;
- hover showing contract, effects, capabilities, and selected slot name;
- go-to-definition to the service contract and provider implementation;
- references and rename based on stable semantic identities;
- diagnostics for missing slots, incompatible providers, effect widening,
  async misuse, and obvious unconditional service cycles.

### 13.3 Required structured diagnostics

At minimum, define stable diagnostics for:

- unsupported native reference parameter;
- unsupported borrowed return;
- host reference type mismatch;
- unprovable direct-host lease;
- aliased mutable host arguments;
- invalid or expired host reborrow provenance;
- call-scoped host handle escape;
- missing service slot or method;
- service contract mismatch;
- provider target not stageable;
- transitive effect or capability violation;
- async service call from invalid context;
- service dispatch generation unavailable;
- explicit base/delegate target unavailable.

Diagnostics should name the service, method, and relevant parameter or dependency
edge. Avoid exposing internal pointer values or host object addresses.

## 14. Crate And Module Ownership

| Area | Primary responsibility |
| --- | --- |
| `vela_common` / definition IDs | Stable `ServiceTraitId`, `ServiceSlotId`, `MethodId`, and diagnostic identities. |
| `vela_host` | Exact direct-host adapter proof, canonical lease identity, scoped reborrow provenance, atomic lease request sets, and RAII guards. |
| `vela_reflect` | Read-only service contract and boundary metadata. |
| `vela_macros` | Rust trait/native signature validation, generated adapters, registrations, and service ports. |
| `vela_hir` | Typed Vela service namespaces and semantic service/method identities. |
| `vela_analysis` and LSP crates | Dependency checks, effects, completion, navigation, hover, and diagnostics. |
| `vela_bytecode` / linker | Linked service-slot/method references and contract fingerprints. |
| `vela_vm` | Execute prepared Rust or Vela call targets without owning deployment selection policy. |
| `vela_engine` | Service registry, slots, generated runtime targets, root-call generation pinning, and context inheritance. |
| `vela_hot_reload` | Candidate validation, immutable generation publication, retirement, and rollback compatibility. |
| examples and docs | Mixed Rust/Vela multi-service scenario, operator workflow, and failure guidance. |

Use the actual repository boundaries discovered during implementation; do not
move unrelated systems merely to match this table. Split focused modules when
ownership would otherwise accumulate in a large existing file.

## 15. Execution Batches

Each batch ends with focused tests, formatting, linting where practical, a
documentation/progress update when status changed, and a small Conventional
Commit. Do not start a later batch to hide a failing earlier checkpoint.

### Batch A: Contract And Compile-Time Proof Surface

- [ ] A1. Resolve the open decisions in Section 19 and record accepted choices
  in `docs/decisions.md`.
- [ ] A2. Define stable service trait, slot, method, contract schema, and target
  generation identities.
- [ ] A3. Define the supported Rust-to-Vela ABI mapping, including sync/async,
  effects, capabilities, and host-reference modes.
- [ ] A4. Add macro compile-pass and compile-fail fixtures for every supported
  and rejected signature family.
- [ ] A5. Add deterministic service-contract fingerprinting and human-readable
  compatibility diffs.
- [ ] A6. Document trusted Rust semantics and the exact-object adapter contract.

Checkpoint: invalid Rust service/native signatures fail at compile time; valid
contracts produce deterministic metadata without changing runtime behavior.

### Batch B: Ordinary Rust Native Parameters

- [ ] B1. Extract one reusable parameter classifier shared by native functions,
  direct methods, and service method generation.
- [ ] B2. Support direct `&T` and `&mut T` parameters for synchronous free and
  context native functions.
- [ ] B3. Align async native functions and direct methods with the same declared
  classifier and lease rules.
- [ ] B4. Generalize atomic multi-lease acquisition to named parameter request
  sets with deterministic rollback.
- [ ] B5. Return the structured alias diagnostic before invoking Rust.
- [ ] B6. Make opaque/non-direct adapters fail closed with an actionable error.
- [ ] B7. Prove RAII cleanup on success, error, panic, re-entry failure,
  cancellation, and dropped futures.
- [ ] B8. Register hidden lease provenance for generated Rust references and
  derive safe shared/exclusive child reborrows for nested boundary calls.
- [ ] B9. Preserve canonical identity across child bindings and reject raw
  pointer/type matches that lack provenance or a valid root-call scope.
- [ ] B10. Stabilize the macro spelling and migration path for existing native
  registration APIs.

Checkpoint: users can expose supported ordinary Rust functions without boundary
wrapper parameters, and no conflicting reference set can enter Rust.

### Batch C: Rust Service Contract, Target, And Port

- [ ] C1. Generate `ServiceContract` metadata from an approved Rust trait.
- [ ] C2. Generate a Rust provider adapter and exact target registration.
- [ ] C3. Generate a typed `ServicePort` for the contract.
- [ ] C4. Route Rust-to-Rust port calls through the existing prepared runtime
  invocation path.
- [ ] C5. Map values, host leases, VM errors, and declared Rust errors without
  exposing boundary wrappers to the service author.
- [ ] C6. Reuse call-scoped host bindings for Rust root calls and provenance
  child bindings for nested service calls, without extending Rust lifetimes.
- [ ] C7. Reject duplicate IDs, incompatible registrations, missing methods, and
  unsupported target state at registry construction.
- [ ] C8. Establish a baseline benchmark for direct Rust calls and port calls.

Checkpoint: a Rust caller invokes a Rust service through a stable port with
normal budget, capability, tracing, and lease semantics.

### Batch D: `ServiceSlot` And Unified Runtime Target

- [ ] D1. Add immutable service slot tables and explicit default-slot identity.
- [ ] D2. Add sealed `ServiceMethodTarget` support to `Runtime::call` and
  `Runtime::call_async`.
- [ ] D3. Resolve Rust and Vela targets into existing prepared invocations.
- [ ] D4. Capture a dispatch generation at every root call and inherit it across
  nested calls, callbacks, re-entry, and futures.
- [ ] D5. Stage complete candidate tables without changing the live table.
- [ ] D6. Publish and retire immutable generations safely.
- [ ] D7. Audit that no duplicate budget, capability, heap, state, or host-access
  context was introduced.

Checkpoint: selection is stable for an entire call tree and the same call API
can prepare either target kind.

### Batch E: Vela Service Calls And Composition

- [ ] E1. Export service contracts and configured slot namespaces into the Vela
  semantic schema.
- [ ] E2. Resolve qualified service calls to exact slot and method IDs in HIR.
- [ ] E3. Preserve IDs and ABI fingerprints through MIR, bytecode, and linking.
- [ ] E4. Execute Vela-to-Rust calls using generated Rust target adapters.
- [ ] E5. Support multiple service dependencies in one Vela provider.
- [ ] E6. Implement explicit async service call syntax and diagnostics for the
  supported slice.
- [ ] E7. Compute transitive effects and reject invalid capability/dependency
  graphs during analysis or staging.
- [ ] E8. Diagnose obvious unconditional self cycles and rely on normal budgets
  for legal dynamic recursion.
- [ ] E9. Add completion, signature, hover, definition, references, and linked-ID
  rename behavior for service namespaces.

Checkpoint: a Vela provider calls multiple Rust services through typed linked
namespaces with normal runtime limits.

### Batch F: Vela Targets And Hot Override

- [ ] F1. Adapt existing Vela provider resolution to the unified service target.
- [ ] F2. Execute Rust-to-Vela calls through a generated service port.
- [ ] F3. Execute Vela-to-Vela calls through a pinned service generation.
- [ ] F4. Permit staged Rust/Vela target-kind changes only under an exact
  compatible service contract.
- [ ] F5. Atomically activate a Vela override for future root calls.
- [ ] F6. Roll back future root calls to the prior Rust target without replaying
  failed invocations.
- [ ] F7. Implement or explicitly defer the approved base/delegate mechanism.
- [ ] F8. Implement whole-service selection first; implement partial override
  only if Section 19 accepts it for this goal.
- [ ] F9. Verify compatible `state`/`extern state` preservation and reject
  incompatible state changes using existing reload policy.
- [ ] F10. Preserve current compatible Vela provider body-reload and handle
  re-resolution behavior.

Checkpoint: Rust and Vela targets can replace each other atomically while active
calls keep their original selection and persistent state follows explicit rules.

### Batch G: Acceptance, Documentation, And Performance

- [ ] G1. Build a multi-service game-server example with at least level, reward,
  and inventory services and both Rust and Vela targets.
- [ ] G2. Demonstrate Rust-to-Rust, Rust-to-Vela, Vela-to-Rust, and Vela-to-Vela
  paths in automated tests.
- [ ] G3. Demonstrate activation, in-flight generation isolation, rollback,
  alias rejection, and capability denial.
- [ ] G4. Document authoring, registration, deployment, observation, rollback,
  and debugging workflows.
- [ ] G5. Update architecture and decisions with the final ownership and ABI
  contract; archive this plan only after acceptance is complete.
- [ ] G6. Record benchmark results and optimize only measured regressions.
- [ ] G7. Audit for duplicate execution APIs, string-based hot-path lookup,
  leaked wrapper types, and unbounded execution paths.
- [ ] G8. Run all focused tests and the full workspace validation gates.
- [ ] G9. Update `docs/progress.md` only when the repository status actually
  reaches the corresponding checkpoint.

Checkpoint: the mixed implementation workflow is documented, observable,
generation-safe, boundary-safe, tested, and suitable for game-server hotfixes.

## 16. Acceptance Matrix

### 16.1 Parameter and lease safety

- [ ] Two distinct `&mut Player` arguments enter Rust and both mutate the
  correct objects.
- [ ] The same player passed to two `&Player` parameters is allowed.
- [ ] The same player passed to `&Player` and `&mut Player` is rejected before
  the Rust body runs.
- [ ] The same player passed to two `&mut Player` parameters is rejected before
  the Rust body runs.
- [ ] A Rust service holding `&mut Player` can pass a scoped reborrow to a nested
  Rust or Vela service and use the parent reference again after the child
  returns.
- [ ] A child reborrow preserves canonical host identity and does not let a
  second alias bypass the active exclusive chain.
- [ ] A reference with no active provenance or valid root scope cannot pretend
  to be a nested service argument based on address/type alone.
- [ ] A failed later argument conversion releases earlier leases.
- [ ] An opaque adapter with a matching type ID but no exact-object proof is
  rejected.
- [ ] Panic, error, cancellation, and dropped-future paths release leases.
- [ ] Conflicting VM re-entry observes the lease and fails safely.
- [ ] A stored call-scoped host handle cannot keep its Rust reference alive and
  fails deterministically after the originating scope closes.

### 16.2 Direction equivalence

- [ ] Rust caller -> Rust level service -> Rust reward and inventory services.
- [ ] Rust caller -> Vela level override -> Rust reward and inventory services.
- [ ] Rust caller -> Vela level override -> Vela reward override -> Rust
  inventory service.
- [ ] Vela caller -> Rust service with copied, shared-host, and exclusive-host
  parameters.
- [ ] Every direction reports the same contract mismatch, capability denial,
  budget exhaustion, and host alias classes.

### 16.3 Generation and reload behavior

- [ ] A root call begun before activation uses the old target for every nested
  service call.
- [ ] A root call begun after activation uses the new target for every nested
  service call.
- [ ] A suspended async call resumes with its pinned generation.
- [ ] Compatible Vela body reload preserves provider-handle re-resolution.
- [ ] Rust-to-Vela and Vela-to-Rust switches reject incompatible method ABI,
  effects, async shape, or state schema before publication.
- [ ] Rollback affects future roots and does not retry or rewind an in-flight
  call.
- [ ] Retired generations stay alive exactly while referenced and are reclaimed
  afterward.

### 16.4 Composition and failure semantics

- [ ] One provider imports and calls at least two different services.
- [ ] A normal self-service call is recursive and consumes the normal budgets.
- [ ] Explicit base/delegate invocation reaches the pinned displaced target if
  that feature is accepted.
- [ ] A selected implementation error propagates without automatic fallback.
- [ ] Static transitive effects and dynamic capabilities both reject invalid
  calls at their intended layer.

### 16.5 Reflection and tooling

- [ ] Reflection reports stable service identities and read-only selection
  metadata without permitting mutation.
- [ ] Completion and signature help use the Vela-facing contract.
- [ ] Definition, references, and rename use exact semantic IDs rather than
  name-text coincidence.
- [ ] Diagnostics remain precise across Rust and Vela definitions and include
  service, method, and parameter/dependency context.

## 17. Performance And Measurement

Record a reproducible baseline before optimization. Benchmark at least:

| Case | What it isolates |
| --- | --- |
| direct concrete Rust call | non-replaceable lower bound |
| service-port Rust target, cold resolution | preparation and registry cost |
| service-port Rust target, generation-local hit | steady Rust dispatch cost |
| Vela-to-Rust scalar call | VM/native conversion and call cost |
| Vela-to-Rust host-lease call | exact-object validation and lease cost |
| Rust-to-Vela call | Rust packaging plus VM entry cost |
| Vela-to-Vela nested call | linked service resolution and nested frame cost |
| multi-service nested business call | context inheritance and transitive checks |
| first call after activation | new-generation cache behavior |
| stale/wrong-generation handle | safe re-resolution or deterministic rejection |

Measure allocations, target-resolution work, lease acquisition, VM instructions,
and end-to-end latency where the harness permits it. Do not set an arbitrary
percentage budget before the baseline exists. Any fast path must retain the
acceptance semantics and have a regression test for the equivalence it relies
on.

## 18. Explicit Non-Goals

This goal does not implement:

- arbitrary discovery and invocation of every Rust trait or function;
- interception of direct calls to concrete Rust objects;
- script-language generics or Rust monomorphization from Vela;
- borrowed values escaping a native/service invocation;
- downcasting opaque adapters based only on a type ID;
- conversion of arbitrary nested `PathProxy` values into Rust field references;
- migration of active frames, active tasks, or provider-private object state;
- transactional rollback of host mutations or automatic retry on provider
  failure;
- script-controlled service installation or slot selection;
- runtime string-based service discovery as the primary linked call path;
- a second VM, provider-only execution context, or provider-only budget model;
- a full general-purpose LSP beyond feature-scoped service diagnostics and
  integration with the existing language-service capabilities;
- sandboxing trusted Rust code at individual field granularity;
- distributed RPC, service discovery, dependency injection across processes, or
  a general application container.

## 19. Open Decisions For Document Iteration

Resolve these before starting the batch that depends on them. The recommended
choice is a starting point, not yet an implementation fact.

### O1. Attribute and derive spelling

Recommended: choose one canonical hard-switch surface such as
`#[vela::service]`, `#[vela::service_impl]`, and `#[vela::native]`, with explicit
compile errors for legacy spellings after a documented migration window. Avoid
multiple macros that generate subtly different ABI metadata.

### O2. Service port ownership in Rust

Recommended: expose ports from the current `NativeCallContext` inside native
service calls and from an explicit clonable runtime service client at application
entry points. Both carry the same runtime authority and dispatch-generation
rules; neither stores a concrete provider reference.

The exact ergonomic forms could be:

```rust
ctx.services().reward().validate_exp(player, amount)?;
runtime.services().level().grant_exp(&mut player, amount)?;
```

### O3. Slot scope

Recommended: one default slot per service trait per runtime deployment in the
first slice, while IDs and metadata support host-configured named slots later.
Do not expose an ambient process-global mutable registry.

### O4. Base/delegate syntax

Recommended: make delegation explicit and statically linked, for example a
reserved `base::method(...)` namespace available only inside an override. It
must resolve to the displaced target in the pinned generation, not to whatever
is globally current.

Alternative: inject a typed delegate service port into provider context. This is
more explicit in Rust but may be noisier in Vela. Decide after a small syntax and
cycle analysis.

### O5. Whole-service versus partial override

Recommended: ship whole-service selection in this goal and record partial
per-method override as a follow-up. If partial override is pulled into scope, it
must use a complete immutable method table materialized during staging.

### O6. Rust provider state ownership

Recommended: the host owns Rust provider instances and registers generation-safe
shared handles whose lifetimes dominate all calls. The service slot owns only a
validated target handle, never moves Rust host state into script GC, and does not
implicitly recreate state during target switching.

### O7. Scoped host-handle escape policy

Recommended: preserve the existing safe call-scoped borrowed-host mechanism,
tag every generated temporary `HostRef` with its originating call scope, reject
attempts to persist it into `state`, `extern state`, globals, or escaping heap
containers, and invalidate all remaining copies when the scope closes. This
gives Rust application callers and nested services ordinary `&T`/`&mut T`
signatures without pretending those references are `'static`.

The minimum safety requirement is deterministic invalidation on scope close;
the preferred authoring experience also reports the escape attempt at the write
site. Resolve the cost of recursively checking nested containers before Batch C
rather than leaving escape semantics implicit.

## 20. Suggested First Vertical Slice Task

```text
Task: Implement the minimal generation-pinned Rust/Vela service override slice.

Context:
  Build on the existing provider handle, runtime call target, HostLeaseRef,
  HostLeaseMut, CallArgs atomic acquisition, and hot-reload generation model.

Expected behavior:
  - A Rust LevelService is registered behind a generated level service port.
  - The default Rust LevelService passes a scoped reborrow of &mut Player to a
    Rust RewardService through its port, then continues using Player.
  - A staged Vela LevelService provider calls the same Rust RewardService.
  - Activation changes future level calls from Rust to Vela atomically.
  - An in-flight call keeps its original service generation.
  - Rollback changes future calls back to Rust without retrying prior failures.
  - Passing one Player to two exclusive host parameters fails before Rust runs.

Tests:
  - rust_service_port_invokes_registered_rust_target
  - vela_provider_calls_rust_service_through_linked_slot
  - rust_service_port_invokes_activated_vela_target
  - service_dispatch_generation_is_pinned_for_nested_calls
  - rollback_changes_future_roots_without_replaying_calls
  - aliased_mutable_service_arguments_fail_before_invocation
  - nested_service_reborrow_preserves_identity_and_restores_parent

Do not change:
  - Do not expose HostRef, HostPath, PathProxy, HostLeaseRef, HostLeaseMut, or
    HostAccess in the user-authored LevelService signature.
  - Do not introduce script generics or borrowed returns.
  - Do not add a second service-only VM execution API.
  - Do not implement partial override or provider-private state migration.

Validation:
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
```

This slice is intentionally narrower than the full goal. It proves the central
execution and safety model before broadening async support, tooling, reflection,
named slots, or base delegation.

## 21. Final Completion Criteria

The goal is complete only when all of the following are true:

1. Supported ordinary Rust native and service signatures expose no Vela-specific
   host wrapper types to their authors.
2. Atomic lease acquisition prevents sibling alias violations before Rust
   references are created, while provenance-preserving child reborrows support
   safe nested service composition without extending lifetimes.
3. Generated service contracts and ports provide stable semantic identities and
   deterministic compatibility checks.
4. Rust-to-Rust, Rust-to-Vela, Vela-to-Rust, and Vela-to-Vela calls share one
   runtime execution and policy path.
5. Vela providers call multiple services using typed, statically linked service
   namespaces.
6. A root call and every nested operation observe one immutable service-dispatch
   generation.
7. Activation and rollback affect future roots atomically without automatic
   replay, host-state rewind, or implicit provider-state migration.
8. Effects, capabilities, budgets, cancellation, tracing, reflection, and
   diagnostics remain consistent across target kinds.
9. The multi-service game-server example and acceptance matrix are green, with
   performance results recorded against a reproducible baseline.
10. Architecture, decisions, authoring/operator documentation, progress, and
    archived plans accurately describe the shipped behavior, and full workspace
    formatting, lint, and test gates pass.
