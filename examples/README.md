# Vela Examples

Each retained example is a standalone Cargo bin with its own Rust entrypoint and
script sources. Checkpoints B-D remove Service/Provider, script async/tasks,
borrowed/transparently host-mutating examples, and dependent Host construction,
method and build-time binding examples. They are not replaced with opaque or
codec APIs.

```bash
cargo run --manifest-path examples/Cargo.toml --bin native_function
cargo run --manifest-path examples/Cargo.toml --bin serde_value
cargo run --manifest-path examples/Cargo.toml --bin modules
cargo run --manifest-path examples/Cargo.toml --bin package_dependency_demo
cargo run --manifest-path examples/Cargo.toml --bin hot_reload_function_swap
cargo run --manifest-path examples/Cargo.toml --bin script_state
cargo run --manifest-path examples/Cargo.toml --bin container_type_hints
cargo run --manifest-path examples/Cargo.toml --bin io_stdlib
```

- `native_function`: existing independent synchronous scalar/collection calls.
- `serde_value`: script-owned snapshots; the original Rust input is unchanged.
- `modules` / `package_dependency_demo`: ordinary source organization and package reload.
- `script_state`: VM-owned state, initialized by Vela and explicitly updated from Rust.
  This is not an extern-state host binding.
- `hot_reload_function_swap` / `hot_reload_function_swap_invalid`: ordinary script
  replacement and incompatible-update rejection.
- `container_type_hints` / `io_stdlib`: retained value collections and opt-in I/O.

Independent expected-error examples retain duplicate-field, generic-hint,
random-capability, reflection-schema-mutation and unknown-script-field rejection.
They validate the expected error and exit successfully. Source-boundary rejection
coverage also lives in `tests/removed_entrypoints.rs`.
