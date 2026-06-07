# Modules

Vela modules are source files resolved by the compiler. File paths and module names are intentionally aligned for clarity.

## Importing

```vela
use game::reward::grant_reward;

fn main(player) {
    return grant_reward(player);
}
```

## Declarations

Modules can define public functions, private helpers, structs, enums, traits, impls, constants, and globals. Module metadata is part of the hot reload ABI surface.

## Why Modules Matter

The module graph gives the compiler stable declaration IDs, import resolution, dependency tracking, and hot reload impact analysis. It also gives future tooling a clean semantic model without adding runtime monkey patching.
