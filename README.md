# Vela

Vela is a Hot Reload First dynamic scripting language implemented in Rust for
host-owned business logic. Game server scripting is the main proving ground,
but the core language, standard library, runtime, and embedding contracts are
domain-neutral.

Read the hosted docs or try the browser playground:

- [Documentation](https://realm-labs.github.io/vela/overview/)
- [Playground](https://realm-labs.github.io/vela/playground/)

Scripts operate on script-owned data with ordinary language syntax:

```vela
fn handle(player, reward) {
    player.level += 1;
    player.inventory.gold += reward.gold;
}
```

The reduced Rust boundary retains independent synchronous native calls and
existing value conversions. Converted data is script-owned and has no implicit
Rust write-back. Replacement opaque-object, conversion and re-entry APIs are
separate follow-up work, not part of this removal pass.

## Current Status

Vela is a runnable prototype with a source-to-bytecode-to-interpreter loop,
script reflection, finite execution budgets, non-moving script GC, ordinary
modules/packages/traits, script hot reload, standard-library helpers, native LSP
and editor integrations, a browser playground, and measured interpreter baselines.

The active work is the removal-only [runtime simplification pass](docs/runtime-simplification-plan.md).
Checkpoint B removes Rust/Vela Service hotfixing and Provider discovery/selection;
ordinary packages, traits and script reload remain. Checkpoints C/D remove script
async/tasks, caller-borrowed Rust values and transparent host paths. Some examples
and implementation surfaces still await those owning checkpoints; their presence
does not establish the target boundary. No replacement features are added here.

See [`docs/progress.md`](docs/progress.md) for the active milestone state.

## Language Snapshot

Vela supports functions, modules, structs, enums, traits, inherent methods,
trait impl methods, closures, match, arrays, maps, sets, Option/Result-style
helpers, reflection, and host-boundary method calls.

```vela
struct DamageResult {
    actor: string,
    applied: int,
}

impl DamageResult {
    fn score(self, bonus: int) -> int {
        return self.applied + bonus;
    }
}

trait Label {
    fn label(self) -> string;
}

impl Label for DamageResult {
    fn label(self) -> string {
        return self.actor;
    }
}

fn main() {
    let result = DamageResult { actor: "knight", applied: 42 };
    return result.score(8);
}
```

There are no script-language generics, overload sets, Rust-style borrow syntax,
script-visible task/coroutine handles, arbitrary `eval`, or runtime monkey
patching. Script `async fn`/`.await` are scheduled for removal; editor worker
scheduling and protocol cancellation are independent tooling concerns.

## Repository Layout

- `crates/vela_common`: spans, symbols, stable IDs, and diagnostics.
- `crates/vela_syntax`: lexer, parser, AST, syntax diagnostics, and recovery.
- `crates/vela_hir`: module graph, imports, declarations, bindings, and
  semantic metadata.
- `crates/vela_analysis`: analysis facts used by diagnostics/tooling.
- `crates/vela_bytecode`: bytecode compiler, program metadata, and verification.
- `crates/vela_vm`: interpreter, values, managed heap, budgets, GC roots, and
  runtime execution primitives.
- `crates/vela_host`: existing host-boundary support, being reduced by checkpoint D.
- `crates/vela_reflect`: type registry, reflection records, permissions, and
  controlled read/write/call helpers.
- `crates/vela_engine`: embedding API, `EngineBuilder`, `Runtime`, call args,
  native functions, host type registration, VM/extern state, and hot reload integration.
- `crates/vela_macros`: derive and helper macros for host/native bindings.
- `crates/vela_hot_reload`: program versions, ABI/schema checks, staged updates,
  and reload reports.
- `crates/vela_cli`: final CLI binary for direct script execution.
- `crates/vela_playground_wasm`: WASM wrapper used by the browser playground.
- `examples`: standalone runnable embedding examples.
- `site`: GitHub Pages documentation and playground source.
- `docs`: product goal, architecture, decisions, progress, validation, grammar,
  and performance notes.

## Quick Start

Use a recent Rust toolchain that supports Rust 2024.

Run the full workspace tests:

```bash
cargo test --workspace
```

Run focused validation while developing:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test-fast
```

`cargo test-fast` runs the workspace suite except the macro UI compile tests,
the workspace-wide unsafe-source audit, and the VM conformance fixtures. Run
the full workspace command before a verified checkpoint.

Run standalone examples:

```bash
cargo run --manifest-path examples/Cargo.toml --bin level_up
cargo run --manifest-path examples/Cargo.toml --bin host_type_methods
cargo run --manifest-path examples/Cargo.toml --bin script_state
cargo run --manifest-path examples/Cargo.toml --bin serde_value
cargo run --manifest-path examples/Cargo.toml --bin io_stdlib
cargo run --manifest-path examples/Cargo.toml --bin async_basic

```

Run the CLI on a simple script:

```bash
cargo run -p vela_cli -- examples/src/bin/io_stdlib/main.vela
cargo run -p vela_cli -- --async examples/src/bin/async_basic/main.vela
```

Some host-boundary examples need their Rust embedding setup and should be run
through `vela_examples`, not directly through `vela_cli`.

## Browser Docs And Playground

The static site under `site/` contains bilingual documentation and a browser
playground backed by `vela_playground_wasm`.

Hosted site: <https://realm-labs.github.io/vela/>

Local playground build:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/vela_playground_wasm.wasm --target web --out-dir site/public/pkg
cd site
npm ci
npm run dev
```

GitHub Pages deploys automatically from the Pages workflow after CI succeeds.

## Project Docs

- [`docs/goal.md`](docs/goal.md): product roadmap and milestone target.
- [`docs/architecture.md`](docs/architecture.md): technical architecture
  contract.
- [`docs/progress.md`](docs/progress.md): current milestone status and gaps.
- [`docs/decisions.md`](docs/decisions.md): active architecture decisions.
- [`docs/validation.md`](docs/validation.md): validation command sets.
- [`docs/grammar.ebnf`](docs/grammar.ebnf): current grammar reference.
- [`docs/performance.md`](docs/performance.md): benchmark rules and baseline
  summaries.

## Standing Constraints

- No script-language generics.
- No function overloading by arity, type hint, or native signature.
- No caller-borrowed Rust roots, arguments, returns, views, leases or transparent
  host paths in the reduced target, including borrowed data hidden in wrappers.
- No Service hotfixing or Provider entrypoints; ordinary traits/packages stay.
- Reflection can query metadata and perform controlled reads, writes, and
  calls, but cannot mutate runtime type structure.
- No monkey patching, JIT, script async/tasks, moving GC, structural state
  migration, or custom full IDE product.
- Active calls and retained closures keep their exact executable generation
  across compatible script reloads; old closures are never rebound by name.
