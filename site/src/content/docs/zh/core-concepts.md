---
title: "核心概念"
description: "Vela 核心概念文档。"
---

本页解释文档里反复出现的几个核心概念。Vela 的语言表面不大，但宿主边界是刻意设计的。

## Engine、Program、Runtime

`Engine` 保存注册信息和策略：host 类型、native 函数、标准库 native、capability profile、反射权限和编译选项。源码通过 Engine 编译成 program。

`Runtime` 保存某个 program version 的执行状态。调用通过函数名或缓存的 entry handle 进入 Runtime，参数放在 `CallArgs` 中，执行由 `CallOptions` 约束，例如指令预算、内存预算和调用深度。

```rust
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .register_script_host::<Player>()
    .build()?;
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);
```

## 脚本值和宿主状态

脚本拥有的值包括基础值、数组、map、set、字符串、record、enum、闭包和 VM 管理的 iterator。宿主持有的值仍然留在 Rust 里。脚本拿到宿主对象时，拿到的是受控 handle，不是 Rust 对象所有权。

宿主写入是立即写穿的。像 `player.inventory.items["gold"].count += amount` 这样的表达式会被降成 host path 操作，宿主 adapter 可以校验、拒绝或应用这次写入。

## 能力和预算

Capability 描述 Runtime 允许执行哪些副作用，例如读取宿主、写入宿主、调用宿主、随机数、时间、I/O 读或 I/O 写。Budget 限制脚本执行，防止无限循环或无边界内存增长。

这些策略由宿主在构建 Engine 或 runtime profile 时决定。同一份脚本在一个 profile 中可以成功，在另一个 profile 中可能因为尝试被拒绝的副作用而失败。

## 热更新边界

热更新会在兼容性检查通过后替换函数或模块边界上的代码。Runtime 会保留旧代码给已经在执行的调用帧，新调用则进入被接受的新版本。

Schema 和 ABI 兼容性很重要。如果变更会破坏当前 host binding、field ID、method ID、effect 或可调用签名，更新会带诊断被拒绝，而不是部分生效。
