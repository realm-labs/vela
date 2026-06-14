---
title: "快速开始"
description: "Vela 快速开始文档。"
---

最快的体验方式是使用 Playground，选择示例、修改源码，然后运行 `main`。

```text
fn main() {
    let rewards = { "gold": 10, "xp": 25 };
    return rewards["gold"] + rewards["xp"];
}
```

Rust 宿主集成入口会使用 Engine 编译源码、创建 Runtime，并用明确的参数和执行预算调用脚本函数。
