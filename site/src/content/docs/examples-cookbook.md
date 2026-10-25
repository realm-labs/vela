---
title: "Examples Cookbook"
description: "The retained synchronous Vela language and removal boundary."
---

Run examples with `cargo run --manifest-path examples/Cargo.toml --bin NAME`.

- `native_function`: independent existing synchronous scalar and owned-collection native calls.
- `serde_value`: script-owned serde snapshots; the original Rust input stays unchanged.
- `modules`: ordinary modules and imports.
- `package_dependency_demo`: ordinary path dependencies and package reload.
- `script_state`: persistent VM-owned state, explicitly inspected or updated from Rust.
- `hot_reload_function_swap`: ordinary script-code replacement.
- `hot_reload_function_swap_invalid`: incompatible update rejection.
- `container_type_hints` and `io_stdlib`: value collections and opt-in I/O.

Independent expected-error examples retain duplicate fields, unsupported general type arguments, random permission denial, reflection structure mutation denial and unknown script fields.

Borrowed/transparently host-mutating and dependent Host construction/method/build-time examples are removed. They are not rewritten using new opaque or codec APIs.
