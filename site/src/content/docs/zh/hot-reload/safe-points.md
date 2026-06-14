---
title: "Safe Point"
description: "Vela 允许 staged update 变成当前版本的位置。"
---

Safe point 是 staged hot reload update 唯一可以变成当前程序版本的位置。
Vela 不会在任意 bytecode 指令中间打断执行并替换函数体。

## 目的

Safe point 给宿主一个可预测的代码替换边界。它避免一个正在执行的函数在
调用中途观察到不同的指令流或 registry。

常见 safe point 包括：

```text
事件结束
tick 边界
队列任务之间
显式 runtime.check_reload()
```

## 旧调用和新调用

safe point 接受更新后，只有后续调用使用新版本。正在执行的调用继续持有
旧 `CodeObject` 和旧 metadata 快照，直到返回。

因此，一个长时间执行的 event handler 可以用进入时的代码完成，而下一个
event handler 使用更新后的代码。

## 宿主责任

宿主应把 safe point 放在自然回到 Rust 控制权的边界上。脚本仍然不能拥有
无预算的无限执行路径；旧调用帧运行期间 execution budget 继续生效。

## 调试和报告

safe-point report 会说明 staged update 是否被应用、被拒绝或不存在。宿主
应记录这些 report 和源码标签，方便把拒绝原因关联到具体文件或部署。
