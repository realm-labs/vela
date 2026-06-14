---
title: "Modules And Imports"
description: "Modules And Imports documentation for Vela."
---

Vela source files do not declare their own module names. Module identity comes from the host compile mode: a single file is a lightweight entry script, while directory compilation maps file paths to module paths.

## Module Identity

In directory mode, paths such as `scripts/game/reward.vela` become modules such as `game::reward`. A directory-compiled entry point is called by its fully qualified function name, for example `game::main::main`.

```text
scripts/game/main.vela   -> game::main
scripts/game/reward.vela -> game::reward
scripts/config.vela      -> config
```

## Imports

`use` imports public declarations from another module. Static paths use `::`; runtime field access uses `.` and is a different operation.

```vela
use game::reward::grant
use config::BASE_REWARD as DEFAULT_REWARD

pub fn main(player) {
    grant(player, DEFAULT_REWARD)
}
```

## Visibility

`pub` marks declarations that can be imported from their owning module or called through the embedding API when exported. Private declarations are module-local implementation details.

```vela
pub const BASE_REWARD: i64 = 10

fn internal_bonus(level: i64) -> i64 {
    return level * 2
}
```

## Hot Reload

Imports participate in dependency impact analysis. Reloading one or more changed source files can update the module graph together, but ABI and schema compatibility still decide whether the staged version becomes active.
