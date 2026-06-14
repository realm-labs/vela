---
title: "函数"
description: "Vela 函数文档。"
---

函数是 Vela 的主要执行单元，也是嵌入调用、反射和热更新的基本单位。源码文件或模块可以用 `pub` 暴露函数，宿主可以按名称调用选定入口。

## 声明

参数可以有类型提示和默认值。返回值提示会增加返回契约检查。Vela 不按参数数量、类型提示、默认值或原生 Rust 签名做函数重载；同一作用域中一个名称只能有一个函数。

```vela
pub fn grant(player, amount: i64 = 1) -> i64 {
    player.gold += amount
    return player.gold
}
```

## 调用和参数

调用支持位置参数和命名参数。目标确定后，命名参数会按参数名匹配，缺省值会从函数签名中补齐。

```vela
fn scale(value: i64, multiplier: i64 = 2, offset: i64 = 0) -> i64 {
    return value * multiplier + offset
}

fn main() -> i64 {
    return scale(10, offset = 5)
}
```

## 宿主边界

宿主调用脚本函数会进入 checked entry。参数 guard、返回 guard、能力检查和调用栈诊断都会保留。脚本仍然不会暴露真实 Rust 引用；宿主状态修改通过注册的宿主值和 HostAccess 完成。

## 热更新

函数代码可以按函数或模块粒度热更新。已有调用帧继续运行旧代码，新调用在 ABI 和 schema 兼容性检查通过后进入新代码。
