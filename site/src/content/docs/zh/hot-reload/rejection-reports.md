---
title: "拒绝报告"
description: "Vela 如何解释失败的热更新。"
---

更新被拒绝是正常运维事件。拒绝表示 runtime 保留旧程序版本，并产生报告
说明 candidate 为什么不能安全应用。

## 报告形态

报告应该既适合机器处理，也适合渲染给人看。它应包含更新状态、源码标
签、可用 span、旧版本和 candidate 版本身份，以及具体兼容性失败原因。

渲染后的文本行适合日志，但 dashboard 或部署 gate 应优先使用结构化字
段。

## 常见原因

典型拒绝原因包括：

```text
语法或语义编译错误
缺失 module 或 unresolved import
重复声明
函数 ABI 不匹配
schema ID 复用
field 或 variant 不兼容
effect 或 permission 扩大
顶层源码副作用
```

## Runtime 安全性

被拒绝的更新不会部分生效。活动调用帧继续使用旧代码，新调用继续进入之
前的当前版本，candidate 会被丢弃或交给宿主策略处理。

## 运维建议

部署工具应展示主错误、related locations 和 repair hints。例如导出 event
函数删除参数时，报告应同时指向旧 ABI 和导致 mismatch 的新声明。
