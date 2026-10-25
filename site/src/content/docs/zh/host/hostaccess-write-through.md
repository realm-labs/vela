---
title: "已移除的宿主写穿"
description: "不再支持透明 Rust 写穿。"
---

透明宿主读取、写入、复合修改与适配器已删除。调用方借用的 Rust 数据不能通过
根、包装器、view、构造器、方法、反射或依赖的 extern-state 绑定进入脚本。

现有序列化值由脚本拥有；修改不会隐式写回 Rust。普通脚本结构体、集合、
迭代器、VM `state`、反射和热重载继续保留。

现有同步 native/value 调用和 serde 快照继续保留，Rust 显式接收结果。本轮删除
不引入替代 opaque 对象、转换或重入 API。参见[嵌入概览](/zh/host/embedding-overview/)。
