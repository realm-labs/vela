---
title: "控制流"
description: "Vela 控制流文档。"
---

Vela 的控制流在需要时可以产生表达式值，但仍然受 VM 执行预算约束。循环、分支和 `match` 都保留源码位置，便于运行时诊断定位。

## If 和块

`if` 可以作为语句，也可以作为表达式。作为表达式使用且没有 `else` 时，未命中的分支结果是 `()`。空块或只有语句的块也会得到 `()`。

```vela
fn label(score: i64) -> String {
    if score >= 90 {
        return "high"
    } else {
        return "normal"
    }
}
```

## 循环

只需要值时使用 `for value in source`。同时需要位置和值时，使用
`for index, value in source`，其中 `index` 是从 0 开始的位置。

`source` 表达式会在循环开始时求值一次。数组、range、string、map、set、
iterator 和宿主提供的 iterable，只要支持迭代，都可以放在这里。

Map 循环会产生 `MapEntry { key, value }` record。只需要 value 时使用
`map.values()`，只需要 key 时使用 `map.keys()`。

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
fn describe(result) -> String {
    match result {
        Result::Ok(value) if value > 0 => "positive",
        Result::Ok(_) => "ok",
        Result::Err(error) => error,
    }
}
```

## Async 函数和 Await

模块函数和脚本方法可以声明为 `async fn`。后缀 `.await` 只能用于调用表达式，
并且只能出现在 async 函数中。静态已知的 async callee 必须 await；awaited
dynamic call 可以解析为同步或异步目标。

```vela
async fn load_profile(repository, player_id) {
    return repository.load(player_id).await;
}
```

Await 保持顺序脚本语义。挂起的调用会在嵌入方 executor 再次 poll 时恢复；
它不会暴露 task handle、手动 resume、`yield`、脚本级线程，也不允许并发使用
同一个 Runtime。宿主效果仍然受 capability、预算和 HostAccess 检查。
