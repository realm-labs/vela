---
title: "Set Methods"
description: "Set method documentation for Vela."
---

Sets store unique script values. Current set keys are intentionally limited to
stable value categories such as `null`, booleans, finite numbers, and strings,
so set behavior stays deterministic across host boundaries.

## Construction And Membership

Use `set::from_array` to build a set from an array. Duplicate values are
deduplicated according to set key equality.

```vela
fn main() {
    let tags = set::from_array(["daily", "quest", "daily"]);
    if tags.has("quest") && tags.len() == 2 {
        return "ok";
    }
    return "missing";
}
```

`values` and `iter` produce iterators over set values. If stable display order
matters, collect and sort before joining.

```vela
fn main() {
    let tags = set::from_array(["raid", "daily", "quest"]);
    return tags.values().collect_array().sort().join(",");
}
```

## Mutation

`add` returns `true` when a value was inserted and `false` when it was already
present. `remove` returns whether a value was removed.

```vela
fn main() {
    let tags = set::from_array(["daily"]);
    let added = tags.add("quest");
    let removed = tags.remove("missing");
    tags.extend(set::from_array(["raid", "daily"]));
    return added && !removed && tags.len() == 3;
}
```

`clear` removes all values and returns `null`.

## Set Algebra

`union`, `intersection`, `difference`, and `symmetric_difference` return new
sets. Relation helpers return booleans.

```vela
fn main() {
    let owned = set::from_array(["daily", "quest", "raid"]);
    let required = set::from_array(["daily", "quest"]);
    let event = set::from_array(["quest", "bonus"]);
    let shared = owned.intersection(event);
    return required.is_subset(owned)
        && owned.is_superset(required)
        && shared.has("quest");
}
```

Callback helpers `map`, `filter`, `find`, `any`, `all`, and `count` mirror the
array callback model.
