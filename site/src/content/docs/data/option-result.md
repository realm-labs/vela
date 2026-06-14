---
title: "Option And Result"
description: "Option And Result documentation for Vela."
---

`Option` and `Result` are standard enum-style values for expected absence and recoverable failure. `Option<T>` and `Result<T, E>` are builtin type-hint contracts for payload checks at boundaries; they are not user-defined generic types.

## Responsibilities

Use `Option::None` when data may be absent as part of normal business logic. Use `Result::Err` when an operation can fail and the script is expected to handle the reason. Reserve VM errors for bugs, contract violations, budget failures, or sandbox denials.

```vela
fn find_reward(rewards, code: String) {
    return rewards.get(code) // Option
}

fn parse_amount(text: String) {
    return text.parse_i64() // Result
}
```

## Common Methods

Standard helpers include predicates and conversions such as `is_some`, `is_none`, `unwrap_or`, `ok_or`, `to_option`, and `to_error_option`.

```vela
fn amount_or_zero(text: String) -> i64 {
    let parsed = text.parse_i64()
    return parsed.unwrap_or(0)
}
```

Payload contracts compose with containers:

```vela
fn load_rewards() -> Result<Map<String, i64>, String> {
    return result::ok({ "xp": 10 })
}

fn xp_or_zero() -> Result<i64, String> {
    let rewards = load_rewards()?
    return result::ok(rewards.get("xp").unwrap_or(0))
}
```

## Pattern Matching

You can also handle these values with `match`, especially when the success or error branch needs custom logic.

```vela
fn describe(result) -> String {
    match result {
        Result::Ok(value) => f"ok:{value}",
        Result::Err(error) => f"error:{error}",
    }
}
```

## Generic Boundary

Only builtin type-hint contracts accept type arguments. `Option<T>` and
`Result<T, E>` are valid contracts; `Player<T>` and other user-defined generic
types are not.
