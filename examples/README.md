# Vela Examples

Each example is a standalone Cargo bin with its own `main`, so examples do not
depend on a parameter-dispatched demo runner.

```bash
cargo run -p vela_examples --bin level_up
cargo run -p vela_examples --bin monster_kill_reward
cargo run -p vela_examples --bin hot_reload_function_swap
cargo run -p vela_examples --bin host_type_methods
cargo run -p vela_examples --bin script_global
cargo run -p vela_examples --bin serde_value
cargo run -p vela_examples --bin container_type_hints
cargo run -p vela_examples --bin io_stdlib
```

Each example directory keeps the Rust entrypoint and script source together:

```text
examples/src/bin/level_up/main.rs
examples/src/bin/level_up/level_up.vela
examples/src/bin/host_type_methods/main.rs
examples/src/bin/host_type_methods/handle.vela
```

The `host_type_methods` example covers the host type method and argument model:

- concrete host type specs for `Player`, `IntIntMap`, `TagSet`, and `RewardSink`
- same method name on different concrete receiver types: `contains`
- call-scoped host object binding through `CallArgs::with_host_mut`
- `player.inventory.items["gold"].count` as keyed host access without cloning a Rust collection
- root and child host method calls resolved through host target plans and `HostMethodId`

The Rust side uses `#[derive(ScriptHost)]` for field/path bindings and
`#[script_methods]` for `&self` / `&mut self` host methods. Script-visible
fields participate in resolved host access by default, so the example does not
hand-write `ScriptHostObject` or `PathSegment` dispatch.

Other useful embedding examples:

- `script_global`: persistent VM-managed globals that Rust can read and update.
- `serde_value`: snapshot-style serde conversion between Rust structs/enums and
  Vela owned values.
- `native_function`: script calls into Rust native functions.
- `container_type_hints`: builtin typed container contracts across arrays,
  value-keyed maps, sets, and nested Result propagation.
- `io_stdlib`: opt-in stdout plus sandboxed file I/O capability checks.

Expected-error examples such as `random_permission_denied` and
`hot_reload_function_swap_invalid` validate the expected diagnostic and then
exit successfully.
