---
title: "类型提示和 Guard"
description: "Vela 类型提示和 runtime guard 文档。"
---

类型提示描述运行时契约和元数据。它们服务于诊断、反射、宿主 schema、热更新兼容性和部分快速路径，但不是静态泛型，也不会让脚本代码单态化。

## 出现位置

类型提示可以出现在参数、返回值、局部变量、全局值、struct 字段、enum 字段和 lambda 参数上。没有提示表示动态值。`any` 是显式擦除的动态元数据，本身不产生契约。

```vela
struct Reward {
    code: string
    amount: i64 = 0
}

fn grant(player, reward: Reward) -> i64 {
    player.gold += reward.amount
    return player.gold
}
```

## Guard

编译器能证明契约时，调用或写入可以走 unchecked 路径。动态值流入带提示的边界时，Vela 会插入运行时 guard。契约 guard 失败是语言错误，不是 inline cache miss。

```vela
fn double(value: i64) -> i64 {
    return value * 2
}

fn call_dynamic(value) -> i64 {
    return double(value)
}
```

## 不是泛型

Vela 明确拒绝 `Array<T>`、`Map<K, V>`、`Option<T>`、`Result<T, E>` 这类脚本泛型语法。容器是动态值；元素约束应放在 API 边界或显式检查中。

## 热更新和宿主元数据

类型提示是公开脚本和宿主契约的一部分。修改函数签名、字段提示、宿主 schema 或导出返回值提示，都可能影响热更新 ABI 兼容性，并在调用方或宿主注册未同步时被拒绝。
