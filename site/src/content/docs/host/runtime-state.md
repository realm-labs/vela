---
title: "Runtime State"
description: "VM-owned state cells and compatible script reload."
---

Persistent module state makes ownership explicit.

```vela
state ticks: i64 = 0;
```

`state` creates one VM-owned cell per Runtime. Its required initializer runs
once during Runtime construction and only for newly added cells during reload.
Initializers are bounded and may construct script values, but cannot read
state, call host/native/reflection APIs, use capabilities, or suspend.
All cells are staged transactionally.

Rust can inspect or replace VM state through its fully qualified name:

```rust
runtime.set_state("main::ticks", 10_i64)?;
let ticks = runtime.state("main::ticks")?;
let typed: i64 = runtime.state_as("main::ticks")?.expect("ticks state");
```

`extern state` host bindings are removed. Ordinary VM state remains script-owned, participates in script GC and survives exact-compatible reload. Retained old closures keep their creation generation and its state. Rust host state is never placed under script GC.
