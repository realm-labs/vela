---
title: "Capabilities And Execution Budgets"
description: "Restricting host effects and bounding Vela execution from the embedding layer."
---

Capabilities decide which host effects a program may use. Budgets decide how
much work one call may perform.

## Capabilities

```rust
let engine = Engine::builder()
    .capability(Capability::HostRead)
    .capability(Capability::HostWrite)
    .build()?;
```

Available capability gates include host read/write, event emit, time, random,
IO read/write, and reflection read/write/call. Native functions and host
methods declare effects; the runtime checks those effects against the active
capability profile.

```rust
let sandboxed = Engine::builder()
    .execution_profile(ExecutionProfile::sandboxed())
    .build()?;
```

## Execution Budgets

`CallOptions` controls instruction count, memory bytes, and call depth.

```rust
let options = CallOptions::new(
    10_000,       // instruction limit
    1024 * 1024,  // memory limit
    64,           // max call depth
);
runtime.call("main", CallArgs::new(), options)?;
```

`CallOptions::unbounded()` is useful for trusted examples and tests, but
production hosts should use explicit limits.

## Denials

Permission denial is a normal runtime diagnostic. For example, reading
`player.level` can be rejected by missing `HostRead`, writing `player.level`
can be rejected by missing `HostWrite`, and `ctx.emit(...)` can be rejected by
a host-call policy.

```vela
fn main(player: Player) {
    player.level += 1; // requires host read and host write access
}
```
