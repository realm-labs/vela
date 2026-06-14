---
title: "运算符和赋值"
description: "Vela 运算符和赋值文档。"
---

Vela 使用常见的运算符，但不会偷偷猜类型，也不会自动把值转换成另一种类型。
如果运算拿到不合适的值，脚本会在对应表达式位置报错。

实际规则可以这样理解：

- `1i64 + 2i64` 可以运行。
- `1i64 + "2"` 会失败，不会把字符串自动转成数字。
- `if ready { ... }` 里的 `ready` 必须是 `bool`。
- `player.gold += 10` 只有在 `player.gold` 可写时才会修改成功。

## 算术和比较

算术运算符是 `+`、`-`、`*`、`/`、`%`，用于数字。左右两边必须是兼容的
数字类型；Vela 不会把字符串当数字，也不会替你混用不兼容的数字标签。

比较运算符是 `==`、`!=`、`<`、`<=`、`>`、`>=`。对象语义相等是显式
选择的能力：record、array、map、set、closure、iterator 和 host ref 不会
因为字段或内容相同就自动做结构比较。需要比较脚本对象或 `HostRef` 的引用
身份时，使用 `===` 和 `!==`。这两个身份运算符不会读取宿主状态，也不会使用
Map/Set 的 key 等价规则。

`null`、布尔值、char、精确标量标签、字符串、bytes 和 range 等内建叶子值按
值比较。数字相等按标签精确比较，所以 `1i64 == 1u64` 为 false。整数运算会
检查错误：溢出、无符号下溢和除以零都会报错。

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

赋值支持 `=`、`+=`、`-=`、`*=`、`/=`、`%=`。

可以赋值给：

- 局部变量：`score = 10`
- 脚本 record 字段：`reward.amount = 25`
- 集合索引位置：`tags["last_reward"] = reward.code`
- 可写的宿主对象字段：`player.gold += reward.amount`

如果目标属于 Rust 宿主对象，Vela 会请求宿主应用这次写入。宿主可以允许写入，
也可以因为字段只读或当前 capability profile 不允许写入而拒绝。

```vela
fn apply(player, reward) {
    player.gold += reward.amount
    player.tags["last_reward"] = reward.code
}
```

## 常见错误

常见错误包括：把非数字用于算术、把非布尔值用于布尔逻辑、给不可赋值的表达
式赋值、写入的值类型不符合预期，或写入宿主标记为只读的字段。只要 Vela 能
定位到具体表达式，错误都会带源码位置。
