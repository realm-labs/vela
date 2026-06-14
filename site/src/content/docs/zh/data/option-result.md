---
title: "Option 和 Result"
description: "Vela Option 和 Result文档。"
---

`Option` 和 `Result` 是用于预期缺失和可恢复失败的标准 enum 风格值。它们是动态值，不是泛型类型。

## 职责

数据在正常业务逻辑中可能不存在时，使用 `Option::None`。操作可能失败且脚本应处理失败原因时，使用 `Result::Err`。VM error 保留给脚本 bug、契约违反、预算失败或沙箱拒绝。

```vela
fn find_reward(rewards, code: String) {
    return rewards.get(code)
}

fn parse_amount(text: String) {
    return text.parse_i64()
}
```

## 常用方法

标准 helper 包括 `is_some`、`is_none`、`unwrap_or`、`ok_or`、`to_option`、`to_error_option` 等谓词和转换。

```vela
fn amount_or_zero(text: String) -> i64 {
    let parsed = text.parse_i64()
    return parsed.unwrap_or(0)
}
```

## Pattern Matching

需要自定义成功或失败分支逻辑时，可以使用 `match` 处理这些值。

```vela
fn describe(result) -> String {
    match result {
        Result::Ok(value) => f"ok:{value}",
        Result::Err(error) => f"error:{error}",
    }
}
```

## 没有泛型语法

写 `Option` 和 `Result`，不要写 `Option<T>` 或 `Result<T, E>`。Payload 的契约应放在函数、字段、宿主或显式验证边界。
