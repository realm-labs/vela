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
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.lang
cargo run -p vela_cli -- examples/game_server_demo/scripts/context_event.lang
cargo run -p vela_cli -- examples/game_server_demo/scripts/monster_kill_reward.lang
cargo run -p vela_cli -- examples/game_server_demo/scripts/quest_progress.lang
cargo run -p vela_cli -- examples/game_server_demo/scripts/reflect_debug.lang
```

Benchmark and fuzz targets are optional until the related crates exist:

```bash
cargo bench --workspace
cargo fuzz run parser
```
