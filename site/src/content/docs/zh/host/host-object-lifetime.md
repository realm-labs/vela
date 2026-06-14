---
title: "Host 对象生命周期"
description: "Vela Host 对象生命周期文档。"
---

本页将记录宿主对象 identity、lifetime 和 stale-reference 行为。

## 后续覆盖内容

- HostRef identity 和 generation checks。
- Call-scoped HostAccess。
- Stale host reference diagnostics。
- 为什么脚本不能持有 Rust `&mut T`。
- Runtime-owned script values 和 Rust-owned host objects 的区别。
