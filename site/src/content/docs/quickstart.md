---
title: "Quickstart"
description: "The synchronous, script-owned Vela boundary."
---

Run a script using the synchronous CLI:

```bash
cargo run -p vela_cli -- examples/src/bin/modules/game/main.vela
```

For runnable Rust embedding examples, use the separate examples manifest:

```bash
cargo run --manifest-path examples/Cargo.toml --bin native_function
cargo run --manifest-path examples/Cargo.toml --bin serde_value
cargo run --manifest-path examples/Cargo.toml --bin script_state
cargo run --manifest-path examples/Cargo.toml --bin modules
cargo run --manifest-path examples/Cargo.toml --bin package_dependency_demo
cargo run --manifest-path examples/Cargo.toml --bin hot_reload_function_swap
```

These use existing synchronous native/value APIs, snapshot conversion, ordinary modules/packages and script reload. Serialized data is script-owned and has no implicit Rust write-back. Borrowed-host examples, Host constructors/methods and dependent extern-state bindings are removed, not replaced with new opaque APIs.
