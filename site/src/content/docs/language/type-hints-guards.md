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
`Any` means the value is intentionally dynamic.

```vela
struct Reward {
    code: String
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

## Builtin Container Contracts

Selected builtin contracts can carry type arguments:

```vela
fn total(values: Array<i64>) -> i64 {
    let sum = 0
    for value in values {
        sum += value
    }
    return sum
}

fn grant(rewards: Map<String, i64>, tags: Set<String>) -> Result<i64, String> {
    rewards.set("tag_count", tags.len())
    return result::ok(rewards.get("xp").unwrap_or(0))
}
```

Allowed parameterized contracts are `Array<T>`, `Set<T>`,
`Map<String, V>`, `Iterator<T>`, `Option<T>`, and `Result<T, E>`.
`Array<Any>`, `Map<String, Any>`, and `Option<Any>` erase the inner contract.

These are contracts, not conversions. A mixed array passed to `Array<i64>`
fails at the checked boundary instead of being converted.

## Not Script Generics

The language still rejects user or schema generic syntax such as `Player<T>`,
`String<T>`, `Map<i64, V>`, and `Function<T>`. Type arguments are reserved for
the builtin contracts above and do not create monomorphized script functions or
generic user-defined types.

`Iterator<T>` contracts validate the outer iterator at checked boundaries
without consuming the cursor. Non-erased item contracts are enforced lazily as
the iterator yields values through `next()`, `for`, or terminal methods.
`Iterator<Any>` and erased `Iterator` remain ordinary outer iterator contracts.

## Hot Reload And Host Metadata

Hints are part of public script and host expectations. Changing a function
signature, field hint, host schema, or exported return hint can affect hot
reload compatibility and may be rejected until callers and host registrations
agree.
