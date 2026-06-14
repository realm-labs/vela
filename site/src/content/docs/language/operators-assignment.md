---
title: "Operators And Assignment"
description: "Operators And Assignment documentation for Vela."
---

Vela operators are intentionally ordinary and explicit. Numeric operators require compatible concrete scalar types, boolean operators operate on booleans, and assignment routes through the correct local, heap, or host boundary.

## Arithmetic And Comparison

Arithmetic operators are `+`, `-`, `*`, `/`, and `%`. Comparison operators are `==`, `!=`, `<`, `<=`, `>`, and `>=`. Integer arithmetic is checked; overflow and unsigned underflow are errors.

```vela
fn score(base: i64, streak: i64) -> i64 {
    let value = base + streak * 3
    if value >= 100 {
        return 100
    }
    return value
}
```

## Boolean And Range Operators

`!`, `&&`, and `||` work on booleans. `..` creates an exclusive range and `..=` creates an inclusive range. Ranges are values and can be iterated.

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

## Assignment Targets

Assignment supports `=`, `+=`, `-=`, `*=`, `/=`, and `%=`. Valid targets include locals, record fields, indexed values, and host paths. Host writes are not direct Rust mutation; they are read/write or read-modify-write operations through HostAccess.

```vela
fn apply(player, reward) {
    player.gold += reward.amount
    player.tags["last_reward"] = reward.code
}
```

## Common Errors

Operator mismatches are reported when the concrete runtime tags do not match the operation. Assignment to a non-assignable expression, a read-only host path, or a target denied by capabilities fails with a source-spanned diagnostic.
