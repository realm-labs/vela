---
title: "HostRef, HostPath, PathProxy"
description: "The handles and path objects Vela uses instead of Rust references."
---

`HostRef`, `HostPath`, and `PathProxy` are the core host-state handles. They
let scripts address Rust-owned state without borrowing it.

## HostRef

`HostRef` identifies a host object by type, object ID, and generation.

```rust
pub struct HostRef {
    pub type_id: HostTypeId,
    pub object_id: HostObjectId,
    pub generation: u32,
}
```

The generation prevents stale references from writing to a different object
after an ID is reused.

## HostPath

`HostPath` is a materialized, readable path used for diagnostics, reflection,
fixtures, and embedding APIs.

```rust
let path = HostPath::new(player_ref).field(FieldId::new(1));
```

Hot bytecode normally stores an interned `HostTargetPlan` instead of building
`HostPath` on every access.

## PathProxy

`PathProxy` stores a root `HostRef`, a target plan, and dynamic index/key
arguments. It is useful when a host method or native function needs to carry a
nested host target without exposing Rust references.

```rust
let proxy = PathProxy::new(player_ref, plan)
    .key("gold")
    .field(FieldId::new(2));
proxy.add(
    adapter,
    &mut access,
    HostValue::Scalar(vela_common::ScalarValue::I64(1)),
    None,
)?;
```

The proxy still routes through `HostAccess`, so schema, capability, generation,
and adapter checks remain active.
