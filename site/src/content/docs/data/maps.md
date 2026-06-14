---
title: "Maps"
description: "Maps documentation for Vela."
---

Maps are key-value collections for dynamic script data. They are useful for configuration, lookup tables, and snapshot values, but they are not a replacement for registered host schemas when Rust-owned state must be mutated safely.

`Map<K, V>` is the parameterized builtin map contract. Map keys use Vela's
`ValueKey` policy: immutable leaf values compare by value, script heap objects
and host refs compare by identity, and transient values such as `PathProxy` are
rejected before mutation. Existing map literals remain convenient for string
keys, and arbitrary runtime key values can be inserted through indexing or map
methods.

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

Typed map contracts check existing keys and values at dynamic boundaries and
protect later typed updates:

```vela
fn add_tag_count(rewards: Map<String, i64>, tags: Set<String>) {
    rewards.set("tag_count", tags.len())
    return rewards.get("tag_count").unwrap_or(0)
}
```

```vela
fn remember_by_id(rewards: Map<i64, String>, id: i64, label: String) {
    rewards.set(id, label)
    return rewards.get(id).unwrap_or("")
}
```

## Views

`keys()`, `values()`, and `entries()` expose repeatable views. `keys()`
returns the stored original key values, and `entries()` yields values with
`key` and `value` fields. Non-string keys stay typed values; they are not
stringified for traversal.

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
