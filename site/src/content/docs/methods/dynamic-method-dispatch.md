---
title: "Dynamic Method Dispatch"
description: "Dynamic Method Dispatch documentation for Vela."
---

Dynamic method dispatch lets ordinary dynamic values call source-static method names when the receiver type is not known at compile time. It is controlled, registry-backed dispatch, not monkey patching.

## Compile-Time Split

If the receiver type is known and the method exists, the compiler emits resolved method dispatch. If the receiver is unknown, it emits dynamic dispatch with the source method name and original argument information.

```vela
fn length(value) -> i64 {
    return value.len()
}
```

## Resolution Order

At runtime, Vela classifies the receiver and resolves in a fixed order: standard value methods, script impl methods, host methods, then a source-spanned missing-method error.

```vela
fn starts_with_q(value) -> bool {
    return value.starts_with("q")
}
```

## Arguments And Guards

Dynamic bytecode preserves positional and named arguments until the target is known. After resolution, the runtime materializes the target signature, fills defaults where supported, and runs type or host conversion guards.

## Cache Boundary

Dynamic method caches are guarded by method name, receiver classification, and relevant program or host schema epochs. A cache miss falls back to resolution; it is not a language error by itself.

## Host Safety

Dynamic host method dispatch still goes through `HostRef`, `HostPath`, `PathProxy`, HostAccess, registered metadata, capability checks, and generation checks. It cannot bypass the host mutation model.
