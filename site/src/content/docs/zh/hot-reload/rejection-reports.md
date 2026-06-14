---
title: "拒绝报告"
description: "Vela 拒绝报告文档。"
---

本章属于 **热更新**。

## 本页目标

TODO：补充 拒绝报告 的语义、示例、宿主边界和常见错误。

## 设计边界

- 不引入脚本侧泛型。
- 不向脚本暴露真实 Rust `&mut T`。
- 宿主状态修改必须通过 HostAccess 相关边界。

## 示例

TODO：补充可运行的 Vela 或 Rust embedding 示例。
