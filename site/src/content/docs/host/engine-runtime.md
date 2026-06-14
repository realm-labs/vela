---
title: "Engine And Runtime"
description: "The split between Vela's immutable engine definitions and mutable runtime execution state."
---

`Engine` and `Runtime` are intentionally separate. Build definitions once,
then create one or more runtimes that execute calls against those definitions.

## Engine

`Engine` stores registered host types, native functions, host methods,
reflection policy, capability defaults, standard natives, and hot-reload
policy.

```rust
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .register_script_host::<Player>()
    .register_typed_native_fn::<(i64, i64), _>(
        NativeFunctionDesc::new("game::bonus", NativeFunctionId::new(10_001))
            .param("amount", TypeHint::i64())
            .param("multiplier", TypeHint::i64())
            .returns(TypeHint::i64())
            .effects(EffectSet::pure()),
        |amount, multiplier| amount * multiplier,
    )
    .build()?;
```

The builder API is explicit so schemas, effects, and reflection metadata are
available before scripts run.

## Runtime

`Runtime` owns execution state for one program image. It is mutable because a
call can allocate script values, update globals, populate inline caches, apply
hot reload, and mutate host state through call arguments or adapters.

```rust
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);
let entry = runtime.entry("handle_tick")?;
let value = runtime.call(&entry, CallArgs::new(), CallOptions::unbounded())?;
let owned = runtime.value_to_owned(&value)?;
```

`VelaFunction` and `VelaMethod` handles cache resolved entries for high
frequency calls. They are handles into the same runtime image, not independent
compiled functions.

## Threading And Ownership

Vela script execution is single-threaded from the script author's point of
view. A `Runtime` may be moved to a worker or actor thread, but it is not
called concurrently. Hosts that need parallelism should use multiple runtimes,
runtime pools, or actor ownership at the Rust layer.
