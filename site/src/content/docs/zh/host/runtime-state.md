---
title: "Runtime State"
description: "VM 拥有的 state cell 与宿主拥有的 extern state binding。"
---

持久模块状态必须明确所有权：

```vela
state ticks: i64 = 0;
pub extern state server: ServerState;
```

`state` 为每个 Runtime 创建一个 VM 拥有的 cell。必需的 initializer 在
Runtime 构建时执行一次；热更新时只初始化新增 cell。Initializer 有预算限制，
可以构造脚本值，但不能读取 state、调用 host/native/reflection/provider API、
使用 capability 或挂起。所有 cell 都以事务方式暂存和发布。

Rust 通过全限定名称读取或替换 VM state：

```rust
runtime.set_state("main::ticks", 10_i64)?;
let ticks = runtime.state("main::ticks")?;
let typed: i64 = runtime.state_as("main::ticks")?.expect("ticks state");
```

`extern state` 不拥有脚本值。构建前绑定，之后使用 state 专用的替换或热更新
暂存 API：

```rust
let mut builder = Runtime::builder(engine, program)?;
builder.bind_extern_state("main::server", server)?;
let mut runtime = builder.build()?;
runtime.replace_extern_state("main::server", replacement)?;
runtime.stage_extern_state("main::added_server", added)?;
```

Extern 读取产生 host reference，嵌套修改经过 HostAccess，Vela 不能替换根。
VM cell 参与脚本 GC；Rust host object 不参与。精确兼容的热更新保留两种状态，
也不会重跑已有 initializer。
