---
title: "数组方法"
description: "Vela 数组方法文档。"
---

数组是有序的脚本自有集合。标准数组方法覆盖查询、修改、eager 转换和
iterator 创建。集合增长仍然受 VM 执行预算和集合预算检查。

## 查询和修改

基础查询使用 `len`、`is_empty`、`first`、`last`、`contains` 和
`index_of`。`contains` 和 `index_of` 使用与 `==` 相同的语义相等边界：
内建叶子值按值比较，没有 `PartialEq` 语义的对象会被拒绝。可能找不到值的
方法返回 `Option`。

```vela
fn main() {
    let rewards = ["gold", "xp"];
    let first = rewards.first().unwrap_or("none");
    let index = rewards.index_of("xp").unwrap_or(-1);
    return first.len() + index;
}
```

修改方法会原地更新数组，并根据操作返回 `null`、`bool` 或 `Option`。

```vela
fn main() {
    let queue = ["spawn"];
    queue.push("reward");
    queue.insert(1, "combat");
    let removed = queue.remove_at(0).unwrap_or("");
    return removed + ":" + queue.join(",");
}
```

## 转换

`slice`、`reverse`、`distinct`、`sort`、`min`、`max`、`sum`、
`group_by` 和 `sort_by` 会立即 materialize 结果。
`distinct` 同样使用语义相等，不做深层结构比较。`sort` 和 `sort_by` 要求
total-order key；float 会被拒绝，直到 Vela 增加显式 total-float ordering
API。
`group_by` 返回 value-keyed `Map<K, Array<T>>`，callback key 遵循普通
Map key 相同的 `ValueKey` 策略。

```vela
fn main() {
    let scores = [5, 1, 3, 5].distinct().sort();
    let best = scores.max().unwrap_or(0);
    return best + scores.sum();
}
```

带 callback 的 helper 会通过 VM 调脚本函数，因此热路径里的 callback 应保持
短小。

```vela
fn main() {
    return [1, 2, 3, 4]
        .filter(|value| value % 2 == 0)
        .map(|value| value * value)
        .sum();
}
```

## Iterator View

`iter` 和 `values` 产生数组值 iterator。Iterator 方法包括 `map`、
`filter`、`take`、`skip`、`find`、`any`、`all`、`count`、
`collect_array` 和 `collect_set`。

```vela
fn main() {
    let names = ["wolf", "boar", "wyrm"];
    return names.iter()
        .filter(|name| name.starts_with("w"))
        .take(1)
        .collect_array()
        .join(",");
}
```
