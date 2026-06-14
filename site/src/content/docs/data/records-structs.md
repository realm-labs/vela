---
title: "Records And Structs"
description: "Records And Structs documentation for Vela."
---

Struct declarations define script-owned record shapes. Records are dynamic values with named fields, optional defaults, and optional field contracts. They are separate from host objects, even when they model the same business concept.

## Declaration And Construction

Fields may have type hints and default values. A record literal uses the type path followed by named fields.

```vela
struct Reward {
    code: String
    amount: i64 = 0
}

fn default_reward() -> Reward {
    return Reward { code: "xp", amount: 10 }
}
```

## Field Access And Mutation

Script record fields use dot access and can be assigned when the value satisfies the field contract. Missing field names and incompatible writes are diagnostics.

```vela
fn boost(reward: Reward) -> Reward {
    reward.amount += 5
    return reward
}
```

## Structs Versus Host Types

A script struct is GC-managed script data. A registered host type is Rust-owned state accessed through `HostRef`, `HostPath`, `PathProxy`, and HostAccess. The same dot syntax can appear at source level, but the runtime boundary is different.

## Reflection And Reload

Field names, hints, defaults, and attributes are reflected metadata and are part of schema compatibility. Hot reload may accept compatible additions but should reject changes that would invalidate existing active frames or host contracts.
