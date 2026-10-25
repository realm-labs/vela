# Native Registration And The Rust Boundary

The supported post-D embedding boundary is synchronous **owned values only**.
No independent opaque-object runtime survived D. Replacement objects, new
conversion APIs and a new re-entry model are separately deferred, not required
for removal acceptance. [Rust/Vela interop](../rust-vela-interop.md) describes
retained authoring; [architecture.md](../architecture.md) owns the boundary.

## Host State Bridge

Rust application state stays in Rust. A native implementation may access it
internally during an invocation, but no caller borrow, wrapper around a borrow,
borrowed return, view, iterator lease or transparent object path enters script.
Inputs are explicitly converted to script-owned values; Rust explicitly receives
or decodes results. Mutating those values does not implicitly write back.

`vela_host` retains owned structural boundary values and errors. Its crate name
is not evidence of a live host-object runtime. There are no host roots, field
adapters, path plans, host-method receivers or extern-state binding APIs.
Ordinary script state, records, collections and script methods remain supported.

## Host Function Registration

Native functions are registered synchronous module callables with one canonical
name and stable ID; there is no function overloading. Descriptors carry parameter
names/hints/default metadata where supported, return contracts, effects, access,
attributes, docs and declaration origins. Registry metadata supports compiler
resolution, ABI checks, reflection and tooling; executable Rust closures live in
the native dispatch table rather than in serialized metadata.

The retained registration paths include explicit `register_native_fn`, typed
native registration, context-aware native registration and generated owned-value
exports. Use the existing conversions rather than adding another binding model.

```rust
#[vela_macros::export(path = "app::normalize")]
pub fn normalize(value: i64) -> i64 {
    value.max(0)
}

let engine = Engine::builder()
    .install_registration(vela_function_normalize())
    .build()?;
```

Vela calls `app::normalize(value)` as an ordinary module function. The
[native example](../../examples/src/bin/native_function/main.rs) exercises
scalar and owned collection calls. Value type/schema registration is metadata
and conversion, not an object constructor, receiver registry or Rust field bridge.

## Effects And Permissions

**Host read and host write remain valid native effects and capabilities.** An
owned-value native may still read or mutate application state internally. The
removal of transparent field access does not make such calls pure or remove
their authorization requirements.

- `EffectSet` records domain-neutral host-read, host-write, I/O, random,
  time, event and reflection effects. Required capabilities use the canonical
  effect-to-capability mapping.
- Engine grants and call options constrain execution. Dynamic/native dispatch
  must enforce the same checks as statically resolved calls; declared effects
  cannot be bypassed through reflection or synchronous nested calls.
- `FunctionAccess` controls public/reflection visibility and callability, not
  deployment grants or arbitrary business ACL strings.
- Reflection field policy is distinct from ordinary callable authorization.
  Retain both; do not copy field permission names into native ABI fingerprints.
- Permission denial, conversion failure and budget traps remain structured,
  source-spanned diagnostics. Native implementations are trusted Rust code;
  declarations and budgets do not preempt arbitrary blocking work inside Rust.

## Direct Call Arguments

`CallArgs` admits owned conversions and values retained by the same Runtime.
Serde inputs are serialized snapshots even when Rust passes a reference to the
serializer. Returned `VelaValue` roots remain runtime-affine and may be converted
explicitly with `value_to_owned` or `from_value`.

Existing context-aware synchronous native re-entry remains on the active call
session with its exact owner, budgets and capabilities. It does not admit
caller-borrowed data and is not approval for a replacement re-entry architecture.

## Removed Registration Families

Service/Provider declarations and entrypoints, async registration, Host derives
and receiver methods, borrowed parameters/returns/views, host factories,
projection/path adapters, leases/release and dependent `extern state` bindings
are removed. Do not preserve them via aliases, reflection, hidden signatures or
opaque wrappers. Old executable/binding/tooling formats reject; current versions
are portable **8**, binding **7**, tooling **4**.
