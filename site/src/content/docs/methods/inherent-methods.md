---
title: "Inherent Methods"
description: "Inherent Methods documentation for Vela."
---

Inherent methods are methods declared directly for a script type. They are receiver-dispatched and compile to stable method metadata when the receiver type is known.

## Declaration

Use `impl Type { ... }` to attach methods to a script struct or enum. The first parameter is conventionally `self` and receives the value being called.

```vela
struct Player {
    level: i64
}

impl Player {
    fn bonus(self, amount: i64) -> i64 {
        return self.level + amount
    }
}
```

## Calls

Method call syntax is `receiver.method(args...)`. If the compiler knows the receiver type, it links to the resolved method. If the receiver is dynamic, runtime method dispatch resolves the target from the actual value.

```vela
fn main(player: Player) -> i64 {
    return player.bonus(5)
}
```

## No Overloading

A type cannot define multiple methods with the same receiver/name pair. Parameter hints, defaults, and arity do not create overload sets.

## Host Boundary

Registered host types can also expose methods, but those calls execute through HostAccess and registered host metadata. Script methods never expose a Rust `&mut T` to script code.
