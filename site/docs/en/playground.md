# Playground

The playground runs Vela in the browser through a small WASM wrapper around `vela_engine`.

## What Works

- Compile diagnostics.
- Runtime diagnostics.
- Records, enums, methods, arrays, maps, sets, strings, math helpers, Option/Result helpers, controlled time, and controlled random.
- Preloaded examples plus editable source.

## Sandbox Boundary

The browser playground does not expose Rust host objects, filesystem I/O, or real server state. That is intentional. The host bridge path is a Rust embedding feature, so examples that mutate Rust-owned state live in `examples/src/bin`.

## Return Values

`run_script` returns a JSON representation of the script value:

```vela
fn main() {
    return { "gold": 10, "xp": 25 };
}
```

The page renders the JSON result in the Output panel. Diagnostics are shown separately so errors stay readable.
