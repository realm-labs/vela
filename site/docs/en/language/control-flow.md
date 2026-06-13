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

`for value in source` evaluates `source` once, creates an iterator, and advances it until the iterator is exhausted. Arrays, sets, maps, strings, and ranges are repeatable sources. Existing iterator values are one-shot cursors, so looping over one consumes it.

String `for in` yields UTF-8 `char` values, matching `text.chars()`. Use `text.bytes()` when byte traversal is required. Indexed `for index, value in source` is syntax-level loop lowering; it does not allocate an eager `enumerate()` collection.

Loop variables are scoped per iteration, so closures do not all capture the final loop value.
