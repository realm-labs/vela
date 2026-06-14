---
title: "运算符和赋值"
description: "Vela 运算符和赋值文档。"
---

Vela 的运算符保持明确和普通。数字运算要求兼容的具体标量类型，布尔运算作用于布尔值，赋值会路由到局部变量、脚本堆对象或宿主边界。

## 算术和比较

算术运算符是 `+`、`-`、`*`、`/`、`%`。比较运算符是 `==`、`!=`、`<`、`<=`、`>`、`>=`。整数运算是 checked 的；溢出和无符号下溢会报错。

```vela
fn score(base: i64, streak: i64) -> i64 {
    let value = base + streak * 3
    if value >= 100 {
        return 100
    }
    return value
}
```

## 布尔和 Range

`!`、`&&`、`||` 作用于布尔值。`..` 创建排除结束值的 range，`..=` 创建包含结束值的 range。Range 是值，可以被迭代。

```vela
fn count_even(limit: i64) -> i64 {
    let count = 0
    for value in 0..=limit {
        if value % 2 == 0 {
            count += 1
        }
    }
    return count
}
```

## 赋值目标

赋值支持 `=`、`+=`、`-=`、`*=`、`/=`、`%=`。合法目标包括局部变量、record 字段、索引位置和宿主路径。宿主写入不是直接 Rust 可变引用，而是通过 HostAccess 完成读写或读改写。

```vela
fn apply(player, reward) {
    player.gold += reward.amount
    player.tags["last_reward"] = reward.code
}
```

## 常见错误

具体运行时标签不匹配时，运算符会报类型错误。对不可赋值表达式、只读宿主路径或能力被拒绝的目标赋值，会产生带源码位置的诊断。
