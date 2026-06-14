---
title: "嵌入概览"
description: "Rust 宿主如何嵌入 Vela，同时把持久状态保留在安全的宿主边界之后。"
---

Vela 作为 Rust 拥有的脚本运行时嵌入应用。脚本描述业务逻辑，宿主
拥有持久状态、服务、IO、调度和部署策略。

## 嵌入形态

典型宿主会创建 `Engine`，把源码编译成 program，创建 `Runtime`，再用
显式参数调用脚本入口。

```rust
use vela_engine::prelude::*;

let engine = Engine::builder()
    .execution_profile(ExecutionProfile::embedded())
    .register_script_host::<Player>()
    .build()?;
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);

let result = runtime.call(
    "handle_tick",
    CallArgs::new().with_host_mut("player", &mut player),
    CallOptions::new(10_000, 1024 * 1024, 64),
)?;
```

`Engine` 是共享定义面。`Runtime` 是可变执行状态：heap、globals、
inline caches 和 hot-reload image。

## 宿主边界

脚本永远拿不到真实 Rust 引用。调用边界上的 `&mut Player` 在 VM 内会变成
调用作用域内的 `HostRef`。字段读写、复合赋值、keyed path 和 host method
调用都会经过 `HostAccess`。

```vela
fn handle_tick(player: Player, amount) {
    player.level += amount;
    return player.level;
}
```

语法是普通字段访问，但运行时操作是显式的：读取当前宿主字段，计算新的
scalar 值，然后立即写穿到 adapter。

## 宿主拥有的内容

宿主拥有持久状态、对象生命周期、capability profile、执行预算、native
服务和热更新策略。脚本拥有的 records、arrays、maps、sets、strings、
closures 和 iterators 位于 Vela heap。宿主对象保留在 `HostRef`、
`HostTargetPlan`、`PathProxy` 和 `HostAccess` 之后。
