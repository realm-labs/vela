---
title: "项目状态和路线图"
description: "Vela 项目状态和路线图文档。"
---

Vela 目前是预发布实现，但已经有较完整的可运行原型。稳定产品方向以 `docs/goal.md` 为准，技术契约以 `docs/architecture.md` 为准，当前 milestone 状态由 `docs/progress.md` 跟踪。

## 当前可用

当前代码库已经包含源码解析、HIR lowering、字节码编译、VM 执行、执行预算、非移动 GC 基础、数组、map、set、字符串、Option/Result 辅助方法、模块、runtime global、标准 native、反射元数据、宿主注册、HostAccess 写穿和热更新流程。

项目还包含浏览器 Playground、文档站、独立嵌入示例、conformance 风格测试、benchmark harness 和 parser fuzz 基础设施。

## 当前工作

当前实现重点在解释器和 inline cache 性能、宿主边界 fast path，以及在保持 HostAccess、热更新兼容性、诊断和受控反射这些产品契约的同时继续清理架构。

性能目标是先把非 JIT 解释器做好。JIT 是 post-MVP 方向，不是当前发布的前置条件。

## MVP 明确非目标

MVP 不包含脚本泛型、monkey patching、任意 `eval`、脚本 async/coroutine、JIT、完整 LSP、通过反射修改运行时类型结构，也不会把 Rust `&mut T` 引用暴露给脚本。

这些限制是有意的，用来保证热更新、宿主所有权、capability enforcement 和诊断仍然可控。

## 文档状态

本站会按章节逐步补齐。页面应该描述当前行为和稳定设计意图；如果提到计划中的能力，需要明确说明是未来工作，不能把未实现功能写成已经可用。
