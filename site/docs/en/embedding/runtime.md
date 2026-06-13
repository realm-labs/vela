# Runtime API

Rust embedding follows a small sequence: build an engine, compile source, create a runtime, call script entries.

```rust
let engine = EngineBuilder::new()
    .with_standard_natives()
    .capability(Capability::Random)
    .with_controlled_random(7)
    .build()?;

let program = engine.compile_source(SourceId::new(1), source)?;
let mut runtime = Runtime::new(engine, program);
let result = runtime.call(
    "main",
    CallArgs::new(),
    CallOptions::new(250_000, 8 * 1024 * 1024, 128),
)?;
```

Use `Runtime::try_new(engine, program)?` instead when the source comes from an
untrusted or interactive boundary and link errors should be reported instead of
treated as an internal invariant failure.

## Call Handles

High-frequency embedders can cache `VelaFunction` handles:

```rust
let handle_tick = runtime.entry("handle_tick")?;
runtime.call(&handle_tick, args, options)?;
```

The handle remains tied to the runtime and participates in hot reload version checks.

## Return Values

`Runtime::call` returns a `VelaValue`, which can be passed back into later calls without first detaching it into an owned snapshot. Rust can convert it to `OwnedValue` or deserialize it through serde when needed.
