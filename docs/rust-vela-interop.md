# Rust/Vela Interop

> **Active direction — 2026-07-22:** the ordinary export/binding and
> HostRef/re-entry sections remain valid. The optional single-callable
> replacement section below is historical implementation documentation and is
> frozen pending deletion. The sole Rust hotfix model is the generated service
> generation in
> [rust-vela-service-hard-switch-plan.md](rust-vela-service-hard-switch-plan.md).

Ordinary bidirectional interop is the default integration model. Rust exports
ordinary functions and methods, Vela calls them with normal source syntax, and
Rust calls public Vela declarations through generated typed bindings. A
service trait, provider, replaceable slot, manual `CallArgs`, erased value, or
runtime target string is not required.

The runnable primary example is
`examples/src/bin/interop_round_trip`. Optional single-callable replacement is
demonstrated separately by `replaceable_handler` and
`replaceable_service_method`.

## Export Rust To Vela

Use `#[vela_macros::export]` for a scattered free function and
`#[vela_macros::methods]` for an explicit method group:

```rust,ignore
#[vela_macros::export(path = "game::normalize")]
pub fn normalize(amount: i64) -> i64 {
    amount.max(0)
}

#[vela_macros::methods(path = "game::Player")]
impl Player {
    pub fn grant(&mut self, amount: i64) -> i64 {
        self.level += amount;
        self.level
    }
}
```

Register the generated bundles and host schemas explicitly:

```rust,ignore
let engine = Engine::builder()
    .register_host_type::<Player>()
    .register_exports(vela_export_bundle_normalize())
    .register_exports(Player::vela_inherent_exports())
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .build()?;
```

Vela uses the same call syntax as it does for Vela targets:

```vela
let amount = game::normalize(input);
player.grant(amount);
```

Shared and exclusive Rust references infer `host_read` and `host_write`.
`effects(...)` adds exceptional effects but cannot remove signature-inferred
effects. A trusted Rust body receiving `&mut T` has ordinary field-level Rust
authority for that invocation; direct Vela path writes still use fine-grained
`HostAccess` checks.

## Generate Typed Rust Bindings

The compiler-owned `RustBindingSchema` is the only generator input. A build
script compiles the same Vela source and passes `program.binding_schema()` to
`vela_bindgen::generate_rust_bindings`, writing the result to `OUT_DIR`. The
application includes that generated file and binds it to an explicit Runtime:

```rust,ignore
include!(concat!(env!("OUT_DIR"), "/vela_bindings.rs"));

let mut package = vela_bindings::bind(&mut runtime)?;
let mut module = package.dev_vela_anonymous_root_module();
let result = module.apply(&mut player, 5)?;
```

Generated calls use stable callable identities and validate schema and
callable fingerprints. Compatible body reload re-resolves the stable target;
parameter, mode, return, effect, or async ABI changes fail before invocation.
Runtime grants and allowlists are deployment policy and do not change the
callable fingerprint.

For nested Rust-to-Vela calls, bind the same generated surface through
`vela_bindings::bind_active(&mut NativeCallContext)`. This pushes a child into
the current execution session and retains its artifact, heap, state, host
boundary, budgets, capabilities, tracing, cancellation, and lease provenance.

## Host References And Errors

Authored signatures use supported values plus ordinary `&T` and `&mut T`.
Generated code internally creates exact call-scoped HostRefs and acquires all
host leases atomically before any Rust reference exists. Conflicting shared and
exclusive aliases fail before the body. Borrowed host returns remain scoped to
the root call tree; compiler-proven last use closes them early, and dynamic
code can use `host::release(value)`.

Interop failures identify the callable and relevant parameter where possible.
Debug ABI failures by comparing the generated binding's recorded source origin
with the current Vela declaration. Alias, expired provenance, capability,
effect-ceiling, budget, cancellation, and reload failures are Runtime errors;
they do not trigger a second execution path or an automatic retry.

Low-level `Runtime::call`, `Runtime::call_async`, `CallArgs`, and runtime values
remain available for genuinely dynamic tools. Ordinary statically known calls
should use generated bindings.

## Historical: Optional Single-Callable Replacement

> **Status:** superseded and frozen pending deletion. Its Actor Runtime
> authority reconciliation is recorded in the
> [final report](archive/rust-vela-interop-actor-runtime-reconciliation-acceptance-2026-07-17.md).

Replacement is an explicit extension. A selected public entry keeps its normal
call shape while the macro moves its body to a private Rust fallback:

```rust,ignore
#[vela_macros::methods(path = "host::pricing::Service")]
impl Service {
    #[vela_macros::replaceable(
        path = "host::pricing::Service::quote",
        authority = "turn",
        index = 0
    )]
    pub fn quote(&self, turn: &mut ActorTurn, value: i64) -> VmResult<i64> {
        let _ = turn;
        Ok(self.adjacent(value))
    }
}
```

`ActorTurn` is framework-owned authority. It holds the pinned `DispatchRoot`
and the Actor's Runtime and implements `DispatchAuthority` by lending a scoped
`&mut SharedRuntime` to `DispatchInvocation`. The named authority parameter is
not part of the Vela callable ABI. A Handler/Service framework macro normally
generates that parameter and splits its actor turn internally, so business
authors and callers do not pass a Runtime, session, HostRef, lease, or dense
slot.

Vela implements only that callable:

```vela
#[override(host::pricing::Service::quote)]
fn patched(service: Service, value: i64) -> i64 {
    return service.adjacent(value) + 1;
}
```

The deployment API constructs a deterministic slot bundle, stages from a
borrowed Runtime, and publishes the candidate for future roots. A root pins
immutable generation selection only; the Actor continues to own its Runtime:

```rust,ignore
let controller = DispatchController::new(Service::vela_replaceable_slots())?;
let candidate = controller.stage_current(&override_runtime)?;
let previous = controller.activate(candidate)?;

let mut actor = Actor {
    runtime: SharedRuntime::from_shared_image(image.clone())?,
    dispatch: DispatchRoot::pin(&controller),
    // actor and business state...
};
let result = actor.handle_message(40)?;

controller.rollback(previous)?;
```

`DispatchRoot` contains no mutable Runtime owner. On an override hit, generated
code asks the current actor turn for a `DispatchInvocation<'turn>` that borrows
its already-exclusive `&mut SharedRuntime`. The scoped async future retains
that borrow across suspension. Nested replaceable calls use
`NativeCallContext` re-entry and therefore inherit the active session's
remaining budgets, artifact, heap, state, HostAccess, capabilities, effect
ceiling, tracing, cancellation, and generation without reacquiring authority.
A staged package remains a coherent partial delta, rollback republishes a prior
generation, and a Vela error propagates without retrying the displaced Rust
body.

The explicit `#[replaceable(...)]` spelling is the low-level mechanism a host
framework macro may emit. `#[methods]` generates `vela_replaceable_slots()`
for an inherent group and a trait-specific slot bundle for an exported trait
group. A Handler/Service host macro can therefore generate stable paths,
indices, authority wiring, registration, and trait forwarding once; business
bodies do not repeat those details or construct a proxy.

The no-override entry performs one dense indexed lookup and empty-entry branch
before the private Rust fallback. It does not perform a string/hash lookup,
global lock, allocation, serialization, or dynamic trait dispatch.

## Historical Callable-Replacement Deployment Checklist

The checklist below records the superseded implementation and must not be used
for new integrations. Current execution requirements live in the unified
service hard-switch plan.

1. Generate bindings from the exact package/source graph used for deployment.
2. Register export bundles, host types, capabilities, and policy explicitly.
3. Treat generated-schema incompatibility as an ABI deployment failure, not a
   grant mismatch.
4. Stage override deltas completely before activation; a failed stage changes
   nothing.
5. Pin one dispatch root per host operation and retain old generations until
   their roots finish.
6. Borrow the current Actor's Runtime directly for each invocation; do not put
   it in a dispatch root, override target, ambient lookup, or Runtime mutex.
7. Roll back by publishing a validated prior generation; never replay an
   in-flight call or rewind completed host effects.
