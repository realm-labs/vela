---
title: "Quickstart"
description: "Quickstart documentation for Vela."
---

The fastest way to try Vela is the [Playground](../playground/). It runs a WASM build of the engine in the browser and is best for syntax, collections, methods, diagnostics, and return values.

## A Small Script

```vela
fn main() {
    let rewards = { "gold": 10, "xp": 25 };
    let tags = ["daily", "vip"];

    if tags.contains("vip") {
        return rewards["gold"] + rewards["xp"];
    }

    return rewards["gold"];
}
```

Run `main` in the Playground. The result is a JSON representation of the returned Vela value.

## Host-Owned State

Vela is most useful when embedded in Rust. The script can look direct:

```vela
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
```

The Rust host still owns `Player`. The call boundary passes a host object binding, and field reads/writes are routed through HostAccess:

```rust
let output = runtime.call(
    "main",
    CallArgs::new().with_host_mut("player", &mut player),
    CallOptions::new(10_000, 1024 * 1024, 64),
)?;
```

The full runnable version lives in `examples/src/bin/level_up`.

## Running Local Examples

From the repository root, run standalone examples with Cargo:

```bash
cargo run -p vela_examples --bin level_up
cargo run -p vela_examples --bin native_function
cargo run -p vela_examples --bin hot_reload_function_swap
```

Use the examples when you need real Rust host objects, native functions, filesystem capabilities, or hot reload behavior. The browser Playground intentionally does not expose those host resources.
