---
title: "HostAccess 写穿模型"
description: "Vela 的宿主读、写、mutation、remove 和 call 如何立即路由。"
---

`HostAccess` 是调用作用域的 host effects 边界。它不是事务日志，也不会在
后续操作失败时回滚之前已经完成的写入。

## 写穿语义

```vela
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
```

复合赋值会解析成 host mutation：

```text
resolve HostAccessSpec(Mutate(Add), player.level)
validate schema, capability, generation, and adapter policy
read or mutate current host value
write the result immediately
```

脚本在同一次调用里可以观察到之前的写入，因为 adapter state 已经改变。

## 拒绝点

Host access 可以被 schema、capability、generation 或 adapter policy 拒绝。
常见情况包括 read-only fields、缺少 `HostWrite`、字段写入被拒绝、方法调用
被拒绝、stale handles，以及不支持 keyed access。

```vela
fn main(player: Player) {
    player.id = 8; // 如果 Player.id 是只读字段，会被拒绝
}
```

## Adapter 契约

Adapter 先 resolve access，再执行操作。

```rust
fn resolve_host_access(&self, spec: HostAccessSpec<'_>)
    -> HostResult<ResolvedHostAccess>;

fn mutate_host(
    &mut self,
    access: ResolvedHostAccess,
    target: HostTargetInstance<'_>,
    op: HostMutationOp,
    rhs: HostValue,
) -> HostResult<()>;
```

复杂 host collection mutation 应由 adapter 定义。默认模型不会把宿主集合 clone
成脚本值、修改 clone、再写回去。
