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
cargo run -p vela_cli -- examples/game_server_demo/scripts/time_clock.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/gameplay_helpers.vela
cargo run -p vela_cli -- --allow-random examples/game_server_demo/scripts/random_allowed.vela
cargo run -p vela_cli -- --allow-random examples/game_server_demo/scripts/random_reflect_allowed.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/random_permission_denied.vela # expected permission-denied failure
cargo run -p vela_cli -- examples/game_server_demo/scripts/host_read_only_denied.vela # expected read-only host field failure
cargo run -p vela_cli -- --deny-player-level-read examples/game_server_demo/scripts/host_permission_denied.vela # expected host read permission failure
cargo run -p vela_cli -- --deny-player-level-write examples/game_server_demo/scripts/host_write_permission_denied.vela # expected host write permission failure
cargo run -p vela_cli -- --deny-ctx-emit-call examples/game_server_demo/scripts/host_call_permission_denied.vela # expected host call permission failure
cargo run -p vela_cli -- --conflict-player-level-before-apply examples/game_server_demo/scripts/host_patch_conflict.vela # expected host patch conflict failure
cargo run -p vela_cli -- --stale-player examples/game_server_demo/scripts/stale_host_ref.vela # expected stale host ref failure
cargo run -p vela_cli -- examples/game_server_demo/scripts/bad_schema_duplicate_field.vela # expected bad-schema failure
cargo run -p vela_cli -- examples/game_server_demo/scripts/generic_type_hint_denied.vela # expected generic type hint failure
cargo run -p vela_cli -- examples/game_server_demo/scripts/reward_preview.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/monster_kill_reward.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/quest_progress.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/reflect_debug.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/reflect_schema_mutation_denied.vela # expected schema mutation denial
cargo run -p vela_cli -- examples/game_server_demo/scripts/reflect_unknown_field_denied.vela # expected reflection candidate failure
cargo run -p vela_cli -- --hot-reload examples/game_server_demo/scripts/hot_reload_function_swap_v1.vela examples/game_server_demo/scripts/hot_reload_function_swap_v2.vela
cargo run -p vela_cli -- --hot-reload examples/game_server_demo/scripts/hot_reload_function_swap_v1.vela examples/game_server_demo/scripts/hot_reload_function_swap_invalid.vela # expected ABI rejection
```

Benchmark targets are optional until the related crates exist. The parser fuzz
target is compile-checkable without installing `cargo-fuzz`:

```bash
cargo bench --workspace
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fuzz run parser
```
