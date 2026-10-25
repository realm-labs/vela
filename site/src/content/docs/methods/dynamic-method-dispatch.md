---
title: "Dynamic Method Dispatch"
description: "The synchronous, script-owned Vela boundary."
---

Dynamic values can call source-static method names without monkey patching. Known receivers use resolved dispatch; unknown receivers use bounded runtime resolution of ordinary value and script impl methods.

```vela
fn length(value) -> i64 {
    return value.len();
}
```

Arguments, defaults, type checks, source-spanned missing-method errors and budgets remain. Method caches belong to the exact executable generation.

Removed Host construction/method registrations and transparent Rust receiver paths do not participate in dispatch. Script collections, source structs and traits remain supported.
