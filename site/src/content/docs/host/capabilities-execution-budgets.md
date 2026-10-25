---
title: "Capabilities And Execution Budgets"
description: "Restricting host effects and bounding Vela execution from the embedding layer."
---

Capabilities decide which host effects a program may use. Budgets decide how
much work one call may perform.

## Capabilities

The existing value-only native API can declare host effects without exposing a
Rust receiver or borrowed value. Here the closures own their Rust state, and
only `i64` and unit values cross the script boundary:

```rust
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

let counter = Arc::new(AtomicI64::new(0));
let read_counter = Arc::clone(&counter);
let write_counter = Arc::clone(&counter);
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .register_typed_native_fn::<(), _>(
        NativeFunctionDesc::new("app::read_counter", NativeFunctionId::new(10_001))
            .returns(TypeHint::i64())
            .effects(EffectSet::host_read()),
        move || read_counter.load(Ordering::Relaxed),
    )
    .register_typed_native_fn::<(i64,), _>(
        NativeFunctionDesc::new("app::write_counter", NativeFunctionId::new(10_002))
            .param("value", TypeHint::i64())
            .returns(TypeHint::unit())
            .effects(EffectSet::host_write()),
        move |value| write_counter.store(value, Ordering::Relaxed),
    )
    .build()?;
```

Available capability gates include host read/write, event emit, time, random,
IO read/write, and reflection read/write/call. Registered synchronous native
functions declare effects; the runtime checks those effects against the active
capability profile. Host read/write permissions do not restore transparent
host paths, constructors, or methods.

```rust
let sandboxed = Engine::builder()
    .execution_profile(ExecutionProfile::sandboxed())
    .build()?;
```

## Execution Budgets

`CallOptions` controls instruction count, memory bytes, and call depth.

```rust
let options = CallOptions::new(
    10_000,       // instruction limit
    1024 * 1024,  // memory limit
    64,           // max call depth
);
runtime.call("main", CallArgs::new(), options)?;
```

`CallOptions::unbounded()` is useful for trusted examples and tests, but
production hosts should use explicit limits.

## Denials

Permission checks apply to the declared effects of registered native calls.
Without `HostRead`, `app::read_counter()` is denied; without `HostWrite`,
`app::write_counter(value)` is denied. Owned-value arguments and results do not
make these native operations pure. A script must call them explicitly:

```vela
fn main() {
    let current = app::read_counter();
    app::write_counter(current + 1);
    return current;
}
```

## Script-Owned Fields

Script-owned fields do not require Rust host grants: `HostRead` or `HostWrite`.
Reading, assigning, or applying `+=` to these fields operates on script data,
including values produced by existing Rust value conversions. Type contracts
and execution budgets still apply; the mutation does not implicitly write back
to Rust.

```vela
struct Counter { value: i64 }

fn increment(counter: Counter) -> i64 {
    counter.value += 1;
    return counter.value;
}
```
