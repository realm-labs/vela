---
title: "Context"
description: "Context standard library documentation for Vela."
---

`Context` is a standard host object schema, not ordinary script-owned state.
Hosts register it when scripts need deterministic time fields, event emission,
or logging. Scripts receive a call-scoped `HostRef`; they never hold a real
Rust mutable reference.

## Fields

The standard context schema exposes `now: i64` and `tick: i64`. These values
come from the host and match the deterministic time model.

```vela
fn main(ctx: Context) {
    let stamp = ctx.now + ctx.tick;
    return stamp;
}
```

Field reads still go through `HostAccess`, so host read permissions and stale
host reference checks apply.

## Events And Logs

`ctx.emit(event, payload?)` records an event emission patch for the host safe
point. `ctx.log(level, message, payload?)` records a log patch. Both are host
methods with event effects.

```vela
fn main(ctx: Context, player: Player) {
    let stamp = ctx.now + ctx.tick;
    ctx.emit("player.level_checked", stamp);
    ctx.log("info", "player.level_checked", stamp);
    return stamp;
}
```

The payload parameter is optional and can be any script value that the host
adapter accepts.

## Capability Boundary

The host must register the context schema and grant the relevant host read,
host call, and event capabilities. Denied context calls are runtime diagnostics,
not `Result::Err`, because no event was accepted by the host boundary.

Context is intentionally domain-neutral. Game-specific event names, payload
shapes, and host types come from the embedding application.
