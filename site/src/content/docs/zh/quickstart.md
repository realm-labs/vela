---
title: "快速开始"
description: "Vela 快速开始文档。"
---

最快的体验方式是 [Playground](../playground/)。它在浏览器里运行 WASM 版 engine，适合尝试语法、集合、方法、诊断和返回值。

## 一个小脚本

```vela
fn main() {
    let rewards = { "gold": 10, "xp": 25 };
    let tags = ["daily", "vip"];

    if tags.contains("vip") {
        return rewards["gold"] + rewards["xp"];
    }

    return rewards["gold"];
}
```

在 Playground 中运行 `main`，结果会以 JSON 形式展示。

## 宿主持有状态

Vela 真正的使用场景通常是在 Rust 中嵌入。脚本看起来可以很直接：

```vela
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
```

但 `Player` 仍然由 Rust 宿主拥有。调用边界传入 host object binding，字段读写通过 HostAccess 路由：

```rust
let output = runtime.call(
    "main",
    CallArgs::new().with_host_mut("player", &mut player),
    CallOptions::new(10_000, 1024 * 1024, 64),
)?;
```

完整可运行版本在 `examples/src/bin/level_up`。

## 运行本地示例

在仓库根目录可以用 Cargo 运行独立示例：

```bash
cargo run -p vela_examples --bin level_up
cargo run -p vela_examples --bin native_function
cargo run -p vela_examples --bin hot_reload_function_swap
```

如果需要真实 Rust host object、native 函数、文件系统 capability 或热更新行为，请使用这些本地示例。浏览器 Playground 有意不暴露这些宿主资源。
