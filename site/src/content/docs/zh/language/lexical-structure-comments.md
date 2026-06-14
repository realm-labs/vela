---
title: "词法结构和注释"
description: "Vela 词法结构和注释文档。"
---

Vela 源文件是 UTF-8 文本文件，扩展名为 `.vela`。简单声明和语句可以用换行或分号结束；`fn`、`struct`、`enum`、`trait`、`impl`、`if`、`match`、`for` 这类块结构自身就是完整语法单元。

## 源文件结构

标识符以 `_` 或 ASCII 字母开头，后续可以包含 ASCII 字母、数字和 `_`。`fn`、`struct`、`match`、`self`、`true`、`false`、`null` 等关键字是保留词，不能作为普通名称。

```vela
#!/usr/bin/env vela

// 文件级声明是普通 item。
pub const BASE_XP: i64 = 100

fn award(level: i64) -> i64 {
    return BASE_XP + level * 10
}
```

## 注释

行注释从 `//` 开始直到行尾。块注释使用 `/* ... */`，并且可以嵌套，所以临时注释一段已经包含注释的代码不会破坏词法结构。

```vela
fn classify(value: i64) -> string {
    /* 嵌套注释是合法的：
       /* disabled note */
    */
    if value > 0 {
        return "positive"
    }
    return "zero-or-negative"
}
```

## 边界

Vela 没有预处理指令、宏展开、`eval`，也不会在运行时解析动态生成的源码字符串。属性会被解析成元数据，但具体含义由语义阶段、宿主注册或工具链定义。
