---
title: "数组"
description: "Vela 数组文档。"
---

数组是有序、可索引、由 GC 管理的集合。数组本身是动态的：语言不支持 `Array<T>`，元素契约应由消费数组的 API 边界或显式脚本检查负责。

## 字面量和索引

数组字面量使用方括号。索引从 0 开始；越界访问会按具体操作产生错误或由方法返回 `Option`。

```vela
fn second_reward() -> i64 {
    let rewards = [10i64, 20i64, 30i64]
    return rewards[1]
}
```

## 修改

数组方法覆盖追加、移除和查询等常见操作。修改脚本数组会改变脚本堆上的值；修改宿主拥有的数组路径必须经过 HostAccess。

```vela
fn collect_large(values) {
    let out = []
    for value in values {
        if value > 10 {
            out.push(value)
        }
    }
    return out
}
```

## 迭代

数组是可重复 sequence。`iter()` 创建 one-shot iterator，`map` 或 `filter` 等惰性适配器会在终端方法运行时消费该 iterator。

```vela
fn increment(values) {
    return values.iter().map(|value| value + 1).collect_array()
}
```

## 边界

数组长度和元素访问都是受预算约束的操作。数组属于脚本堆，除非它是宿主转换返回的快照；Rust 宿主存储不会被放到脚本 GC 下。
