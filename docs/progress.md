# Progress

This file is the current implementation status. Detailed historical progress
before this compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).

## Current Focus

M0-M11 are complete enough as a runnable prototype. Current work is centered on
M12/M13 reflection and standard-library completion, with targeted M14/M15
Engine API and hot-reload source workflow work as it unblocks embedding.

Post-MVP performance remains a separate track: optimize the non-JIT bytecode
interpreter toward Lua 5.x comparable gameplay workloads, then add debugger
runtime/DAP support and Cranelift JIT once the interpreter, inline-cache, and
conformance contracts are stable.

## Milestone Status

| Milestone | Status | Notes |
|---|---|---|
| M0-M6 | Complete | Source -> bytecode -> VM -> HostRef/HostPath/PatchTx -> hot reload loop exists. |
| M7 | Complete | Execution budgets, managed heap, GC roots, and managed heap entrypoints exist. |
| M8 | Complete enough | HIR, module graph, imports, declarations, binding maps, and compiler integration are active. |
| M9 | Complete enough | Broad executable language surface works; conformance catches edge cases. |
| M10 | Complete enough | Stable script metadata, shapes, slots, traits, and dispatch foundations exist. |
| M11 | Complete enough | HostRef, HostPath, PathProxy, PatchTx overlays, and rollback-safe host boundaries exist. |
| M12 | In progress | Reflection metadata surface and permission-aware queries are still being polished. |
| M13 | In progress | Standard library helpers are broad but still need final gameplay/string/math/context polish. |
| M14 | Partial | Engine APIs, native descriptors, context helpers, and macros exist in slices. |
| M15 | Partial | Function, descriptor, module, trait, schema, and source reload ABI checks exist. |
| M16 | Partial | Runtime diagnostics and common rendering have started. |
| M17 | Partial | Conformance fixture and demo harnesses exist; game-server demo can still expand. |
| M18 | Partial | VM baseline harness covers scalar, stdlib, host PatchTx, and managed heap workloads; official baselines remain. |
| M19-M20 | Not started | Interpreter optimization plus inline caches and specialization. |
| M21 | Not started | Debugger runtime hooks and DAP integration. |
| M22 | Not started | Cranelift JIT backend after interpreter/cache/debug contracts are stable. |
| M23 | Not started | Release hardening, public docs, validation gates, and performance targets. |

## Active Capabilities

- Source files use `.vela`; future bytecode-only artifacts use `.vbc`.
- Parser covers declarations, statements, expressions, attributes, and recovery
  with source spans.
- HIR owns module graph resolution, imports, declaration IDs, binding maps,
  type-hint metadata, and top-level effect checks.
- Bytecode compiler consumes HIR diagnostics and emits register bytecode for
  functions, closures, control flow, collections, records, enums, slots,
  host paths, method dispatch, Option/Result-style propagation, and iteration.
- VM supports managed heap execution, non-moving GC, execution budgets,
  script value methods, standard natives, reflection natives, and host-aware
  execution.
- Host mutation goes through HostRef, HostPath, PathProxy, PatchTx, overlays,
  permissions, and safe-point apply.
- Reflection covers types, fields, methods, variants, traits, modules,
  functions, attributes, permissions, source spans, controlled reads/writes,
  and controlled calls.
- Engine API registers schemas, native functions, context helpers, standard
  natives, reflection permissions, compiler options, and hot-reload policies.
- Hot reload validates function, method, module, trait, schema, effect, access,
  and source diagnostics before version advancement.
- CLI demo scripts and conformance fixtures use `.vela`.

## Current Gaps

- Finish M12/M13 polish around reflection metadata, permissions, standard
  library completeness, and gameplay helper coverage.
- Continue hardening M14/M15 embedding and production safe-point reload
  workflows.
- Expand M16/M17 diagnostics, fixtures, and game-server demo coverage.
- Keep M18+ performance work benchmark-driven and separate from semantic
  changes.
- Plan M21 debugger and M22 Cranelift JIT from stable source-span, frame-map,
  GC-root, budget, PatchTx, hot-reload, and conformance contracts.

## Update Rules

- Update this file when milestone status, current focus, active capability
  coverage, or major gaps change.
- Do not append every small implementation detail here; that belongs in commit
  history or the relevant module tests.
- Move long historical sections into `docs/archive/` when this file stops being
  quick to scan.
