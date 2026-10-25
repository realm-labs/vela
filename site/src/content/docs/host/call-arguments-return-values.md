---
title: "Call Arguments And Return Values"
description: "The synchronous, script-owned Vela boundary."
---

Calls use `CallArgs` and `CallOptions`. Existing scalar/owned-value, named/positional, same-runtime `VelaValue`, and serde snapshot arguments remain. Borrowed Rust roots, arguments, returns, views and host handles are not accepted.

```rust
let output = runtime.call(
    "main",
    CallArgs::new().with_value("amount", 5_i64),
    CallOptions::new(10_000, 1024 * 1024, 64),
)?;
let result: i64 = runtime.from_value(&output)?;
```

`Runtime::call` returns a runtime-managed script value. `value_to_owned` explicitly obtains a detached result; same-runtime values may be passed back with `with_vela_value`. Runtime affinity, argument conversion, permissions and budgets still apply.

Serialized inputs are script-owned snapshots. Script changes do not mutate the original Rust value. See the retained `serde_value` and `script_state` examples.
