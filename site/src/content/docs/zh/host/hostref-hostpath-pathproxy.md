---
title: "HostRef、HostPath、PathProxy"
description: "Vela 用来替代 Rust 引用的 handles 和 path objects。"
---

`HostRef`、`HostPath` 和 `PathProxy` 是宿主状态 handle 的核心。它们让脚本
可以定位 Rust 拥有的状态，但不会借用它。

## HostRef

`HostRef` 用 type、object ID 和 generation 标识一个宿主对象。

```rust
pub struct HostRef {
    pub type_id: HostTypeId,
    pub object_id: HostObjectId,
    pub generation: u32,
}
```

Generation 防止对象 ID 复用后，旧 handle 写到另一个对象上。

## HostPath

`HostPath` 是 materialized readable path，用于 diagnostics、reflection、
fixtures 和 embedding APIs。

```rust
let path = HostPath::new(player_ref).field(FieldId::new(1));
```

热路径 bytecode 通常存储 interned `HostTargetPlan`，而不是每次访问都构造
`HostPath`。

## PathProxy

`PathProxy` 保存 root `HostRef`、target plan 和动态 index/key 参数。当 host
method 或 native function 需要携带嵌套 host target 时，可以使用它，而不是
暴露 Rust 引用。

```rust
let proxy = PathProxy::new(player_ref, plan)
    .key("gold")
    .field(FieldId::new(2));
proxy.add(
    adapter,
    &mut access,
    HostValue::Scalar(vela_common::ScalarValue::I64(1)),
    None,
)?;
```

Proxy 仍然通过 `HostAccess` 路由，所以 schema、capability、generation 和
adapter checks 仍然生效。
