# Rust/Vela Unified Interop And Call Model Plan

> Track: ordinary Rust signatures, signature-derived effects, grouped export
> bundles, generated bidirectional bindings, unified call execution,
> host-reference lease safety, and optional hot-swappable service dispatch
>
> Status: approved design direction; implementation has not started
>
> Baseline: `master` at `bf524975e` on 2026-07-16
>
> Execution: coherent pre-release batches; reuse the existing VM call, native,
> method, provider, and re-entry paths
>
> Roadmap: queued work; this document does not replace the active checkpoint in
> `progress.md` until explicitly scheduled

This document defines a general Rust/Vela interoperability model. Its primary
goal is not service replacement. Its primary goal is that explicitly exported
Rust and Vela functions and methods can call each other with ordinary source
syntax and ordinary business types.

Rust authors should write copied or owned values and call-scoped `&T`/`&mut T`
parameters. Vela authors should call Rust functions and methods with the same
syntax used for Vela functions and methods. Rust callers should use generated,
typed Vela bindings rather than manually constructing `CallArgs`, `OwnedValue`,
`HostRef`, or method targets.

The authoring surface is wrapper-free; the runtime boundary is not. Generated
code must still perform conversion, ABI validation, capability checks, exact
host-identity validation, atomic lease acquisition, generation pinning, and
error conversion. `HostRef`, `HostPath`, `PathProxy`, `HostLeaseRef`,
`HostLeaseMut`, and `HostAccess` remain internal safety and execution
primitives, but they are not normal business-function parameter types.

Service contracts, provider selection, dispatch slots, and generation-safe hot
override are an optional layer built on this general callable model. Ordinary
Rust/Vela interop must not require a service trait or a service slot.

This is deliberately not arbitrary Rust ABI reflection. Only explicitly
exported Rust items and explicitly exported Vela items enter the cross-language
contract. Registration and generated metadata are required; handwritten
boundary wrappers are not.

## 0. Codex Goal

Use the following command when this plan is approved for execution and
scheduled:

```text
/goal Execute docs/rust-vela-interop-model-plan.md in full.
```

The execution goal is complete only after every required batch and acceptance
case in this document is implemented, verified, documented, and committed.
During execution, keep `docs/progress.md` aligned with the active checkpoint and
record accepted design decisions in `docs/decisions.md`.

### Fixed design constraints

1. Ordinary supported Rust functions and methods use ordinary Rust parameter
   and return types. Normal business code must not mention Vela host-boundary
   wrappers.
2. Vela uses the same function-call, qualified-call, and method-call syntax
   regardless of whether the statically resolved target is implemented in Vela
   or Rust.
3. Rust calls Vela through generated typed bindings tied to an explicit
   `Runtime` or active `NativeCallContext`. There is no ambient process-global
   runtime.
4. Generated adapters may use `HostRef`, `HostPath`, `PathProxy`,
   `HostLeaseRef`, `HostLeaseMut`, `HostAccess`, `CallArgs`, and runtime values
   internally. These types do not leak into ordinary authored signatures.
5. Vela never receives or stores a real Rust reference. A generated adapter may
   create an invocation-scoped `&T` or `&mut T` only after the boundary has
   proved exact type, canonical identity, lifetime, and lease authority.
6. All host-parameter leases for one invocation are validated and acquired as
   one atomic request set before any Rust reference is created.
7. A nested cross-language call may derive a scoped child reborrow from a live
   parent lease. It inherits canonical identity and provenance and is not an
   unrelated second acquisition.
8. Borrowed returns and escaped call-scoped host references are unsupported.
   Generated references cannot be stored in Vela state, returned to Vela,
   cached in native state, or moved into an unscoped task.
9. Direct Vela field, index, and path mutations continue through
   `HostRef`/`HostPath`/`PathProxy` and `HostAccess` with their normal fine-grain
   direct-operation policy. That policy is not imported into ordinary callable
   contracts.
10. Invoking a trusted Rust callable with `&mut T` is one controlled host call.
    `HostAccess` gates the callable, its effect-derived coarse capabilities,
    exact object, and exclusive lease at the call boundary; the trusted Rust
    body may then mutate any field permitted by Rust for that invocation.
11. Field-level sandboxing inside trusted Rust bodies is not an initial goal.
    Security is initially enforced at callable, capability, effect, type, and
    lease granularity. Later sandbox refinement must not distort ordinary
    signatures or create a second execution model. Arbitrary business
    permission strings are not part of ordinary native-call authorization.
12. Every cross-language callable has stable semantic identity, deterministic
    boundary metadata, and an ABI fingerprint. Static IDs are used where
    available; runtime strings are not the linked hot-path locator.
13. Rust functions, Vela functions, methods, providers, and optional slot
    targets enter the existing `Runtime::call`/`Runtime::call_async` and
    same-session re-entry model. Do not create language-direction-specific call
    APIs or execution loops.
14. Nested calls inherit the pinned linked artifact, state view, heap, host
    boundary, remaining budgets, capabilities, tracing, and cancellation.
15. Sync versus async, parameter modes, return type, and the effective effect
    upper bound are callable ABI. For Rust exports, the effective set is the
    signature-inferred base effect union explicit additional effects. Coarse
    required capabilities are a deterministic projection of that final set.
    Active deployment grants, callable allowlists, policy profiles, and
    reflection-tool permissions are runtime policy, not callable ABI.
16. A direct call to a concrete Rust function or value remains an ordinary Rust
    call. It is not intercepted or made hot-swappable implicitly.
17. Optional service hot override uses an immutable dispatch generation pinned
    by the root call. It does not redefine the ordinary callable boundary.
18. No `unsafe` reference fabrication is allowed. Lease and binding guards use
    safe Rust and RAII across success, error, panic, cancellation, re-entry
    failure, and dropped futures.
19. Scattered Rust functions may use item-level export attributes. Related
    functions and methods use explicit module/impl export groups that infer
    stable paths and base effects, generate one deterministic registration
    bundle, and never rely on ambient inventory or process-global discovery.

### Required implementation batches

- Batch A: shared callable contract and compile-time proof surface.
- Batch B: ordinary Rust export signatures and generated adapters.
- Batch C: natural Vela-to-Rust function and method calls.
- Batch D: generated typed Rust-to-Vela bindings.
- Batch E: nested bidirectional calls, reborrows, async, and unified policy.
- Batch F: optional Rust/Vela service slots and generation-safe hot override.
- Batch G: end-to-end acceptance, tooling, documentation, and performance.

### Never-complete conditions

Do not declare this goal complete while any of the following remains true:

- a supported Rust business signature must mention `HostRef`, `PathProxy`, a
  lease guard, `OwnedValue`, or another boundary wrapper;
- Vela needs target-specific syntax to distinguish a Rust callable from a Vela
  callable;
- ordinary typed Rust-to-Vela calls require users to assemble `CallArgs`,
  convert `OwnedValue`, or look up runtime strings manually;
- ordinary function interop requires declaring a service trait or installing a
  service slot;
- conflicting host aliases can enter Rust as simultaneous live references;
- a nested call cannot safely reborrow a live Rust reference and restore parent
  use afterward;
- any language direction uses a second execution, budget, capability, heap,
  cancellation, or tracing path;
- generated Rust bindings can silently call a Vela function with an
  incompatible ABI after reload;
- the trusted-native sandbox boundary is ambiguous about field-level policy;
- callable contracts, binding schemas, ABI fingerprints, or native-call hot
  paths contain arbitrary business permission strings or active deployment
  grants;
- ordinary `&T`/`&mut T` effects must be repeated manually, or a related
  function group requires one path prefix and Engine registration call per
  function;
- service override remains the only demonstrated use of the interop layer;
- focused, workspace, formatting, and lint validation are not green.

## 1. Product Promise And Terminology

### 1.1 Natural call

In this document, a natural call means:

- Vela calls an exported Rust free function with normal `module::function(...)`
  or imported function syntax;
- Vela calls an exported Rust method with normal `receiver.method(...)` syntax;
- Rust calls an exported Vela function or method through generated typed Rust
  functions or methods;
- parameters and results use ordinary domain types supported by the boundary
  contract;
- boundary handles, value erasure, lease guards, target lookup, and runtime
  bookkeeping are generated and hidden;
- sync, async, error, capability, and mutation behavior remain explicit and
  deterministic.

Natural does not mean that a dynamic Vela function literally becomes a native
Rust symbol. A Rust-to-Vela call must identify the owning Runtime because code,
state, hot-reload generation, budgets, and capabilities are runtime-local. The
generated binding carries that authority without making the caller operate the
boundary manually.

### 1.2 Wrapper-free authoring

The requirement is no user-authored boundary wrapper, not no internal adapter.
The following types are runtime implementation details for the ordinary path:

```text
HostRef / HostPath / PathProxy
HostLeaseRef / HostLeaseMut / LeaseProvenanceId
HostValue / OwnedValue / VelaValue
CallArgs / prepared invocation records
native erased thunks / linked call targets
```

Low-level embedding APIs may remain available for dynamic tooling, reflection,
tests, and hosts that genuinely need runtime-selected calls. They are not the
target authoring experience for statically known cross-language calls.

### 1.3 Explicit export, not arbitrary discovery

An ordinary Rust item is not script-visible merely because it exists. An item
enters the interop schema only through an approved export attribute, derive,
registration API, or generated contract. The export surface fixes its stable
path, signature, effective effect upper bound, docs, and semantic
visibility/reflection access. Its coarse capability requirement is derived
from the effect set. The
active `ExecutionProfile`, capability grants, callable allowlists, and other
deployment policy do not enter the export schema or callable fingerprint.

Likewise, only public Vela items included in an emitted binding schema become
typed Rust bindings. Private helpers remain private and need no Rust-facing
ABI.

## 2. Target User Experience

The syntax below fixes the intended authoring experience. Minor generated type
or helper names may change during implementation, but export grouping, path
defaults, signature-inferred base effects, and explicit-extra-effect spelling
are part of this plan.

### 2.1 Export an ordinary Rust function to Vela

```rust
#[vela::export(path = "game::grant_exp")]
pub fn grant_exp(player: &mut Player, amount: i64) -> VmResult<()> {
    player.exp += amount.max(0);
    Ok(())
}
```

Vela calls it like any other function:

```vela
use game::grant_exp;

pub fn level_up(player: Player, amount: i64) {
    grant_exp(player, amount);
}
```

The Vela value is a host-object handle. The generated Rust adapter resolves its
exact identity, acquires an exclusive call-scoped lease, invokes `grant_exp`
with `&mut Player`, and releases the lease. Vela never observes the Rust
reference.

### 2.2 Optional native call context

An exported Rust function may request runtime services with a hidden context:

```rust
#[vela::export(path = "game::grant_exp")]
pub fn grant_exp(
    _ctx: &mut NativeCallContext<'_, '_>,
    player: &mut Player,
    amount: i64,
) -> VmResult<()> {
    player.exp += amount.max(0);
    Ok(())
}
```

`NativeCallContext` is supplied by the runtime and is not a Vela-visible
argument. The runtime checks the coarse capability projection of the effective
effect set before the body starts; the context does not introduce arbitrary
business permission strings. Functions that do not need re-entry, state,
tracing, cancellation, or other runtime services omit it.

### 2.3 Export ordinary Rust methods

```rust
#[vela::methods]
impl Player {
    pub fn grant_exp(&mut self, amount: i64) -> VmResult<()> {
        self.exp += amount.max(0);
        Ok(())
    }

    pub fn level(&self) -> i64 {
        self.level
    }

    fn normalize_amount(amount: i64) -> i64 {
        amount.max(0)
    }
}
```

Vela uses ordinary method syntax:

```vela
player.grant_exp(10);
let before = player.level();
```

Receiver syntax is not a special host proxy API. The compiler resolves the
registered method and the generated adapter performs the required shared or
exclusive receiver lease. An explicit `#[vela::methods]` block exports its
supported public methods; private helpers remain ordinary Rust-only methods.
Per-method `#[vela::export(...)]` is reserved for a rename, access override, or
additional effects.

### 2.4 Export Rust trait implementations

Vela traits are runtime protocols, not a projection of Rust's complete trait
system. Export is explicit: implementing a Rust trait does not automatically
make that trait, its methods, its associated items, or its supertraits visible
to Vela.

For a trait and implementation authored in the integration crate, export the
protocol contract and annotate the existing implementation directly:

```rust,ignore
#[vela::trait_export(path = "game::Damageable")]
pub trait Damageable {
    fn take_damage(&mut self, amount: i64);
    fn is_alive(&self) -> bool;
}

#[vela::methods]
impl Damageable for Player {
    fn take_damage(&mut self, amount: i64) {
        self.hp -= amount.max(0);
    }

    fn is_alive(&self) -> bool {
        self.hp > 0
    }
}
```

`#[vela::methods]` therefore supports both explicit inherent impl blocks and
explicit trait impl blocks. On a trait impl it generates callable adapters for
the boundary-safe trait methods, records that the receiver implements the
Vela protocol, and adds both the protocol and implementation to the enclosing
export bundle. It does not generate a second Rust trait impl or change Rust
method resolution. A per-method skip or export override may narrow the exposed
surface. Every selected method must be boundary-safe; an unsupported selected
method fails at its declaration instead of disappearing silently.

The same form works when the trait comes from another crate if the application
owns and can annotate the legal Rust impl block:

```rust,ignore
#[vela::methods(protocol = "game::Damageable")]
impl external_game::Damageable for Player {
    fn take_damage(&mut self, amount: i64) {
        external_game::damage_player(self, amount);
    }

    fn is_alive(&self) -> bool {
        self.hp > 0
    }
}
```

The explicit `protocol` path maps the Rust trait to a stable Vela protocol
identity; Rust trait paths are not used as accidental public ABI.

When the type, trait, and existing impl all live in external crates, the
application cannot attach an attribute or create a duplicate impl. A
declaration-only adapter lists the selected boundary surface and generates the
UFCS call thunks without user-authored wrapper bodies:

```rust,ignore
vela::export_external_trait_impl! {
    type external_game::Player;
    trait external_game::Damageable as "game::Damageable";

    fn take_damage(&mut self, amount: i64);
    fn is_alive(&self) -> bool;
}
```

This declaration repeats the exported method signatures because stable Rust
provides no general reflection API for enumerating an external trait or impl.
Generated code must type-check each declared signature against the referenced
UFCS method. If an integration crate already ships a Vela export bundle, the
application registers that bundle instead of restating the declarations.

Trait methods follow the same parameter classifier, effect inference, lease,
return, async, and ABI rules as inherent methods. Generic methods, exposed
associated types, uncontrolled lifetimes, and Rust-only parameters such as
`Formatter<'_>` cannot be exported directly. Such a method needs an explicit
boundary-safe mapping, for example mapping `Display` to a Vela `to_string()`
method. Marker and implementation-detail traits such as `Send`, `Sync`, and
`Serialize` are not exposed merely because a type implements them.

Vela does not inherit Rust's UFCS ambiguity rules. A type's directly callable
script-visible method names must remain unique across its inherent and trait
surfaces. A collision requires an explicit script-visible rename or a future
protocol-qualified call form; the first slice rejects an unresolved collision
at registration.

The current `#[script(implements = "...")]` metadata annotation is not this
feature: it records a reflected name but does not prove a Rust trait bound or
install trait-method call targets. Implementation must replace that metadata-
only claim for the new export path with generated, type-checked protocol and
implementation descriptors.

### 2.5 Signature-inferred effects and explicit extras

The export macro derives a conservative base effect from the classified Rust
signature:

| Signature fact | Inferred base effect |
| --- | --- |
| no host receiver or host parameter | `pure` |
| one or more `&T` / `&self` host borrows | `host_read` |
| any `&mut T` / `&mut self` host borrow | `host_write` |

`host_write` subsumes signature-visible host reads. `HiddenContext` does not by
itself add an effect. Extra effects that are not visible in the signature use
an identifier list and are unioned with the inferred base:

```rust,ignore
#[vela::export(
    path = "game::roll_and_notify",
    effects(random, event_emit)
)]
pub fn roll_and_notify(
    ctx: &mut NativeCallContext<'_, '_>,
    player: &mut Player,
) -> VmResult<()> {
    todo!("use capability-scoped random/event APIs through ctx and mutate player")
}
```

Explicit effects may widen but never remove the signature-inferred base. The
effective set, not whether a component was inferred or written, participates
in the callable fingerprint. Therefore removing a redundant explicit
`host_write` annotation is not an ABI change.

### 2.6 Bulk export and one-time registration

Hosts with many related functions use one explicit export-module boundary.
Immediate supported `pub fn` items are exported under the configured prefix;
private helpers are not:

```rust,ignore
#[vela::export_module(path = "game")]
mod exports {
    pub fn normalize(amount: i64) -> i64 {
        amount.max(0)
    }

    pub fn settle_level(
        ctx: &mut NativeCallContext<'_, '_>,
        player: &mut Player,
        amount: i64,
    ) -> VmResult<Reward> {
        let mut rules = ctx.bindings::<game_bindings::Rules>()?;
        let reward = rules.calculate_reward(player, player.level)?;
        player.exp += amount.max(0);
        Ok(reward)
    }

    fn normalize_amount(amount: i64) -> i64 {
        amount.max(0)
    }
}
```

The module macro derives `game::normalize` and `game::settle_level`, emits one
deterministic descriptor/adapter bundle, and exposes one generated registration
entrypoint:

```rust,ignore
let engine = Engine::builder()
    .register_exports(exports::vela_exports())
    .build()?;
```

Within an export module, a function-level `#[vela::export(...)]` is needed only
for a rename, access override, docs/metadata override, or explicit additional
effects. The export module is the explicit approval boundary; this is not
process-wide discovery or automatic exposure of unrelated Rust items.
An unsupported immediate public function is a declaration-time error rather
than being silently skipped; make a helper private or move it outside the
export module.

### 2.7 Call an exported Vela function from Rust

Vela source:

```vela
pub fn calculate_reward(player: Player, level: i64) -> Reward {
    return Reward {
        amount: math::max(level * 10, 0),
        rare: level >= 20,
    };
}
```

Generated Rust binding:

```rust,ignore
let mut game = runtime.bindings::<game_bindings::Game>()?;
let reward: Reward = game.calculate_reward(&mut player, level)?;
```

The exact generated type name is open, but the caller must not manually:

- locate `"game::calculate_reward"` at runtime;
- construct `CallArgs`;
- turn `Player` into `HostRef`;
- turn `Reward` into or out of `OwnedValue`;
- select sync versus async execution through a separate API family;
- validate the Vela function ABI after reload.

The binding is runtime-bound authority and compile-time type information, not a
business-object proxy. A Runtime remains explicit because a Vela function has
runtime-local code, state, policy, and generation ownership.

### 2.8 Nested Rust-to-Vela re-entry

An exported Rust function can call a typed Vela binding through its active
context:

```rust,ignore
#[vela::export(path = "game::settle_level")]
pub fn settle_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: &mut Player,
    amount: i64,
) -> VmResult<Reward> {
    let mut rules = ctx.bindings::<game_bindings::Rules>()?;
    let reward = rules.calculate_reward(player, player.level)?;
    player.exp += amount.max(0);
    Ok(reward)
}
```

The nested call derives a child reborrow from the active `player` lease. Parent
use is suspended by Rust for the child call and resumes afterward. The child
inherits the current execution session rather than starting a new Runtime
execution.

### 2.9 Async calls

Vela uses its existing explicit await syntax:

```vela
let profile = load_profile(player_id).await;
```

Generated Rust bindings expose an ordinary Rust async method or function:

```rust,ignore
let profile = game.load_profile(player_id).await?;
```

Sync versus async is part of the callable contract. A generated sync binding
cannot invoke an async target accidentally, and an async binding uses the same
scoped `Send` execution future and session driver as `Runtime::call_async`.

### 2.10 Optional replaceable service

Only operations that require implementation selection or hot override need a
service contract and slot:

```rust,ignore
#[vela::service(path = "game::LevelService")]
pub trait LevelService {
    fn grant_exp(&self, player: &mut Player, amount: i64) -> VmResult<()>;
}
```

The service generator reuses the same parameter classifier, callable
contracts, Rust adapters, Vela bindings, lease handling, and runtime execution
path. It adds only stable slot selection and dispatch-generation behavior.

Ordinary `game::grant_exp(...)` does not require this trait. A direct concrete
Rust call also remains direct and is not intercepted.

## 3. Core Model

### 3.1 `CallableContract`

Every exported function or method has a language-neutral callable contract:

```text
CallableContract {
    identity
    public_path
    callable_kind
    parameters
    return_type
    sync_or_async
    effects
    access
    ABI_fingerprint
    docs_and_origin
}
```

`CallableContract` is a logical shared model, not necessarily one new public
Rust struct or one global numeric ID. Existing `FunctionKey`,
`NativeFunctionId`, `MethodId`, `HostMethodId`, package identity, and descriptor
types should converge on the same signature and validation rules without
discarding identity distinctions that remain useful.

The contract is reflection metadata and a compile/link/runtime validation
input. It exposes neither Rust layout nor mutable runtime type structure.
`effects` is the effective upper bound: the signature-inferred base union
explicit additional effects. It deterministically projects to the
domain-neutral `CapabilitySet` used by runtime authorization. `access` contains
semantic public/reflection visibility, not deployment grants or arbitrary
permission names. The active profile, callable allowlist, host-type allowlist,
and reflection policy remain outside this contract.

### 3.2 Boundary modes

Each parameter has a stable boundary mode:

```text
Value
ReadOnlyValueBorrow
SharedHost
ExclusiveHost
HiddenContext
```

Mode is ABI. Changing a parameter between owned value, `&T`, and `&mut T` is
not a compatible body-only change. Neither is changing sync/async shape,
parameter order or stable identity, return type, or the effective effect upper
bound. The derived coarse capability requirement changes with the effect set;
it is not a separately authored second ABI field.

### 3.3 Resolved call target

The existing sealed call-target model should resolve all supported target
kinds into one prepared invocation path. Conceptually, resolved targets include:

```text
Vela function
Vela bound method
Rust exported function
Rust exported host method
Rust exported trait method
Vela provider method
optional service-slot method
```

The target kind affects preparation and adapter selection, not session,
budget, heap, capability, tracing, cancellation, or error ownership.

Do not introduce public execution families such as:

```text
call_rust / call_vela
call_native_typed / call_script_typed
call_service / call_provider_reentry
run_export / run_import
```

Generated bindings lower to the existing root or re-entry authority and the
same sealed target abstraction.

### 3.4 Generated Rust export adapter

An exported Rust callable produces an erased runtime adapter that:

1. exposes deterministic signature and reflection metadata;
2. validates argument count, names, modes, and runtime values;
3. builds the complete host-lease request set;
4. validates exact type and canonical host identity;
5. acquires every lease atomically;
6. converts ordinary values and invocation-scoped borrows;
7. invokes the authored Rust function or method;
8. converts its result or error;
9. releases all leases and temporary bindings by RAII.

The authored function is not itself an erased `Fn(&[Value])` wrapper and does
not perform these steps manually.

### 3.5 Generated Vela import binding for Rust

An emitted Vela interface schema generates typed Rust functions or methods.
The generated binding contains:

- stable package/module/function or method identity;
- expected callable ABI fingerprint;
- ordinary Rust parameter and return types;
- root Runtime or active `NativeCallContext` authority;
- generated value and host-binding conversion;
- compatible hot-reload re-resolution behavior;
- source metadata for diagnostics.

Binding generation must consume the compiler's authoritative semantic/export
schema. It must not implement a second ad hoc Vela parser in a procedural macro
or build script.

### 3.6 Optional dispatch slot

A dispatch slot is indirection over one or more `CallableContract` entries. It
is needed only when the host selects among implementations, such as a Rust
default and a Vela hotfix.

```text
ServiceContract = stable group of CallableContract entries
ServiceSlotId = configured dependency identity
ServiceTarget = validated Rust or Vela implementation
ServiceDispatchGeneration = immutable selected-target table
```

Slots do not own values, VM frames, heap state, host state, budgets, or
capabilities. They resolve a target before the ordinary prepared-call path.

## 4. Signature Boundary And ABI

### 4.1 Initial supported mapping

| Rust declaration | Boundary mode | Vela view | Rule |
| --- | --- | --- | --- |
| `&mut NativeCallContext<'_, '_>` | hidden context | no argument | Supplied by runtime and never storable. |
| `bool`, chars, numeric scalars, unit | value | corresponding value | Existing scalar conversion and guards. |
| approved owned records/enums | value | corresponding record/enum | Stable schema and generated conversion required. |
| `String`, bytes, approved owned containers | value | string/bytes/container | Ownership crosses explicitly. |
| `&str`, `&[u8]` | read-only value borrow | string/bytes | Only from invocation-scoped stable storage. |
| `&T` for a registered direct host type | shared host | host object | Exact-object shared lease for the call. |
| `&mut T` for a registered direct host type | exclusive host | host object | Exact-object exclusive lease for the call. |
| `&self`, `&mut self` on exported host methods | shared/exclusive host | method receiver | Same classifier and lease model as parameters. |
| supported `Option<T>`/`Result<T, E>` | structured value | Option/Result form | Every nested type must be boundary-safe. |
| `VmResult<T>` return | runtime result | value or call error | Error maps through normal VM diagnostics. |

The first implementation retains the current `Send + Sync` requirement for
mutable direct host origins. Any later relaxation must be proved separately for
sync and async execution and cannot be hidden in macro expansion.

### 4.2 Unsupported first-slice signatures

Reject these at export or binding generation time with a diagnostic on the
exact item or parameter:

- borrowed returns including `&T`, `&mut T`, `&str`, slices, and borrowed
  container views;
- raw pointers, pinned references, mutex guards, lease guards, task-local
  guards, and implementation types carrying uncontrolled lifetimes;
- generic exported functions or methods that require Vela monomorphization;
- exposed associated types or user-defined higher-ranked lifetimes;
- variadic, `extern`, or `unsafe` exported functions;
- overloaded script-visible names;
- types without stable boundary metadata and conversion support;
- arbitrary cross-boundary Rust closure captures or Vela closure retention in
  the first vertical slice.

Rust implementation generics that do not enter the exported ABI may remain an
internal Rust detail after macro expansion proves one concrete exported
signature. This must not add script-language generics.

### 4.3 Return and error mapping

Borrowed values cannot leave an invocation. Initial returns are copied/owned
boundary values, Runtime-managed Vela values retained by generated bindings, or
explicit host handles whose ownership already dominates the call.

The generated API must distinguish:

- Vela `Result<T, E>` as an ordinary language value;
- Rust `Result<T, E>` exported as a declared structured result when `E` is a
  boundary type;
- `VmResult<T>` or host/runtime errors as call failure with structured
  diagnostics.

An implementation error never triggers automatic fallback execution after the
target may have produced effects.

### 4.4 Trusted Rust boundary and coarse sandbox

An exported Rust body is trusted native code. Before it begins, the runtime
enforces:

- callable visibility and registration;
- the effective effect upper bound and its derived coarse capabilities;
- parameter and return ABI;
- exact host type and canonical identity;
- shared/exclusive lease compatibility;
- execution and host-call budgets;
- tracing and cancellation entry policy.

Once a valid exclusive lease has produced `&mut T`, Rust may mutate any field
permitted by Rust. Script-visible field access metadata does not sandbox the
inside of trusted Rust code.

This is intentional. The initial sandbox is callable-grained, not
field-grained. A deployment that later needs stronger isolation may:

- expose fewer native callables;
- apply stricter capability profiles and callable allowlists;
- separate trusted and restricted export sets;
- opt a specific restricted function into the low-level `HostAccess` API.

It must not make all ordinary functions accept proxies merely to support a
possible future fine-grained sandbox.

### 4.5 Callable ABI and deployment policy

The interop boundary keeps semantic contracts separate from deployment policy.

Callable ABI contains:

- stable callable identity and kind;
- ordered parameters, boundary modes, return/error mapping, and sync/async
  shape;
- the effective `EffectSet` upper bound;
- semantic public and reflection-access flags.

Deployment policy contains:

- the active `ExecutionProfile` and granted `CapabilitySet`;
- the registered/exported callable surface and any callable or host-type
  allowlist;
- execution and host-call budgets;
- reflection-tool permissions and filesystem sandbox configuration.

The required `CapabilitySet` for a callable is derived deterministically from
its `EffectSet`; it is not independently authored. Ordinary callable metadata,
generated binding schemas, and ABI fingerprints must not contain arbitrary
business permission strings or reuse reflection member
`required_permissions`. Changing active grants may require validation or
restaging, but it is not an interop callable or generated-binding ABI change.

Initial Runtime policy is fixed for the Runtime or deployment generation so
prepared target caches do not depend on a per-user, per-object, or per-call
permission graph. If mutable policy is introduced later, one coarse
Runtime-level policy generation may invalidate prepared authorization caches;
it must not add field-level policy dimensions to ordinary call targets.

### 4.6 Effect inference and nested enforcement

The shared parameter classifier computes the Rust signature's base effect at
macro expansion time. `SharedHost` contributes `host_read`, `ExclusiveHost`
contributes `host_write`, and `host_write` dominates `host_read`. Value borrows
such as `&str` and `&[u8]` do not count as host reads. A hidden
`NativeCallContext` contributes no effect by itself.

Explicit `effects(...)` entries add to that base. The macro rejects attempts to
remove or contradict an inferred effect and emits only the final normalized
`EffectSet` into `CallableContract`, reflection, binding schemas, and ABI
fingerprints. It does not scan arbitrary Rust function bodies or helper-call
graphs; such scanning would be incomplete across traits, macros, aliases, and
conditional compilation.

The unified export path must not inherit the existing shape-specific macro
fallback that treats every omitted effect as `pure`. Omission means "use the
classified signature base". Low-level `HostRef`/`NativeCallContext` descriptors
whose effects are not visible in an ordinary signature continue to declare
their effects explicitly.

Capability-scoped `NativeCallContext` operations and generated nested bindings
enforce two conditions before the operation or child callable begins:

1. the requested operation or child's effective effects are a subset of the
   current Rust callable's effective effect ceiling;
2. the active Runtime profile grants the derived coarse capabilities.

For example, a signature-inferred `host_write` callable may invoke a pure or
`host_read` child. It must explicitly add `random` before invoking a child that
uses randomness. A mismatch fails before the child body runs and never widens
the parent contract dynamically.

Trusted Rust can still perform an undeclared direct global Rust side effect
without going through `NativeCallContext`; the runtime cannot introspect or
undo arbitrary Rust code. Policy-sensitive IO, events, time, random,
reflection, and re-entry should therefore use capability-scoped context APIs.
Bypassing them is a trusted-native contract violation, not a reason to add
proxies to ordinary parameters.

## 5. Host Lease, Alias, And Lifetime Safety

### 5.1 Invocation preflight

Generated adapters build the entire request set before creating Rust
references:

```text
HostParamLeaseRequest {
    callable_identity
    parameter_identity
    argument_index
    canonical_host_identity
    expected_concrete_type
    mode: Shared | Exclusive
    source: RootBinding | Reborrow(LeaseProvenanceId)
}
```

The runtime validates every request and all pairwise conflicts, then acquires
the set atomically. A later value conversion failure rolls back earlier
acquisitions without invoking authored Rust code.

### 5.2 Alias matrix

| Same canonical host object | Result |
| --- | --- |
| shared + shared | allowed |
| shared + exclusive | rejected |
| exclusive + shared | rejected |
| exclusive + exclusive | rejected |

Different canonical objects may be acquired together, including multiple
objects with the same Rust type.

Alias failure is a stable structured diagnostic such as
`AliasedMutableHostArguments`. It names the callable and both parameters and
never depends on a Rust panic from an attempted borrow.

### 5.3 Exact-object requirement

Ordinary Rust references are available only for a direct host object whose
registered adapter proves:

- exact concrete Rust type;
- canonical lease identity shared by every alias;
- pinned object address and lifetime for the invocation;
- safe temporary reference production without identity change;
- appropriate shared or exclusive lease support.

A matching Rust `TypeId` is insufficient. Opaque or generic adapters fail
closed when they cannot provide this proof. A `PathProxy` to a nested field does
not automatically become `&mut Field`; nested paths continue through
`HostPath`/`HostAccess` unless that nested value is independently registered as
an exact direct host object.

### 5.4 Scoped reborrow provenance

A nested call may safely pass a reborrow of a current Rust reference:

```rust,ignore
pub fn apply_rule(
    ctx: &mut NativeCallContext<'_, '_>,
    player: &mut Player,
) -> VmResult<()> {
    ctx.bindings::<game_bindings::Rules>()?
        .validate_player(player)?;
    player.validated = true;
    Ok(())
}
```

Generated entry adapters register a hidden `LeaseProvenanceId` for each live
host reference. Generated Rust-to-Vela bindings derive a child binding with the
same canonical identity while Rust suspends use of the parent borrow.

| Parent provenance | Child request | Result |
| --- | --- | --- |
| shared | shared | allowed scoped child reborrow |
| shared | exclusive | rejected |
| exclusive | shared | allowed scoped child reborrow |
| exclusive | exclusive | allowed scoped child reborrow |

The parent lease remains the authority. A child cannot outlive it, change
identity, bypass an active exclusive chain, or be retained by an unscoped task.
Pointer address and type alone never authorize a reborrow.

### 5.5 Lifetime, escape, and cleanup

Invocation-scoped references and handles cannot escape into Vela `state`,
`extern state`, globals, returned heap containers, a native cache, or an
unscoped task. The preferred authoring experience diagnoses escape at the write
site. The minimum runtime requirement is deterministic invalidation when the
originating scope closes.

RAII releases temporary references, bindings, and leases on:

- normal return;
- returned error;
- converted Rust panic;
- nested re-entry failure;
- async suspension completion or cancellation;
- future drop;
- callback unwind.

## 6. Vela-To-Rust Calls

### 6.1 Registration and schema

Rust export macros generate or register:

- canonical public path and stable native/method identity;
- `CallableContract` and ABI fingerprint;
- parameter names, types, modes, defaults, and docs;
- signature-inferred base effects, explicit additional effects, the normalized
  effective upper bound, derived coarse capabilities, visibility, and
  reflection access;
- the erased export adapter;
- compile-time rejection for unsupported Rust signatures.

Scattered exports may use item-level `#[vela::export(path = "...")]`. Related
free functions should use `#[vela::export_module(path = "...")]`, which treats
supported immediate `pub fn` items as the explicit export set, derives their
paths from the prefix and Rust names, and generates one deterministic
`vela_exports()` registration bundle. `#[vela::methods]` provides the same
explicit block boundary for supported public inherent methods and supported
trait impl methods. `#[vela::trait_export]` contributes a Vela protocol
contract, while a declaration-only external-trait adapter contributes the
selected signatures and generated UFCS thunks for an impl that cannot be
annotated. Private items and unselected Rust traits remain Rust-only.
Unsupported public functions or methods inside an explicit group fail at
their declaration instead of silently disappearing from the export schema.

The generated bundle is registered once through `EngineBuilder::register_exports`.
It is an ordinary value produced by generated code, not ambient inventory,
linker-section discovery, a process-global registry, or runtime source
scanning. Multiple bundles may be registered explicitly; normal duplicate path
and stable-identity validation applies to their combined schema.

Duplicate stable identities or public paths are rejected during registration
or compilation. Macro-generated descriptors and hand-written low-level
descriptors must converge on the same registry schema.

### 6.2 Static resolution

The Vela resolver treats an exported Rust function as a normal module function
and an exported Rust method as a normal resolved method. HIR retains exact
semantic identity; MIR and linked bytecode lower to stable target identifiers
or linked call entries. Names remain for source, reflection, and diagnostics,
not per-call hot-path lookup.

Known signature facts drive:

- argument count and named-argument checks;
- explicit async/await diagnostics;
- effect analysis and its coarse capability projection;
- host shared/exclusive mode validation where facts are available;
- completion, signature help, hover, definition, and references.

### 6.3 Runtime preparation

At runtime, a linked Rust target uses the ordinary prepared-invocation path:

```text
linked call target
  -> callable ABI validation
  -> HostAccess callable/effect-derived capability gate
  -> value conversion and atomic lease preflight
  -> generated Rust adapter
  -> authored Rust body
  -> result/error conversion
  -> ordinary session continuation
```

No provider, service slot, or separate native execution loop is required.

## 7. Rust-To-Vela Calls

### 7.1 Authoritative binding schema

The Vela compiler or Engine emits a deterministic Rust-binding schema from the
same package/module graph, HIR, registry facts, and exported callable metadata
used for linking. It includes:

- stable package and module identity;
- exported function and method identities;
- parameter names, types, modes, and defaults;
- return and error mapping;
- sync/async shape;
- effective effect upper bounds and derived coarse capability requirements;
- contract fingerprints and source origins.

The schema does not serialize the Runtime's active grants, callable allowlist,
reflection-tool permissions, or other deployment policy. Those facts bind when
the generated surface is attached to a Runtime.

Binding generation must not infer API from source text with a second parser.
Generation may be exposed through a CLI, build helper, or Engine API, but all
forms consume the same schema and produce deterministic Rust code.

### 7.2 Typed runtime binding

Generated code provides a module- or package-shaped Rust surface. Binding to a
Runtime validates its expected schema against the active `LinkedArtifact` once
and prepares stable handles. Calls then use IDs or linked handles rather than
runtime strings.

A generated binding may internally create `CallArgs` and conversion guards,
but callers pass ordinary Rust values and references. Compatible body-only hot
reload re-resolves stable targets. An incompatible signature, mode, asyncness,
or effect change is rejected before the binding can call it. A deployment
profile or grant change is policy validation, not binding ABI incompatibility.

### 7.3 Root calls and re-entry

The same generated interface has two authorities:

- a root binding borrowed from `Runtime` starts one ordinary root execution;
- a nested binding borrowed from `NativeCallContext` pushes a child call into
  the current `ExecutionSession`.

These may be separate generated carrier types if Rust lifetimes require it,
but they expose the same callable methods and do not duplicate target
resolution or conversion rules.

Nested calls inherit the current linked artifact and host boundary. Root calls
capture the active artifact and create a new call-scoped host boundary from the
ordinary arguments.

### 7.4 Dynamic escape hatch

`Runtime::call`, `Runtime::call_async`, function handles, `CallArgs`, and
runtime-managed values remain valid low-level APIs for dynamic names, generic
tools, reflection, plugin managers, and tests. They are not removed merely
because typed generated bindings exist.

The documentation should lead ordinary static integrations to generated
bindings and reserve manual APIs for genuinely dynamic cases.

## 8. Unified Execution And Context Inheritance

Every root call constructs one execution-owned host boundary and drives the
existing `ExecutionSession`. Every nested cross-language call pushes through
the same session and inherits:

- pinned `LinkedArtifact` and program generation;
- optional pinned service-dispatch generation;
- VM `state` and host-provided `extern state` view;
- heap, GC roots, scope stack, and frame ownership;
- instruction, allocation, recursion, host-call, and collection budgets;
- host access, exact bindings, and lease provenance;
- effect policy and capability grants;
- tracing correlation and cancellation token.

Only the selected callable target changes. A nested Rust-to-Vela or
Vela-to-Rust call must not create a fresh Runtime context, replenish budgets,
or observe a newer hot-reload generation partway through one operation.

Rust-to-Rust calls may remain direct when the author calls a concrete Rust
function. Such calls have ordinary Rust semantics and are outside Runtime
policy. A caller that wants runtime policy or replaceability uses an exported
binding or optional service slot explicitly.

## 9. Host Mutation Semantics

### 9.1 Direct Vela mutation

Vela expressions such as:

```vela
player.exp += 10;
account.ledger[entry_id].amount = value;
```

continue to lower through `HostRef`, resolved `HostTargetPlan`, `HostPath` or
`PathProxy`, and fine-grained `HostAccess`. Field, index, method, source-span,
and permission metadata remain authoritative for these direct script
operations. They do not become permission strings or field ACLs on an ordinary
Rust callable contract.

### 9.2 Trusted Rust mutation

A Vela call such as:

```vela
grant_exp(player, 10);
```

that resolves to Rust is represented as one controlled native host call.
`HostAccess` validates permission to invoke the callable and grants its exact
shared/exclusive leases. The Rust body then mutates through ordinary Rust
references for the invocation.

This path does not synthesize a field-by-field mutation journal and does not
pretend to enforce script field permissions inside trusted Rust. Reads after
the call observe current host state. A later script or runtime failure does not
roll back completed Rust mutation automatically.

### 9.3 Future sandbox refinement

Future security work should prefer coarse controls that preserve the authoring
model:

- callable allow/deny lists;
- capability profiles;
- effect ceilings;
- trusted versus restricted native export sets;
- host-type allowlists;
- execution and host-call budgets;
- separate process or WASM isolation for truly untrusted native code, if ever
  required.

Field-level control inside arbitrary Rust code is not realistically enforceable
after granting `&mut T`. A restricted callable may explicitly opt into
`HostAccess` instead, but that is an advanced security API rather than the
default business-function signature. These controls remain deployment policy;
they do not change ordinary callable ABI or generated binding fingerprints.

## 10. Optional Service And Hot-Override Layer

### 10.1 Service contract as a callable group

A service contract groups stable `CallableContract` entries behind one
configured dependency identity. Rust and Vela implementations are validated
against the same group contract. The group adds no new parameter conversion or
lease semantics.

```text
ServiceTraitId
  + stable method CallableContract entries
  + configured slot name
  -> ServiceSlotId
```

The first slice may support one default slot per service trait while preserving
ID room for host-configured named instances.

### 10.2 Target selection

```rust,ignore
enum ServiceTarget {
    Rust(RustServiceTarget),
    Vela(VelaServiceTarget),
}
```

The selected target must match the complete service contract. Initial
implementation selects one target for the whole service. A later partial
override must materialize a complete immutable per-method table during staging;
runtime must never use `try override, then fallback on missing/error`.

### 10.3 Typed namespaces and ports

Vela service namespaces and generated Rust service ports are generated views of
the same callable contracts. Their methods use the same ordinary signatures as
direct exports. They resolve a slot and then enter the common prepared-call
path.

Static Vela resolution preserves `ServiceSlotId` and method identity through
HIR, MIR, bytecode, linking, diagnostics, and tooling. Runtime strings are not
the primary locator.

### 10.4 Dispatch generations

The runtime publishes immutable slot tables by generation. A root call captures
one generation token; nested service calls, callbacks, re-entry, and futures
inherit it. Activation publishes a new generation for future roots while old
generations remain alive exactly as long as active calls reference them.

The dispatch generation owns selection only. It does not duplicate VM state,
heap, HostAccess, budgets, capabilities, tracing, or cancellation.

### 10.5 Staging, activation, and rollback

Staging validates the complete candidate slot table:

- every target exists and implements the exact contract;
- every dependency resolves to a configured slot and method;
- transitive effects and their derived coarse capabilities fit deployment
  policy;
- async shape and host parameter modes match;
- persistent `state`/`extern state` schemas remain compatible;
- no unresolved provider or target IDs remain.

A failed stage changes nothing. Activation atomically changes future roots.
Rollback publishes another validated generation, commonly selecting the prior
Rust target. It never retries a call that may already have produced effects and
does not rewind host state.

### 10.6 Provider identity and state

Existing `ProviderKey` remains the identity of one Vela provider declaration.
`ServiceSlotId` identifies a configured dependency. They must not be conflated.

Provider-private state is not migrated implicitly. Persistent business state
belongs in compatible Vela `state`, host `extern state`, or explicit Rust host
storage. Any future provider-private migration requires a separate schema and
migration design.

## 11. Effects, Capabilities, And Trust

Every exported callable publishes one normalized effective `EffectSet` upper
bound. For Rust exports this is the signature-inferred base union explicit
additional effects. Its required domain-neutral `CapabilitySet` is derived by
the same canonical mapping during registration, analysis, linking, and runtime
dispatch. Callables do not author a second capability list and do not carry
arbitrary business permission strings.

For Vela functions that call other exported functions or service slots, static
analysis computes the transitive upper bound of known effects and its coarse
capability projection. Runtime checks remain authoritative because the active
profile, callable surface, allowlists, and slot configuration are
deployment-specific.

The effective effect upper bound participates in callable ABI. Active grants,
the selected `ExecutionProfile`, callable/host-type allowlists, reflection-tool
permissions, and filesystem policy do not. A policy may reject binding,
staging, or invocation before authored code runs, but a policy difference must
not be reported as an interop callable or generated-binding ABI mismatch.

Trust rules are:

- Vela direct host operations obey fine-grained HostAccess policy;
- trusted Rust exports obey callable-level gates and lease safety;
- a Vela implementation may not widen its declared contract effects;
- capability-scoped context operations and nested bindings may not exceed the
  current Rust callable's effective effect ceiling;
- target kind is recorded for tracing but is not a security decision after
  validation;
- reflection member `required_permissions` remain reflection tooling/policy
  metadata and are not native-call business authorization;
- reflection cannot install exports, alter contracts, or change slot
  selection.

## 12. Async, Re-entry, And Cancellation

- Sync versus async is callable ABI.
- Vela uses explicit `await` under its existing rules.
- Generated Rust bindings expose matching Rust async calls.
- A host lease crosses suspension only when the scoped async lease model proves
  lifetime, `Send` safety, and cancellation cleanup.
- Generated code never turns a borrowed `&mut T` into an unscoped `'static`
  capture.
- Re-entry while an exclusive lease is held rejects unrelated aliases and
  allows only provenance-authorized descendants.
- Cancellation and dropped futures release leases and bindings by RAII.
- A Rust panic follows existing panic-to-error policy and never triggers
  automatic fallback.
- No core crate owns an executor and no language direction adds another frame
  driver.

The first vertical slice may be synchronous if contract metadata, diagnostics,
and generated schemas already reserve and correctly reject unsupported async
forms.

## 13. Reflection, Tooling, And Diagnostics

### 13.1 Reflection

Reflection may expose read-only callable metadata:

- public path and stable identity;
- source language and callable kind;
- parameters, boundary modes, return type, and asyncness;
- effects, derived coarse capability requirements, docs, and source origin;
- optional service slot and selected target kind where policy permits.

Reflection cannot synthesize Rust references, mutate callable contracts,
install providers, change slots, or rewrite type structure.
It also does not project active Runtime grants, callable allowlists, or
business permission strings into the callable ABI.

### 13.2 Tooling

The analysis/LSP path should provide the same experience for Rust and Vela
targets:

- completion for exported functions, methods, and generated service
  namespaces;
- signature help from the Vela-facing callable contract;
- hover for types, modes, effects, derived capability requirements, docs, and
  origin;
- go-to-definition to source-backed Rust schema origins or Vela declarations;
- references and rename based on semantic identity;
- diagnostics for unsupported signatures, ABI mismatch, alias conflicts,
  async misuse, and unavailable bindings.

Generated Rust bindings should retain source comments and origin metadata where
practical so Rust compiler errors can name the Vela declaration that produced a
binding.

### 13.3 Structured diagnostics

At minimum define stable diagnostics for:

- unsupported exported Rust parameter or return;
- unsupported generated Vela binding type;
- duplicate exported callable identity or path;
- callable ABI or binding fingerprint mismatch;
- host reference type mismatch;
- unprovable direct-host lease;
- aliased mutable host arguments;
- invalid or expired reborrow provenance;
- call-scoped host handle escape;
- missing exported callable or method;
- async call from an invalid context;
- effect or capability denial;
- optional service target or dispatch generation unavailable.

Diagnostics name callable, parameter, source origin, and dependency edge where
applicable. They never expose pointer values or raw host addresses.

## 14. Crate And Module Ownership

| Area | Primary responsibility |
| --- | --- |
| `vela_common` / definition IDs | Stable callable, function, method, service, diagnostic, and source identities. |
| `vela_host` | Exact-object proof, canonical lease identity, atomic requests, reborrow provenance, HostAccess gates, and RAII. |
| `vela_reflect` | Read-only callable contracts, type and Vela protocol metadata, implemented-protocol relationships, effects, derived coarse capabilities, and origins; no live deployment grants. |
| `vela_macros` | Rust signature classification, function/inherent/trait export adapters, external-impl UFCS declarations, descriptors, and compile-time diagnostics. |
| `vela_hir` | Resolve Rust exports like normal functions/methods and retain exact callable identity. |
| `vela_analysis` / LSP crates | Call facts, effects, completion, navigation, hover, and diagnostics. |
| `vela_bytecode` / linker | Linked callable targets, binding schemas, and ABI fingerprints. |
| `vela_vm` | Execute prepared Rust or Vela targets on one session without deployment-selection policy. |
| `vela_engine` | Registration, authoritative binding-schema emission, Runtime policy/profile ownership, target preparation, and root-call authority. |
| `vela_hot_reload` | Callable ABI comparison, artifact publication, and optional slot-generation publication/retirement. |
| optional bindgen module or crate | Deterministic Rust code generation from Engine/compiler-owned binding schema. |
| examples and docs | Non-service round-trip interop first; mixed Rust/Vela service override as an extension example. |

Use repository boundaries discovered during implementation rather than moving
unrelated systems merely to match this table. Share one parameter classifier
and one contract model instead of growing parallel macro-specific mappings.

## 15. Execution Batches

Each batch ends with focused tests, formatting, linting where practical, a
progress update only when status changed, and a small Conventional Commit. Do
not start a later batch to hide a failing earlier checkpoint.

### Batch A: Callable Contract And Proof Surface

- [ ] A1. Implement the fixed export/effect spelling from Section 2 and resolve
  the remaining bindgen delivery decisions from Section 19.
- [ ] A2. Define the shared callable contract, boundary modes, fingerprints,
  normalized effective effects, human-readable ABI diffs, and one canonical
  effect-to-capability projection.
- [ ] A3. Extract one parameter classifier shared by free functions, context
  functions, host methods, async methods, and optional services. It must return
  both boundary modes and the signature-inferred base effect.
- [ ] A4. Define deterministic conversion traits or generated operations for
  every supported value, host, return, and error family.
- [ ] A5. Add macro and bindgen compile-pass/compile-fail fixtures for all
  supported and rejected signatures, inferred effects, and explicit additive
  effect lists.
- [ ] A6. Keep deployment grants, allowlists, reflection member permissions,
  and arbitrary business permission strings out of callable contracts,
  binding schemas, fingerprints, and native-call hot paths.
- [ ] A7. Record callable-grained trusted Rust semantics, the ABI/policy split,
  and deferred field-level sandboxing in architecture and authoring docs.
- [ ] A8. Define stable Vela protocol identities and deterministic trait-method
  contracts without treating Rust trait paths or `TypeId` as public ABI.

Checkpoint: valid contracts produce deterministic metadata and normalized
effects; invalid signatures or contradictory effect declarations fail at their
declaration without changing runtime behavior; changing Runtime grants or
removing a redundant explicit effect does not change a callable fingerprint.

### Batch B: Ordinary Rust Exports

- [ ] B1. Support ordinary copied/owned parameters for item-level
  `#[vela::export]` and module-level `#[vela::export_module]` through one
  canonical descriptor/adapter path.
- [ ] B2. Support direct `&T` and `&mut T` parameters for synchronous free and
  context functions.
- [ ] B3. Align exported `&self`/`&mut self` methods with the same classifier.
- [ ] B4. Generalize atomic multi-lease acquisition to named callable
  parameters with deterministic rollback.
- [ ] B5. Produce structured alias and exact-object diagnostics before entering
  authored Rust.
- [ ] B6. Register hidden lease provenance for every generated Rust reference.
- [ ] B7. Prove cleanup on success, error, panic, re-entry failure,
  cancellation, and dropped futures.
- [ ] B8. Keep low-level descriptor APIs available without making them the
  default authoring surface.
- [ ] B9. Generate one deterministic `vela_exports()` bundle per export module
  and register it explicitly with one `register_exports` call, without ambient
  inventory or runtime discovery.
- [ ] B10. Extend `#[vela::methods]` to explicit trait impl blocks, add
  `#[vela::trait_export]`, and generate reflection metadata and call thunks
  through the same method adapter path used by inherent methods.
- [ ] B11. Add a declaration-only external-trait adapter for an existing impl
  that cannot be annotated. Require selected signatures, UFCS type checking,
  an already boundary-supported receiver type, and no duplicate Rust impl.

Checkpoint: supported Rust exports use ordinary signatures, many related
functions register as one explicit bundle, and no conflicting reference set
can enter authored Rust.

### Batch C: Natural Vela-To-Rust Calls

- [ ] C1. Export Rust functions and methods into the semantic registry and
  compiler facts.
- [ ] C2. Resolve Vela function, qualified, and method calls to exact Rust
  target identities.
- [ ] C3. Preserve identities and ABI fingerprints through HIR, MIR, bytecode,
  linking, reflection, and diagnostics.
- [ ] C4. Execute exported Rust targets through the existing prepared runtime
  path and session continuation.
- [ ] C5. Make ordinary Vela calls independent of service/provider setup.
- [ ] C6. Apply callable effects, derived coarse capabilities, budgets,
  tracing, and cancellation consistently before authored Rust runs.
- [ ] C7. Add completion, signature, hover, definition, and reference coverage
  for Rust exports.
- [ ] C8. Resolve exported trait methods through stable Vela protocol and
  implementation identities, including runtime `implements` checks and
  dynamic protocol dispatch.

Checkpoint: Vela calls Rust free functions and methods with ordinary syntax and
ordinary Vela values or host objects.

### Batch D: Typed Rust-To-Vela Bindings

- [ ] D1. Define one authoritative exported Vela binding schema from semantic
  and linked metadata that excludes live Runtime grants and deployment policy.
- [ ] D2. Generate deterministic Rust module/package bindings without a second
  Vela parser.
- [ ] D3. Bind generated code to a Runtime and validate contract fingerprints.
- [ ] D4. Convert ordinary Rust values, references, results, and errors without
  user-authored `CallArgs` or `OwnedValue` handling.
- [ ] D5. Support stable-ID target lookup and compatible body-reload
  re-resolution.
- [ ] D6. Generate active-context bindings for same-session re-entry.
- [ ] D7. Report source-backed Rust diagnostics for missing or incompatible
  Vela exports.

Checkpoint: Rust calls exported Vela functions through typed generated methods
without runtime strings or manual boundary values.

### Batch E: Nested Bidirectional And Async Calls

- [ ] E1. Preserve canonical host identity across generated child reborrows.
- [ ] E2. Support Vela -> Rust -> Vela and Rust -> Vela -> Rust call trees on
  one execution session.
- [ ] E3. Restore parent reference usability after a child shared or exclusive
  reborrow returns.
- [ ] E4. Reject unrelated aliases, expired provenance, and scoped-handle
  escape deterministically.
- [ ] E5. Align generated sync and async exports/bindings with the existing
  scoped `Send` future model.
- [ ] E6. Prove budget, coarse capability profile, heap, state, tracing,
  generation, and cancellation inheritance across every language transition
  without a per-call permission graph.
- [ ] E7. Reject a capability-scoped context operation or nested binding whose
  effects exceed the current Rust callable's effective ceiling before the
  operation or child body runs.
- [ ] E8. Establish round-trip and boundary-cost benchmarks before optimizing.

Checkpoint: nested bidirectional calls behave like one call tree and preserve
Rust alias safety, Runtime policy, and hot-reload ownership.

### Batch F: Optional Service Slots And Hot Override

- [ ] F1. Generate service contracts as groups of the shared callable
  contracts.
- [ ] F2. Generate Rust service adapters/ports and Vela service namespaces from
  the same signatures.
- [ ] F3. Add immutable slot tables and a sealed slot-method target to the
  existing call-target model.
- [ ] F4. Resolve Rust and Vela service implementations into common prepared
  invocations.
- [ ] F5. Pin one dispatch generation across each root call tree.
- [ ] F6. Stage and atomically publish compatible Rust/Vela target switches.
- [ ] F7. Roll back future roots without replaying calls or rewinding state.
- [ ] F8. Preserve existing provider identity, discovery, body reload, and
  handle re-resolution behavior.
- [ ] F9. Ship whole-service selection first and defer partial override unless
  separately approved.

Checkpoint: optional Rust/Vela service targets can replace each other without
changing the ordinary interop ABI or active-call selection.

### Batch G: Acceptance, Documentation, And Performance

- [ ] G1. Build a non-service round-trip example whose Vela code calls ordinary
  Rust functions/methods and whose Rust host calls exported Vela functions.
- [ ] G2. Build a separate mixed Rust/Vela multi-service hot-override example.
- [ ] G3. Cover signature conversion, alias rejection, nested reborrow,
  inferred/additional effects, nested effect-ceiling denial, capability denial
  before authored code, policy-versus-ABI separation, local and external trait
  export, async cancellation, and reload ABI mismatch.
- [ ] G4. Document export, binding generation, registration, calling,
  debugging, deployment, activation, and rollback workflows.
- [ ] G5. Audit public examples and docs for unnecessary `HostRef`, `CallArgs`,
  `OwnedValue`, lease, proxy, and runtime-string ceremony.
- [ ] G6. Record reproducible boundary benchmarks and optimize only measured
  regressions.
- [ ] G7. Audit for duplicate execution APIs, duplicate signature classifiers,
  per-function path/effect ceremony that should be inferred, string-based
  linked or permission lookup, ambient export discovery, live grants in
  fingerprints, escaped wrappers, and unbounded paths.
- [ ] G8. Run focused and full workspace validation gates.
- [ ] G9. Update `docs/progress.md` only when the repository reaches the
  corresponding checkpoint.

Checkpoint: ordinary bidirectional interop is the primary documented workflow;
service override is a tested optional extension; all safety and validation
gates pass.

## 16. Acceptance Matrix

### 16.1 Authoring ergonomics

- [ ] An exported Rust scalar function uses only ordinary Rust types.
- [ ] An exported Rust host-mutating function accepts `&mut T` without authored
  host wrappers or a redundant `host_write` annotation.
- [ ] An exported Rust host method accepts ordinary `&self`/`&mut self`.
- [ ] An annotatable Rust trait impl exports through `#[vela::methods]`
  without an inherent forwarding method or user-authored wrapper body.
- [ ] An existing external trait impl exports a selected, explicitly declared
  boundary-safe surface with UFCS signature checking and no duplicate impl.
- [ ] Implementing a Rust trait alone exposes nothing to Vela; marker traits
  and unsupported methods remain Rust-only.
- [ ] `&T`/`&self` infer `host_read`, `&mut T`/`&mut self` infer
  `host_write`, and value-only signatures infer `pure`.
- [ ] Explicit `effects(...)` add to but cannot remove the signature-inferred
  base, and only the normalized final set enters the fingerprint.
- [ ] One explicit export module registers many supported public functions
  through one generated bundle; private helpers remain unexported.
- [ ] Unsupported public functions or methods inside an explicit export group
  fail at declaration time rather than being silently skipped.
- [ ] Vela calls Rust exports with normal function, qualified, and method
  syntax.
- [ ] Rust calls a Vela export through generated typed code without `CallArgs`,
  `OwnedValue`, `HostRef`, or a runtime string.
- [ ] Ordinary interop works with no service trait, provider, or slot.

### 16.2 Parameter and lease safety

- [ ] Two distinct `&mut Player` arguments enter Rust and mutate the correct
  objects.
- [ ] The same player passed to two `&Player` parameters is allowed.
- [ ] Shared plus exclusive, or two exclusive sibling parameters for one
  object, fail before the Rust body runs.
- [ ] A nested Vela or Rust call receives an authorized child reborrow and the
  parent reference is usable afterward.
- [ ] A child preserves canonical identity and cannot bypass an active
  exclusive chain.
- [ ] Pointer/type coincidence without provenance or a valid root scope is
  rejected.
- [ ] A failed later conversion releases earlier leases.
- [ ] Opaque adapters with type ID but no exact-object proof are rejected.
- [ ] Panic, error, cancellation, re-entry failure, and future drop release
  leases.
- [ ] Borrowed results and scoped-handle escapes fail deterministically.

### 16.3 Direction and nesting equivalence

- [ ] Vela calls a Rust free function.
- [ ] Vela calls a Rust `&self` method.
- [ ] Vela calls a Rust `&mut self` method.
- [ ] Rust calls a Vela free function through generated binding.
- [ ] Vela -> Rust -> Vela re-entry uses one session.
- [ ] Rust -> Vela -> Rust nested dispatch uses one session.
- [ ] Every direction reports equivalent ABI, capability, budget, alias, and
  cancellation error classes.
- [ ] A nested binding whose effect set exceeds its Rust parent's effective
  ceiling fails before the child body runs.

### 16.4 Reload and generation behavior

- [ ] Compatible Vela body reload keeps generated Rust bindings valid through
  stable re-resolution.
- [ ] Incompatible parameter, mode, return, effect, or async change is rejected
  before invocation.
- [ ] Changing active Runtime grants or allowlists is handled as policy
  validation or restaging, not interop callable/binding ABI incompatibility.
- [ ] An active nested call tree retains one linked artifact generation.
- [ ] Optional service activation changes future roots only.
- [ ] Optional service rollback does not retry or rewind an in-flight call.
- [ ] Suspended async calls retain their pinned artifact and optional dispatch
  generation.

### 16.5 Trust, reflection, and tooling

- [ ] Callable capability denial occurs before a trusted Rust body runs.
- [ ] Capability-scoped context operations cannot exceed the current Rust
  callable's effective effect ceiling even when the Runtime grants more.
- [ ] Callable contracts, generated binding schemas, and fingerprints contain
  no arbitrary business permission strings or active deployment grants.
- [ ] Coarse callable requirements are derived from `EffectSet`; reflection
  member `required_permissions` are not reused for native-call authorization.
- [ ] Documentation explicitly states that `&mut T` grants field-level Rust
  authority for the invocation.
- [ ] Direct Vela path writes retain fine-grained HostAccess checks.
- [ ] Reflection reports callable metadata without creating references or
  mutating contracts.
- [ ] Reflection reports stable Vela protocol identities, selected trait
  methods, and implemented-protocol relationships without claiming that all
  Rust traits are Vela-visible.
- [ ] Completion, signature help, hover, definition, and references work for
  Rust exports.
- [ ] Generated Rust binding errors name their Vela source declaration.

## 17. Performance And Measurement

Record reproducible baselines before optimization:

| Case | What it isolates |
| --- | --- |
| direct concrete Rust call | non-runtime lower bound |
| Vela-to-Rust scalar export | linked target and scalar conversion |
| Vela-to-Rust shared host call | exact identity and shared lease |
| Vela-to-Rust exclusive host call | exclusive lease and adapter thunk |
| Vela-to-Rust exported method | receiver resolution and lease |
| Vela-to-Rust exported trait method | protocol implementation resolution and lease |
| Rust-to-Vela generated root call | binding, root host scope, and VM entry |
| Rust-to-Vela same-session re-entry | child binding and frame push/pop |
| Vela -> Rust -> Vela round trip | provenance and context inheritance |
| generated binding after reload | stable re-resolution and ABI guard |
| optional service slot local hit | generation-local target resolution |
| first optional service call after activation | new-generation cache behavior |

Measure allocation, conversion, target resolution, lease acquisition, VM
instructions, and end-to-end latency where the harness permits it. Do not set
an arbitrary overhead budget before the baseline exists. Fast paths must retain
the same safety and policy semantics and have fallback-equivalence tests.
Ordinary authorization stays on the fixed-bitset path derived from
`EffectSet`; benchmarks and cache designs must not introduce dynamic string or
reflection-permission lookups into each native call.

## 18. Explicit Non-Goals

This plan does not implement:

- automatic exposure or invocation of every Rust item;
- automatic exposure of every Rust trait implemented by an exported type;
- runtime enumeration of external Rust trait definitions or impl graphs;
- arbitrary Rust ABI reflection;
- interception of direct calls to concrete Rust functions or objects;
- an ambient global Runtime hidden behind generated calls;
- literal absence of internal generated adapters or conversion code;
- field-level sandboxing inside trusted Rust functions that receive `&mut T`;
- per-user, per-object, or per-field permission graphs in the ordinary
  callable hot path;
- arbitrary business permission strings in callable contracts, binding
  schemas, or ABI fingerprints;
- script-language generics or Rust monomorphization from Vela;
- borrowed data escaping an invocation;
- downcasting opaque adapters from type ID alone;
- arbitrary nested `PathProxy` conversion into Rust field references;
- arbitrary retained cross-language closures in the first slice;
- automatic retry or transactional rollback after callable side effects;
- script-controlled native registration, provider installation, or slot
  selection;
- a second VM, language-direction-specific execution context, or separate
  budget model;
- provider-private state migration;
- distributed RPC, process service discovery, or a general dependency
  injection container.

## 19. Resolved Authoring And Open Delivery Decisions

The authoring spelling in R1 is resolved. Resolve each remaining open item
before the batch that depends on it. These delivery decisions are not reasons
to return to a service-first model.

### R1. Rust export grouping and effect spelling

Decision: use item-level `#[vela::export(path = "...")]` for scattered
functions, `#[vela::export_module(path = "...")]` for an explicit module of
exported public functions, and `#[vela::methods]` for an explicit inherent or
trait impl boundary. Module/impl grouping supplies default paths and one
registration bundle. Signature classification infers the base effect;
per-function
`#[vela::export(effects(...))]` adds exceptional effects or overrides metadata.
Avoid separate function/context/host macros or module-wide default effects that
create subtly different ABI models or silently overgrant every function.

### R2. Rust trait implementation export

Decision: Rust trait implementation does not imply Vela exposure. A Vela
protocol has its own stable public identity. A locally authored trait uses
`#[vela::trait_export]`, and an annotatable `impl Trait for Type` uses the same
`#[vela::methods]` boundary and generated adapter path as inherent methods. An
external trait maps explicitly to a Vela protocol identity. If the type, trait,
and existing impl cannot be annotated, a declaration-only external adapter
must list the selected method signatures and generate type-checked UFCS thunks;
it must not generate a duplicate Rust impl. Unsupported signatures require an
explicit boundary-safe mapping. No design may depend on runtime Rust trait
enumeration, automatic exposure of marker traits, or user-authored forwarding
methods for otherwise boundary-safe annotatable impls.

### O2. Rust binding generation command and artifact

Recommended: Engine/compiler emits a deterministic language-neutral binding
schema, and one official generator produces Rust code from it. A CLI and build
helper may wrap the same generator. Generated files record a schema checksum
and Vela source origins.

### O3. Generated Rust binding shape

Recommended: a runtime-bound package/module object with ordinary typed methods,
plus the same method surface borrowed from `NativeCallContext` for re-entry.
Avoid global state and avoid requiring a user-authored Rust trait solely to
call ordinary Vela functions.

### O4. Exported error spelling

Recommended: keep `VmResult<T>` as call failure, map declared boundary-safe
`Result<T, E>` as a Vela Result value, and generate matching Rust binding
types. Do not silently turn all errors into panics or strings.

### O5. Restricted native profile

Recommended: defer a special restricted profile until a real deployment needs
it. Initial control is callable visibility, the effect-derived coarse
capability profile, type/lease safety, and budgets. A later restricted export
may opt into `HostAccess` without changing the default ordinary-reference
surface. It must not add business permission strings to `CallableContract`.

### O6. Service delegate and partial override

Recommended: whole-service selection first. Defer partial override. If explicit
delegation to the displaced implementation is required, generate a
generation-pinned typed delegate; never infer fallback after an error.

### O7. Scoped host-handle escape diagnostics

Recommended: recursively reject obvious writes of scoped handles into escaping
containers/state at the write site and always retain deterministic scope-close
invalidation as the safety backstop.

## 20. Suggested First Vertical Slice Task

```text
Task: Implement the minimal ordinary Rust/Vela round-trip interop slice.

Context:
  Build on existing native descriptors, direct host leases, Runtime call
  targets, NativeCallContext re-entry, linked artifacts, and hot reload.
  Do not require service/provider setup for this slice.

Expected behavior:
  - One export module registers scalar normalize and grant_exp functions with
    one generated bundle and no repeated game:: path prefix.
  - normalize infers pure from ordinary value parameters.
  - grant_exp(player: &mut Player, amount: i64) infers host_write without an
    authored effect annotation.
  - One ordinary &self Player method infers host_read and one &mut self method
    infers host_write.
  - One context function explicitly adds random or event_emit beyond its
    signature-inferred base.
  - Vela calls all three with normal function/method syntax.
  - Engine emits a typed Rust binding for a public Vela level_up function.
  - Rust calls level_up through that binding without CallArgs or OwnedValue.
  - One exported Rust function re-enters a Vela helper while holding
    &mut Player, using an authorized child reborrow.
  - Passing one Player to two exclusive Rust parameters fails before Rust runs.
  - A nested binding whose effects exceed its Rust parent's ceiling fails
    before the child body runs.
  - The inferred host_write effective set derives the fixed host-write
    capability requirement, and a Runtime without that grant rejects the call
    before the Rust body runs.
  - The callable fingerprint and generated schema contain no active Runtime
    grants or arbitrary business permission strings.
  - Compatible Vela body reload keeps the Rust binding valid; incompatible ABI
    is rejected.

Tests:
  - export_module_registers_public_functions_once
  - private_export_module_helpers_remain_unregistered
  - unsupported_public_export_group_item_fails_at_declaration
  - rust_signature_infers_normalized_effects
  - explicit_effects_only_add_to_inferred_base
  - vela_calls_ordinary_rust_export
  - vela_calls_ordinary_rust_host_method
  - rust_typed_binding_calls_vela_export
  - round_trip_reentry_preserves_one_execution_session
  - nested_reborrow_restores_parent_reference
  - aliased_mutable_export_arguments_fail_before_invocation
  - coarse_capability_denial_precedes_rust_body
  - nested_binding_cannot_widen_parent_effects
  - deployment_grants_do_not_change_callable_fingerprint
  - generated_binding_re_resolves_compatible_reload
  - generated_binding_rejects_incompatible_reload_abi

Do not change:
  - Do not expose HostRef, HostPath, PathProxy, HostLeaseRef, HostLeaseMut,
    CallArgs, OwnedValue, or HostAccess in ordinary authored signatures.
  - Do not introduce a service trait or slot for ordinary function calls.
  - Do not add script generics, borrowed returns, or arbitrary Rust discovery.
  - Do not add another Runtime execution API or frame driver.
  - Do not implement field-level sandboxing inside trusted Rust code.
  - Do not scan arbitrary Rust bodies to guess effects.
  - Do not add module-wide default effects or ambient export inventory.
  - Do not add arbitrary permission strings or live policy grants to callable
    metadata, generated schemas, or ABI fingerprints.

Validation:
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  cargo test --workspace --all-features --no-fail-fast
```

This slice proves the general interop and safety model first. Optional service
slots and hot override belong to Batch F after ordinary round-trip calls work.

## 21. Final Completion Criteria

The goal is complete only when all of the following are true:

1. Supported Rust exports use ordinary copied/owned parameters and
   invocation-scoped `&T`/`&mut T` without authored boundary wrappers.
2. Vela calls exported Rust functions and methods with the same syntax used for
   Vela callables.
3. Rust calls exported Vela functions through generated typed bindings without
   runtime strings, `CallArgs`, or manual value conversion.
4. Atomic lease acquisition and exact identity prevent sibling alias
   violations before Rust references exist.
5. Provenance-preserving child reborrows support nested bidirectional calls and
   restore parent use without extending lifetimes.
6. Rust/Vela transitions share one Runtime call target, execution session,
   policy context, heap, state view, budgets, tracing, cancellation, and pinned
   artifact generation.
7. Callable contracts, generated bindings, reflection, tooling, and reload use
   deterministic stable identities and ABI fingerprints.
8. Rust signatures infer normalized `pure`/`host_read`/`host_write` base
   effects, explicit effects only add to that base, and nested context/binding
   operations cannot widen the current callable ceiling dynamically.
9. Explicit export modules and inherent or trait method groups register many
   supported public callables through deterministic bundles while private
   helpers and unselected Rust traits remain Rust-only; no ambient inventory or
   repeated per-function path/effect ceremony is required.
10. Callable effects deterministically derive coarse capability requirements;
    active grants, allowlists, reflection permissions, and arbitrary business
    permission strings remain outside callable ABI and generated fingerprints.
11. Trusted Rust mutation is clearly callable-grained: invocation capability
   and lease checks are enforced, while field-level sandboxing inside `&mut T`
   bodies is explicitly deferred.
12. Optional service contracts and hot override reuse the general callable
   model instead of defining a parallel boundary or execution path.
13. Local and external Rust trait implementations expose only their selected,
    boundary-safe Vela protocol surface and use the ordinary method call,
    lease, reflection, effect, and ABI paths.
14. Non-service round-trip and optional mixed-service examples, acceptance
    tests, documentation, benchmarks, formatting, lint, and workspace tests are
    complete and green.
