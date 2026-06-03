# Vela

Vela is a Hot Reload First dynamic scripting language implemented in Rust for
game server logic. It is designed around Rust host state models, controlled
reflection, safe patch transactions, and reliable function-level hot reload:
scripts can express gameplay logic naturally while host mutation is recorded
through `HostRef`, `HostPath`, and `PatchTx` instead of exposing Rust mutable
references.

## Current Status

Vela is a runnable prototype. Milestones M0-M13 are complete enough to cover
the source-to-bytecode-to-VM loop, host patch transactions, reflection,
budgets, managed heap/GC entrypoints, broad executable language coverage, and
game-server demo scripts.

Current work is focused on targeted M14/M15 embedding and reload workflows:
Engine API registration, native descriptors, context helpers, macros,
safe-point reload, ABI/schema/effect checks, and source-file update workflows
that unblock embedding.

## Repository Layout

- `crates/vela_common`: spans, symbols, stable IDs, and diagnostics.
- `crates/vela_syntax`: lexer, parser, AST/CST-oriented syntax, and recovery.
- `crates/vela_hir` and `crates/vela_analysis`: module resolution, bindings,
  HIR, semantic metadata, and analysis facts.
- `crates/vela_bytecode` and `crates/vela_vm`: bytecode, compiler integration,
  VM runtime, values, call frames, budgets, and managed heap support.
- `crates/vela_host`, `crates/vela_reflect`, and `crates/vela_engine`: host
  boundary, patch transactions, reflection metadata, Engine API, and Runtime
  embedding surface.
- `crates/vela_hot_reload`: program versions, hot-reload staging, ABI checks,
  and update reports.
- `crates/vela_macros`: derive and helper macros for host and reflection
  bindings.
- `crates/vela_cli`: runnable demo CLI for script execution and hot reload.
- `examples/game_server_demo`: `.vela` scripts that exercise gameplay,
  context helpers, reflection, permissions, and hot reload.
- `docs`: product goals, architecture, progress, decisions, and validation
  commands.

## Quick Start

Use a recent Rust toolchain that supports the workspace's Rust 2024 edition.

Run the workspace tests:

```bash
cargo test --workspace
```

Run a basic game-server demo script:

```bash
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.vela
```

## Demo Commands

Run representative demo scripts:

```bash
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/monster_kill_reward.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/quest_progress.vela
cargo run -p vela_cli -- examples/game_server_demo/scripts/reflect_debug.vela
```

Run permissioned random demos:

```bash
cargo run -p vela_cli -- --allow-random examples/game_server_demo/scripts/random_allowed.vela
cargo run -p vela_cli -- --allow-random examples/game_server_demo/scripts/random_reflect_allowed.vela
```

Run a hot-reload function swap demo:

```bash
cargo run -p vela_cli -- --hot-reload examples/game_server_demo/scripts/hot_reload_function_swap_v1.vela examples/game_server_demo/scripts/hot_reload_function_swap_v2.vela
```

The full validation command set, including expected failure cases, lives in
[`docs/validation.md`](docs/validation.md).

## Project Docs

- [`docs/goal.md`](docs/goal.md): authoritative product roadmap and milestone
  target.
- [`docs/architecture.md`](docs/architecture.md): technical architecture
  contract.
- [`docs/progress.md`](docs/progress.md): current milestone and implementation
  status.
- [`docs/decisions.md`](docs/decisions.md): active architecture decisions and
  standing constraints.
- [`docs/validation.md`](docs/validation.md): validation commands for normal
  checkpoints and demos.

## Constraints

- The script language does not support generics.
- Scripts never receive real Rust `&mut T` references.
- Host mutation must go through `HostRef`, `HostPath`, `PathProxy`, and
  `PatchTx`.
- Reflection may query metadata and perform controlled reads, writes, and
  calls, but it must not mutate runtime type structure or implement monkey
  patching.
- The MVP does not include JIT, script async/coroutines, a moving GC, or a full
  LSP.
