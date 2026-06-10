# Clean Identity Refactor

This document is the architecture entrypoint for the breaking definition
registry and linked-bytecode refactor. The executable checklist lives in
[definition-registry-linked-bytecode-refactor-plan.md](../definition-registry-linked-bytecode-refactor-plan.md).

## Policy

This track does not preserve old internal compatibility. The following surfaces
may be deleted or replaced as part of the refactor:

- handwritten stdlib ID constants such as `standard_ids.rs`;
- raw `0xff00_...` stdlib, builtin, type, variant, and field ID spaces;
- old bytecode instruction shapes that mix runtime operands with diagnostic
  names;
- runtime string fallback dispatch for native, stdlib, method, and script
  calls;
- old serialized `ProgramImage` assumptions;
- identity maps carried through `CompilerOptions`;
- public or internal engine, compiler, VM, and runtime APIs that exist only to
  preserve the old implementation shape.

Product compatibility still matters where it is part of the language contract:
hot reload ABI checks, schema ABI checks, source-spanned diagnostics,
HostAccess validation, reflection permissions, execution budgets, GC roots, and
external host safety invariants must remain enforced.

## Target Shape

The architecture target separates four concepts that are currently mixed in
several paths:

```text
source spelling        names from source text
semantic identity      typed DefId values from canonical DefPath values
runtime dispatch key   dense handles, slots, and linked targets
diagnostic name        debug/reflection names in side tables
```

The desired pipeline is:

```text
stdlib / host / script declarations
        |
DefinitionRegistry
        |
compiler emits unlinked bytecode with typed DefIds
        |
linker resolves DefIds to dense handles, slots, and cache-ready operands
        |
VM executes linked bytecode only
```

Names are source and debug data. `DefId` values are semantic identity.
Handles, slots, and linked targets are runtime operands.

## Runtime Image Context

The runtime image/state split in
[runtime-image-state-refactor-plan.md](../runtime-image-state-refactor-plan.md)
is supporting context for this refactor. `ProgramVersion` and runtime images
should own linked code, debug tables, runtime handle layouts, profile metadata,
and cache invalidation metadata. Per-runtime mutable state such as globals,
script heap roots, retained runtime values, and inline caches remains
runtime-local.

## Phase 0 Test Inspection

Task 0.1 searched the repository for legacy ID compatibility tests. Existing
`standard_id_dispatch` and `standard_string_id_dispatch` tests exercise current
ID-based stdlib dispatch behavior through `standard_ids`; they do not assert
that old raw numeric ID values are a compatibility contract. They should remain
until the manifest, registry, and runtime binding replacement tasks provide
equivalent behavior coverage. Tests that assert raw legacy ID numbers should be
removed or rewritten when found.
