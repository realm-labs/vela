---
title: "HostAccess Write-Through Model"
description: "How Vela host reads, writes, mutations, removals, and calls are routed immediately."
---

`HostAccess` is the call-scoped boundary for host effects. It is not a
transaction log and does not roll back earlier writes if a later operation
fails.

## Write-Through Semantics

```vela
fn main(player: Player) {
    player.level += 1;
    return player.level;
}
```

The compound assignment resolves to a host mutation:

```text
resolve HostAccessSpec(Mutate(Add), player.level)
validate schema, capability, generation, and adapter policy
read or mutate current host value
write the result immediately
```

Scripts observe previous writes in the same call because the adapter state has
already changed.

## Rejection Points

Host access can be rejected by schema, capability, generation, or adapter
policy. Examples include read-only fields, missing `HostWrite`, denied field
writes, denied method calls, stale handles, and unsupported keyed access.

```vela
fn main(player: Player) {
    player.id = 8; // rejected if Player.id is read-only
}
```

## Adapter Contract

Adapters resolve access first, then execute the operation.

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

Complex host collection mutation should be adapter-defined. The default model
does not clone a host collection into a script value, mutate the clone, and
write it back.
