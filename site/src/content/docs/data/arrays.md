---
title: "Arrays"
description: "Arrays documentation for Vela."
---

Arrays are ordered, indexable, GC-managed collections. `Array<T>` is a builtin
type-hint contract for checked boundaries; it is not a general script generic
type and it does not convert elements.

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

When a value has a trusted `Array<i64>` fact, compatible mutations can avoid
extra runtime guards while incompatible mutations are rejected or checked before
the write:

```vela
fn append_score(scores: Array<i64>, value) {
    scores.push(4)      // statically compatible
    scores.push(value)  // dynamic value, guarded before mutation
    return scores
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
