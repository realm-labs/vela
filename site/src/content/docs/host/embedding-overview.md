---
title: "Embedding Overview"
description: "How a Rust host embeds Vela while keeping durable state behind safe host boundaries."
---

Vela is embedded as a Rust-owned scripting runtime. The script describes
business logic, while the host owns durable state, services, IO, scheduling,
and deployment policy.

## Embedding Shape

A typical host creates an `Engine`, compiles source into a program, creates a
`Runtime`, and calls named script entries with explicit arguments.

```rust
use vela_engine::prelude::*;

let engine = Engine::builder()
    .execution_profile(ExecutionProfile::embedded())
    .register_script_host::<Player>()
    .build()?;
let program = engine.compile_source(source)?;
let mut runtime = Runtime::new(engine, program);

let result = runtime.call(
    "handle_tick",
    CallArgs::new().with_host_mut("player", &mut player),
    CallOptions::new(10_000, 1024 * 1024, 64),
)?;
```

`Engine` is the shared definition surface. `Runtime` is the mutable execution
state: heap, globals, inline caches, and hot-reload image.

## Host Boundary

Scripts never receive real Rust references. A call-boundary `&mut Player`
becomes a call-scoped `HostRef` inside the VM. Field reads, writes, compound
assignments, keyed paths, and host method calls route through `HostAccess`.

```vela
fn handle_tick(player: Player, amount) {
    player.level += amount;
    return player.level;
}
```

This syntax is intentionally ordinary, but the runtime operation is explicit:
read the current host field, compute the new scalar value, and write through
the adapter immediately.

## What The Host Owns

The host owns durable state, object lifetimes, capability profiles, execution
budgets, native services, and hot-reload policy. Script-owned records, arrays,
maps, sets, strings, closures, and iterators live in the Vela heap. Host-owned
objects stay behind `HostRef`, `HostTargetPlan`, `PathProxy`, and
`HostAccess`.
