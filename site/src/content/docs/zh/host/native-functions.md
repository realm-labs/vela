---
title: "Native 函数"
description: "把 Rust 函数注册为 Vela 可调用函数，并显式声明 effects 和转换规则。"
---

Native functions 把 Rust 功能暴露给脚本，但不暴露 Rust 状态指针。注册时
需要稳定名称、签名、effects 和 reflection access metadata。

## Pure Native Functions

Pure functions 接收复制后的脚本值，并返回复制后的脚本值。

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

需要宿主服务的函数使用 `NativeCallContext`。这是 native 代码接触
capabilities、budgets 和 `HostAccess` 的允许入口。

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

## 规则

Native functions 不重载。每个 callable 使用一个公开名称。如果 native 需要
修改宿主状态，它必须通过 `NativeCallContext`、`HostAccess`、已注册的
adapter method，或者返回一个值让脚本之后通过普通 host path 写入。
