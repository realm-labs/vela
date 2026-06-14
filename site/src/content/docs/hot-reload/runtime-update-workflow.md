---
title: "Runtime Update Workflow"
description: "The host-side flow for compiling, staging, and applying Vela updates."
---

Embedding hosts own the update loop. Vela provides compile, stage, safe-point
apply, and report APIs so the host can decide when an update is allowed to take
effect.

## Initial Version

The initial version is compiled before the runtime starts serving work. The
standalone examples use `compile_hot_reload_initial` and then create a runtime
from that version.

```rust
let initial = engine.compile_hot_reload_initial(source)?;
let mut runtime = Runtime::from_hot_reload_version(engine, initial);
```

The initial compile builds bytecode, metadata, and the baseline ABI snapshot.

## Stage An Update

Source changes are compiled into an update candidate. Staging keeps the current
runtime version active until the next safe point.

```rust
let update = runtime.compile_hot_reload_update(updated_source)?;
runtime.stage_hot_update_result(update)?;
```

Compilation errors and compatibility errors are reported as structured failures.
They do not advance the active version.

## Apply At A Safe Point

The host chooses the safe point. A game server might check at a tick boundary; a
job runner might check between jobs.

```rust
if let Some(report) = runtime.check_reload_at_tick_boundary()? {
    for line in report.render_lines() {
        println!("{}", line.text);
    }
}
```

If the report contains a new version, future calls enter that version. If the
update was rejected, the runtime keeps serving the previous version.

## Example Programs

The repository includes runnable hot reload examples under
`examples/src/bin/hot_reload_function_swap` and
`examples/src/bin/hot_reload_function_swap_invalid`. They demonstrate a
compatible function body change and an incompatible update that is rejected.
