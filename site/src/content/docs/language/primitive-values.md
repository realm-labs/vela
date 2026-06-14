---
title: "Primitive Values"
description: "Primitive Values documentation for Vela."
---

Vela is dynamically typed, but primitive values use explicit concrete runtime tags. This keeps host conversion, reflection, bytecode guards, and future optimization precise without introducing script-language generics.

## Primitive Set

The primitive value categories are `null`, `bool`, `char`, signed integers `i8` through `i64`, unsigned integers `u8` through `u64`, floats `f32` and `f64`, `string`, and `bytes`.

```vela
let enabled = true
let letter = 'A'
let count = 12i64
let ratio = 0.25f64
let label = "ready"
let packet = b"\x01\x02"
```

## Numeric Literals

Unsuffixed integer literals default to `i64` when no more specific context exists. Unsuffixed float literals default to `f64`. A hinted parameter, field, or local can context-type a literal, but operators do not perform implicit widening or integer-to-float conversion.

```vela
fn add_i32(lhs: i32, rhs: i32) -> i32 {
    return lhs + rhs
}

fn main() -> i32 {
    return add_i32(1, 2) // literals are checked as i32 here
}
```

## Null

`null` means no meaningful value, a statement-only block result, or nullable host/metadata interop. Prefer `Option` for expected absence and `Result` for recoverable failure so APIs do not overload `null` with too many meanings.

```vela
fn maybe_message(enabled: bool) {
    if enabled {
        return "enabled"
    }
    return null
}
```

## Boundary Rules

Primitive hints are contracts, not conversions. `1i32 + 2i64` is an error when statically known, and the same mismatch is a runtime error when values are dynamic. Checked integer overflow is also an error; explicit wrapping and conversion helpers belong to the standard library.
