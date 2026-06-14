---
title: "性能和 Benchmark"
description: "Vela 性能和 Benchmark文档。"
---

本章属于 **反射和工具**。

## 本页目标

TODO：补充 性能和 Benchmark 的语义、示例、宿主边界和常见错误。

## 设计边界

- 不引入脚本侧泛型。
- 不向脚本暴露真实 Rust `&mut T`。
- 宿主状态修改必须通过 HostAccess 相关边界。

## 示例

TODO：补充可运行的 Vela 或 Rust embedding 示例。
