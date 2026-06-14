---
title: "Engine API Reference"
description: "A high-level reference for the Rust embedding API."
---

The Rust API is the primary embedding surface. This page is a stable overview,
not a generated API reference. Consult crate docs and source for exact
signatures while the project is pre-release.

## Engine Builder

`Engine::builder()` configures host types, native functions, standard natives,
capabilities, reflection policy, compiler options, and hot reload policy.

```rust
let engine = Engine::builder()
    .with_standard_natives()
    .capability(Capability::Time)
    .build()?;
```

## Compilation And Runtime

Engines compile files, source strings, modules, program images, and hot reload
versions. A `Runtime` owns execution state and calls script entries with
`CallArgs` and `CallOptions`.

```rust
let program = engine.compile_file(path)?;
let mut runtime = Runtime::new(engine, program);
let value = runtime.call("main", CallArgs::new(), CallOptions::new(10_000, 1024 * 1024, 64))?;
```

## Host Boundary

Host state is registered through schemas, host refs, native functions, and
adapters. Scripts never receive Rust `&mut T`; mutation is represented through
`HostRef`, `HostPath`, `PathProxy`, `HostAccess`, and `ScriptStateAdapter`.

## Values And Handles

Embedding code can use owned values for detached snapshots and runtime-managed
value handles for same-runtime reuse. Host capabilities and execution budgets
should be configured explicitly for production runtimes.
