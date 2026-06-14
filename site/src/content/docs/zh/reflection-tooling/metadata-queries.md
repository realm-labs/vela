---
title: "元数据查询"
description: "查询 Vela type、module、function、field 和 method。"
---

元数据查询返回的是复制出的 reflection value。它描述已注册 schema 和脚本
声明，但不暴露可变 runtime 内部结构。

## Type 和 Value

使用 `reflect::type_info(name)` 按名称查找类型，使用 `reflect::type_of(value)`
检查一个值。

```vela
let player_type = reflect::type_info("Player");
let current_type = reflect::type_of(player);

if reflect::kind(player_type) == "host" {
    return reflect::fields(player_type);
}
```

## 成员和 Variant

`reflect::fields`、`reflect::methods`、`reflect::variants` 和
`reflect::traits` 返回结构化记录。registry 有数据时，这些记录可以包含
名称、稳定 ID、type hint、docs、attribute、source span、effects 和所需权
限。

## Module 和 Function

`reflect::module`、`reflect::modules`、`reflect::function`、
`reflect::functions` 和 `reflect::exports` 描述当前 registry 中安装的
module/function 表面。

当 engine 安装对应 standard natives 后，`math`、`time`、`option`、
`result`、`set`、`bytes` 等标准模块也可以被反射。

## 缺失元数据

宿主提供的 schema 不一定都有 source span。缺失 source origin 应表示为缺
省 metadata，而不是伪造文件位置。
