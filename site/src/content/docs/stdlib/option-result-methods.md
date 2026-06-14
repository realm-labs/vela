---
title: "Option And Result Methods"
description: "Option and Result method documentation for Vela."
---

`Option` and `Result` are standard enum values. They are used by the standard
library for ordinary absence and recoverable failures; they do not replace VM
diagnostics for type errors, denied capabilities, or exhausted budgets.

## Constructors And Predicates

Use module constructors when a script needs to create values explicitly.

```vela
fn main() {
    let present = option::some(4);
    let missing = option::none();
    let ok = result::ok("ready");
    return present.is_some() && missing.is_none() && ok.is_ok();
}
```

Both module functions and value methods expose common predicates:
`is_some`, `is_none`, `is_ok`, and `is_err`.

## Fallbacks And Conversion

`unwrap_or` works on both `Option` and `Result`. `ok_or` converts an `Option`
to a `Result`; `to_option` and `to_error_option` inspect a `Result`.

```vela
fn main() {
    let parsed = "42".parse_i64();
    let checked = parsed.ok_or("not a number");
    let value = checked.unwrap_or(0);
    let error = checked.to_error_option().unwrap_or("");
    return value + error.len();
}
```

`flatten` removes one layer from nested `Option` or nested `Result` values.

## Callback Helpers And Propagation

`map`, `and_then`, `or_else`, and `filter` operate on the success/present path.
`Result.map_err` transforms the error payload.

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

The `?` operator propagates `Option::None` or `Result::Err` from the current
function. Use it for script-visible control flow, not for host permission or VM
runtime failures.
