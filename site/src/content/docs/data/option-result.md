---
title: "Option And Result"
description: "Option And Result documentation for Vela."
---

`Option` and `Result` are standard enum-style values for expected absence and recoverable failure. They are dynamic values, not generic types.

## Responsibilities

Use `Option::None` when data may be absent as part of normal business logic. Use `Result::Err` when an operation can fail and the script is expected to handle the reason. Reserve VM errors for bugs, contract violations, budget failures, or sandbox denials.

```vela
fn find_reward(rewards, code: string) {
    return rewards.get(code) // Option
}

fn parse_amount(text: string) {
    return text.parse_i64() // Result
}
```

## Common Methods

Standard helpers include predicates and conversions such as `is_some`, `is_none`, `unwrap_or`, `ok_or`, `to_option`, and `to_error_option`.

```vela
fn amount_or_zero(text: string) -> i64 {
    let parsed = text.parse_i64()
    return parsed.unwrap_or(0)
}
```

## Pattern Matching

You can also handle these values with `match`, especially when the success or error branch needs custom logic.

```vela
fn describe(result) -> string {
    match result {
        Result::Ok(value) => f"ok:{value}",
        Result::Err(error) => f"error:{error}",
    }
}
```

## No Generic Syntax

Write `Option` and `Result`, not `Option<T>` or `Result<T, E>`. Contracts for payload values belong at function, field, host, or explicit validation boundaries.
