---
title: "Engine 和 Runtime"
description: "Vela 中不可变 Engine 定义面和可变 Runtime 执行状态的分工。"
---

`Engine` 和 `Runtime` 故意分离。定义先构建好，然后可以创建一个或多个
runtime 来执行这些定义。

## Engine

`Engine` 保存已注册的 host types、native functions、host methods、
reflection policy、capability 默认值、standard natives 和 hot-reload policy。

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

Builder API 保持显式，这样 schemas、effects 和 reflection metadata 都会
在脚本执行前准备好。

## Runtime

`Runtime` 拥有一个 program image 的执行状态。它是可变的，因为一次调用
可能分配脚本值、更新 globals、填充 inline caches、应用热更新，并通过
调用参数或 adapter 修改宿主状态。

```rust
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);
let entry = runtime.entry("handle_tick")?;
let value = runtime.call(&entry, CallArgs::new(), CallOptions::unbounded())?;
let owned = runtime.value_to_owned(&value)?;
```

`VelaFunction` 和 `VelaMethod` 可以缓存解析后的入口，适合高频调用。它们
是同一个 runtime image 里的 handle，不是独立编译函数。

## 线程和所有权

从脚本作者角度看，Vela 执行是单线程的。`Runtime` 可以移动到 worker 或
actor 线程，但不能并发调用。同一应用需要并行时，应在 Rust 层使用多个
runtime、runtime pool 或 actor ownership。
