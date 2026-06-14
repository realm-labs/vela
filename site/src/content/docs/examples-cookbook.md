---
title: "Examples And Cookbook"
description: "Task-oriented examples and recipes for Vela."
---

The repository includes standalone examples under `examples/src/bin`. They are the best source for complete host setup because they compile real Rust, register host types, and exercise runtime capabilities.

## Run A Script From Rust

The `level_up` example shows the smallest host write-through shape:

```vela
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
```

Run it with:

```bash
cargo run -p vela_examples --bin level_up
```

The host registers `Player`, grants host read/write capabilities, passes `player` as a mutable host binding, and receives the script result.

## Register Native Functions

The `native_function` example covers manual and macro-generated native functions:

```vela
fn main(player: Player) {
    let manual = game::bonus_manual(3, 4);
    let generated = game::bonus_macro(10, 7);
    let collection = game::collection_bonus(
        { "quest": 5, "raid": 8 },
        ["vip", "daily"],
    );
    let level = game::grant_level(player, collection);

    return manual + generated + level;
}
```

Use this example when you need Rust functions callable from scripts, including context-aware functions that route writes through `NativeCallContext`.

## Use Modules And Globals

The `modules` example compiles a directory and calls a fully qualified entry:

```vela
use game::reward::grant

fn main() {
    return grant(4) + game::config::BASE_REWARD;
}
```

The `script_global` example shows runtime-managed globals declared in Vela and initialized or updated from Rust.

## Exercise Boundaries

Several examples intentionally fail to demonstrate diagnostics and policy enforcement:

- `host_permission_denied`
- `host_read_only_denied`
- `host_write_permission_denied`
- `host_call_permission_denied`
- `generic_type_hint_denied`
- `reflect_schema_mutation_denied`

Use these when checking capability profiles, read-only schemas, unsupported type hints, and reflection limits.
