# Global 和 Serde

Vela 支持模块级 `global` 声明，并由 runtime 持久存储支持。

## Script-Owned Global

```vela
struct ServerState {
    level: int,
    total_gold: int,
}

global state: ServerState;

fn handle_tick(gold) {
    state.total_gold += gold;
    return state.total_gold;
}
```

Rust 插入初始值：

```rust
runtime.insert_global(
    "state",
    OwnedValue::record("ServerState", [("level", 1), ("total_gold", 0)]),
)?;
```

## Serde Snapshot

启用 `serde` feature 后，Rust struct/enum 可以转换成 VM-owned script value，也可以从返回值解码。这是 snapshot 路径，和 host handle 分开。

脚本 owned 数据用 serde。脚本需要直接修改 Rust-owned 状态时，用 host handle。
