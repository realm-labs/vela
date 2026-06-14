---
title: "Core Concepts"
description: "Core concepts documentation for Vela."
---

This page defines the terms used throughout the documentation. It starts with
the embedding model because Vela is designed to run inside a Rust host, not as
a standalone shell-only language.

The shortest mental model is:

```text
Engine  = configured compiler and host registry
Program = compiled Vela code
Runtime = mutable execution instance for a Program
```

## Engine, Program, Runtime

An `Engine` is the long-lived object that knows what the host has made
available to scripts. It owns host type registrations, native functions,
standard library natives, capability profiles, reflection permissions, and
compiler options. If a script mentions `Player.level`, the engine is where that
host schema was registered.

A `Program` is compiled Vela code. It is the result of compiling one source
file, a set of module sources, or a source directory through an `Engine`. A
program contains bytecode, metadata, stable IDs, cache sites, and entry points,
but it is not itself "running" yet.

A `Runtime` is where a program actually executes. It owns the mutable VM state
for one active program version: script heap, globals, inline caches, execution
budgets, and hot reload state. Calls enter the runtime through named entries or
cached entry handles, pass values through `CallArgs`, and are bounded by
`CallOptions` such as instruction budget, memory budget, and call depth.

In ordinary embedding code the flow is:

```text
build Engine -> compile Program -> create Runtime -> call script entries
```

```rust
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .register_script_host::<Player>()
    .build()?;
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);
```

Keep the boundaries separate:

- Change host registrations or policy before compiling by configuring the
  `Engine`.
- Change script code by compiling a new `Program`.
- Change running script state by calling or hot-reloading the `Runtime`.

## Script Values And Host State

Script-owned values include primitives, arrays, maps, sets, strings, records, enums, closures, and iterator values managed by the VM. Host-owned values remain in Rust. When script code receives a host object, it receives a controlled handle, not ownership of the Rust object.

Host writes are immediate write-through operations. A script expression such as `player.inventory.items["gold"].count += amount` is lowered into a host path operation that the host adapter can validate, reject, or apply.

## Capabilities And Budgets

Capabilities describe what effects a runtime may perform, such as host read, host write, host call, random, time, I/O read, or I/O write. Budgets bound execution so script code cannot run forever or grow memory without limits.

The host decides these policies when building the engine or runtime profile. The same script can succeed in one profile and fail in another if it attempts a denied effect.

## Hot Reload Boundary

Hot reload replaces code at function or module boundaries after compatibility checks. The runtime preserves old code for frames that are already executing, then routes new calls to the accepted version.

Schema and ABI compatibility matter. Changes that would invalidate active host bindings, field IDs, method IDs, effects, or callable signatures can be rejected with diagnostics instead of partially applied.
