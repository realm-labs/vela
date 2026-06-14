---
title: "变量和常量"
description: "Vela 变量和常量文档。"
---

Vela 有局部变量、模块常量和宿主/运行时全局值。默认是动态类型；没有类型提示的绑定只保存当前值，有类型提示的绑定会增加运行时契约。

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

## 全局值和宿主状态

`global` 声明由运行时或宿主嵌入层提供的命名值。脚本可以读取并在契约允许时写入全局值，但 Rust 拥有的宿主状态仍然经过 HostAccess；脚本不会拿到真实的 Rust `&mut T`。

```vela
global player: Player

fn level_up() {
    player.level += 1
}
```

## 常见错误

写入违反绑定、字段、参数、返回值或全局值契约的值，会产生类型契约诊断。把 `const` 当成可变存储，或者把 `global` 当成绕过宿主权限的通道，都会被边界规则拒绝。
