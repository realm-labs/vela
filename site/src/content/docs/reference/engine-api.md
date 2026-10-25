---
title: "Engine API Reference"
description: "The synchronous, script-owned Vela boundary."
---

The Rust API is the primary embedding surface. Consult the current crate documentation for exact signatures.

`Engine::builder()` configures independent native/value registrations, standard natives, capabilities and compiler options. An Engine compiles source, files and ordinary packages. A `Runtime` owns script execution and state.

```rust
let engine = Engine::builder().with_standard_natives().build()?;
let program = engine.compile_source("fn main() -> i64 { return 7; }")?;
let mut runtime = Runtime::new(engine, program)?;
let value = runtime.call("main", CallArgs::new(), CallOptions::new(10_000, 1024 * 1024, 64))?;
let result: i64 = runtime.from_value(&value)?;
```

Value conversion and serde snapshots produce script-owned data, with no implicit Rust write-back. Ordinary VM state and exact-generation script reload remain. Borrowed host bindings, Host methods/constructors, transparent paths, extern state and script async/tasks are removed. No replacement object API is introduced.
