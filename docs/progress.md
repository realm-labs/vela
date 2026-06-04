# Progress

This file is the rolling implementation status for the current milestone. It
records what is true now and what remains to close next; it is not a changelog.

Detailed historical progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Later history should be read from git unless a durable milestone summary needs
to be archived.

## Current Focus

M0-M16 are complete enough as a runnable prototype, embedding surface,
production hot-reload workflow, and diagnostics/tooling foundation. Current work
is centered on M17 fixtures and demo coverage around the now-stable reload and
embedding surface:

```text
runtime diagnostics and common rendering
source spans, frame maps, and report surfaces
conformance fixtures and game-server demo workflows
```

Post-MVP performance remains a separate track: measure first, then optimize the
non-JIT bytecode interpreter toward Lua 5.x comparable gameplay workloads
before debugger/DAP work and Cranelift JIT.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M6 | Complete | Source -> bytecode -> VM -> HostRef/HostPath/PatchTx -> hot reload loop exists. |
| M7 | Complete | Execution budgets, managed heap, GC roots, and managed heap entrypoints exist. |
| M8 | Complete enough | HIR, module graph, imports, declarations, binding maps, and compiler integration are active. |
| M9 | Complete enough | Broad executable language surface works; conformance catches edge cases. |
| M10 | Complete enough | Stable script metadata, shapes, slots, traits, and dispatch foundations exist. |
| M11 | Complete enough | HostRef, HostPath, PathProxy, PatchTx overlays, and rollback-safe host boundaries exist. |
| M12 | Complete enough | Reflection metadata, permission-aware queries, candidate spans, and schema-safe mutation denial are covered. |
| M13 | Complete enough | Collections, strings, Option/Result propagation, math, context, random permissions, lambda facts, and demo helper coverage are validated. |
| M14 | Complete enough | EngineBuilder registration, source compilation, Runtime::call, descriptors, stable-ID rejection, permissions, signature conversion, and macro parity are covered. |
| M15 | Complete enough | Safe-point staging, old-frame lifetime, new-call entry, source workflows, ABI/schema rejection, compatible additions, and repair reports are covered. |
| M16 | Complete enough | Parser, semantic, runtime/call-stack, host, reflection, hot reload, TypeFact, flow-narrowing, and completion snapshot fixtures exist. |
| M17 | Partial | Conformance fixtures and demo harnesses exist; game-server demo can still expand. |
| M18 | Partial | Baseline harnesses exist; official baseline reporting and follow-up bottleneck tracking remain. |
| M19-M20 | Not started | Interpreter optimization, inline caches, and specialization follow M18 baselines. |
| M21 | Not started | Debugger runtime hooks and DAP integration follow stable runtime/tooling contracts. |
| M22 | Not started | Cranelift JIT follows interpreter/cache/debugger/conformance stability. |
| M23 | Not started | Release hardening, public docs, validation gates, and performance targets. |

## Current Milestone State

### Available Now

- `.vela` source parsing, HIR lowering, bytecode compilation, VM execution,
  managed heap entrypoints, execution budgets, and non-moving GC foundations.
- Host mutation through `HostRef`, `HostPath`, `PathProxy`, `PatchTx`, overlays,
  permissions, and safe-point apply.
- Reflection for types, fields, methods, variants, traits, modules, functions,
  attributes, permissions, controlled reads/writes/calls, and candidate spans.
- Standard library runtime and analysis coverage for arrays, maps, sets,
  strings, Option/Result helpers and propagation, math, context time/event/log
  helpers, controlled random permissions, lambda TypeFacts, and gameplay demo
  helpers.
- Engine registration for host types, native functions, context helpers,
  standard natives, reflection permissions, compiler options, hot-reload
  policies, derive-generated host bindings, and reflection schemas.
- Macro-generated host and native bindings with stable IDs, rename aliases,
  permission-aware registration, and budget-aware context helper coverage.
- Hot reload staging and safe-point reports for source-file, directory, and
  changed-file workflows, including accepted compatible additions/renames and
  rejected ABI/schema/effect/access/source changes without advancing the active
  version.
- CLI demo scripts and conformance fixtures covering gameplay helpers,
  reflection, schema-safe mutation denial, permissions, read-only host boundary
  rejection, bad schema diagnostics, generic type hint rejection, and
  tick-boundary hot reload.

### Remaining Gaps

- M17: expand conformance fixtures and game-server demo coverage around the
  stable embedding and reload surface.
- M18+: keep performance work benchmark-driven and separate from semantic
  changes.

### Validation

Use the relevant subset of [validation.md](validation.md) for each change.
Default full validation remains:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For current M16/M17 work, prefer targeted diagnostics, conformance, and demo
tests plus at least one workflow-facing CLI demo run when behavior changes.

## Next Up

- Broaden M17 conformance and game-server demo workflows around the stable
  embedding and reload surface.
- Keep M18 measurement baselines ahead of M19/M20 optimization work.
- Plan M21 debugger and M22 Cranelift JIT only from stable source-span,
  frame-map, GC-root, budget, PatchTx, hot-reload, and conformance contracts.

## Update Rules

- Update this file when current focus, milestone status, available capability
  coverage, validation expectations, or remaining current gaps change.
- Do not append routine implementation details, small refactors, or every
  commit result here; those belong in commit history or focused tests.
- Keep the file quick to scan. If durable historical context becomes necessary,
  summarize it once and archive the long form under `docs/archive/`.
