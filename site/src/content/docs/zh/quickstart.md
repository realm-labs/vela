---
title: "快速开始"
description: "Vela 同步执行与脚本自有值边界。"
---

Vela 仅同步执行。保留普通模块、包、结构体、trait、脚本集合与迭代器、VM 自有 `state`、受控反射和精确旧代际热重载。

保留现有独立的同步 native/value 调用与 serde 快照转换。序列化输入由脚本拥有；脚本修改不会隐式写回 Rust，Rust 必须显式接收结果。`native_function`、`serde_value`、`modules`、`package_dependency_demo`、`script_state` 和脚本热重载示例继续可用。

已移除借用的 Rust 根、参数、返回值、view、租约及显式释放操作，透明 Host 路径、Host 构造器和方法，以及依赖这些能力的 `extern state` 绑定。普通 VM `state` 不受此删除影响。没有新的 opaque 对象、codec、转换或重入替代 API。

工具元数据格式为 **4**：拒绝旧格式 3、Host/view 类型、scoped-resource、构造器、storage/receiver 能力等已移除字段（包括 null 或空值）。注册 Value 类型仍保留标识与 ABI 校验。LSP 工作线程及请求调度不是脚本异步执行。

详情与现有调用示例见 [English reference](/quickstart/)。
