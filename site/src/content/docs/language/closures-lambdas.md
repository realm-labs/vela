---
title: "Closures And Lambdas"
description: "Closure and lambda documentation for Vela."
---

Lambdas create function values that can be passed to standard library helpers, stored in script values, or called later inside the same runtime. They are ordinary script closures, not host threads or async tasks.

## Syntax

Lambda parameters are written between `|` delimiters. The body can be an expression or a block, and parameters may carry type hints.

```vela
fn add_one(values) {
    return values.iter()
        .map(|value: i64| value + 1)
        .collect_array()
}
```

## Captures

Closures capture script-visible values from their surrounding scope. Captured host values remain host references or path proxies; a closure does not turn host state into GC-owned script memory or expose Rust `&mut T`.

```vela
fn above(limit: i64, values) {
    return values.iter()
        .filter(|value| value > limit)
        .collect_array()
}
```

## Callback Use

The core standard-library callback sites are iterator adapters such as `map`, `filter`, `any`, `all`, and `find`. Lazy adapters are one-shot and consume their source when a terminal method such as `collect_array()` or `count()` runs.

## Runtime Boundaries

Closure execution is budgeted like other script calls. The MVP does not promise coroutine suspension, async hot reload, or moving closures across unrelated runtime instances.
