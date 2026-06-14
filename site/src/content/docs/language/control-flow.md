---
title: "Control Flow"
description: "Control Flow documentation for Vela."
---

Control flow is expression-oriented where useful, but stays bounded by the VM execution budget. Loops, branches, and matches preserve source spans so runtime diagnostics can point at the responsible construct.

## If And Blocks

`if` can be used as a statement or expression. If an expression-valued `if` has no `else`, the untaken branch evaluates to `null`. Empty or statement-only blocks also evaluate to `null`.

```vela
fn label(score: i64) -> string {
    if score >= 90 {
        return "high"
    } else {
        return "normal"
    }
}
```

## Loops

`for value in source` evaluates the source once and then consumes or creates an iterator. Indexed loops use `for index, value in source` and do not require a separate `enumerate()` adapter.

```vela
fn sum(values) -> i64 {
    let total = 0
    for index, value in values {
        total += value + index
    }
    return total
}
```

`break` exits the nearest loop and `continue` advances it. Infinite loops are still subject to execution budgets.

## Match

`match` compares one value against literal, binding, wildcard, path, tuple-variant, or record-variant patterns. Guards can refine an arm with `if`.

```vela
fn describe(result) -> string {
    match result {
        Result::Ok(value) if value > 0 => "positive",
        Result::Ok(_) => "ok",
        Result::Err(error) => error,
    }
}
```

## Boundaries

Vela does not include `async`, coroutines, `yield`, or script-level threads in the MVP. Host effects inside control flow are still checked through capabilities, budgets, and HostAccess.
