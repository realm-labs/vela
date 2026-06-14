---
title: "Functions"
description: "Functions documentation for Vela."
---

Functions are the primary unit of execution, embedding calls, reflection, and hot reload. A source file or module can expose public functions with `pub`, and the host can call selected entries by name.

## Declaration

Parameters may be hinted and may have defaults. A return hint adds a checked return contract. Vela does not overload functions by arity, hints, defaults, or native signature; one scope has one function for a given name.

```vela
pub fn grant(player, amount: i64 = 1) -> i64 {
    player.gold += amount
    return player.gold
}
```

## Calls And Arguments

Calls support positional arguments and named arguments. After the target is known, named arguments are matched by parameter name and defaults are filled from the function signature.

```vela
fn scale(value: i64, multiplier: i64 = 2, offset: i64 = 0) -> i64 {
    return value * multiplier + offset
}

fn main() -> i64 {
    return scale(10, offset = 5)
}
```

## Host Boundary

Host calls into script functions use the checked entry path. That means parameter guards, return guards, capability checks, and call-stack diagnostics are preserved. Scripts still cannot expose real Rust references; host mutation happens through registered host values and HostAccess.

## Hot Reload

Function code is reloadable at function or module granularity. Existing frames continue running old code, while new calls enter the new code after ABI and schema compatibility checks pass.
