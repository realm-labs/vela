---
title: "Vela 文档"
description: "Vela 文档。"
sidebar:
  hidden: true
---

# Vela 文档

Vela 是一个面向 Rust 应用的 Hot Reload First 动态脚本语言。Rust 宿主继续拥有持久状态，脚本负责表达业务规则；类型、native 函数、能力、预算和热更新策略都由宿主注册和控制。

可以从[概览](./overview/)了解整体模型，从[快速开始](./quickstart/)运行第一个脚本，也可以直接打开 [Playground](./playground/) 在浏览器里试语言特性。

## 主要阅读路径

- 语言基础从[词法结构和注释](./language/lexical-structure-comments/)开始。
- 宿主集成先看[嵌入概览](./host/embedding-overview/)和 [HostAccess 写穿模型](./host/hostaccess-write-through/)。
- 热更新语义看[热更新模型](./hot-reload/model/)。
- 当前实现范围看[项目状态和路线图](./project-status-roadmap/)。

## 当前范围

当前实现是一个可运行的预发布系统，已经包含解析、HIR、字节码、VM、标准库辅助函数、HostAccess、反射、热更新流程、示例、benchmark 和 WASM Playground。文档会描述当前契约，并在功能明确不属于 MVP 时直接标出边界。
