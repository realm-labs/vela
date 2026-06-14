---
title: "Maps"
description: "Maps documentation for Vela."
---

Maps are key-value collections for dynamic script data. They are useful for configuration, lookup tables, and snapshot values, but they are not a replacement for registered host schemas when Rust-owned state must be mutated safely.

## Literals And Access

Map literals use `{ key: value }`. Keys can be identifiers, strings, chars, numbers, or paths according to the grammar. Indexing reads or writes an entry by key.

```vela
fn reward_table() {
    return {
        xp: 10i64,
        "gold": 5i64,
    }
}
```

## Updates

Common map methods include insertion, removal, containment checks, `get`, and `get_or`. Missing lookup APIs should prefer `Option` rather than `null` when absence is expected.

```vela
fn add_reward(rewards, code: String, amount: i64) {
    let current = rewards.get_or(code, 0)
    rewards[code] = current + amount
    return rewards
}
```

## Views

`keys()`, `values()`, and `entries()` expose repeatable views. `entries()` yields values with `key` and `value` fields, which keeps map traversal explicit.

```vela
fn total(rewards) -> i64 {
    let sum = 0
    for entry in rewards.entries() {
        sum += entry.value
    }
    return sum
}
```

## Host Boundary

Indexing into a host path can represent a HostAccess operation rather than a script map mutation. Capabilities, read-only fields, generations, and schema epochs are still checked by the host adapter.
