---
title: "Native Functions"
description: "Registering Rust functions as Vela callable functions with explicit effects and conversions."
---

Native functions expose Rust functionality to scripts without exposing Rust
state pointers. They are registered with stable names, signatures, effects,
and reflection access metadata.

## Pure Native Functions

Pure functions receive copied script values and return copied script values.

```rust
fn bonus_manual(amount: i64, multiplier: i64) -> i64 {
    amount * multiplier
}

let engine = Engine::builder()
    .register_typed_native_fn::<(i64, i64), _>(
        NativeFunctionDesc::new("game::bonus_manual", NativeFunctionId::new(10_001))
            .param("amount", TypeHint::i64())
            .param("multiplier", TypeHint::i64())
            .returns(TypeHint::i64())
            .effects(EffectSet::pure())
            .access(FunctionAccess::public().reflect_callable(true)),
        bonus_manual,
    )
    .build()?;
```

```vela
fn main() {
    return game::bonus_manual(3, 4);
}
```

## Context Native Functions

Functions that need host services use `NativeCallContext`. This is the allowed
entry point for capabilities, budgets, and `HostAccess`.

```rust
#[script_context_function(name = "game::grant_level", effect = "write_host")]
fn grant_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    amount: i64,
) -> vela_vm::error::VmResult<i64> {
    let path = Player::vela_field_path_level(player);
    ctx.add_path(
        path.clone(),
        HostValue::Scalar(vela_common::ScalarValue::I64(amount)),
        None,
    )?;
    match ctx.read_path(&path, None)? {
        HostValue::Scalar(vela_common::ScalarValue::I64(level)) => Ok(level),
        _ => Ok(0),
    }
}
```

## Rules

Native functions are not overloaded. Use one public name per callable. If a
native needs to mutate host state, it must go through `NativeCallContext`,
`HostAccess`, a registered adapter method, or return a value that script code
later writes through a normal host path.
