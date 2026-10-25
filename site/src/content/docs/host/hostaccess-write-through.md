---
title: "Removed Host Write-Through"
description: "Transparent Rust write-through is no longer supported."
---

Transparent host reads, writes, compound mutation and adapters are removed.
Caller-borrowed Rust data cannot enter scripts through roots, wrappers, views,
constructors, methods, reflection or dependent extern-state bindings.

现有序列化值由脚本拥有；修改不会隐式写回 Rust。普通脚本结构体、集合、
迭代器、VM `state`、反射和热重载继续保留。

Existing synchronous native/value calls and serde snapshots remain. Rust
explicitly receives results. No replacement opaque-object, conversion or re-entry
API is introduced by this removal. See [embedding](/host/embedding-overview/).
