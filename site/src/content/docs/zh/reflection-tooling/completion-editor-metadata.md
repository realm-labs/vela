---
title: "补全和编辑器元数据"
description: "Vela 为补全、hover、诊断和未来 LSP 保留的元数据。"
---

Vela 的设计目标是让编辑器支持复用 compiler 和 runtime 使用的语义与反射
元数据。完整 LSP 不属于 MVP，但架构会保留所需数据。

## 元数据来源

编辑器功能会组合 parser spans、module graph bindings、`TypeFact` 分析、
`TypeRegistry` metadata 和 reflection descriptors。

```text
completion -> SymbolTable + TypeFact + TypeRegistry
hover -> TypeFact + docs + effects + declaration origin
go to definition -> BindingMap + declaration origin
diagnostics -> parser + semantic model + registry
semantic tokens -> tokens + resolved symbols
```

## 补全质量

已知 host ref、已知 script record、type hint 和 narrowed enum variant 应提
供精确补全。未知动态值会退化为 `Any`，而不是阻止 bytecode 生成。

## Source Origins

脚本声明可以携带 source span。宿主生成的 schema 可以携带 docs 和可选
origin，但不需要伪造源码位置。

## Runtime 边界

编辑器元数据是描述性的。它不会授权修改 runtime 类型结构、绕过 reflection
policy，或 monkey patch 已注册 schema。
