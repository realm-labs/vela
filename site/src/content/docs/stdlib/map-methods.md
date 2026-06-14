---
title: "Map Methods"
description: "Map method documentation for Vela."
---

Maps are script-owned value-keyed collections. Standard map helpers are
designed for explicit lookup and explicit traversal; missing keys do not trap
unless the script uses direct indexing. Key equality is defined by `ValueKey`,
not by user-visible equality or ordering implementations.

## Lookup And Update

Use `has`, `get`, and `get_or` when the key may be absent. `get` returns
`Option`, while `get_or` returns the stored value or the fallback argument.

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10};
    let gold = rewards.get("gold").unwrap_or(0);
    let gems = rewards.get_or("gems", 0);
    return gold + gems;
}
```

`set`, `remove`, `clear`, `extend`, and `merge` mutate or combine maps.
`remove` returns the removed value as `Option`.

```vela
fn main() {
    let rewards = {"gold": 3};
    rewards.set("xp", 10);
    let removed = rewards.remove("gold").unwrap_or(0);
    rewards.extend({"gems": 1});
    return removed + rewards.len();
}
```

## Views And Entries

`keys`, `values`, and `entries` return iterators. Entry values are `MapEntry`
records with `key` and `value` fields.

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10};
    let labels = rewards.keys().collect_array().sort().join(",");
    let total = rewards.values().collect_array().sum();
    return labels == "gold,xp" && total == 13;
}
```

Use `entries` when both key and value are needed in the same pipeline.

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10};
    let entry = rewards.entries()
        .find(|entry| entry.value >= 10)
        .unwrap_or(MapEntry { key: "", value: 0 });
    return entry.key;
}
```

## Callback Helpers

Eager helpers such as `map_values`, `filter`, `find`, `any`, `all`, and `count`
accept callbacks. Most helpers support either value-only callbacks or
key-and-value callbacks where that is meaningful.

```vela
fn main() {
    let rewards = {"gold": 3, "xp": 10, "quest": 1};
    let doubled = rewards.map_values(|value| value * 2);
    let big = rewards.filter(|key, value| key.len() <= 4 && value >= 3);
    return doubled["xp"] + big.len();
}
```
