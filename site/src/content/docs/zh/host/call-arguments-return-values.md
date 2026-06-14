---
title: "调用参数和返回值"
description: "Rust 如何向 Vela 调用传入值和 host handles，并读取返回结果。"
---

调用使用 `CallArgs` 和 `CallOptions`。参数可以是 positional、named、
脚本拥有的值、runtime-managed `VelaValue`、serde snapshot，或调用作用域的
host handle。

## 传入参数

普通复制值使用 `with_value`，直接宿主对象绑定使用 `with_host_mut` 或
`with_host_ref`。

```rust
let output = runtime.call(
    "main",
    CallArgs::new()
        .with_value("amount", 5_i64)
        .with_host_mut("player", &mut player),
    CallOptions::new(10_000, 1024 * 1024, 64),
)?;
```

```vela
fn main(player: Player, amount) {
    player.level += amount;
    return player.level;
}
```

`with_host_ref` 是只读 handle。`with_host_mut` 允许写入，但脚本在 VM 内拿到
的仍然只是 `HostRef`，不是 Rust `&mut T`。

## 返回值

`Runtime::call` 返回 `VelaValue`，也就是 runtime-managed handle。宿主需要
脱离 runtime 的 Rust 值时再转换。

```rust
let value = runtime.call("score", CallArgs::new(), CallOptions::unbounded())?;
let score: i64 = runtime.from_value(&value)?;
let owned = runtime.value_to_owned(&value)?;
```

把 `VelaValue` 重新传回同一个 runtime，可以避免 materialize 成
`OwnedValue`。

```rust
let snapshot = runtime.call("snapshot_state", CallArgs::new(), CallOptions::unbounded())?;
let projected = runtime.call(
    "projected_score",
    CallArgs::new().with_vela_value(snapshot).with(4_i64),
    CallOptions::unbounded(),
)?;
```

## 错误边界

使用 named arguments 时，参数名必须和入口签名匹配。Runtime-managed value
属于创建它的 runtime。Host object 必须先注册 schema，脚本代码才能 type-check
并访问它。
