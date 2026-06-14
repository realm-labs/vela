---
title: "Context"
description: "Vela Context 标准库文档。"
---

`Context` 是标准宿主对象 schema，不是普通脚本自有状态。脚本需要确定性时间
字段、事件发送或日志时，由宿主注册它。脚本拿到的是 call-scoped `HostRef`，
不会持有真实 Rust mutable reference。

## 字段

标准 context schema 暴露 `now: i64` 和 `tick: i64`。这些值来自宿主，并遵循
确定性 time 模型。

```vela
fn main(ctx: Context) {
    let stamp = ctx.now + ctx.tick;
    return stamp;
}
```

字段读取仍然经过 `HostAccess`，因此宿主读权限和 stale host reference 检查
都会生效。

## 事件和日志

`ctx.emit(event, payload?)` 为宿主 safe point 记录 event emission patch。
`ctx.log(level, message, payload?)` 记录 log patch。两者都是带 event effect
的宿主方法。

```vela
fn main(ctx: Context, player: Player) {
    let stamp = ctx.now + ctx.tick;
    ctx.emit("player.level_checked", stamp);
    ctx.log("info", "player.level_checked", stamp);
    return stamp;
}
```

`payload` 参数是可选的，可以是宿主 adapter 接受的任意脚本值。

## Capability 边界

宿主必须注册 context schema，并授予相应的 host read、host call 和 event
capability。被拒绝的 context 调用是 runtime diagnostic，不是 `Result::Err`，
因为 host boundary 没有接受该事件。

Context 保持领域中立。游戏里的事件名、payload shape 和 host type 都由
embedding 应用提供。
