---
title: "Runtime 更新流程"
description: "宿主侧编译、暂存和应用 Vela 更新的流程。"
---

嵌入宿主负责更新循环。Vela 提供编译、stage、safe point 应用和报告 API，
宿主决定什么时候让更新生效。

## 初始版本

初始版本在 runtime 开始处理业务前编译。仓库示例使用
`compile_hot_reload_initial`，再从该版本创建 runtime。

```rust
let initial = engine.compile_hot_reload_initial(source)?;
let mut runtime = Runtime::from_hot_reload_version(engine, initial);
```

初始编译会生成 bytecode、metadata 和基线 ABI 快照。

## 暂存更新

源码变更会被编译成 update candidate。stage 后当前 runtime 版本仍然保持
活跃，直到下一个 safe point。

```rust
let update = runtime.compile_hot_reload_update(updated_source)?;
runtime.stage_hot_update_result(update)?;
```

编译错误和兼容性错误会以结构化失败返回，不会推进当前版本。

## 在 Safe Point 应用

safe point 由宿主选择。游戏服务器可以在 tick 边界检查；任务系统可以在
两个 job 之间检查。

```rust
if let Some(report) = runtime.check_reload_at_tick_boundary()? {
    for line in report.render_lines() {
        println!("{}", line.text);
    }
}
```

如果 report 中包含新版本，后续调用进入新版本。如果更新被拒绝，runtime
继续使用旧版本。

## 示例程序

仓库包含 `examples/src/bin/hot_reload_function_swap` 和
`examples/src/bin/hot_reload_function_swap_invalid`。它们分别展示兼容的函数
体更新，以及会被拒绝的不兼容更新。
