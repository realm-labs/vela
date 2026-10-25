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

Closures capture script-owned values from their surrounding scope, including values produced by existing Rust value conversions. Capturing a value does not create an implicit Rust write-back path. Caller-borrowed Rust data cannot be captured or retained by a script closure.

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

Closure execution is synchronous and budgeted like other script calls. Active
calls and retained closures keep their exact original executable generation
during hot reload, including nested dispatch and state. Closures cannot move
across unrelated Runtime instances or schedule background work.
