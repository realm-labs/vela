---
title: "类型提示和运行时检查"
description: "Vela 如何检查带类型提示的值。"
---

类型提示告诉 Vela 某个边界期望什么样的值。它们可以让错误更清楚，
也可以描述宿主 schema，并帮助热更新判断一次变更是否兼容。类型提示不是静
态泛型，也不会把一个值自动转换成另一种类型。

## 出现位置

类型提示可以出现在参数、返回值、局部变量、全局值、struct 字段、enum 字段
和 lambda 参数上。没有提示表示动态值。`Any` 表示这个值有意保持动态。

```vela
struct Reward {
    code: String
    amount: i64 = 0
}

fn grant(player, reward: Reward) -> i64 {
    player.gold += reward.amount
    return player.gold
}
```

## 运行时检查

当一个值进入带类型提示的边界时，Vela 会检查它是否符合提示。如果值的类型
不符合预期，这次操作会失败，并给出带源码位置的诊断。

```vela
fn double(value: i64) -> i64 {
    return value * 2
}

fn call_dynamic(value) -> i64 {
    return double(value) // 如果 value 不是 i64，这里会失败
}
```

## 内建容器契约

部分内建契约可以带类型参数：

```vela
fn total(values: Array<i64>) -> i64 {
    let sum = 0
    for value in values {
        sum += value
    }
    return sum
}

fn grant(rewards: Map<String, i64>, tags: Set<String>) -> Result<i64, String> {
    rewards.set("tag_count", tags.len())
    return result::ok(rewards.get("xp").unwrap_or(0))
}
```

允许的参数化契约是 `Array<T>`、`Map<String, V>`、`Iterator<T>`、
`Option<T>` 和 `Result<T, E>`。`Set<T>` 也可用，但 `T` 目前必须是
set 可 key 化的值：`null`、`bool`、`i64`、`f64` 或 `String`。

这些只是边界契约，不是转换。混合元素数组传给 `Array<i64>` 时会在检查边界失败，
不会被转换。

## 不是脚本泛型

Vela 仍然拒绝 `Player<T>`、`String<T>`、`Map<i64, V>`、`Function<T>`
这类用户泛型或非内建参数化语法。类型参数只保留给上面的内建契约，不会生成泛型函数
或泛型用户类型。

## 热更新和宿主元数据

类型提示是公开脚本和宿主预期的一部分。修改函数签名、字段提示、宿主 schema
或导出返回值提示，都可能影响热更新兼容性，并在调用方或宿主注册未同步时被
拒绝。
