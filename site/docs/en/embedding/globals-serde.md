# Globals And Serde

Vela supports module-level `global` declarations backed by persistent runtime storage.

## Script-Owned Globals

```vela
struct ServerState {
    level: int,
    total_gold: int,
}

global state: ServerState;

fn handle_tick(gold) {
    state.total_gold += gold;
    return state.total_gold;
}
```

Rust inserts the initial value:

```rust
runtime.insert_global(
    "state",
    OwnedValue::record("ServerState", [("level", 1), ("total_gold", 0)]),
)?;
```

## Serde Snapshots

With the `serde` feature, Rust structs and enums can be converted into VM-owned script values and decoded from returned values. This is a snapshot path, separate from host handles.

Use serde for owned script data. Use host handles when scripts must mutate Rust-owned state directly.
