---
title: "Controlled Reads, Writes And Calls"
description: "The synchronous, script-owned Vela boundary."
---

Reflection can inspect script metadata and perform permitted operations on script-owned values. The registry is structurally read-only: reflection cannot add fields, mutate type structure or monkey patch methods.

```vela
struct Player { level: i64 }
fn main() {
    let player = Player { level: 1 };
    reflect::set(player, "level", 12);
    return reflect::get(player, "level");
}
```

Register the existing standard natives and grant the required reflection capabilities in Rust. Permission, argument, unknown-field and budget errors remain normal runtime failures.

Reflection cannot recover borrowed children, transparent Rust paths, opaque layout or removed Host constructors/methods. `reflect_schema_mutation_denied` and `reflect_unknown_field_denied` retain independent rejection coverage over script values.
