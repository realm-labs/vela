---
title: "Completion And Editor Metadata"
description: "Metadata Vela keeps for completion, hover, diagnostics, and future LSP support."
---

Vela is designed so editor support can share the same semantic and reflection
metadata used by the compiler and runtime. A full LSP is not part of the MVP,
but the architecture keeps the required data available.

## Metadata Sources

Editor features use a combination of parser spans, module graph bindings,
`TypeFact` analysis, `TypeRegistry` metadata, and reflection descriptors.

```text
completion -> SymbolTable + TypeFact + TypeRegistry
hover -> TypeFact + docs + effects + declaration origin
go to definition -> BindingMap + declaration origin
diagnostics -> parser + semantic model + registry
semantic tokens -> tokens + resolved symbols
```

## Completion Quality

Known host refs, known script records, type hints, and narrowed enum variants
should produce precise completions. Unknown dynamic values degrade to `Any`
rather than blocking bytecode generation.

## Source Origins

Script declarations can carry source spans. Host-generated schemas may carry
docs and optional origins, but they do not need fake source locations.

## Runtime Boundary

Editor metadata is descriptive. It does not grant permission to mutate runtime
type structure, bypass reflection policy, or monkey patch registered schemas.
