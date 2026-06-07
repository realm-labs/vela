# 热更新

Vela 把热更新作为 runtime 合约，而不是开发期附加功能。

## 函数级替换

新源码会编译成新的 program version。新的调用进入新代码。已有调用帧继续运行旧的 `CodeObject`，直到自然返回。

## 兼容性

热更新兼容性由 ABI 和 schema 检查约束。兼容的新增和函数体修改可以接受。不兼容的函数签名、schema、effect 或 access 变化会被拒绝，并且不会推进当前 runtime version。

## Safe Point

Host 决定何时检查或应用待处理热更新。游戏服务器通常在事件边界或 tick 边界处理，这样状态变化更可预测。

## Playground 范围

浏览器 playground 主要提供编译和运行反馈。完整热更新 staging 依赖 host runtime policy 和 version 管理，所以放在独立 Rust 示例中展示。

## 典型 Host 循环

```rust
runtime.stage_hot_reload_update_file("scripts/logic.vela")?;

if let Some(report) = runtime.check_reload_at_tick_boundary()? {
    println!("{report:?}");
}
```

被拒绝的更新不会替换当前版本。被接受的更新会重建 runtime 元数据，后续调用进入新代码。
