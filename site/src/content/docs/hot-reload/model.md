---
title: "Hot Reload Model"
description: "How Vela replaces code while preserving active call frames."
---

Vela treats hot reload as a runtime versioning problem, not as source text
patching. A successful update creates a new `ProgramVersion` with its own code,
registry snapshot, ABI metadata, and cache state.

## Versioned Programs

Function calls resolve through stable function identity. When a script calls
`billing.on_invoice_paid(...)`, the runtime looks up the current
`ProgramVersion` and then enters the `CodeObject` for that function.

Reloading replaces that mapping for future calls. It does not rewrite bytecode
already running on the stack, and it does not mutate the old registry in place.

## Active Frames

Active frames continue on the code version they entered with. New calls after a
successful safe point enter the new code version. The old version remains alive
until all frames that reference it have returned.

This rule is the core reliability boundary: a function never changes its
instruction stream halfway through execution.

```text
old event frame -> old CodeObject
safe point applies update
new event frame -> new CodeObject
old CodeObject is released after old frames exit
```

## Registry Snapshots

Each version owns a `TypeRegistry` snapshot. Reflection, diagnostics, editor
metadata, and ABI checks observe the snapshot for the version they are working
with. Runtime reflection cannot add fields, remove methods, or monkey patch type
structure.

## What Reload Can Change

Hot reload is intended for function and module updates: Function bodies, local
logic, private helpers, compatible exported functions, and compatible schema
additions. ABI, schema, capability, and source-boundary checks reject updates
that would make running code or host integrations ambiguous.
