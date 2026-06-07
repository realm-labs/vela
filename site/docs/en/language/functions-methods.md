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
    applied: Int,
}

impl DamageResult {
    fn score(self, bonus) -> Int {
        return self.applied + bonus;
    }
}
```

Traits are available when several types should share a protocol or a default
method body:

```vela
trait DamageSummary {
    fn score(self, bonus) -> Int;
}

impl DamageSummary for DamageResult {}
```

## Host Methods

Rust can register methods on concrete host types. The script syntax is the same:

```vela
player.inventory.grant("gold", 10);
```

The VM resolves the receiver type and method ID, then routes the call through `HostAccess`.
