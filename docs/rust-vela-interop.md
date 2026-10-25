# Rust/Vela Interop

The supported baseline after D (`226900e81`) is synchronous calls with owned
values. **No independent opaque-object runtime survived D.** Replacement
objects, new conversion APIs and a new re-entry model are separately deferred;
this guide does not design or promise them. A–E removal implementation and
software gates are complete; see [current progress](progress.md) and the
[acceptance report](archive/runtime-simplification-acceptance.md). Strict/native
Input/Render coverage remains incomplete.

## Export Rust To Vela

Use the existing owned-value export/registration paths. For example:

```rust
#[vela_macros::export(path = "app::normalize")]
pub fn normalize(value: i64) -> i64 {
    value.max(0)
}

let engine = Engine::builder()
    .install_registration(vela_function_normalize())
    .build()?;
```

Scripts call `app::normalize(value)` normally. Explicit native descriptors and
typed/context-aware synchronous native registration remain supported. Native
metadata includes stable identity, parameter/return contracts, effects, access,
docs and source origins. See [native registration](architecture/host-and-registration.md)
and the [native example](../examples/src/bin/native_function/main.rs).

Owned Rust values and existing value derives/conversions are distinct from
host-object registration. No Host derive, Rust receiver method, borrowed
parameter/result, factory arena or transparent Rust-field projection is part of
this boundary. Rust may borrow its own state internally for one native invocation,
but the borrow cannot become a script value or retained wrapper.

## Call Vela From Rust

Compile ordinary source/modules/packages, construct a Runtime and call a public
script function synchronously using `Runtime::call` and `CallArgs`. Existing
generated Rust bindings use the compiler-owned `RustBindingSchema`, stable
callable identities and contract fingerprints for supported value signatures.
Generated bindings are not a Service or Provider entrypoint.

Runtime-managed `VelaValue` results stay rooted in their creating Runtime. They
can be reused by later calls on that Runtime, explicitly detached with
`value_to_owned`, or deserialized with `from_value` when the `serde` feature is
enabled. Runtime affinity, script type/identity and lifetime checks remain.

With serde, `CallArgs::with_serde_value` serializes an input into script-owned
data, even though serialization itself reads a Rust reference. Script edits do
not implicitly mutate the source Rust value. Rust must explicitly receive and
decode a result. See [serde_value](../examples/src/bin/serde_value/main.rs).

## Effects, Budgets And Nested Calls

Owned-value native APIs can still read/write application state internally.
**Native host-read/write effects and permissions remain**, along with I/O,
event, reflection, random/time and other retained capability checks. Pure value signatures
do not justify falsely declaring an effectful Rust body pure. Reflection calls
must satisfy both reflection policy and native capability checks.

Existing synchronous `NativeCallContext` re-entry remains on the active session,
sharing its exact executable owner, heap, budgets, capabilities, cancellation
and tracing. This is retained behavior, not approval for a new re-entry model.
Native implementations are trusted Rust and must bound their own work.

## Script State And Reload

Ordinary VM-owned `state` and explicit Rust reads/updates remain. There are no
extern-state host bindings or implicit Rust write-through. Compatible script
state survives reload; new entries use new code, while active frames and retained
closures keep their exact old code, nested dispatch, state, caches and plans.
Stable names never rebind old closures. See [hot reload](architecture/hot-reload.md).

## Formats And Removed Surfaces

Current versions are portable program **8**, Rust-binding schema **7**, tooling
schema **4**. Obsolete versions reject before payload use/linking/activation;
there is no legacy execution wrapper.

Service/Provider execution, script async/await and tasks, caller-borrowed roots,
arguments/returns/views, transparent host paths, leases/release, dependent host
registrations and JIT are removed. Historical Service/interop/projection/async
plans are explicitly superseded records, not future work to resume. The
[runtime simplification plan](runtime-simplification-plan.md) owns removal scope;
[validation.md](validation.md) owns commands. Missing objects/conversions/re-entry
design require a separately authorized follow-up.
