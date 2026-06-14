---
title: "Fuzzing And Validation"
description: "Validation layers used to keep Vela parsing, bytecode, and runtime behavior stable."
---

Vela uses layered validation. Parser tests, compiler tests, VM tests, examples,
benchmarks, fuzz targets, and CI checks cover different failure modes.

## Standard Validation

The default full validation target is:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For site syntax highlighting, the documentation site also provides:

```bash
cd site && npm run test:syntax
```

## Fuzzing

The repository includes a parser fuzz target under `fuzz/`. The local command
when `cargo-fuzz` is installed is:

```bash
cargo fuzz run parser
```

Fuzzing should find parser crashes, recovery bugs, and unexpected panics. It
does not replace semantic or VM conformance tests.

## Example Coverage

Runnable examples under `examples/src/bin` exercise hot reload, reflection,
host permissions, stale host refs, schema rejection, I/O capabilities, and
domain-neutral standard helpers.

## Validation Discipline

Do not delete tests to make a failure pass. New runtime behavior should have a
focused test at the layer that owns the contract, then broader examples or
conformance fixtures when the behavior crosses subsystem boundaries.
