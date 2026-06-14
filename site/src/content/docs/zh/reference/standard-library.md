---
title: "标准库"
description: "Vela 当前 domain-neutral 标准库表面的概览。"
---

Vela 标准库保持 domain-neutral。游戏、计费、工作流或产品概念应来自宿主
注册、native functions、schemas 和 examples，而不是内建语言功能。

## 核心 Value Helpers

当前标准表面包括 strings、bytes、arrays、maps、sets、ranges、iterators、
Option-style values、Result-style values、math、time、random、context
helpers 和受控 I/O 的 helpers/methods。

仓库示例覆盖 `math`、`time`、`option`、`result`、`set`、`bytes` 和 `io`
等模块。

## Capabilities

有 effect 的 API 需要显式宿主 capability。time、random、event emit、host
read/write/call、standard I/O 和 file I/O 都应由嵌入宿主配置。

sandboxed file APIs 保持在配置的 filesystem root 下，并要求 I/O
capabilities。

## Iteration 和 Collections

arrays、maps、sets、ranges、strings、bytes 和宿主提供的 iterables 可以参与
`for` 循环以及部分 iterator-style helpers。lazy iterator adapters 和
terminal helpers 会和 collection materialization 分开 benchmark。

## Reference 状态

本页是概览，不是完整生成的标准库索引。长期目标是通过 metadata 暴露
names、params、return hints、docs、effects 和 reflection access，让该页最
终可以生成。
