---
title: "Host 对象生命周期"
description: "宿主状态的 object identity、generation 和 lifetime boundaries。"
---

Host objects 由 Rust 拥有。Vela 存储的是指向它们的 handles，不是对象本身。

## 调用作用域 Handle

`CallArgs::with_host_ref` 和 `CallArgs::with_host_mut` 会为一次调用绑定 Rust
值。VM 看到的是 `HostRef` handle 和 call-local adapter binding。

```rust
runtime.call(
    "main",
    CallArgs::new().with_host_mut("player", &mut player),
    CallOptions::unbounded(),
)?;
```

调用返回后，direct binding 就结束了。持久状态仍在 Rust 里，不在脚本 heap
里。

## 持久 Globals

Runtime globals 可以保存 Rust 插入的 persistent host objects。这些对象必须
是 `Send`，因为 runtime 可以被移动到 worker 线程。

```rust
let player_ref = runtime.insert_host_global("main::player", player);
```

Script-value globals 不同：它们是由 runtime root 的 VM-managed records、
arrays、maps、sets、enums 和 scalars。

## Stale References

`HostRef` 包含 generation。如果对象被移除或替换后 slot 被复用，adapter 可以
拒绝 stale handle，而不是静默写入错误对象。拒绝是 runtime diagnostic，不是
best-effort fallback。
