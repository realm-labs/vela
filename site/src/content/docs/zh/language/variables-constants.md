---
title: "变量和常量"
description: "Vela 变量和常量文档。"
---

Vela 有局部变量、模块常量、VM 拥有的 state 和宿主拥有的 extern state。默认是动态类型；没有类型提示的局部绑定只保存当前值，有类型提示的绑定会增加运行时契约。

## 局部变量

`let` 创建局部绑定。绑定可以有类型提示、初始化表达式，或者两者都有。类型提示是检查契约，不是泛型，也不会做隐式转换。

```vela
fn total(base: i64, bonus) -> i64 {
    let adjusted: i64 = base + 10
    let dynamic_bonus = bonus
    return adjusted + dynamic_bonus
}
```

## 常量

`const` 声明模块级不可重新赋值的值，适合稳定的脚本配置和会参与反射或热更新 ABI 检查的名称。

```vela
pub const START_LEVEL: i64 = 1
const LEVEL_STEP: i64 = 5

fn next_level(current: i64) -> i64 {
    return current + LEVEL_STEP
}
```

## 持久状态

`state` 必须有类型和 initializer，并为每个 Runtime 创建一个 VM 拥有的 cell。`extern state` 必须有类型、禁止 initializer，并由宿主绑定。脚本可以替换 VM state 根；extern 根不可替换，嵌套修改必须经过 HostAccess。脚本不会拿到真实的 Rust `&mut T`。

```vela
extern state player: Player;
state level_ups: i64 = 0;

fn level_up() {
    player.level += 1
    level_ups += 1
}
```

## 常见错误

写入违反绑定、字段、参数、返回值或 state 契约的值会产生类型契约诊断。Extern 根不能赋值，VM initializer 不能执行外部 effect，常量也不能作为可变存储。
