# Rust/Vela Interop

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

## Optional Single-Callable Replacement

> **Post-review status:** the current API is an experimental mechanism slice,
> not the completed production replacement contract. Single-level activation,
> partial deltas, rollback, error propagation, and the empty-slot fast path are
> demonstrated. Controller-owned generations, static override linking, and
> complete inherited contract validation are now implemented. Same-Runtime
> nested replacement, same-session budget/policy inheritance, ordinary
> business and borrowed-return execution mapping, and host-business-macro
> ergonomics remain open in the
> [unified plan](rust-vela-interop-model-plan.md#post-implementation-review-correction--2026-07-17).

Replacement is an explicit extension. A selected public entry keeps its normal
call shape while the macro moves its body to a private Rust fallback:

```rust,ignore
#[vela_macros::methods(path = "host::pricing::Service")]
impl Service {
    #[vela_macros::replaceable(
        path = "host::pricing::Service::quote",
        authority = "self",
        index = 0
    )]
    pub fn quote(&self, value: i64) -> VmResult<i64> {
        Ok(self.adjacent(value))
    }
}
```

Vela implements only that callable:

```vela
#[override(host::pricing::Service::quote)]
fn patched(service: Service, value: i64) -> i64 {
    return service.adjacent(value) + 1;
}
```

The host constructs a deterministic slot bundle, stages the override Runtime,
and publishes the candidate for future roots:

```rust,ignore
let controller = DispatchController::new(vec![
    Service::vela_replaceable_slot_quote(),
])?;
let candidate = controller.stage_current(&override_runtime)?;
let previous = controller.activate(candidate)?;

let service = Service {
    dispatch: DispatchRoot::pin(&controller, override_runtime.clone())?,
    // business fields...
};
let result = service.quote(40)?;

controller.rollback(previous)?;
```

Pin a `DispatchRoot` with an explicit `SharedRuntime` at the host operation
boundary, such as an actor mailbox turn. Active roots retain their immutable
target selection while activation changes future roots. Nested replaceable
calls re-enter the active session and inherit its remaining budgets, artifact,
heap, state, HostAccess, capabilities, cancellation, and generation. Separate
host roots may use distinct `SharedRuntime` instances over one immutable
`SharedImage`, so package code sharing does not impose one global Runtime lock.
A staged package is a coherent partial delta, rollback republishes a prior
generation, and a Vela error propagates without retrying the displaced Rust
body.

The no-override entry performs one dense indexed lookup and empty-entry branch
before the private Rust fallback. It does not perform a string/hash lookup,
global lock, allocation, serialization, or dynamic trait dispatch.

## Deployment Checklist

The ordinary generated interop checklist below is production-oriented. Treat
the optional replacement steps as evaluation-only until the post-review
closure receives a replacement acceptance report.

1. Generate bindings from the exact package/source graph used for deployment.
2. Register export bundles, host types, capabilities, and policy explicitly.
3. Treat generated-schema incompatibility as an ABI deployment failure, not a
   grant mismatch.
4. Stage override deltas completely before activation; a failed stage changes
   nothing.
5. Pin one dispatch root per host operation and retain old generations until
   their roots finish.
6. Roll back by publishing a validated prior generation; never replay an
   in-flight call or rewind completed host effects.
