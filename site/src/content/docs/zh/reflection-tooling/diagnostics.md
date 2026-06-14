---
title: "诊断"
description: "Vela 如何报告 parser、semantic、runtime、reflection 和 reload 错误。"
---

诊断是 runtime contract 的一部分。Vela 错误应携带足够结构化信息，供 CLI
输出、编辑器标记、热更新报告和宿主日志使用。

## 诊断数据

有用的诊断通常包含：

```text
错误类型
source span
message
related locations
候选名称
repair hint
涉及 runtime 执行时的 call stack
```

脚本声明和许多 runtime error 都可以提供 source span。宿主生成的 schema
可能没有 source span。

## 候选提示

反射和宿主 schema 错误应尽量包含 candidates。字段拼写错误可以指向注册
类型上的相近字段。

```text
FieldNotFound
type: Player
field: levle
candidates: ["level"]
```

## Runtime 上下文

runtime 诊断应跨脚本调用、native call、host access 和 reflection 保留
call-stack 与 source 信息。热更新诊断应包含 candidate 无法应用时的版本和
update 上下文。

## CLI 渲染

`vela_cli` 会为脚本执行渲染 source error 和 VM error。它是结构化诊断的一
个消费者，而不是唯一表示形式。
