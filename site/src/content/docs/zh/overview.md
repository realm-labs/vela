---
title: "概览"
description: "Vela 概览文档。"
---

Vela 是一个面向 Rust 宿主持有状态业务逻辑的 Hot Reload First 动态脚本语言。脚本表达规则，宿主拥有持久状态，并通过受控 HostAccess 边界完成读写。

## 设计重点

- Rust 宿主继续拥有 durable state。
- 脚本通过 HostRef、HostPath、PathProxy 和 HostAccess 修改宿主状态。
- 热更新以函数和模块版本为边界，旧调用帧继续运行旧 CodeObject。
- 反射可以查询元数据并执行受控读写调用，但不能修改类型结构。

## 当前文档状态

这版网站先建立完整大纲。正文会随着语言、标准库和宿主集成 API 稳定逐章补齐。
