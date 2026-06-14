---
title: "Installation And CLI"
description: "Installation And CLI documentation for Vela."
---

Vela is currently used from the source repository. Public package installation and bytecode artifact distribution are not stable release surfaces yet.

## Requirements

You need a recent Rust toolchain with Cargo. The website also uses Node and npm, but those are only needed when working on the documentation site.

Clone the repository and validate the workspace:

```bash
cargo fmt --all -- --check
cargo test --workspace
```

## CLI Usage

The CLI can run a `.vela` source file through the local workspace binary:

```bash
cargo run -p vela_cli -- path/to/script.vela
```

The CLI is useful for direct script execution and smoke checks. Most production-style usage embeds Vela through `vela_engine` so the host can register types, functions, capabilities, budgets, globals, and hot reload policy.

## Examples And Site

Runnable embedding examples live under `examples/src/bin`:

```bash
cargo run -p vela_examples --bin modules
cargo run -p vela_examples --bin host_type_methods
cargo run -p vela_examples --bin script_global
```

To work on the documentation site:

```bash
cd site
npm ci
npm run dev
```

The site build expects generated playground WASM assets in release workflows. Local documentation-only edits can still use `npm run build` once dependencies are installed.
