---
title: "Derive Macros"
description: "Generating host schemas, native functions, and host method bindings for Vela."
---

The `vela_macros` crate removes most schema boilerplate while preserving the
same explicit host boundary. Macros generate descriptors and thunks; they do
not expose Rust references to scripts.

## Host Types

Use `ScriptHost` for Rust types that scripts may read, write, or call through
host paths.

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

`get` exposes a readable field. `set` exposes a write target. `hint` gives the
script-facing type name used by diagnostics, reflection, and compilation.

## Native Function Macros

`script_function` registers copied-value functions. `script_context_function`
registers functions that receive `NativeCallContext`.

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

The generated registration helpers are chained into `Engine::builder()`.

## Generated Contract

Macros generate type descriptors, field descriptors, method metadata, stable
schema IDs derived from public script paths, conversion thunks, and reflection
metadata. Rename aliases may preserve a schema identity across intentional
renames, but scripts still see one canonical public name.
