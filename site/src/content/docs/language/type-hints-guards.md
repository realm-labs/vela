---
title: "Type Hints And Guards"
description: "Type hint and runtime guard documentation for Vela."
---

Type hints describe runtime contracts and metadata. They support diagnostics, reflection, host schemas, hot reload compatibility, and selected fast paths, but they are not static generics and do not monomorphize script code.

## Hint Locations

Hints can appear on parameters, return values, locals, globals, struct fields, enum fields, and lambda parameters. Missing hints leave the value dynamic. `any` is explicit erased metadata and creates no contract by itself.

```vela
struct Reward {
    code: string
    amount: i64 = 0
}

fn grant(player, reward: Reward) -> i64 {
    player.gold += reward.amount
    return player.gold
}
```

## Guards

When the compiler can prove a hint, the call or write can use an unchecked path. When a dynamic value flows into a hinted boundary, Vela inserts a runtime guard. A failed contract guard is a language error with source location, not a cache miss.

```vela
fn double(value: i64) -> i64 {
    return value * 2
}

fn call_dynamic(value) -> i64 {
    return double(value) // checked at the function boundary
}
```

## Not Generics

The language deliberately rejects script generic syntax such as `Array<T>`, `Map<K, V>`, `Option<T>`, and `Result<T, E>`. Containers are dynamic values; element expectations should be checked at API boundaries or by explicit code.

## Hot Reload And Host Metadata

Hints are part of public script and host contracts. Changing a function signature, field hint, host schema, or exported return hint can affect ABI compatibility during hot reload and may be rejected until callers and host registrations agree.
