---
title: "Variables And Constants"
description: "Variables And Constants documentation for Vela."
---

Vela has local variables, module constants, and host/runtime globals. The language is dynamic by default: an unhinted binding records the value it currently holds, while a hinted binding adds a runtime contract.

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

## Globals And Host State

`global` declares a named value provided by the runtime or host embedding layer. Scripts can read and write compatible globals, but host-owned state still goes through the HostAccess model; scripts never receive real Rust `&mut T` references.

```vela
global player: Player

fn level_up() {
    player.level += 1
}
```

## Common Errors

Assigning a value that violates a binding, field, parameter, return, or global contract raises a type contract diagnostic. Reusing constants as mutable storage and using `global` as a way to bypass host permissions are both rejected by the runtime boundary.
