---
title: "核心概念"
description: "Vela 核心概念文档。"
---

本页解释文档里反复出现的几个核心概念。这里先讲嵌入模型，因为 Vela
主要是运行在 Rust 宿主里的脚本语言，不是只面向命令行单独运行的语言。

最短的心智模型是：

```text
Engine  = 配置好的编译器和宿主注册表
Program = 编译后的 Vela 代码
Runtime = 执行 Program 的可变运行实例
```

## Engine、Program、Runtime

`Engine` 是一个相对长期存在的对象，它知道宿主开放了什么能力给脚本。它保存
host 类型注册、native 函数、标准库 native、capability profile、反射权限和
编译选项。如果脚本里写了 `Player.level`，这个 host schema 就是在 Engine
里注册进去的。

`Program` 是编译后的 Vela 代码。它由 Engine 编译单个源码文件、一组模块源码
或源码目录得到。Program 包含字节码、元数据、稳定 ID、cache site 和入口函
数，但它本身还没有在运行。

`Runtime` 是真正执行 Program 的地方。它保存某个 active program version 的
可变 VM 状态：脚本堆、global、inline cache、执行预算和热更新状态。调用通过
函数名或缓存的 entry handle 进入 Runtime，参数放在 `CallArgs` 中，执行由
`CallOptions` 约束，例如指令预算、内存预算和调用深度。

普通嵌入代码的流程是：

```text
构建 Engine -> 编译 Program -> 创建 Runtime -> 调用脚本入口
```

```rust
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .register_script_host::<Player>()
    .build()?;
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);
```

这三个边界不要混在一起：

- 要改变宿主注册信息或策略，先配置 `Engine`。
- 要改变脚本代码，编译新的 `Program`。
- 要改变正在运行的脚本状态，调用或热更新 `Runtime`。

## 脚本值和宿主状态

脚本拥有的值包括基础值、数组、map、set、字符串、record、enum、闭包和 VM 管理的 iterator。宿主持有的值仍然留在 Rust 里。脚本拿到宿主对象时，拿到的是受控 handle，不是 Rust 对象所有权。

宿主写入是立即写穿的。像 `player.inventory.items["gold"].count += amount` 这样的表达式会被降成 host path 操作，宿主 adapter 可以校验、拒绝或应用这次写入。

## 能力和预算

Capability 描述 Runtime 允许执行哪些副作用，例如读取宿主、写入宿主、调用宿主、随机数、时间、I/O 读或 I/O 写。Budget 限制脚本执行，防止无限循环或无边界内存增长。

这些策略由宿主在构建 Engine 或 runtime profile 时决定。同一份脚本在一个 profile 中可以成功，在另一个 profile 中可能因为尝试被拒绝的副作用而失败。

## 热更新边界

热更新会在兼容性检查通过后替换函数或模块边界上的代码。Runtime 会保留旧代码给已经在执行的调用帧，新调用则进入被接受的新版本。

Schema 和 ABI 兼容性很重要。如果变更会破坏当前 host binding、field ID、method ID、effect 或可调用签名，更新会带诊断被拒绝，而不是部分生效。
