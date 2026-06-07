# Control Flow

Vela supports ordinary control flow for business logic: `if`, `match`, loops, early returns, and fallible Option/Result-style helpers.

## If

```vela
fn reward(enabled, amount) {
    if enabled {
        return amount;
    }
    return 0;
}
```

## Match

```vela
fn score(result) {
    match result {
        Check::Pass { score } => return score,
        Check::Fail { reason } => return 0,
    }
}
```

## For In

`for in` supports ordinary item iteration and indexed iteration.

```vela
fn total(values) {
    let sum = 0;
    for index, value in values {
        sum += value + index;
    }
    return sum;
}
```

Loop variables are scoped per iteration, so closures do not all capture the final loop value.
