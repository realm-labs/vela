---
title: "核心概念"
description: "Vela 核心概念文档。"
---

本页将说明 Vela 的核心执行模型：源码编译、Engine 配置、Runtime 执行、HostAccess 边界和热更新。

## 后续覆盖内容

- Source、Engine、Program 和 Runtime 的职责。
- 脚本拥有的值和 Rust 宿主持有状态的区别。
- HostRef、HostPath、PathProxy 和 HostAccess 的整体关系。
- 函数级热更新和旧调用帧行为。
- capability 和 execution budget 边界。
