---
title: "Runtime Global"
description: "把持久 runtime 状态绑定到脚本 global declarations。"
---

Runtime globals 把模块级脚本声明连接到 Rust 提供的实例。声明是 metadata；
值由 Rust 提供。

## 声明 Globals

```vela
struct ServerState {
    level: i64,
    name: String,
    total_gold: i64,
}

global state: ServerState;

fn handle_tick(level_gain, gold_gain) {
    state.level += level_gain;
    state.total_gold += gold_gain;
    return state.level + state.total_gold;
}
```

全限定 global 名称按模块生成，例如 `main::state`。

## 插入值

Rust 插入或替换 runtime instance。

```rust
runtime.insert_global("main::state", &initial_state)?;
runtime.set_global("main::state", &updated_state)?;
```

启用 serde 时，Rust structs 可以作为 script-owned snapshot values 插入。如果
global 应该是 persistent host object，也可以使用 `insert_host_global`。

## 读回

`global_as` 可以反序列化 runtime global，不需要宿主先 materialize 中间
`OwnedValue`。

```rust
let final_state: ServerState = runtime
    .global_as("main::state")?
    .expect("state global should exist");
```

Globals 会跨调用保留，并作为 runtime roots 参与 GC。
