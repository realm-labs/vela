# Functions And Methods

Functions are declared with `fn` and are called by name. Methods can be script methods on script types or host methods registered by Rust.

## Functions

```vela
fn add(left, right) {
    return left + right;
}

fn main() {
    return add(20, 22);
}
```

## Script Methods

```vela
struct DamageResult {
    applied: int,
}

impl DamageResult {
    fn score(self, bonus) -> int {
        return self.applied + bonus;
    }
}
```

Traits are available when several types should share a protocol or a default
method body:

```vela
trait DamageSummary {
    fn score(self, bonus) -> int;
}

impl DamageSummary for DamageResult {}
```

## Host Methods

Rust can register methods on concrete host types. The script syntax is the same:

```vela
player.inventory.grant("gold", 10);
```

The VM resolves the receiver type and method ID, then routes the call through `HostAccess`.

## Dynamic Receiver Calls

If the compiler knows the receiver type, existing methods use the linked stable
ID fast path and provably missing methods can be compile-time errors. If the
receiver type is unknown, a source-static method call still compiles and links:

```vela
fn starts_with_q(value) {
    return value.starts_with("q");
}
```

At runtime the VM resolves the method from the actual receiver. Strings,
script values, and registered host refs can all dispatch this way. A receiver
that does not support the method raises a source-spanned runtime error.

Dynamic script methods support named arguments and defaults after the target is
resolved:

```vela
fn wrapped(value) {
    return value.wrap(suffix = "}", prefix = "{");
}
```
