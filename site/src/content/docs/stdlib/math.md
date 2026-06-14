---
title: "Math"
description: "Math standard library documentation for Vela."
---

The math module provides deterministic numeric helpers. The implemented helpers
accept finite numeric values and return either integers or finite floats
according to the operation. Invalid domains, non-finite values, overflow, or
wrong arity are VM diagnostics.

## Scalar Helpers

Use `max`, `min`, `clamp`, `sign`, `floor`, `ceil`, `round`, and `abs` for
ordinary scalar work.

```vela
fn main() {
    let raw = -12;
    let normalized = math::clamp(math::abs(raw), 0, 10);
    return normalized + math::sign(-3);
}
```

When every argument is an integer and the operation can stay integral, helpers
return an integer. Float inputs generally produce `f64`, while `floor`, `ceil`,
and `round` return `i64`.

## Movement, Distance, And Power

`lerp`, `move_towards`, `distance2d`, `distance3d`, `pow`, and `sqrt` cover
common gameplay and simulation formulas without making them domain-specific.

```vela
fn main() {
    let step = math::move_towards(0, 10, 3);
    let distance = math::distance2d(0, 0, 3, 4);
    let root = math::sqrt(81);
    return step + math::round(distance) + math::round(root);
}
```

`move_towards` rejects negative deltas. `sqrt` rejects negative inputs. `pow`
uses checked integer power for non-negative integer exponents and finite float
power otherwise.

## Numeric Conversion Helpers

Explicit conversion helpers live in standard modules named after primitive
types. Widening helpers are infallible; narrowing helpers return `Result`.

```vela
fn main() {
    let wide = i64::from_i32(12);
    let narrow = u8::try_from_u64(255).unwrap_or(0);
    return wide + narrow;
}
```

Wrapping and bit helpers are explicit functions such as `u8::wrapping_add`,
`u8::bit_and`, `u8::shift_left`, and `u8::rotate_right`. Arithmetic operators
do not imply wrapping behavior.

`math::random` is documented separately because it is only installed when the
host enables controlled random support.
