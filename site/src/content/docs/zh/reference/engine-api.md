---
title: "Engine API 参考"
description: "Rust 嵌入 API 的高层参考。"
---

Rust API 是 Vela 的主要嵌入表面。本页是稳定概要，不是自动生成的 API 参
考。项目仍处于 pre-release 阶段，精确签名请以 crate docs 和源码为准。

## Engine Builder

`Engine::builder()` 配置 host types、native functions、standard natives、
capabilities、reflection policy、compiler options 和 hot reload policy。

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .capability(Capability::Time)
    .build()?;
```

## 编译和 Runtime

Engine 可以编译文件、源码字符串、module、program image 和 hot reload
version。`Runtime` 拥有执行状态，并用 `CallArgs` 和 `CallOptions` 调用脚本
entry。

```rust
let program = engine.compile_file(path)?;
let mut runtime = Runtime::new(engine, program);
let value = runtime.call("main", CallArgs::new(), CallOptions::new(10_000, 1024 * 1024, 64))?;
```

## 宿主边界

宿主状态通过 schemas、host refs、native functions 和 adapters 注册。脚本
永远不会拿到 Rust `&mut T`；修改通过 `HostRef`、`HostPath`、`PathProxy`、
`HostAccess` 和 `ScriptStateAdapter` 表示。

## Values 和 Handles

嵌入代码可以使用 owned values 表示 detached snapshots，也可以使用 runtime
managed value handles 在同一个 runtime 内复用值。生产 runtime 应显式配置
host capabilities 和 execution budgets。
