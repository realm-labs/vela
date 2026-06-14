---
title: "Host Types And Schemas"
description: "How Rust host types become stable script-visible schemas."
---

Host schemas define what script code may see. They describe type names, fields,
methods, index capability, effects, stable IDs, and reflection access.

## Schema Surface

```rust
#[derive(Debug, ScriptHost)]
#[script(path = "examples::host_type_methods::ItemStack")]
struct ItemStack {
    #[script(get, set, hint = "i64")]
    count: i64,
}
```

The script sees `ItemStack.count` as a readable and writable `i64` field.
Rust still owns the object. Reads and writes pass through host access checks.

## Concrete Host Types

Scripts do not see Rust generics. A host can register a concrete map-like type
with an index capability and a script-facing name.

```rust
fn string_item_map_type() -> HostTypeSpec {
    HostTypeSpec::new(
        TypeDesc::new(TypeKey::new(TypeId::new(8_801), "StringItemMap"))
            .index_capability(
                HostIndexCapability::new()
                    .readable(true)
                    .writable(true)
                    .key_type("string")
                    .value_type("ItemStack"),
            ),
    )
}
```

```vela
player.inventory.items["gold"].count += amount;
```

The VM does not treat this as a Rust `BTreeMap`. It treats it as a registered
host type with keyed path support.

## Compatibility

Stable IDs and schema hashes are part of hot reload and reflection. Changing a
field's writability, type hint, method effect, or callable surface can make a
new program image incompatible with an existing runtime.
