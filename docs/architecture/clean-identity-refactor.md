# Clean Identity Refactor

This document records retained definition-registry and linked-bytecode identity
contracts. The historical refactor checklist lives in
[definition-registry-linked-bytecode-refactor-plan.md](../archive/definition-registry-linked-bytecode-refactor-plan.md).

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
native effect/capability checks, reflection permissions, execution budgets, GC
roots, runtime-affinity and owned-value boundary invariants remain enforced.

## Target Shape

The architecture separates four concepts:

```text
source spelling        names from source text
semantic identity      typed DefId values from canonical DefPath values
runtime dispatch key   dense handles, slots, and linked targets
diagnostic name        debug/reflection names in side tables
```

The retained pipeline is:

```text
stdlib / owned native/value / script declarations
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
[runtime-image-state-refactor-plan.md](../archive/runtime-image-state-refactor-plan.md)
is supporting context for this refactor. `ProgramVersion` and runtime images
should own linked code, debug tables, runtime handle layouts, profile metadata,
and immutable cache/profile layouts. Script heap roots, VM state and retained
runtime values remain runtime-local. Generation-stable mutable caches/profiles
use exact-generation execution data; optional lane-local storage requires measured
need. Old frames/closures keep their owner rather than rebasing handles.

## Phase 0 Test Inspection

Task 0.1 searched the repository for legacy ID compatibility tests. Existing
`standard_id_dispatch` and `standard_string_id_dispatch` tests exercise current
ID-based stdlib dispatch behavior through manifest-derived stdlib method IDs;
they do not assert that old raw numeric ID values are a compatibility contract.
Update obsolete raw-ID expectations with the owning format/identity change;
retain safety and rejection coverage rather than deleting tests to pass.
