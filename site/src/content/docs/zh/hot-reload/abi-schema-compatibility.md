---
title: "ABI 和 Schema 兼容性"
description: "决定 Vela 热更新是否安全的规则。"
---

热更新兼容性是保守的。只有当新代码可以和旧调用帧、宿主 schema、反射
元数据、已注册 capability 共存时，更新才会被接受。

## 函数 ABI

函数体变更是最常见的 reload 场景。本地变量、私有 helper、兼容的新 public
函数通常可以接受。

导出函数或宿主会调用的函数检查更严格。删除参数、重排参数、改变必要返
回语义，或在没有宿主批准的情况下扩大 effect，可能会被拒绝。

## Schema 兼容性

struct、enum、trait、field、method、variant、module 和 function 都使用稳
定 ID。名称用于诊断，但兼容性不能只依赖名称。

通常安全的变更包括：

```text
添加带默认值的字段
保留 FieldId 的字段重命名
添加方法
添加 enum variant
添加私有 helper 函数
```

通常会被拒绝的变更包括：

```text
把 FieldId 或 VariantId 复用到不同含义
删除旧代码需要的字段
不兼容地改变已有 variant 结构
删除导出函数参数
未经批准扩大 host effect
```

## Effects 和权限

capability 需求也是兼容性边界的一部分。如果 reload 把纯函数改成需要
host write、random、time、文件系统或 event 权限的函数，必须由宿主策略
批准。

## 反射稳定性

反射观察版本化 registry 快照。reload 可以创建新 registry，但不能原地
修改旧 registry。这让活动调用帧、调试器视图和管理工具保持一致。
