# Vela

Vela is a Hot Reload First dynamic scripting language implemented in Rust for
host-owned business logic. Game server scripting is the main proving ground,
but the core language, standard library, runtime, and embedding contracts are
domain-neutral.

Scripts can read and mutate Rust-owned state with natural syntax while the
runtime keeps the boundary explicit:

```vela
fn handle(player, reward) {
    player.level += 1;
    player.inventory.gold += reward.gold;
}
```

The script never receives a real Rust `&mut T`. Host mutation is routed through
`HostRef`, `HostPath`, `PathProxy`, and write-through `HostAccess`.

## Current Status

Vela is a runnable prototype with the source-to-bytecode-to-VM loop, host
write-through access, reflection, execution budgets, managed heap/GC
foundations, module resolution, hot reload, standard-library helpers, standalone
embedding examples, a browser playground, and measured interpreter baselines.

Current work is focused on M19.5 performance architecture prep before M20
inline caches:

- move hot dispatch operands from names to IDs, slots, resolved targets, or path
  keys;
- keep VM dispatch split behind focused call/access/object/iteration
  boundaries;
- preserve hot reload, host access, reflection, GC, and runtime semantics while
  preparing cache-ready invariants.

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
async/coroutines, arbitrary `eval`, or runtime monkey patching.

## Repository Layout

- `crates/vela_common`: spans, symbols, stable IDs, and diagnostics.
- `crates/vela_syntax`: lexer, parser, AST, syntax diagnostics, and recovery.
- `crates/vela_hir`: module graph, imports, declarations, bindings, and
  semantic metadata.
- `crates/vela_analysis`: analysis facts used by diagnostics/tooling.
- `crates/vela_bytecode`: bytecode compiler, program metadata, and verification.
- `crates/vela_vm`: interpreter, values, managed heap, budgets, GC roots, and
  runtime execution primitives.
- `crates/vela_host`: `HostRef`, `HostPath`, `PathProxy`, `HostAccess`, and host
  state adapter traits.
- `crates/vela_reflect`: type registry, reflection records, permissions, and
  controlled read/write/call helpers.
- `crates/vela_engine`: embedding API, `EngineBuilder`, `Runtime`, call args,
  native functions, host type registration, globals, and hot reload integration.
- `crates/vela_macros`: derive and helper macros for host/native bindings.
- `crates/vela_hot_reload`: program versions, ABI/schema checks, staged updates,
  and reload reports.
- `crates/vela_cli`: final CLI binary for direct script execution.
- `crates/vela_playground_wasm`: WASM wrapper used by the browser playground.
- `crates/vela_c_api`: external C ABI surface with opaque engine/runtime handles.
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
cargo test --workspace
```

Run standalone examples:

```bash
cargo run -p vela_examples --bin level_up
cargo run -p vela_examples --bin host_type_methods
cargo run -p vela_examples --bin script_global
cargo run -p vela_examples --bin serde_value
cargo run -p vela_examples --bin io_stdlib
```

Run the CLI on a simple script:

```bash
cargo run -p vela_cli -- examples/src/bin/io_stdlib/main.vela
```

Some host-boundary examples need their Rust embedding setup and should be run
through `vela_examples`, not directly through `vela_cli`.

## Browser Docs And Playground

The static site under `site/` contains bilingual documentation and a browser
playground backed by `vela_playground_wasm`.

Local playground build:

```bash
rustup target add wasm32-unknown-unknown
cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/vela_playground_wasm.wasm --target web --out-dir site/pkg
python3 -m http.server 8080 --directory site
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
- Scripts never receive real Rust `&mut T` references.
- Host mutation goes through `HostRef`, `HostPath`, `PathProxy`, and
  `HostAccess`.
- Reflection can query metadata and perform controlled reads, writes, and
  calls, but cannot mutate runtime type structure.
- No monkey patching, MVP JIT, script async/coroutines, moving GC, or full LSP.
