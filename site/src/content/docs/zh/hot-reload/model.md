---
title: "热更新模型"
description: "Vela 如何替换代码，同时保留正在运行的调用帧。"
---

Vela 把热更新设计成 runtime versioning，而不是源码文本补丁。一次成功
更新会创建新的 `ProgramVersion`，其中包含自己的代码、registry 快照、
ABI 元数据和缓存状态。

## 版本化程序

函数调用通过稳定的函数身份间接解析。脚本调用
`billing.on_invoice_paid(...)` 时，runtime 会先找到当前
`ProgramVersion`，再进入该函数对应的 `CodeObject`。

热更新替换的是后续调用使用的映射关系。它不会修改已经在栈上执行的
bytecode，也不会原地修改旧 registry。

## 正在运行的调用帧

已经进入旧版本的调用帧会继续执行旧 `CodeObject`。safe point 接受更新
后，新的调用才会进入新版本。旧版本会一直保留到所有引用它的调用帧退
出。

这是热更新可靠性的核心边界：函数不会在执行到一半时突然切换指令流。

```text
旧 event frame -> 旧 CodeObject
safe point 应用更新
新 event frame -> 新 CodeObject
旧 CodeObject 在旧 frame 退出后释放
```

## Registry 快照

每个版本拥有自己的 `TypeRegistry` 快照。反射、诊断、编辑器元数据和 ABI
检查都观察对应版本的快照。runtime 反射不能添加字段、删除方法，也不能
monkey patch 类型结构。

## 可以更新什么

热更新面向函数和模块更新：函数体、本地逻辑、私有 helper、兼容的导出
函数以及兼容的 schema 增量。ABI、schema、capability 和源码边界检查会
拒绝让旧调用帧或宿主集成变得含糊的更新。
