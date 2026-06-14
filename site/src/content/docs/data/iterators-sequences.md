---
title: "Iterators And Sequences"
description: "Iterators And Sequences documentation for Vela."
---

Vela uses one iteration model across arrays, maps, sets, ranges, strings, and host-returned iterables. The model separates repeatable sources from one-shot cursors.

## Iterable, Sequence, Iterator

An iterable can create or provide an iterator. A sequence is repeatable and creates a fresh iterator for each traversal. An iterator is a one-shot cursor; `next()` advances the same state future calls observe.

```vela
fn first_two(values) {
    let iter = values.iter()
    let first = iter.next()
    let second = iter.next()
    return [first, second]
}
```

## For-In

`for value in source` evaluates `source` once, gets an iterator, and advances it until completion. `for index, value in source` is syntax-level indexed loop lowering.

```vela
fn total(values) -> i64 {
    let sum = 0
    for index, value in values {
        sum += value + index
    }
    return sum
}
```

## Lazy Adapters

Methods such as `map`, `filter`, `take`, and `skip` are lazy and one-shot. Terminal methods such as `count`, `any`, `all`, `find`, and `collect_array` consume the cursor.

```vela
fn active_codes(items) {
    return items.iter()
        .filter(|item| item.active)
        .map(|item| item.code)
        .collect_array()
}
```

## Host Iterables

Hosts may return snapshot iterables, but host-owned state is not placed under the script GC. Any later host mutation still uses HostAccess or an explicit native function boundary.
