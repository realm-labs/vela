---
title: "控制流"
description: "Vela 控制流文档。"
---

Vela 的控制流在需要时可以产生表达式值，但仍然受 VM 执行预算约束。循环、分支和 `match` 都保留源码位置，便于运行时诊断定位。

## If 和块

`if` 可以作为语句，也可以作为表达式。作为表达式使用且没有 `else` 时，未命中的分支结果是 `null`。空块或只有语句的块也会得到 `null`。

```vela
fn label(score: i64) -> string {
    if score >= 90 {
        return "high"
    } else {
        return "normal"
    }
}
```

## 循环

`for value in source` 会先求值 `source`，然后创建或消费迭代器。`for index, value in source` 是语法级 indexed loop lowering，不需要额外的 `enumerate()` 适配器。

```vela
fn sum(values) -> i64 {
    let total = 0
    for index, value in values {
        total += value + index
    }
    return total
}
```

`break` 退出最近的循环，`continue` 进入下一轮。无限循环仍然会被执行预算限制。

## Match

`match` 可以匹配字面量、绑定、通配符、路径、tuple variant 和 record variant。`if` guard 可以进一步限制某个分支。

```vela
fn describe(result) -> string {
    match result {
        Result::Ok(value) if value > 0 => "positive",
        Result::Ok(_) => "ok",
        Result::Err(error) => error,
    }
}
```

## 边界

MVP 不包含 `async`、协程、`yield` 或脚本级线程。控制流里的宿主效果仍然受能力、预算和 HostAccess 检查。
