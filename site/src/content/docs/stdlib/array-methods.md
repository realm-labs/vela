---
title: "Array Methods"
description: "Array method documentation for Vela."
---

Arrays are ordered script-owned collections. Standard array methods cover
lookup, mutation, eager transformations, and iterator creation. Collection
growth is still checked by the VM's execution and collection budgets.

## Lookup And Mutation

Use `len`, `is_empty`, `first`, `last`, `contains`, and `index_of` for basic
queries. Methods that may not find a value return `Option`.

```vela
fn main() {
    let rewards = ["gold", "xp"];
    let first = rewards.first().unwrap_or("none");
    let index = rewards.index_of("xp").unwrap_or(-1);
    return first.len() + index;
}
```

Mutation methods update the array value in place and return either `null`,
`bool`, or an `Option` depending on the operation.

```vela
fn main() {
    let queue = ["spawn"];
    queue.push("reward");
    queue.insert(1, "combat");
    let removed = queue.remove_at(0).unwrap_or("");
    return removed + ":" + queue.join(",");
}
```

## Transformations

Array helpers such as `slice`, `reverse`, `distinct`, `sort`, `min`, `max`,
`sum`, `group_by`, and `sort_by` materialize a result immediately.
`group_by` returns a value-keyed `Map<K, Array<T>>`, so callback keys follow
the same `ValueKey` policy as ordinary map keys.

```vela
fn main() {
    let scores = [5, 1, 3, 5].distinct().sort();
    let best = scores.max().unwrap_or(0);
    return best + scores.sum();
}
```

Callback helpers call script functions through the VM, so keep callback bodies
small in hot paths.

```vela
fn main() {
    return [1, 2, 3, 4]
        .filter(|value| value % 2 == 0)
        .map(|value| value * value)
        .sum();
}
```

## Iterator Views

`iter` and `values` produce iterators over array values. Iterator methods such
as `map`, `filter`, `take`, `skip`, `find`, `any`, `all`, `count`,
`collect_array`, and `collect_set` let scripts build lazy pipelines.

```vela
fn main() {
    let names = ["wolf", "boar", "wyrm"];
    return names.iter()
        .filter(|name| name.starts_with("w"))
        .take(1)
        .collect_array()
        .join(",");
}
```
