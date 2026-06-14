---
title: "Core Concepts"
description: "Core concepts documentation for Vela."
---

This page defines the terms used throughout the documentation. Vela has a small language surface, but its host boundary is deliberate.

## Engine, Program, Runtime

`Engine` owns registration and policy: host types, native functions, standard natives, capability profiles, reflection permissions, and compiler options. Source is compiled through the engine into a program.

`Runtime` owns execution state for one running program version. Calls enter the runtime through named entries or cached entry handles, pass values through `CallArgs`, and are bounded by `CallOptions` such as instruction budget, memory budget, and call depth.

```rust
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .register_script_host::<Player>()
    .build()?;
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);
```

## Script Values And Host State

Script-owned values include primitives, arrays, maps, sets, strings, records, enums, closures, and iterator values managed by the VM. Host-owned values remain in Rust. When script code receives a host object, it receives a controlled handle, not ownership of the Rust object.

Host writes are immediate write-through operations. A script expression such as `player.inventory.items["gold"].count += amount` is lowered into a host path operation that the host adapter can validate, reject, or apply.

## Capabilities And Budgets

Capabilities describe what effects a runtime may perform, such as host read, host write, host call, random, time, I/O read, or I/O write. Budgets bound execution so script code cannot run forever or grow memory without limits.

The host decides these policies when building the engine or runtime profile. The same script can succeed in one profile and fail in another if it attempts a denied effect.

## Hot Reload Boundary

Hot reload replaces code at function or module boundaries after compatibility checks. The runtime preserves old code for frames that are already executing, then routes new calls to the accepted version.

Schema and ABI compatibility matter. Changes that would invalidate active host bindings, field IDs, method IDs, effects, or callable signatures can be rejected with diagnostics instead of partially applied.
