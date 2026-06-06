# Progress

This file is the rolling implementation status for the current milestone. It
records what is true now and what remains to close next; it is not a changelog.

Detailed historical progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Later history should be read from git unless a durable milestone summary needs
to be archived.

## Current Focus

M0-M19 are complete enough as a runnable prototype, embedding surface,
production hot-reload workflow, diagnostics/tooling foundation, runnable
embedding/conformance proof, measured performance baselines, and non-JIT
interpreter/heap optimization checkpoint. Current work is centered on M20
inline caches and specialization:

```text
preserve all runtime, host, reflection, GC, and hot-reload semantics
specialize only operations with measured hot paths
keep guarded slow-path fallback for cache misses and invalidation
```

Post-MVP performance remains a separate track: measure first, then optimize the
non-JIT bytecode interpreter toward Lua 5.x comparable host-boundary workloads
through M20 cache work before debugger/DAP work and Cranelift JIT.

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
| M13 | Complete enough | Collections, strings, Option/Result propagation, math, context, random capability gating, lambda facts, and domain-neutral helper coverage are validated. |
| M14 | Complete enough | EngineBuilder registration, source compilation, Runtime::call, descriptors, stable-ID rejection, capability profiles, signature conversion, and macro parity are covered. |
| M15 | Complete enough | Safe-point staging, old-frame lifetime, new-call entry, source workflows, ABI/schema rejection, compatible additions, and repair reports are covered. |
| M16 | Complete enough | Parser, semantic, runtime/call-stack, host, reflection, hot reload, TypeFact, flow-narrowing, and completion snapshot fixtures exist. |
| M17 | Complete enough | Game-server demos, negative workflows, conformance fixtures, and parser fuzz harness exist. |
| M18 | Complete enough | Quick and full/default baseline captures exist with environment metadata and checksums. |
| M19 | Complete enough | Non-JIT interpreter and heap optimization has a recorded exit checkpoint. Accepted work includes GC pacing, direct heap aggregate construction, argument materialization/storage cleanup, borrowed receiver/runtime views, stdlib collection/string/Option/Result fast paths, scalar/equality/constant/peephole/range-loop lowering, small script-field and short-array construction, and expanded benchmark coverage. Remaining Lua 5.x deltas are measured and belong to M20 cache/specialization families rather than more unguarded M19 micro-optimization. |
| M20 | Partial | Inline caches and specialization are now the active focus, starting with script record field, host field/path, method dispatch, stdlib method, and hot bytecode offset profiling guards. |
| M21 | Not started | Debugger runtime hooks and DAP integration follow stable runtime/tooling contracts. |
| M22 | Not started | Cranelift JIT follows interpreter/cache/debugger/conformance stability. |
| M23 | Not started | Release hardening, public docs, validation gates, and performance targets. |

## Current Milestone State

### Available Now

- `.vela` source parsing, HIR lowering, bytecode compilation, VM execution,
  managed heap entrypoints, execution budgets, and non-moving GC foundations.
- Host mutation through `HostRef`, `HostPath`, `PathProxy`, `PatchTx`, overlays,
  capability-gated effects, and safe-point apply.
- Reflection for types, fields, methods, variants, traits, modules, functions,
  attributes, permissions, controlled reads/writes/calls, and candidate spans.
- Standard library runtime and analysis coverage for arrays, maps, sets,
  strings, Option/Result helpers and propagation, math, deterministic time,
  context event/log helpers, controlled random capability gating, lambda TypeFacts, and
  domain-neutral helpers.
- Engine registration for host types, native functions, context helpers,
  standard natives, capability profiles, reflection permissions, compiler options, hot-reload
  policies, derive-generated host bindings, and reflection schemas.
- Macro-generated host and native bindings with stable IDs, rename aliases,
  effect-aware registration, and budget-aware context helper coverage.
- Hot reload staging and safe-point reports for source-file, directory, and
  changed-file workflows, including accepted compatible additions/renames and
  rejected ABI/schema/effect/access/source changes without advancing the active
  version.
- CLI demo scripts and conformance fixtures covering domain-neutral stdlib helpers,
  reflection, schema-safe mutation denial, capability gating, read-only host boundary
  rejection, host read/write/call capability denial, stale host ref generation
  rejection, host patch conflict reporting, reflection candidate diagnostics,
  bad schema diagnostics, generic type hint rejection, and tick-boundary hot
  reload.
- A parser fuzz target exists under `fuzz/` and can be compile-checked even
  when the local machine has not installed `cargo-fuzz`.
- Current benchmark rules, baseline summaries, and M19 exit conclusions live in
  [performance.md](performance.md). Detailed M18/M19 benchmark history is
  archived in [archive/performance-full-2026-06-06.md](archive/performance-full-2026-06-06.md).
- The M19 interpreter/heap phase is complete enough for M20. Accepted work
  covered GC pacing, direct heap aggregate construction, argument
  materialization and storage, borrowed receiver/runtime views, collection and
  string fast paths, Option/Result helpers, scalar equality and constant loads,
  peephole/range-loop lowering, small record/enum field construction, and short
  array construction.
- The remaining Lua 5.x deltas are concentrated in cache-shaped paths: script
  record field slots, host field/path reads and writes, method and stdlib
  dispatch, callback invocation, hot closure calls, hot bytecode offsets, and
  cache invalidation across hot reload or schema ABI changes.

### Remaining Gaps

- M20: implement guarded inline caches and specialization for script record
  fields, host field/path reads and writes, method dispatch, stdlib value
  methods, and hot bytecode offsets. Cache misses, guard failures, hot reload,
  and schema ABI changes must fall back or invalidate without changing
  semantics.
- Lua 5.x comparable performance remains a measured target for cache-enabled
  non-JIT host-boundary workloads; scalar, array, string, function-call, and
  callback deltas should be tracked separately from host-boundary benchmarks.

### Validation

Use the relevant subset of [validation.md](validation.md) for each change.
Default full validation remains:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For current M20 work, run focused correctness tests for touched runtime/cache
areas plus interpreter-only versus cache-enabled benchmark rows. Specialized
paths must preserve ExecutionBudget, PatchTx, reflection policy, GC roots, hot
reload ownership, schema invalidation, and source-spanned diagnostics.

## Next Up

- Start M20 with the smallest guarded cache family that can be tested end to
  end. Script record field slot reads/writes are the narrowest first target;
  host field/path and method dispatch caches should follow once versioned cache
  ownership and invalidation are proven.
- Keep benchmark evidence ahead of M20 specialization work, reporting
  interpreter-only versus cache-enabled rows.
- Plan M21 debugger and M22 Cranelift JIT only from stable source-span,
  frame-map, GC-root, budget, PatchTx, hot-reload, and conformance contracts.

## Update Rules

- Update this file when current focus, milestone status, available capability
  coverage, validation expectations, or remaining current gaps change.
- Do not append routine implementation details, small refactors, or every
  commit result here; those belong in commit history or focused tests.
- Keep the file quick to scan. If durable historical context becomes necessary,
  summarize it once and archive the long form under `docs/archive/`.
