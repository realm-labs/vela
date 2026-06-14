---
title: "Random"
description: "Random standard library documentation for Vela."
---

Randomness is opt-in. The current standard surface is a controlled
`math::random(min, max)` function installed by the host with a deterministic
seed. It returns an `i64` in the inclusive range `[min, max]`.

## Controlled Random

The host installs the function with controlled random support. Scripts use it
through the `math` module.

```vela
fn main() {
    let first = math::random(1, 6);
    let second = math::random(10, 12);
    if first >= 1 && first <= 6 && second >= 10 && second <= 12 {
        return first * 100 + second;
    }
    return 0;
}
```

The function rejects non-integer bounds and `min > max` as VM diagnostics.

## Capability And Determinism

`math::random` carries the `random` effect. A host may register the function so
the script compiles, then deny execution through capabilities.

```vela
fn main() {
    let roll = math::random(1, 20);
    return roll >= 10;
}
```

For deterministic tests and replay, use the same seed and call order. Scripts
should not assume any particular algorithm or sequence beyond deterministic
host policy for a given engine configuration.
