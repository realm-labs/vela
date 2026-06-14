---
title: "Map 方法"
description: "Vela Map 方法文档。"
---

Map 是脚本自有的字符串 key 集合。标准 Map helper 强调显式查询和显式遍历；
key 缺失不会自动 trap，除非脚本使用直接索引。

## 查询和更新

key 可能不存在时使用 `has`、`get` 和 `get_or`。`get` 返回 `Option`，
`get_or` 返回存储值或 fallback 参数。

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10};
    let gold = rewards.get("gold").unwrap_or(0);
    let gems = rewards.get_or("gems", 0);
    return gold + gems;
}
```

`set`、`remove`、`clear`、`extend` 和 `merge` 用于修改或组合 Map。
`remove` 返回被移除值的 `Option`。

```vela
fn main() {
    let rewards = {"gold": 3};
    rewards.set("xp", 10);
    let removed = rewards.remove("gold").unwrap_or(0);
    rewards.extend({"gems": 1});
    return removed + rewards.len();
}
```

## View 和 Entry

`keys`、`values` 和 `entries` 返回 iterator。Entry 值是带 `key` 和
`value` 字段的 `MapEntry` record。

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10};
    let labels = rewards.keys().collect_array().sort().join(",");
    let total = rewards.values().collect_array().sum();
    return labels == "gold,xp" && total == 13;
}
```

需要同时访问 key 和 value 时使用 `entries`。

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10};
    let entry = rewards.entries()
        .find(|entry| entry.value >= 10)
        .unwrap_or(MapEntry { key: "", value: 0 });
    return entry.key;
}
```

## Callback Helper

`map_values`、`filter`、`find`、`any`、`all` 和 `count` 接收 callback。
多数 helper 在语义允许时支持 value-only callback 或 key/value callback。

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10, "quest": 1};
    let doubled = rewards.map_values(|value| value * 2);
    let big = rewards.filter(|key, value| key.len() <= 4 && value >= 3);
    return doubled["xp"] + big.len();
}
```
