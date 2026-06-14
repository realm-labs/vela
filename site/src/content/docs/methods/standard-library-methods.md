---
title: "Standard Library Methods"
description: "Standard Library Methods documentation for Vela."
---

Standard library methods provide the built-in behavior for primitive and collection values. They are receiver-dispatched like script and host methods, but their targets are registered by the runtime.

## Value Families

Standard methods currently cover strings, bytes, arrays, maps, sets, ranges, iterators, `Option`, and `Result`, plus selected numeric conversion helpers.

```vela
fn summarize(name: string, values) -> string {
    let total = values.iter().count()
    return f"{name}:{total}"
}
```

## Collections And Iterators

Collection methods expose explicit views and lazy adapters. `collect_array()` is the standard terminal for materializing iterator output.

```vela
fn doubled(values) {
    return values.iter()
        .filter(|value| value > 0)
        .map(|value| value * 2)
        .collect_array()
}
```

## Option And Result Helpers

`Option` and `Result` helpers keep absence and recoverable failure visible without turning expected cases into VM traps.

```vela
fn safe_amount(text: string) -> i64 {
    return text.parse_i64().unwrap_or(0)
}
```

## Dispatch And Compatibility

Standard methods use stable IDs internally where possible, with dynamic dispatch available for unknown receivers. Adding or changing standard methods must preserve the same host boundary, budget, and hot reload compatibility rules as other callable targets.
