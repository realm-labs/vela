# Progress

This file is the rolling implementation status for the current milestone. It
records what is true now and what remains to close next; it is not a changelog.

Detailed historical progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Later history should be read from git unless a durable milestone summary needs
to be archived.

## Current Focus

M0-M17 are complete enough as a runnable prototype, embedding surface,
production hot-reload workflow, diagnostics/tooling foundation, and runnable
game-server/conformance proof. Current work is centered on M18 performance
measurement baselines:

```text
official benchmark commands and quick validation
baseline result capture with environment metadata
follow-up bottleneck notes before optimization work
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
| M17 | Complete enough | Game-server demos, negative workflows, conformance fixtures, and parser fuzz harness exist. |
| M18 | Partial | Baseline harnesses and quick result capture exist; full/default baseline reporting remains. |
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
  rejection, host read/write/call permission denial, stale host ref generation
  rejection, host patch conflict reporting, reflection candidate diagnostics,
  bad schema diagnostics, generic type hint rejection, and tick-boundary hot
  reload.
- A parser fuzz target exists under `fuzz/` and can be compile-checked even
  when the local machine has not installed `cargo-fuzz`.
- M18 quick benchmark output is recorded in [performance.md](performance.md)
  with environment metadata, checksums, external runtime availability, and
  initial bottleneck notes.

### Remaining Gaps

- M18: run and record full/default benchmark baselines when practical,
  including Lua 5.x/LuaJIT/Rhai versions when those runtimes are available.
- M19+: keep performance work benchmark-driven and separate from semantic
  changes.

### Validation

Use the relevant subset of [validation.md](validation.md) for each change.
Default full validation remains:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For current M18 work, prefer benchmark compile checks, quick benchmark runs,
and focused correctness tests for touched runtime areas. Keep optimization out
of scope until baseline outputs and bottleneck notes are recorded.

## Next Up

- Expand quick M18 baseline capture into full/default benchmark reporting when
  runtime availability and machine time allow.
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
