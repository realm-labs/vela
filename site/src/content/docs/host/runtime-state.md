---
title: "Runtime State"
description: "VM-owned state cells and host-owned extern state bindings."
---

Persistent module state makes ownership explicit.

```vela
state ticks: i64 = 0;
pub extern state server: ServerState;
```

`state` creates one VM-owned cell per Runtime. Its required initializer runs
once during Runtime construction and only for newly added cells during reload.
Initializers are bounded and may construct script values, but cannot read
state, call host/native/reflection/provider APIs, use capabilities, or suspend.
All cells are staged transactionally.

Rust can inspect or replace VM state through its fully qualified name:

```rust
runtime.set_state("main::ticks", 10_i64)?;
let ticks = runtime.state("main::ticks")?;
let typed: i64 = runtime.state_as("main::ticks")?.expect("ticks state");
```

`extern state` owns no script value. Bind it before construction and use
state-specific replacement or reload staging afterward:

```rust
let mut builder = Runtime::builder(engine, program)?;
builder.bind_extern_state("main::server", server)?;
let mut runtime = builder.build()?;
runtime.replace_extern_state("main::server", replacement)?;
runtime.stage_extern_state("main::added_server", added)?;
```

Extern reads yield host references, nested mutation goes through HostAccess,
and Vela cannot replace the root. VM cells participate in script GC; Rust host
objects do not. Exact-compatible reload preserves both ownership forms without
rerunning existing initializers.
