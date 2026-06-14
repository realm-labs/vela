---
title: "Attributes"
description: "Attributes documentation for Vela."
---

Attributes attach structured metadata to declarations, fields, variants, statements, or other supported syntax nodes. The parser accepts the shape; semantic phases, host registration, reflection, and tooling define what each attribute means.

## Syntax

An attribute starts with `#` and uses bracketed metadata. Arguments may be positional or named, and values can be literals, paths, arrays, or maps.

```vela
#[event("player.level_up")]
pub fn on_level_up(player, amount: i64) {
    player.level += amount
}

#[schema(name = "Reward", tags = ["economy", "drop"])]
struct Reward {
    code: string
    amount: i64
}
```

## Metadata, Not Macros

Attributes do not expand code and do not run arbitrary script during compilation. They are metadata for systems such as event routing, host schemas, reflection visibility, diagnostics, or future tooling.

## Reflection And Hot Reload

Attribute metadata can be reflected when the runtime grants permission. Public ABI-affecting attributes should be treated as part of the reload compatibility surface when host code depends on them.

## Common Errors

Unknown attributes are accepted only when the active compiler or host policy allows them. Invalid argument shapes, duplicate metadata where uniqueness is required, or attributes on unsupported targets should produce source-spanned diagnostics.
