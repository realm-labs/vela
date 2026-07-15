---
title: "Host Object Lifetime"
description: "Object identity, generations, and lifetime boundaries for host-owned state."
---

Host objects are owned by Rust. Vela stores handles to them, not the objects
themselves.

## Call-Scoped Handles

`CallArgs::with_host_ref` and `CallArgs::with_host_mut` bind a Rust value for
one call. The VM sees a `HostRef` handle and a call-local adapter binding.

```rust
runtime.call(
    "main",
    CallArgs::new().with_host_mut("player", &mut player),
    CallOptions::unbounded(),
)?;
```

When the call returns, the direct binding is gone. Any durable state remains in
Rust, not in the script heap.

## Persistent Extern State

An `extern state` binding stores a persistent host object owned by Rust. The
object must be `Send` because a runtime can move to a worker thread.

```rust
let runtime = Runtime::builder(engine, program)?
    .bind_extern_state("main::player", player)?
    .build()?;
```

VM `state` is different: its records, arrays, maps, sets, enums, and scalars
are rooted and traced by the Runtime's script heap.

## Stale References

`HostRef` includes a generation. If an object slot is reused after a host object
is removed or replaced, the adapter can reject a stale handle instead of
silently writing to the wrong object. Rejection is a runtime diagnostic, not a
best-effort fallback.
