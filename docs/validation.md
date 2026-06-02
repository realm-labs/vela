# Validation

Run these commands before committing normal implementation checkpoints:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Milestones after M6 should also validate at least one runnable game server demo
script:

```bash
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/context_event.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/context_clock.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/gameplay_helpers.vela
cargo run -p vela_cli -- --allow-random examples/game_server_demo/scripts/random_allowed.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/random_permission_denied.vela # expected permission-denied failure
cargo run -p vela_cli -- examples/game_server_demo/scripts/monster_kill_reward.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/quest_progress.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/reflect_debug.vela
cargo run -p vela_cli -- --hot-reload examples/game_server_demo/scripts/hot_reload_function_swap_v1.vela examples/game_server_demo/scripts/hot_reload_function_swap_v2.vela
cargo run -p vela_cli -- --hot-reload examples/game_server_demo/scripts/hot_reload_function_swap_v1.vela examples/game_server_demo/scripts/hot_reload_function_swap_invalid.vela # expected ABI rejection
```

Benchmark and fuzz targets are optional until the related crates exist:

```bash
cargo bench --workspace
cargo fuzz run parser
```
