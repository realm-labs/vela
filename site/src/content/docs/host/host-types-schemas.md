---
title: "Registered Value Schemas"
description: "The synchronous, script-owned Vela boundary."
---

Tooling exports existing registered Value types, fields, traits, synchronous native signatures, documentation and source spans. These describe script-owned values, not transparent Rust objects.

Tooling schema format **4** rejects schema 3, Host/view facts, scoped-resource metadata, receiver/storage capabilities and constructor fields. Value type bindings retain their stable identity and ABI fingerprint. Unknown removed fields are errors even when null or empty. There is no replacement opaque schema.

Source structs, enums and traits remain available. See [records and structs](/data/records-structs/) and [snapshot values](/host/serde-snapshot-values/).
