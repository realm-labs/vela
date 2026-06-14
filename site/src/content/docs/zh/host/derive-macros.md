---
title: "Derive 宏"
description: "为 Vela 生成 host schemas、native functions 和 host method bindings。"
---

`vela_macros` crate 用来减少 schema 样板代码，同时保留显式宿主边界。
宏生成 descriptors 和 thunks，但不会把 Rust 引用暴露给脚本。

## Host Types

当 Rust 类型需要被脚本通过 host path 读取、写入或调用时，使用
`ScriptHost`。

```rust
#[derive(Debug, ScriptHost)]
#[script(path = "examples::native_function::Player")]
struct Player {
    #[script(get, set, hint = "i64")]
    level: i64,
}

#[script_methods]
impl Player {}
```

`get` 暴露可读字段。`set` 暴露可写目标。`hint` 是脚本侧类型名称，用于
diagnostics、reflection 和 compilation。

## Native Function 宏

`script_function` 注册复制值函数。`script_context_function` 注册接收
`NativeCallContext` 的函数。

```rust
#[script_function(name = "game::bonus_macro", effect = "pure", reflect = true)]
fn bonus_macro(amount: i64, extra: i64) -> i64 {
    amount + extra
}

#[script_context_function(name = "game::grant_level", effect = "write_host")]
fn grant_level(
    ctx: &mut NativeCallContext<'_, '_>,
    player: HostRef,
    amount: i64,
) -> vela_vm::error::VmResult<i64> {
    /* route through HostAccess */
    Ok(amount)
}
```

生成出的 registration helpers 会串到 `Engine::builder()` 里。

## 生成契约

宏会生成 type descriptors、field descriptors、method metadata、从公开脚本
路径推导的 stable schema IDs、conversion thunks 和 reflection metadata。
Rename aliases 可以在有意重命名时保留 schema identity，但脚本仍只看到一个
canonical public name。
