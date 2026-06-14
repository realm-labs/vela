---
title: "Reflection Overview"
description: "What Vela reflection can inspect and what it cannot mutate."
---

Reflection gives scripts and host tooling a controlled view of Vela metadata.
It exists for host integration, diagnostics, admin tools, debuggers, editors,
and hot reload checks.

## What Reflection Can See

Reflection can query types, fields, methods, variants, traits, modules,
functions, attributes, source origins, effect metadata, and permission metadata.
It can also inspect the runtime type of a value.

```vela
fn main(player: Player) {
    let player_type = reflect::type_of(player);
    let fields = reflect::fields(player_type);
    let level = reflect::field(player, "level");
    return reflect::name(player_type);
}
```

## Controlled Operations

Reflection can perform controlled reads, writes, and calls when the active
policy permits them. Those operations still go through the same runtime and host
access boundaries as normal script code.

Reflection is not a bypass around `HostAccess`, execution budgets, capability
checks, read-only fields, or stale host reference validation.

## What Reflection Cannot Do

Reflection cannot mutate type structure at runtime. It cannot add fields,
remove methods, replace functions, monkey patch types, or evaluate generated
source strings.

## Versioned Metadata

Hot reload creates new registry snapshots. Reflection observes the registry for
the relevant program version, which keeps active frames and tooling views stable
while new calls move to the new version.
