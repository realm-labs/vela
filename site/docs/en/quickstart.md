# Quickstart

The fastest way to try Vela is the browser playground. Select an example, edit the script, and run the `main` function.

## Playground Loop

1. Open the playground.
2. Pick an example.
3. Edit the source.
4. Press Compile to check diagnostics.
5. Press Run to execute the selected entry function.

## Minimal Script

```vela
fn main() {
    let rewards = { "gold": 10, "xp": 25 };
    return rewards["gold"] + rewards["xp"];
}
```

## Records And Methods

```vela
struct DamageResult {
    actor: String,
    applied: Int,
}

impl DamageResult {
    fn score(self, bonus) -> Int {
        return self.applied + bonus;
    }
}

fn main() {
    let result = DamageResult {
        actor: "knight",
        applied: 42,
    };
    return result.score(8);
}
```

## CLI Shape

The CLI is the final script execution binary, similar to how Lua users run `.lua` files.

```bash
cargo run -p vela_cli -- examples/src/bin/level_up/level_up.vela
```

## Embedding Shape

Rust hosts compile source into a program, create a runtime, then call script entries with explicit call arguments and execution budgets. Host-owned state is passed through host handles or registered globals when scripts need to mutate durable Rust data.

```rust
let engine = EngineBuilder::new()
    .with_standard_natives()
    .build()?;

let program = engine.compile_source(SourceId::new(1), source)?;
let mut runtime = Runtime::new(engine, program);
let value = runtime.call("main", CallArgs::new(), CallOptions::unbounded())?;
```
