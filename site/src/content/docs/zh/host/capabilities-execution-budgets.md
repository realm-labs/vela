---
title: "能力和执行预算"
description: "从嵌入层限制 host effects，并约束 Vela 调用能执行多少工作。"
---

Capabilities 决定程序可以使用哪些宿主 effects。Budgets 决定一次调用可以做
多少工作。

## Capabilities

```rust
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .build()?;
```

当前 capability gates 包括 host read/write、event emit、time、random、IO
read/write，以及 reflection read/write/call。Native functions 和 host methods
声明 effects；runtime 会把这些 effects 与 active capability profile 对比。

```rust
let sandboxed = Engine::builder()
    .execution_profile(ExecutionProfile::sandboxed())
    .build()?;
```

## 执行预算

`CallOptions` 控制 instruction count、memory bytes 和 call depth。

```rust
let options = CallOptions::new(
    10_000,       // instruction limit
    1024 * 1024,  // memory limit
    64,           // max call depth
);
runtime.call("main", CallArgs::new(), options)?;
```

`CallOptions::unbounded()` 适合可信 examples 和 tests，但生产宿主应该使用明确
限制。

## Denials

Permission denial 是普通 runtime diagnostic。例如，读取 `player.level` 可能被
缺失 `HostRead` 拒绝，写入 `player.level` 可能被缺失 `HostWrite` 拒绝，
`ctx.emit(...)` 可能被 host-call policy 拒绝。

```vela
fn main(player: Player) {
    player.level += 1; // 需要 host read 和 host write access
}
```
