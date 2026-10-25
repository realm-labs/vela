---
title: "Traits And Trait Methods"
description: "Traits And Trait Methods documentation for Vela."
---

Traits are runtime protocols. They describe methods a script type can implement
and support dynamic protocol-style dispatch without becoming Rust traits in
script syntax. They do not provide Rust host-object trait receivers or method
registrations.

## Trait Declaration

Trait methods can be required signatures or default methods. Type hints on trait methods are runtime contracts and reflection metadata.

```vela
trait BonusSource {
    fn bonus(self, amount: i64) -> i64 {
        return amount
    }
}
```

## Implementations

Use `impl Trait for Type` to implement a protocol for a script type. An explicit method overrides the trait default.

```vela
struct Player {
    level: i64
}

impl BonusSource for Player {
    fn bonus(self, amount: i64) -> i64 {
        return self.level + amount
    }
}
```

## Dispatch

Trait method calls are still receiver-dispatched. Known receiver calls can use linked method IDs; dynamic calls resolve through the runtime receiver classification and registry-backed metadata.

## Boundaries

Traits do not allow monkey patching or runtime mutation of type structure. Ordinary script traits retain capability checks and exact-generation hot reload compatibility.
