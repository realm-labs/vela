---
title: "非目标和约束"
description: "让 Vela 保持可嵌入和热更新安全的设计约束。"
---

Vela 是面向宿主持有业务逻辑的动态脚本语言。它不是 dynamic Rust，也不是
Lua clone。部分功能被明确排除在第一版之外。

## 语言非目标

MVP 不包含脚本侧泛型、按类型的函数重载、Rust-style borrow checker、任意
`eval`、macros、monkey patching、classes、脚本线程、async/coroutine hot
reload 或 JIT compilation。

## 宿主边界约束

脚本永远不会拿到真实 Rust `&mut T`。宿主修改必须通过 `HostRef`、
`HostPath`、`PathProxy`、`HostAccess` 和 host adapter。宿主状态不会放到
脚本 GC 下面。

## 反射约束

反射可以查询 metadata，并执行受控 read、write 和 call。它不能修改类型
结构、替换方法、添加字段，也不能形成 monkey-patching 系统。

## Runtime 约束

执行必须有预算。优化必须保留 source diagnostics、GC roots、hot reload
versioning、reflection permissions 和 host access checks。JIT 是 post-MVP
backend 目标，不是第一版 interpreter 的要求。
