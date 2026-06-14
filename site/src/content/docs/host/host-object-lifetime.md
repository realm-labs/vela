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

## Persistent Globals

Runtime globals may store persistent host objects when inserted by Rust. Those
objects must be `Send` because a runtime can be moved to a worker thread.

```rust
let player_ref = runtime.insert_host_global("main::player", player);
```

Script-value globals are different: they are VM-managed records, arrays, maps,
sets, enums, and scalars rooted by the runtime.

## Stale References

`HostRef` includes a generation. If an object slot is reused after a host object
is removed or replaced, the adapter can reject a stale handle instead of
silently writing to the wrong object. Rejection is a runtime diagnostic, not a
best-effort fallback.
