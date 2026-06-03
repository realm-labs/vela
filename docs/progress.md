# Progress

This file is the rolling implementation status for the current milestone. It
records what is true now and what remains to close next; it is not a changelog.

Detailed historical progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Later history should be read from git unless a durable milestone summary needs
to be archived.

## Current Focus

M0-M13 are complete enough as a runnable prototype. Current work is centered on
M14/M15 embedding and hot-reload source workflows, specifically the pieces that
unblock realistic embedding:

```text
Engine API registration
native descriptors and Rust signature conversion
context helpers and host macros
safe-point reload staging and reports
function, schema, effect, access, and source reload ABI checks
source-file, directory, and changed-file update workflows
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
| M14 | Partial | Engine APIs, native descriptors, context helpers, and macros exist in slices; close the remaining embedding proof. |
| M15 | Partial | Function, descriptor, module, trait, schema, and source reload ABI checks exist; close production workflow proof. |
| M16 | Partial | Runtime diagnostics, common rendering, and bytecode/runtime frame maps have started. |
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
  reflection, permissions, and tick-boundary hot reload.

### Remaining Gaps

- M14: close the remaining embedding proof against the milestone checkpoint:
  EngineBuilder registration, `compile_file`/`compile_dir`, `Runtime::call`,
  native descriptors, stable ID rejection, permissioned native calls, signature
  conversion, and derive macro schema parity.
- M15: close the production reload proof against the milestone checkpoint:
  safe-point staging, old-frame lifetime, new-call version entry, source update
  workflows, ABI/schema/effect rejection, compatible additions, and repair-hint
  reports.
- M16/M17: expand diagnostics, fixtures, and game-server demo coverage only
  after the current embedding/reload checkpoint no longer blocks them.
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

For current M14/M15 work, prefer targeted engine/hot-reload tests and at least
one workflow-facing CLI demo run when the change affects embedding or reload
behavior.

## Next Up

- Finish enough M14/M15 embedding and reload proof to unblock realistic host
  integration.
- Then broaden M16/M17 diagnostics, conformance fixtures, and game-server demo
  workflows around the stable embedding surface.
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
