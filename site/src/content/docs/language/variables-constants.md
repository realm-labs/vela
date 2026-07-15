---
title: "Variables And Constants"
description: "Variables And Constants documentation for Vela."
---

Vela has local variables, module constants, VM-owned state, and host-owned extern state. The language is dynamic by default: an unhinted local records the value it currently holds, while a hinted binding adds a runtime contract.

## Local Variables

`let` creates a local binding. A binding may have a type hint, an initializer, or both. Hints are checked contracts; they are not generic types and they do not convert values.

```vela
fn total(base: i64, bonus) -> i64 {
    let adjusted: i64 = base + 10
    let dynamic_bonus = bonus
    return adjusted + dynamic_bonus
}
```

## Constants

`const` declares a module-level value that is computed from constant expressions and cannot be reassigned by script code. Use constants for stable script configuration and names that participate in reflection or hot reload ABI checks.

```vela
pub const START_LEVEL: i64 = 1
const LEVEL_STEP: i64 = 5

fn next_level(current: i64) -> i64 {
    return current + LEVEL_STEP
}
```

## Persistent State

`state` requires a type and initializer and creates one VM-owned cell per Runtime. `extern state` requires a type, forbids an initializer, and must be bound by the host. Scripts can assign VM state roots; an extern root is immutable and only nested mutation is allowed through HostAccess. Scripts never receive real Rust `&mut T` references.

```vela
extern state player: Player;
state level_ups: i64 = 0;

fn level_up() {
    player.level += 1
    level_ups += 1
}
```

## Common Errors

Assigning a value that violates a binding, field, parameter, return, or state contract raises a type contract diagnostic. Extern roots cannot be assigned, VM initializers cannot perform external effects, and constants cannot be reused as mutable storage.
