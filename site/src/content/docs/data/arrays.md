---
title: "Arrays"
description: "Arrays documentation for Vela."
---

Arrays are ordered, indexable, GC-managed collections. They are dynamic: the language does not support `Array<T>`, so element contracts are enforced by the API boundary that consumes the array or by explicit script checks.

## Literals And Indexing

Array literals use square brackets. Indexing uses zero-based numeric indexes and reports an error or returns method-specific `Option` values depending on the operation.

```vela
fn second_reward() -> i64 {
    let rewards = [10i64, 20i64, 30i64]
    return rewards[1]
}
```

## Mutation

Array methods cover common update operations such as appending, removing, and querying. Mutating a script array changes that script heap value; mutating a host-owned array path must go through HostAccess.

```vela
fn collect_large(values) {
    let out = []
    for value in values {
        if value > 10 {
            out.push(value)
        }
    }
    return out
}
```

## Iteration

Arrays are repeatable sequences. `iter()` creates a one-shot iterator, and lazy adapters such as `map` or `filter` consume that iterator when a terminal method runs.

```vela
fn increment(values) {
    return values.iter().map(|value| value + 1).collect_array()
}
```

## Boundaries

Array length and element access are budgeted operations. Arrays belong to the script heap unless they are snapshots returned from host conversion; Rust host storage is not placed under the script GC.
