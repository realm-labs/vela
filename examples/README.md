# Vela Examples

Each example is a standalone Cargo bin with its own `main`, so examples do not
depend on a parameter-dispatched demo runner.

```bash
cargo run --manifest-path examples/Cargo.toml --bin level_up
cargo run --manifest-path examples/Cargo.toml --bin monster_kill_reward
cargo run --manifest-path examples/Cargo.toml --bin hot_reload_function_swap
cargo run --manifest-path examples/Cargo.toml --bin host_type_methods
cargo run --manifest-path examples/Cargo.toml --bin script_state
cargo run --manifest-path examples/Cargo.toml --bin async_stateful_reentry
cargo run --manifest-path examples/Cargo.toml --bin serde_value
cargo run --manifest-path examples/Cargo.toml --bin container_type_hints
cargo run --manifest-path examples/Cargo.toml --bin io_stdlib
cargo run --manifest-path examples/Cargo.toml --bin interop_round_trip
cargo run --manifest-path examples/Cargo.toml --bin replaceable_handler
cargo run --manifest-path examples/Cargo.toml --bin replaceable_service_method
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

- `interop_round_trip`: the primary ordinary interop workflow. Vela calls an
  exported Rust function and methods using normal syntax, while Rust calls the
  Vela entry through build-time generated typed bindings. Authored calls do not
  assemble `CallArgs`, erase values, or resolve runtime target strings.
- `replaceable_handler` and `replaceable_service_method`: optional
  single-callable override extensions. They demonstrate direct fallback,
  future-root activation, adjacent Rust methods, and rollback without a proxy
  or complete Vela service implementation.
- `async_stateful_reentry`: a mutable state lease held across a Rust service
  await, followed by same-session Vela reentry with an explicit mutable
  reborrow. Its actor-shaped container keeps Runtime and host storage disjoint
  without depending on an actor framework or async executor crate.
- `script_state`: per-Runtime VM state initialized by Vela and updateable from Rust.
- `serde_value`: snapshot-style serde conversion between Rust structs/enums and
  Vela owned values.
- `native_function`: script calls into Rust native functions.
- `container_type_hints`: builtin typed container contracts across arrays,
  value-keyed maps, sets, and nested Result propagation.
- `io_stdlib`: opt-in stdout plus sandboxed file I/O capability checks.

Expected-error examples such as `random_permission_denied` and
`hot_reload_function_swap_invalid` validate the expected diagnostic and then
exit successfully.
