---
title: "Option 和 Result 方法"
description: "Vela Option 和 Result 方法文档。"
---

`Option` 和 `Result` 是标准 enum 值。标准库用它们表达普通缺失和可恢复失败；
它们不会替代类型错误、capability 拒绝或预算耗尽这类 VM diagnostic。

## 构造和判断

脚本需要显式创建值时，可以使用模块构造函数。

```vela
fn main() {
    let present = option::some(4);
    let missing = option::none();
    let ok = result::ok("ready");
    return present.is_some() && missing.is_none() && ok.is_ok();
}
```

模块函数和值方法都提供常见 predicate：`is_some`、`is_none`、`is_ok` 和
`is_err`。

## Fallback 和转换

`unwrap_or` 可用于 `Option` 和 `Result`。`ok_or` 把 `Option` 转成
`Result`；`to_option` 和 `to_error_option` 用来检查 `Result`。

```vela
fn main() {
    let parsed = "42".parse_i64();
    let checked = parsed.ok_or("not a number");
    let value = checked.unwrap_or(0);
    let error = checked.to_error_option().unwrap_or("");
    return value + error.len();
}
```

`flatten` 会去掉一层嵌套的 `Option` 或 `Result`。

## Callback Helper 和传播

`map`、`and_then`、`or_else` 和 `filter` 作用在成功或存在的分支上。
`Result.map_err` 转换错误 payload。

```vela
fn main() {
    let score = "5"
        .parse_i64()
        .map(|value| value * 2)
        .filter(|value| value >= 10)
        .unwrap_or(0);
    return score;
}
```

`?` 操作符会从当前函数传播 `Option::None` 或 `Result::Err`。它适合脚本可见
的控制流，不用于宿主权限或 VM 运行时失败。
