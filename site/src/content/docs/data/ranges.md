---
title: "Ranges"
description: "Range value documentation for Vela."
---

Ranges describe integer sequences and are repeatable sequence values. They are common in loops and can use specialized bytecode when the compiler proves `i64` bounds.

## Syntax

`start..end` excludes the end value. `start..=end` includes it.

```vela
fn sum_to(limit: i64) -> i64 {
    let total = 0
    for value in 0..=limit {
        total += value
    }
    return total
}
```

## Iteration

Ranges are repeatable, so each `for` loop creates a fresh traversal. Indexed loops can still be used when both the index and value are needed.

```vela
fn weighted(limit: i64) -> i64 {
    let total = 0
    for index, value in 1..limit {
        total += index * value
    }
    return total
}
```

## Methods

Ranges support standard methods such as `len()` and `is_empty()` where the bounds make those operations meaningful.

## Performance Boundary

The VM may lower proven integer range loops to typed scalar bytecode. That optimization must preserve the same observable behavior and does not change the language model.
