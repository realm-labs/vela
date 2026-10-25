---
title: "运行时状态"
description: "VM 自有状态与脚本热重载。"
---

`state ticks: i64 = 0;` 为每个 Runtime 创建脚本拥有的状态单元。初始化器有预算限制，已有状态在兼容热重载时保留，不重新运行初始化器。旧闭包保留创建时的代码代际及其状态。

Rust 可通过现有 `set_state`、`state`、`state_as` 显式读写脚本状态。`script_state` 示例继续展示这条值边界。

依赖透明宿主访问的 `extern state` 绑定已删除。它不是普通 VM `state`；删除它不会删除脚本状态或热重载。Rust 宿主状态不进入脚本 GC，也没有替代绑定 API。
