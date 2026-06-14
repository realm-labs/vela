---
title: "Type Hints And Runtime Checks"
description: "How Vela checks values that have type hints."
---

Type hints tell Vela what kind of value a boundary expects. They can make
errors clearer, document host schemas, and help hot reload decide whether a
change is compatible. They are not static generics, and they do not convert a
value from one type to another.

## Hint Locations

Hints can appear on parameters, return values, locals, globals, struct fields,
enum fields, and lambda parameters. Missing hints leave the value dynamic.
`any` means the value is intentionally dynamic.

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

## Runtime Checks

When a value reaches a hinted boundary, Vela checks that the value matches the
hint. If the value has the wrong type, the operation fails with a source-spanned
diagnostic.

```vela
fn double(value: i64) -> i64 {
    return value * 2
}

fn call_dynamic(value) -> i64 {
    return double(value) // fails if value is not an i64
}
```

## Not Generics

The language deliberately rejects script generic syntax such as `Array<T>`, `Map<K, V>`, `Option<T>`, and `Result<T, E>`. Containers are dynamic values; element expectations should be checked at API boundaries or by explicit code.

## Hot Reload And Host Metadata

Hints are part of public script and host expectations. Changing a function
signature, field hint, host schema, or exported return hint can affect hot
reload compatibility and may be rejected until callers and host registrations
agree.
