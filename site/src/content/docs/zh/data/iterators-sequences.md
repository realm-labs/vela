---
title: "Iterator 和 Sequence"
description: "Vela Iterator 和 Sequence文档。"
---

Vela 在数组、map、set、range、string 和宿主返回的 iterable 上使用同一套迭代模型。这个模型区分可重复来源和 one-shot cursor。

## Iterable、Sequence、Iterator

Iterable 可以创建或提供 iterator。Sequence 是可重复 iterable，每次遍历都会创建新的 iterator。Iterator 是 one-shot cursor；`next()` 会推进同一个状态，后续调用能观察到推进后的结果。

```vela
fn first_two(values) {
    let iter = values.iter()
    let first = iter.next()
    let second = iter.next()
    return [first, second]
}
```

## For-In

`for value in source` 会先求值 `source`，取得 iterator，然后推进直到结束。`for index, value in source` 是语法级 indexed loop lowering。

```vela
fn total(values) -> i64 {
    let sum = 0
    for index, value in values {
        sum += value + index
    }
    return sum
}
```

## 惰性适配器

`map`、`filter`、`take`、`skip` 等方法是 lazy 且 one-shot 的。`count`、`any`、`all`、`find`、`collect_array` 等终端方法会消费 cursor。

```vela
fn active_codes(items) {
    return items.iter()
        .filter(|item| item.active)
        .map(|item| item.code)
        .collect_array()
}
```

## 宿主 Iterable

宿主可以返回 snapshot iterable，但宿主拥有的状态不会被放到脚本 GC 下。后续宿主修改仍然需要 HostAccess 或显式 native function 边界。
