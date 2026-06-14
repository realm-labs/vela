---
title: "Serde Snapshot 值"
description: "使用 serde 在宿主边界传递复制的 script-owned snapshots。"
---

Serde integration 用于 snapshot values。它把 Rust 数据复制成脚本拥有的
records、arrays、maps、enums、strings、bytes 和 scalars。它不是 host object
reference model。

## 传入 Snapshots

```rust
#[derive(Serialize, Deserialize)]
struct DamageEvent {
    actor: DamageActor,
    amount: i64,
    multiplier: i64,
    reason: String,
}

let args = CallArgs::new().with_serde_value("event", &event)?;
let output = runtime.call("handle_damage", args, CallOptions::unbounded())?;
```

```vela
fn handle_damage(event: DamageEvent) {
    return DamageResult {
        actor_name: event.actor.name,
        applied: event.amount * event.multiplier + event.actor.level,
        label: event.reason,
    };
}
```

脚本里修改 `event` 不会修改原始 Rust struct，因为脚本拿到的是复制值。

## 返回 Snapshots

使用 `from_value` 反序列化结果。

```rust
let result: DamageResult = runtime.from_value(&output)?;
```

如果宿主想继续对返回值调用脚本方法，或者把它传回同一个 runtime，也可以
保留为 `VelaValue`。

```rust
let score_method = runtime.method(&output, "score")?;
let score = runtime.call_method(
    &output,
    &score_method,
    CallArgs::new().with_value("bonus", 5_i64),
    CallOptions::unbounded(),
)?;
```

## 什么时候用 HostRef

复制是预期行为时使用 serde：events、config snapshots、request payloads 和
return DTOs。脚本写入需要立即更新持久 Rust 状态时，使用 `HostRef` 和
`HostAccess`。
