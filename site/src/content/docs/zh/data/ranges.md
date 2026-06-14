---
title: "Range"
description: "Vela Range 值文档。"
---

Range 描述整数序列，是可重复 sequence 值。它们常用于循环；当编译器能证明 `i64` 边界时，可以使用专门的字节码。

## 语法

`start..end` 不包含结束值。`start..=end` 包含结束值。

```vela
fn sum_to(limit: i64) -> i64 {
    let total = 0
    for value in 0..=limit {
        total += value
    }
    return total
}
```

## 迭代

Range 是可重复的，每次 `for` 都会创建新的遍历。需要 index 和 value 时可以使用带索引的循环。

```vela
fn weighted(limit: i64) -> i64 {
    let total = 0
    for index, value in 1..limit {
        total += index * value
    }
    return total
}
```

## 方法

Range 支持 `len()`、`is_empty()` 等标准方法，前提是边界让这些操作有明确意义。

## 性能边界

VM 可以把已证明的整数 range loop 降低为 typed scalar bytecode。这个优化必须保持相同可观察行为，不改变语言模型。
