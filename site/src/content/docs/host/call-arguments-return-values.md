---
title: "Call Arguments And Return Values"
description: "How Rust passes values and host handles into Vela calls and reads results back."
---

Calls use `CallArgs` plus `CallOptions`. Arguments may be positional, named,
script-owned values, runtime-managed `VelaValue` handles, serde snapshots, or
call-scoped host handles.

## Passing Arguments

Use `with_value` for ordinary copied values and `with_host_mut` or
`with_host_ref` for direct host object bindings.

```rust
let output = runtime.call(
    "main",
    CallArgs::new()
        .with_value("amount", 5_i64)
        .with_host_mut("player", &mut player),
    CallOptions::new(10_000, 1024 * 1024, 64),
)?;
```

```vela
fn main(player: Player, amount) {
    player.level += amount;
    return player.level;
}
```

`with_host_ref` is read-only. `with_host_mut` allows writes, but the script
still receives only a `HostRef` handle inside the VM, never Rust `&mut T`.

## Return Values

`Runtime::call` returns a `VelaValue`, a runtime-managed handle. Convert it
when the host needs a detached Rust value.

```rust
let value = runtime.call("score", CallArgs::new(), CallOptions::unbounded())?;
let score: i64 = runtime.from_value(&value)?;
let owned = runtime.value_to_owned(&value)?;
```

Passing a `VelaValue` back into the same runtime avoids materializing an
`OwnedValue`.

```rust
let snapshot = runtime.call("snapshot_state", CallArgs::new(), CallOptions::unbounded())?;
let projected = runtime.call(
    "projected_score",
    CallArgs::new().with_vela_value(snapshot).with(4_i64),
    CallOptions::unbounded(),
)?;
```

## Error Boundaries

Argument names must match the entry signature when using named arguments.
Runtime-managed values belong to their runtime. Host objects must be registered
with schemas before script code can type-check and access them.
