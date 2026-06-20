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
cargo run --manifest-path examples/Cargo.toml --bin level_up
cargo run --manifest-path examples/Cargo.toml --bin context_event
cargo run --manifest-path examples/Cargo.toml --bin time_clock
cargo run --manifest-path examples/Cargo.toml --bin gameplay_helpers
cargo run --manifest-path examples/Cargo.toml --bin random_allowed
cargo run --manifest-path examples/Cargo.toml --bin random_reflect_allowed
cargo run --manifest-path examples/Cargo.toml --bin random_permission_denied
cargo run --manifest-path examples/Cargo.toml --bin host_read_only_denied
cargo run --manifest-path examples/Cargo.toml --bin host_permission_denied
cargo run --manifest-path examples/Cargo.toml --bin host_write_permission_denied
cargo run --manifest-path examples/Cargo.toml --bin host_call_permission_denied
cargo run --manifest-path examples/Cargo.toml --bin host_compound_write_denied
cargo run --manifest-path examples/Cargo.toml --bin stale_host_ref
cargo run --manifest-path examples/Cargo.toml --bin bad_schema_duplicate_field
cargo run --manifest-path examples/Cargo.toml --bin generic_type_hint_denied
cargo run --manifest-path examples/Cargo.toml --bin reward_preview
cargo run --manifest-path examples/Cargo.toml --bin monster_kill_reward
cargo run --manifest-path examples/Cargo.toml --bin quest_progress
cargo run --manifest-path examples/Cargo.toml --bin reflect_debug
cargo run --manifest-path examples/Cargo.toml --bin reflect_schema_mutation_denied
cargo run --manifest-path examples/Cargo.toml --bin reflect_unknown_field_denied
cargo run --manifest-path examples/Cargo.toml --bin hot_reload_function_swap
cargo run --manifest-path examples/Cargo.toml --bin hot_reload_function_swap_invalid
cargo run --manifest-path examples/Cargo.toml --bin host_type_methods
```

Benchmark targets are optional until the related crates exist. The parser fuzz
target is compile-checkable without installing `cargo-fuzz`:

```bash
cargo bench --workspace
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fuzz run parser
```
