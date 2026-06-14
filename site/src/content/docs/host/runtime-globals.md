---
title: "Runtime Globals"
description: "Binding persistent runtime state to script global declarations."
---

Runtime globals connect module-level script declarations to Rust-provided
instances. The declaration is metadata; Rust supplies the value.

## Declaring Globals

```vela
struct ServerState {
    level: i64,
    name: string,
    total_gold: i64,
}

global state: ServerState;

fn handle_tick(level_gain, gold_gain) {
    state.level += level_gain;
    state.total_gold += gold_gain;
    return state.level + state.total_gold;
}
```

The fully qualified global name is module-based, such as `main::state`.

## Inserting Values

Rust inserts or replaces the runtime instance.

```rust
runtime.insert_global("main::state", &initial_state)?;
runtime.set_global("main::state", &updated_state)?;
```

With serde enabled, Rust structs can be inserted as script-owned snapshot
values. Host globals can also be inserted with `insert_host_global` when the
global should be a persistent host object.

## Reading Back

`global_as` deserializes a runtime global without requiring the host to
materialize an intermediate `OwnedValue`.

```rust
let final_state: ServerState = runtime
    .global_as("main::state")?
    .expect("state global should exist");
```

Globals are rooted across calls and participate in GC as runtime roots.
