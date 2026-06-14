---
title: "Host 类型和 Schema"
description: "Rust host types 如何成为稳定的脚本可见 schema。"
---

Host schemas 定义脚本可以看到什么。它们描述类型名、字段、方法、index
capability、effects、stable IDs 和 reflection access。

## Schema Surface

```rust
#[derive(Debug, ScriptHost)]
#[script(path = "examples::host_type_methods::ItemStack")]
struct ItemStack {
    #[script(get, set, hint = "i64")]
    count: i64,
}
```

脚本会看到 `ItemStack.count` 是一个可读可写的 `i64` 字段。Rust 仍然拥有
对象本身。读写必须经过 host access checks。

## 具体 Host 类型

脚本不看到 Rust 泛型。宿主可以注册一个 map-like 具体类型，给它
script-facing 名称和 index capability。

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

VM 不把它当作 Rust `BTreeMap` 特判，而是把它当作一个带 keyed path 能力的
已注册 host type。

## 兼容性

Stable IDs 和 schema hashes 是 hot reload 与 reflection 的一部分。修改字段
可写性、type hint、method effect 或 callable surface，都可能让新的 program
image 和现有 runtime 不兼容。
