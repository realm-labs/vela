---
title: "Operators And Assignment"
description: "Operators And Assignment documentation for Vela."
---

Vela uses familiar operators, but it does not silently guess or convert types.
If an operation receives a value of the wrong kind, the script fails with a
diagnostic at that expression.

In practice:

- `1i64 + 2i64` works.
- `1i64 + "2"` fails instead of converting the string.
- `if ready { ... }` expects `ready` to be `bool`.
- `player.gold += 10` changes `player.gold` only if that field is writable.

## Arithmetic And Comparison

Arithmetic operators are `+`, `-`, `*`, `/`, and `%`. They work on numeric
values. The two sides must be compatible numeric types; Vela does not turn
strings into numbers or mix incompatible numeric tags for you.

Comparison operators are `==`, `!=`, `<`, `<=`, `>`, and `>=`. Semantic object
equality is opt-in: records, arrays, maps, sets, closures, iterators, and host
refs do not become structurally comparable just because their fields or
contents match. Use `===` and `!==` when you need reference identity for script
objects or `HostRef` values. These identity operators do not read host state
and do not use Map/Set key equivalence.

Builtin leaf values such as `null`, booleans, chars, exact scalar tags,
strings, bytes, and ranges compare by value. Numeric equality is tag-exact, so
`1i64 == 1u64` is false. Integer arithmetic is checked. Overflow, unsigned
underflow, and division by zero are errors.

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

Assignment supports `=`, `+=`, `-=`, `*=`, `/=`, and `%=`.

You can assign to:

- a local variable: `score = 10`
- a script record field: `reward.amount = 25`
- an indexed collection entry: `tags["last_reward"] = reward.code`
- a writable field on a host object: `player.gold += reward.amount`

When the target belongs to a Rust host object, Vela asks the host to apply the
write. The host can allow it, reject it because the field is read-only, or
reject it because the current capability profile does not allow that write.

```vela
fn apply(player, reward) {
    player.gold += reward.amount
    player.tags["last_reward"] = reward.code
}
```

## Common Errors

Common errors include using a non-number in arithmetic, using a non-boolean in
boolean logic, assigning to something that is not assignable, writing a value
with the wrong type, or writing a host field that the host has marked read-only.
These errors include source locations when Vela can identify the responsible
expression.
