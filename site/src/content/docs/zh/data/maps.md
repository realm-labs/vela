---
title: "Map"
description: "Vela Map文档。"
---

Map 是动态脚本数据的键值集合，适合配置、查找表和快照值。需要安全修改 Rust 拥有的状态时，应使用注册宿主 schema，而不是把 map 当成宿主模型替代品。

`Map<K, V>` 是内建参数化 Map 契约。Map key 使用 Vela 的 `ValueKey` 策略：
不可变叶子值按值比较，脚本堆对象和 host ref 按身份比较，`PathProxy` 等临时
值会在修改前被拒绝。现有 Map 字面量仍然适合字符串 key，其他 runtime key
值可以通过索引或 Map 方法插入。

## 字面量和访问

Map 字面量使用 `{ key: value }`。key 可以是标识符、字符串、字符、数字或路径。索引用于按 key 读写 entry。

```vela
fn reward_table() {
    return {
        xp: 10i64,
        "gold": 5i64,
    }
}
```

## 更新

常见 map 方法包括插入、移除、包含检查、`get` 和 `get_or`。预期缺失的 lookup API 应优先返回 `Option`，而不是用 `null` 表示。

```vela
fn add_reward(rewards, code: String, amount: i64) {
    let current = rewards.get_or(code, 0)
    rewards[code] = current + amount
    return rewards
}
```

## 视图

`keys()`、`values()` 和 `entries()` 暴露可重复视图。`entries()` 产生带 `key` 和 `value` 字段的值，使 map 遍历更明确。

```vela
fn total(rewards) -> i64 {
    let sum = 0
    for entry in rewards.entries() {
        sum += entry.value
    }
    return sum
}
```

## 宿主边界

对宿主路径使用索引可能表示 HostAccess 操作，而不是脚本 map 修改。能力、只读字段、generation 和 schema epoch 仍由宿主 adapter 检查。
