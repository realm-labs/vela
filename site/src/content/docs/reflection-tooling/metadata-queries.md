---
title: "Metadata Queries"
description: "Querying Vela types, modules, functions, fields, and methods."
---

Metadata queries return copied reflection values. They describe the registered
schema and script declarations without exposing mutable runtime internals.

## Types And Values

Use `reflect::type_info(name)` to look up a type by name and
`reflect::type_of(value)` to inspect a value.

```vela
let player_type = reflect::type_info("Player");
let current_type = reflect::type_of(player);

if reflect::kind(player_type) == "host" {
    return reflect::fields(player_type);
}
```

## Members And Variants

`reflect::fields`, `reflect::methods`, `reflect::variants`, and
`reflect::traits` expose structured records. These records may include names,
stable IDs, type hints, docs, attributes, source spans, effects, and required
permissions when the registry has that data.

## Modules And Functions

`reflect::module`, `reflect::modules`, `reflect::function`,
`reflect::functions`, and `reflect::exports` describe the module/function
surface installed in the current registry.

Standard modules such as `math`, `time`, `option`, `result`, `set`, and `bytes`
are reflected when the engine installs the corresponding standard natives.

## Missing Metadata

Host-provided schemas may not always have source spans. Missing source origins
should be represented as absent metadata, not as fake file locations.
