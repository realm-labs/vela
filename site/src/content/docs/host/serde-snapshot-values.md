---
title: "Serde Snapshot Values"
description: "Using serde to pass copied script-owned snapshots across the host boundary."
---

Serde integration is for snapshot values. It copies Rust data into script-owned
records, arrays, maps, enums, strings, bytes, and scalars. It is not a host
object reference model.

## Passing Snapshots

```rust
#[derive(Serialize, Deserialize)]
struct DamageEvent {
    actor: DamageActor,
    amount: i64,
    multiplier: i64,
    reason: String,
}

let args = CallArgs::new().with_serde_value("event", &event)?;
let output = runtime.call("handle_damage", args, CallOptions::unbounded())?;
```

```vela
fn handle_damage(event: DamageEvent) {
    return DamageResult {
        actor_name: event.actor.name,
        applied: event.amount * event.multiplier + event.actor.level,
        label: event.reason,
    };
}
```

Changing `event` in the script would not mutate the original Rust struct,
because the script owns a copied value.

## Returning Snapshots

Deserialize a result with `from_value`.

```rust
let result: DamageResult = runtime.from_value(&output)?;
```

The returned value may also remain a `VelaValue` when the host wants to call a
script method on it or pass it back into the same runtime.

```rust
let score_method = runtime.method(&output, "score")?;
let score = runtime.call_method(
    &output,
    &score_method,
    CallArgs::new().with_value("bonus", 5_i64),
    CallOptions::unbounded(),
)?;
```

## When To Use HostRef Instead

Use serde when copying is the intended behavior: events, config snapshots,
request payloads, and return DTOs. Use `HostRef` and `HostAccess` when script
writes must update durable Rust state immediately.
