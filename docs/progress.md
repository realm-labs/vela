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
interpreter/heap optimization checkpoint. Current work is centered on M19.5
performance architecture prep before M20 inline caches:

```text
preserve all runtime, host, reflection, GC, and hot-reload semantics
move hot dispatch operands from names to IDs, slots, or resolved targets
split growing VM hot dispatch families behind focused boundaries
prepare cache/JIT-facing invariants while keeping generic fallback behavior
finish verified-bytecode, profile ownership, HostTargetPlan/HostAccess boundaries, and callback/closure materialization prep before M20
```

Post-MVP performance remains a separate track: measure first, then optimize the
non-JIT bytecode interpreter toward Lua 5.x comparable host-boundary workloads
through M19.5 architecture prep and M20 cache work before debugger/DAP work and
Cranelift JIT.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M6 | Complete | Source -> bytecode -> VM -> HostRef/HostPath/HostAccess -> hot reload loop exists. |
| M7 | Complete | Execution budgets, managed heap, GC roots, and managed heap entrypoints exist. |
| M8 | Complete enough | HIR, module graph, imports, declarations, binding maps, and compiler integration are active. |
| M9 | Complete enough | Broad executable language surface works; conformance catches edge cases. |
| M10 | Complete enough | Stable script metadata, shapes, slots, traits, and dispatch foundations exist. |
| M11 | Complete enough | HostRef, HostPath, PathProxy, and write-through HostAccess host boundaries exist. |
| M12 | Complete enough | Reflection metadata, permission-aware queries, candidate spans, and schema-safe mutation denial are covered. |
| M13 | Complete enough | Collections, strings, Option/Result propagation, math, context, random capability gating, lambda facts, and domain-neutral helper coverage are validated. |
| M14 | Complete enough | EngineBuilder registration, source compilation, Runtime::call, descriptors, stable-ID rejection, capability profiles, signature conversion, and macro parity are covered. |
| M15 | Complete enough | Safe-point staging, old-frame lifetime, new-call entry, source workflows, ABI/schema rejection, compatible additions, and repair reports are covered. |
| M16 | Complete enough | Parser, semantic, runtime/call-stack, host, reflection, hot reload, TypeFact, flow-narrowing, and completion snapshot fixtures exist. |
| M17 | Complete enough | Game-server demos, negative workflows, conformance fixtures, and parser fuzz harness exist. |
| M18 | Complete enough | Quick and full/default baseline captures exist with environment metadata and checksums. |
| M19 | Complete enough | Non-JIT interpreter and heap optimization has a recorded exit checkpoint. Accepted work includes GC pacing, direct heap aggregate construction, argument materialization/storage cleanup, borrowed receiver/runtime views, stdlib collection/string/Option/Result fast paths, scalar/equality/constant/peephole/range-loop lowering, small script-field and short-array construction, and expanded benchmark coverage. Remaining Lua 5.x deltas are measured and belong to M20 cache/specialization families rather than more unguarded M19 micro-optimization. |
| M19.5 | Active | Required M20 gate: resolve hot call sites to IDs/slots/targets, split hot dispatch families out of the main VM loop, prepare method/native/stdlib dispatch for cache-ready lookup, prepare HostTargetPlan/HostAccess boundaries for cache-ready lookup, reduce callback/closure materialization, and define verified-bytecode/profile/JIT-facing interpreter invariants before M20 cache state. |
| M20 | Not started | Inline caches and specialization start after M19.5, beginning with script record field, host field/path, method dispatch, stdlib method, and hot bytecode offset profiling guards. |
| M21 | Not started | Debugger runtime hooks and DAP integration follow stable runtime/tooling contracts. |
| M22 | Not started | Cranelift JIT follows interpreter/cache/debugger/conformance stability. |
| M23 | Not started | Release hardening, public docs, validation gates, and performance targets. |

## Current Milestone State

### Available Now

- `.vela` source parsing, HIR lowering, bytecode compilation, VM execution
  with ordinary and indexed `for-in`, inherent `impl Type` methods, trait
  `impl Trait for Type` methods,
  managed heap entrypoints, execution budgets, and non-moving GC foundations.
- Host mutation through `HostRef`, `HostPath`, `PathProxy`, write-through
  `HostAccess`, and capability-gated effects.
- Reflection for types, fields, methods, variants, traits, modules, functions,
  attributes, permissions, controlled reads/writes/calls, and candidate spans.
- Standard library runtime and analysis coverage for arrays, maps, sets,
  strings, Option/Result helpers and propagation, math, deterministic time,
  context event/log helpers, controlled random capability gating, opt-in
  stdio and sandboxed filesystem helpers with `io_read`/`io_write`
  capability gating, lambda TypeFacts, and domain-neutral helpers.
- Engine registration for host types, native functions, context helpers,
  standard natives, capability profiles, reflection permissions, compiler options, dynamic
  `CallArgs`, direct call-boundary `&T`/`&mut T` host object bindings,
  module-level `global` declarations backed by persistent Rust-defined host objects
  or Runtime-owned script values with unified `insert_global` support for
  `OwnedValue`, serde snapshots, and same-runtime `VelaValue` handles,
  feature-gated serde conversion between Rust structs/enums and script-owned
  `OwnedValue` records/enums for snapshot-style arguments and results, direct
  serde decoding from runtime-managed `VelaValue` and globals without
  materializing detached `OwnedValue`,
  runtime-managed `VelaValue` call returns that can be passed back to later calls
  on the same runtime without owned materialization, cached `VelaFunction`
  entry handles and `VelaMethod` script-value method handles for high-frequency
  embedding calls, `Send` Runtime and `VelaValue` handles for worker/actor ownership transfer,
  direct host object method dispatch with receiver paths, unified concrete host
  type specs, host index capability metadata, typed host path arguments,
  string-key host path segments, hot-reload policies, derive-generated host
  bindings, and reflection schemas.
- A dedicated `vela_c_api` crate exists for the external C ABI boundary,
  separate from hot-reload ABI. The first slice exposes opaque engine/runtime
  handles, source compilation, no-argument entry calls, scalar C result values,
  and ABI-owned string/value cleanup.
- Macro-generated host and native bindings with stable IDs, rename aliases,
  effect-aware registration, and budget-aware context helper coverage.
- Hot reload staging and safe-point reports for source-file, directory, and
  changed-file workflows, including accepted compatible additions/renames and
  rejected ABI/schema/effect/access/source changes without advancing the active
  version.
- Standalone `vela_examples` bins and conformance fixtures covering domain-neutral stdlib helpers,
  reflection, schema-safe mutation denial, capability gating, read-only host boundary
  rejection, host read/write/call capability denial, stale host ref generation
  rejection, host write/call denial diagnostics, reflection candidate
  diagnostics, bad schema diagnostics, generic type hint rejection, and
  tick-boundary hot reload. A standalone host type method example covers
  concrete host type specs, receiver-path methods, keyed host paths, child
  receiver method calls, and typed host path arguments. A standalone script
  global example covers VM-owned global initialization, script mutation, Rust
  `OwnedValue` constructor/macro updates, and later script reads of the same
  persistent value. A standalone I/O stdlib example covers stdout plus
  sandboxed file read/write.
- A GitHub Pages site source exists under `site/`, with book-style bilingual
  Markdown docs, current-language sidebar navigation, and a browser playground
  backed by the `vela_playground_wasm` wrapper. The Pages workflow builds the
  WASM target, generates `wasm-bindgen` browser bindings, and deploys the
  static artifact.
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
- The remaining Lua 5.x deltas are concentrated in cache-shaped paths, but
  M20 should not start until their operands are cache-ready: script record
  fields need shape/slot representations, host field/path reads and writes
  need `HostTargetPlan` and resolved access boundaries, method and stdlib dispatch need ID/target lookup,
  callback and closure calls need lower materialization overhead, and hot
  bytecode offsets need versioned ownership for invalidation.
- M19.5 has started with native call operands: compiled native calls can carry
  stable `FunctionId` metadata while preserving names for diagnostics and
  fallback, and Engine-installed plus standard native functions register ID
  lookup targets. Native call dispatch is routed through a focused VM call
  boundary, preserving ID-first lookup, name fallback, HostAccess routing
  checks, and source-spanned errors. Standard value method calls can also carry
  optional `HostMethodId` metadata, with string/range `len`/`is_empty` using an
  ID fast path before name fallback, and script/value method dispatch is routed
  through a focused VM call boundary. Host field/path reads, writes, compound
  mutations, and host method calls are routed through a focused VM
  host-access boundary, giving later path-key or direct-adapter work one
  replacement point. The host adapter boundary now resolves `HostTargetPlan`
  shapes into `ResolvedHostAccess` handles before executing read, write,
  mutate, remove, or call operations, and the mock adapter stores successful
  values by target instance identity while materializing diagnostic paths only
  for current error/reporting surfaces. HostPath construction now has an exact-capacity/static
  segment materialization boundary so field-only paths can bypass dynamic
  index/key conversion, and HostPath no longer carries a root-inclusive cache
  key sidecar. Bytecode
  `CodeObject` values now own interned `HostTargetPlan` tables and the
  collapsed `HostRead`/`HostWrite`/`HostMutate`/`HostRemove`/`HostCall`
  instruction family has verifier coverage for target bounds, contiguous
  dynamic arguments, and cache-site kind matching. Source compiler lowering
  now interns host field, path, mutation, remove, push, and method-call targets
  into those tables and emits the collapsed family through the focused
  host-access boundary, with registered host type IDs preserved for typed root
  plans and mock storage canonicalized across static and dynamic key shapes.
  `PathProxy` now stores a root `HostRef`, `HostTargetPlan`, and owned dynamic
  args, routing operations through `HostTargetInstance` and materializing
  `HostPath` only at explicit diagnostic/embedding conversion edges.
  Runtime inline caches now have host access entries guarded by root type,
  target-plan ID, operation, and host schema epoch; collapsed host bytecode
  resolves through that cache boundary while adapter execution still validates
  generations, permissions, and source-spanned slow paths.
  The HostPath/HostAccess M19.5 gap is complete: hot execution uses
  `HostTargetPlan`, `HostTargetInstance`, and `ResolvedHostAccess`, with
  `HostPath` reserved for diagnostics, reflection, embedding materialization,
  and fixture setup.
  Host-boundary
  conversion failures are covered as HostAccess slow paths that leave adapter
  state unchanged.
  Source and module compilation now verifies bytecode before returning
  `CodeObject` or `Program` values, covering register, constant, jump,
  frame-slot, call-argument, host-path dynamic segment, and nested closure
  invariants before future unchecked register, operand, or cache fast paths
  are introduced. Program verification also rejects script method metadata
  whose resolved target function is missing, keeping MethodId dispatch and
  future method-cache metadata target-complete before M20.
  ProgramVersion now owns bytecode-offset profile layout metadata for each
  function and rebuilds that sidecar when hot reload creates a new version, so
  future counters, cache state, or JIT decisions can be version-scoped and
  invalidated with the version; rejected reloads keep the previous version
  profile unchanged.
  Script function dispatch is being isolated behind a focused call boundary so
  later resolved-target work does not grow the main VM loop or change current
  hot-reload rename semantics. Closure creation and invocation now have a
  focused VM boundary that preserves protected roots and call-site offsets
  while materializing common capture counts through inline small storage.
  Higher-order callback dispatch now reuses the shared execution-call
  descriptor and borrows closure metadata instead of cloning the full closure
  value for each callback.
  Persistent runtime-managed `VelaValue` handles are now included in
  script-global collection roots, so retained call results survive later
  `insert_global`/`update_global` heap collections.
  Runtime `CallOptions` budget checkpoints now cover both instruction limits
  and recursive call-depth limits at the embedding boundary, including
  source-spanned call-stack reports.
  Script array/map/range construction, record/enum construction, and script
  field reads/writes now route through focused script aggregate/object
  boundaries while preserving current name fallback, small-field construction,
  and slot guards. Generic iterator and range-loop stepping now route through a
  focused iteration boundary with jump validation kept on the VM side of the
  bytecode contract. Declared global reads now carry `GlobalSlot` metadata so
  VM-owned script globals and runtime host globals can use slot lookup on the
  common path while preserving names for diagnostics and fallback.

### Remaining Gaps

- M19.5 exit checklist before M20:
  - hot script, native, stdlib, method, and host-boundary dispatch operands
    use IDs, slots, resolved targets, path keys, or an explicit remaining
    fallback reason;
  - diagnostic names are split from hot operands where practical and remain
    available for reflection, source reports, and errors;
  - VM execution delegates host access, script calls, stdlib/method dispatch,
    callback/closure calls, aggregate/object construction, and iteration
    through focused boundaries instead of growing `execution.rs`;
  - native and stdlib hot paths have borrowed `Value` view coverage or a named
    reason to defer the remaining conversions to M20/JIT work;
  - HostTargetPlan/HostAccess resolved targets and direct adapter-thunk boundaries are
    implemented enough for M20 host field/path caches;
  - root host receiver index lowering such as `scores[1]` needs HIR/TypeFacts
    receiver-type plumbing before compile-time index capability diagnostics can
    be complete; field-derived host paths such as `player.scores[1]` continue
    to lower through HostPath;
  - callback and closure materialization uses inline/small storage on common
    arities, with remaining allocation costs measured;
  - verified-bytecode and runtime tests cover the invariants needed by later
    unchecked register, operand, and cache fast paths;
  - ProgramVersion-owned profile metadata covers hot bytecode offsets and has
    hot-reload/schema invalidation tests;
  - interpreter-only benchmark rows identify which remaining costs belong to
    M20 cache work versus later JIT work.
- M20: after M19.5, implement guarded inline caches and specialization for
  script record fields, host field/path reads and writes, method dispatch,
  stdlib value methods, and hot bytecode offsets. Cache misses, guard failures,
  hot reload, and schema ABI changes must fall back or invalidate without
  changing semantics.
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

For current M19.5 work, run focused correctness tests for touched bytecode,
runtime dispatch, host-boundary, and stdlib/native call paths plus
interpreter-only before/after benchmark rows. Preparatory fast paths must
preserve ExecutionBudget, HostAccess, reflection policy, GC roots, hot reload
ownership, schema invalidation, and source-spanned diagnostics.

## Next Up

- Continue M19.5 before M20: first finish ID/slot/target-ready focused modules
  for native, stdlib, script function, method, callback, and host-boundary
  dispatch; then prepare HostTargetPlan/HostAccess resolved targets and direct adapter
  thunks; then reduce callback/closure materialization; then define
  version-owned profile metadata and JIT-facing frame/GC/budget/HostAccess
  invariants for future cache state.
- Keep benchmark evidence ahead of M20 specialization work. M19.5 reports
  interpreter-only before/after rows; M20 reports interpreter-only versus
  cache-enabled rows.
- Plan M21 debugger and M22 Cranelift JIT only from stable source-span,
  frame-map, GC-root, budget, HostAccess, hot-reload, and conformance contracts.

## Update Rules

- Update this file when current focus, milestone status, available capability
  coverage, validation expectations, or remaining current gaps change.
- Do not append routine implementation details, small refactors, or every
  commit result here; those belong in commit history or focused tests.
- Keep the file quick to scan. If durable historical context becomes necessary,
  summarize it once and archive the long form under `docs/archive/`.
