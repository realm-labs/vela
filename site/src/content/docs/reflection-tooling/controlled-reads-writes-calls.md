---
title: "Controlled Reads, Writes, And Calls"
description: "Dynamic reflection operations and their safety boundaries."
---

Reflection can dynamically read fields, write fields, and call functions or
methods. These operations are intentionally controlled and policy-gated.

## Reads

`reflect::get(target, field)` reads a field when the target shape and active
policy allow it.

```vela
let level = reflect::get(player, "level");
```

For host objects, the read is routed through host access machinery. For script
records and enums, reflection uses registered script metadata when available so
diagnostics can name the script type.

## Writes

`reflect::set(target, field, value)` writes only when the field is writable and
the active reflection and host policies allow the mutation.

```vela
reflect::set(player, "level", 12);
```

This still does not expose Rust `&mut T` to the script. Host mutation flows
through `HostRef`, `HostPath`, `PathProxy`, `HostAccess`, and the host adapter.

## Calls

`reflect::call(target, args...)` invokes a reflected function or method only
when it is marked callable through reflection. Effects, capabilities, budgets,
and argument conversion still apply.

## Failure Modes

Controlled operations can fail with unknown field, read-only field, permission
denied, stale host reference, argument mismatch, effect denial, or budget
exhaustion diagnostics. These failures should be handled as normal runtime
errors, not as reflection metadata corruption.
