---
title: "Overview"
description: "Vela 同步语言与删除边界。"
---

Vela 是同步、解释执行、Hot Reload First 的动态脚本语言。保留普通函数、结构体、枚举、trait、模块与包、脚本集合及迭代器、受控反射、非移动 GC 和有限执行预算。

现有独立同步 native/value API 与 serde 快照转换继续可用，脚本修改不会隐式写回 Rust。普通 VM `state` 在兼容热重载时保留；活动调用与旧闭包保留精确的旧代码代际、嵌套调用和状态。

已删除 Service/Provider、脚本 async/tasks、借用的 Rust 根/参数/返回值/view、租约/释放、透明宿主路径、依赖的 `extern state` 绑定和 Host 构造器/方法。普通脚本状态和 LSP 工作线程不是删除对象。没有新增 opaque 对象、codec、转换或重入 API；这些设计问题单独延期。

继续使用 `native_function`、`serde_value`、`modules`、`package_dependency_demo`、`script_state` 和脚本热重载示例。工具元数据使用严格格式 4；旧格式与已删除字段被拒绝。修改后的夹具不代表新的原生编辑器采集，也不代表完整仓库验收通过。

参见 [English reference](/overview/) 和仓库中的权威目标、架构、进度与删除计划文档。
