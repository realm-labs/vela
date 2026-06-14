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

闭包会捕获周围作用域中的脚本可见值。被捕获的宿主值仍然是 host ref 或 path proxy；闭包不会把宿主状态变成脚本 GC 拥有的数据，也不会暴露 Rust `&mut T`。

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

闭包执行和其他脚本调用一样受预算约束。MVP 不承诺协程挂起、async 热更新，也不承诺把闭包跨无关 runtime 移动。
