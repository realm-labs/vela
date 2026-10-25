---
title: "闭包和 Lambda"
description: "Vela 闭包和 Lambda 文档。"
---

Lambda 创建可以传给标准库 helper、存入脚本值或稍后在同一运行时内调用的函数值。它们是普通脚本闭包，不是宿主线程或 async 任务。

## 语法

Lambda 参数写在 `|` 分隔符之间。函数体可以是表达式，也可以是块；参数可以带类型提示。

```vela
fn add_one(values) {
    return values.iter()
        .map(|value: i64| value + 1)
        .collect_array()
}
```

## 捕获

闭包会捕获周围作用域中的脚本自有值，包括现有 Rust 值转换产生的数据。捕获不会产生隐式写回 Rust 的路径。脚本闭包不能捕获或保留调用方借用的 Rust 数据。

```vela
fn above(limit: i64, values) {
    return values.iter()
        .filter(|value| value > limit)
        .collect_array()
}
```

## 回调用途

核心标准库回调点包括 `map`、`filter`、`any`、`all`、`find` 等迭代器适配器。惰性适配器是 one-shot 的，终端方法如 `collect_array()` 或 `count()` 会消费它们。

## 运行时边界

闭包和其他脚本调用一样同步执行并受预算约束。热更新时，活动调用和保留的
闭包继续持有创建时的精确可执行代，包括嵌套分派和状态。闭包不能跨无关
Runtime 移动，也不能调度后台工作。
