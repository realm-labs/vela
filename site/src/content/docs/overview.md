---
title: "Overview"
description: "Overview documentation for Vela."
---

Vela is a scripting language for Rust hosts that own application state. It is designed for server-side business logic that needs to change quickly while keeping mutation, capabilities, and runtime effects under host control.

Game server scripting is the primary proving ground, but the language core is domain-neutral. Domain objects such as players, quests, rewards, orders, accounts, or workflows come from host registration and examples, not from built-in language magic.

## What Vela Optimizes For

Vela is Hot Reload First. Code is compiled into versioned bytecode, and updates replace function or module code objects at safe boundaries. Existing call frames continue on the old code, while new calls enter the new code when the update is accepted.

Vela is also HostAccess First. Scripts can use ordinary field syntax:

```vela
fn level_up(player: Player) {
    player.level += 1;
    return player.level;
}
```

That syntax does not expose a Rust `&mut Player` to script code. The VM routes the read-modify-write through `HostRef`, `HostPath`, `PathProxy`, and `HostAccess`, where the host adapter validates permissions and applies the write.

## What Vela Is Not

Vela is not dynamic Rust, not a Lua table/metatable clone, and not an unbounded plugin sandbox. The MVP intentionally excludes script-language generics, JIT compilation, script async/coroutines, monkey patching, runtime type-structure mutation, and exposing real Rust references to scripts.

The language is dynamic, but its embedding boundary is explicit. Hosts choose which types, fields, methods, native functions, globals, capabilities, and budgets are available to each runtime.

## System Shape

The normal pipeline is:

```text
source -> parser -> HIR -> bytecode -> VM -> HostAccess -> Rust host state
```

The host creates an `Engine`, compiles source or directories into a program, creates a `Runtime`, and calls script entries with `CallArgs` and `CallOptions`. Reflection and diagnostics are available for controlled metadata queries and error reporting, but they do not mutate the registered schema at runtime.
