---
title: "错误和诊断码"
description: "常见 Vela 诊断族以及如何理解它们。"
---

Vela 诊断按子系统组织。精确 code 列表仍在稳定中，因此本页记录持久的诊
断族，而不是假装已有完整自动生成目录。

## Parse 和 Semantic Errors

parser 和 semantic errors 覆盖非法语法、unresolved names、重复声明、无
效赋值目标、被拒绝的泛型类型语法、顶层副作用和非法 module imports。

这些错误应尽量包含 source spans 和 related locations。

## Runtime Errors

runtime errors 覆盖 type guard failures、bad calls、arithmetic failures、
budget exhaustion、stack depth limits、missing entries 和 value conversion
failures。

## Host 和 Reflection Errors

host 和 reflection errors 覆盖 field not found、field not writable、
permission denied、required capability missing、stale host ref generation、
unknown reflected item 和 reflect-call denial。

## Hot Reload Errors

hot reload diagnostics 覆盖 compile failures、ABI mismatches、schema
incompatibilities、effect/access expansion、source graph problems 和被拒绝的
顶层副作用。

报告应明确说明更新被拒绝时，之前的 active version 仍然是当前版本。
