# Rust/Vela Interop

> **Active direction — 2026-07-23:** ordinary export/binding and
> HostRef/re-entry remain the general Rust/Vela call model. The sole Rust
> hotfix model is the generated service generation in
> [rust-vela-service-hard-switch-plan.md](rust-vela-service-hard-switch-plan.md).

Ordinary bidirectional interop is the default integration model. Rust exports
ordinary functions and methods, Vela calls them with normal source syntax, and
Rust calls public Vela declarations through generated typed bindings. A
service trait, provider, manual `CallArgs`, erased value, or runtime target
string is not required.

The runnable primary example is `examples/src/bin/interop_round_trip`. The
generated Rust-default service baseline for the hard switch is
`examples/src/bin/service_hard_switch_fixture`; it pins one whole service
generation and keeps the default call chain on direct Rust trait dispatch.
The complete target service authoring and deployment surface is collected in
[Rust/Vela Service Patchability — Final Usage Shape](rust-vela-service-patchability-usage.md).

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
