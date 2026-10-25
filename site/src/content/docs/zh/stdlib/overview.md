---
title: "标准库概览"
description: "Vela 标准库概览。"
---

Vela 标准库分成两类：始终可用的纯值 helper，以及由宿主显式安装的
capability。纯值 helper 作用在脚本拥有的字符串、数组、Map、Set、Bytes、
`Option`、`Result` 和数字上。Time、Random 和 I/O 这类
能力由 embedding 应用安装，避免脚本默认依赖进程全局状态。

## 纯值 Helper

日常使用最多的是值方法。它们走和普通脚本方法相同的 dispatch 入口，
但实现是 native，并且有稳定 method ID；它们不暴露 Rust 宿主对象的方法。

```vela
fn main() {
    let tags = ["daily", "quest", "daily"].distinct().sort();
    let label = tags.join(":").to_upper();
    return label;
}
```

集合同时提供 eager helper 和 iterator view。多个转换需要组合时，优先用
iterator pipeline，最后再 materialize。

```vela
fn main() {
    let total = [1, 2, 3, 4]
        .iter()
        .filter(|value| value > 2)
        .map(|value| value * 10)
        .collect_array()
        .sum();
    return total;
}
```

## 模块和构造函数

模块函数用于创建标准 enum 值、转换集合和调用数值工具。常见例子包括
`option::some`、`result::ok`、`set::from_array`、`bytes::from_hex` 和
`math::*`。

```vela
fn main() {
    let decoded = bytes::from_hex("ff00");
    let fallback = result::unwrap_or(decoded, b"");
    return fallback.len();
}
```

`Option` 表达普通缺失，比如查询失败或 parse 失败。`Result` 表达脚本可处理
的可恢复失败。VM trap、权限拒绝和预算耗尽是 diagnostic，不是
`Result::Err`。

## Capability 边界

默认标准 native 不授予非确定性或影响进程的 effect。宿主通过 engine builder
显式开启这些能力。

```vela
fn main() {
    let now = time::now();
    let roll = math::random(1, 6);
    return roll;
}
```

这个例子里，`time::now` 和 `math::random` 只有在宿主安装对应的 native 并授予
time、random capability 后才可执行。沙箱文件系统和 stdout helper 也同样是
opt-in。独立同步 native 函数使用现有自有值 API，其声明的 effects 仍需授权；
脚本字段和集合修改不会隐式写回 Rust。
