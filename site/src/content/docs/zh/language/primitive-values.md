---
title: "基础值"
description: "Vela 基础值文档。"
---

Vela 是动态语言，但基础值有明确的运行时标签。这样宿主转换、反射、字节码 guard 和后续优化都能基于稳定的基础类型工作，而不需要脚本语言泛型。

## 基础类型集合

基础值包括 `null`、`bool`、`char`、有符号整数 `i8` 到 `i64`、无符号整数 `u8` 到 `u64`、浮点数 `f32` 和 `f64`、`string`、`bytes`。

```vela
let enabled = true
let letter = 'A'
let count = 12i64
let ratio = 0.25f64
let label = "ready"
let packet = b"\x01\x02"
```

## 数字字面量

没有后缀的整数字面量在没有更具体上下文时默认是 `i64`。没有后缀的浮点字面量默认是 `f64`。带提示的参数、字段或局部变量可以给字面量提供上下文，但运算符不会做隐式拓宽或整数到浮点的转换。

```vela
fn add_i32(lhs: i32, rhs: i32) -> i32 {
    return lhs + rhs
}

fn main() -> i32 {
    return add_i32(1, 2)
}
```

## Null

`null` 表示没有有意义的值、只有语句的块结果，或宿主/元数据边界上的 nullable 互操作。预期缺失应优先使用 `Option`，可恢复失败应使用 `Result`，不要把所有“没有结果”都塞进 `null`。

```vela
fn maybe_message(enabled: bool) {
    if enabled {
        return "enabled"
    }
    return null
}
```

## 边界规则

基础类型提示是契约，不是转换。`1i32 + 2i64` 在静态已知时是错误；动态值发生同样不匹配时会在运行时报错。整数溢出也是错误；显式 wrapping 和转换能力属于标准库 API。
