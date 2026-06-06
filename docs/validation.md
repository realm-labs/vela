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
cargo run -p vela_examples --bin level_up
cargo run -p vela_examples --bin context_event
cargo run -p vela_examples --bin time_clock
cargo run -p vela_examples --bin gameplay_helpers
cargo run -p vela_examples --bin random_allowed
cargo run -p vela_examples --bin random_reflect_allowed
cargo run -p vela_examples --bin random_permission_denied
cargo run -p vela_examples --bin host_read_only_denied
cargo run -p vela_examples --bin host_permission_denied
cargo run -p vela_examples --bin host_write_permission_denied
cargo run -p vela_examples --bin host_call_permission_denied
cargo run -p vela_examples --bin host_compound_write_denied
cargo run -p vela_examples --bin stale_host_ref
cargo run -p vela_examples --bin bad_schema_duplicate_field
cargo run -p vela_examples --bin generic_type_hint_denied
cargo run -p vela_examples --bin reward_preview
cargo run -p vela_examples --bin monster_kill_reward
cargo run -p vela_examples --bin quest_progress
cargo run -p vela_examples --bin reflect_debug
cargo run -p vela_examples --bin reflect_schema_mutation_denied
cargo run -p vela_examples --bin reflect_unknown_field_denied
cargo run -p vela_examples --bin hot_reload_function_swap
cargo run -p vela_examples --bin hot_reload_function_swap_invalid
cargo run -p vela_examples --bin host_type_methods
```

Benchmark targets are optional until the related crates exist. The parser fuzz
target is compile-checkable without installing `cargo-fuzz`:

```bash
cargo bench --workspace
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo fuzz run parser
```
