# Progress

This file is the rolling implementation status for the current milestone. It
records what is true now and what remains to close next; it is not a changelog.

Detailed historical progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Later history should be read from git unless a durable milestone summary needs
to be archived.

## Breaking Clean Architecture Track

The active clean-architecture refactor is a breaking internal architecture
track. Old handwritten stdlib IDs, raw `0xff00_...` identity spaces, old
bytecode shapes, old serialized `ProgramImage` assumptions, internal/public
APIs kept only for the old implementation shape, runtime string fallback
dispatch, and old internal `int`/`float` compatibility are not compatibility
requirements. The primitive scalar, bytes, type-hint contract, and guard-plan
checklist in
[archive/vela_primitives_type_hints_guards_plan.md](archive/vela_primitives_type_hints_guards_plan.md)
is complete and validated through the default full workspace checks.
The prior definition-registry and linked-bytecode checklist is complete and
validated through the default full workspace checks; follow-on work should
advance M20 cache/specialization prep rather than restoring old compatibility
paths.

This does not weaken product contracts: hot reload ABI/schema compatibility,
HostAccess safety, reflection permissioning, execution budgets, GC roots,
source-spanned diagnostics, and the no-Rust-`&mut` script boundary remain
required.

## Current Focus

M0-M19 are complete enough as a runnable prototype, embedding surface,
production hot-reload workflow, diagnostics/tooling foundation, runnable
embedding/conformance proof, measured performance baselines, and non-JIT
interpreter/heap optimization checkpoint. The primitive scalar, bytes,
type-hint contract, and guard-plan refactor is complete as a breaking M19.5
architecture continuation. M20 inline-cache work is now in close-out mode, not
open-ended cache expansion. Declared global reads, script record fields,
host access, native calls, linked method dispatch, broad stdlib value methods,
callbacks, string/bytes, Option/Result, and selected map/set/array targets have
guarded cache entries or explicit benchmark rows. Remaining M20 work should
start from a cache-family audit and then do exactly one of these:

```text
close a named cache-family gap with hit, miss, guard, fallback, and invalidation tests
interpret a measured cache delta and record whether to keep, investigate, or defer it
defer a remaining cost to M21/M22/JIT/value-layout work with an explicit reason
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
| M19.5 | Complete enough | Primitive scalar, bytes, type-hint contract, guard-plan, verified-bytecode, profile ownership, HostTargetPlan/HostAccess, and linked-dispatch prep are complete and fully validated. |
| M20 | Active | Declared global, record field, host access, native call, resolved method dispatch, dynamic method dispatch, stdlib value-method, callback, string/bytes, Option/Result, and selected collection caches exist with benchmark coverage; active work is cache-family audit, measured delta interpretation, and closing only named remaining gaps. |
| M21 | Not started | Debugger runtime hooks and DAP integration follow stable runtime/tooling contracts. |
| M22 | Not started | Cranelift JIT follows interpreter/cache/debugger/conformance stability. |
| M23 | Not started | Release hardening, public docs, validation gates, and performance targets. |

## Current Milestone State

### Available Now

- `.vela` source parsing, HIR lowering, bytecode compilation, VM execution
  with instruction, memory, call-depth, and collection growth budgets,
  ordinary and indexed `for-in`, inherent `impl Type` methods, trait
  `impl Trait for Type` methods, single-line and multiline strings, explicit
  `f"..."` and `f"""..."""` string interpolation,
  managed heap entrypoints, execution budgets, and non-moving GC foundations.
- Host mutation through `HostRef`, `HostPath`, `PathProxy`, write-through
  `HostAccess`, and capability-gated effects.
- Reflection for types, fields, methods, variants, traits, modules, functions,
  attributes, permissions, controlled reads/writes/calls, and candidate spans.
- Standard library runtime and analysis coverage for arrays, maps, sets,
  strings, Option/Result helpers and propagation, math, deterministic time,
  context event/log helpers, controlled random capability gating, opt-in
  stdio and sandboxed filesystem helpers with `io_read`/`io_write`
  capability gating, lambda TypeFacts, explicit iterator creation methods, core
  one-shot iterator terminals and lazy `map`/`filter`/`take`/`skip` adapters,
  and domain-neutral helpers.
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
- The typed scalar bytecode optimization pass has landed through the first
  non-JIT i64 slice: opcode visibility exists for external comparison
  workloads, linked jump/range structural checks are verifier-owned, verified
  `i64` arithmetic/immediate bytecode executes with checked semantics and
  source-spanned errors, the compiler lowers only proven i64 facts to typed
  scalar ops, direct integer `for` ranges lower to `I64RangeNext`, and linked
  execution has a no-hook mode split for inactive budget/profiler paths.
  Superinstructions are intentionally deferred until a profile-backed fused
  condition lowering can prove temporary-register liveness or lower directly
  from source-owned condition structure.
- The M19 interpreter/heap phase is complete enough for M20. Accepted work
  covered GC pacing, direct heap aggregate construction, argument
  materialization and storage, borrowed receiver/runtime views, collection and
  string fast paths, Option/Result helpers, scalar equality and constant loads,
  peephole/range-loop lowering, small record/enum field construction, and short
  array construction.
- The remaining Lua 5.x deltas are concentrated in cache-shaped paths:
  script record fields use shape/slot representations, host field/path reads
  and writes use `HostTargetPlan` and resolved access boundaries, method
  dispatch uses resolved targets, broader stdlib and callback dispatch has
  receiver-guarded targets, callback and closure calls need lower
  materialization overhead, and hot bytecode offsets need interpreter-vs-cache
  measurement.
- M19.5 has started with native call operands: compiled native calls can carry
  stable `FunctionId` metadata while preserving names for diagnostics and
  fallback, and Engine-installed plus standard native functions register ID
  lookup targets. Native call dispatch is routed through a focused VM call
  boundary, preserving ID-first lookup, name fallback, HostAccess routing
  checks, and source-spanned errors. Standard value method calls can also carry
  optional `HostMethodId` metadata, with string/range/collection
  `len`/`is_empty`, string predicates/transforms/Option/split/parse helpers,
  collection predicates, array lookup/transform helpers, array/map/set mutators,
  and Option/Result predicates using an ID fast path before name fallback, and
  script/value method dispatch is routed through a focused VM call boundary.
  Host field/path reads, writes, compound
  mutations, and host method calls are routed through a focused VM
  host-access boundary, giving later path-key or direct-adapter work one
  replacement point. The host adapter boundary now resolves `HostTargetPlan`
  shapes into `ResolvedHostAccess` handles before executing read, write,
  mutate, remove, or call operations, and the mock adapter stores successful
  values by target instance identity while materializing diagnostic paths only
  for current error/reporting surfaces. HostPath construction now has an exact-capacity/static
  segment materialization boundary so field-only paths can bypass dynamic
  index/key conversion, and HostPath no longer carries a root-inclusive cache
  key sidecar. Unlinked bytecode
  `UnlinkedCodeObject` values now own interned `HostTargetPlan` tables and the
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
  generations, permissions, and source-spanned slow paths. Runtime inline
  caches are scoped to the active runtime image, undersized cache providers are
  rejected before execution, and accepted hot reloads clear stale entries
  before reused cache-site indexes can repopulate from the new bytecode.
  `ProgramImage` rebases embedded global and host cache-site operands to
  image-wide IDs so multi-function images cannot alias cache entries by local
  site index.
  The HostPath/HostAccess M19.5 gap is complete: hot execution uses
  `HostTargetPlan`, `HostTargetInstance`, and `ResolvedHostAccess`, with
  `HostPath` reserved for diagnostics, reflection, embedding materialization,
  and fixture setup.
  Host-boundary
  conversion failures are covered as HostAccess slow paths that leave adapter
  state unchanged.
  Source and module compilation now verifies bytecode before returning
  `UnlinkedCodeObject` or `UnlinkedProgram` values, covering register, constant, jump,
  frame-slot, call-argument, host-path dynamic segment, and nested closure
  invariants before future unchecked register, operand, or cache fast paths
  are introduced. Bytecode verification also validates cache-site sidecar IDs,
  instruction offsets, and instruction-kind matches for cacheable operations.
  Program verification also rejects script method metadata whose resolved
  target function is missing, keeping MethodId dispatch and future method-cache
  metadata target-complete before M20.
  Compiler output is now explicitly unlinked bytecode:
  `UnlinkedProgram`, `UnlinkedCodeObject`, `UnlinkedInstruction`, and
  `UnlinkedInstructionKind` carry semantic IDs without requiring runtime
  handles during compilation.
  The linked-bytecode representation now exists separately as `LinkedProgram`,
  `LinkedCodeObject`, `Instruction`, and `InstructionKind`, with executable
  operands shaped as dense handles or slots and debug names stored in a side
  table. Linked bytecode verification now rejects invalid debug-name
  references, out-of-bounds dense handles, and invalid local register,
  constant, jump, cache-site, and host-target operands before execution, and
  validates linked cache-site sidecar IDs, offsets, and instruction kinds.
  ProgramVersion now owns bytecode-offset profile layout metadata for each
  function and rebuilds that sidecar when hot reload creates a new version, so
  future counters, cache state, or JIT decisions can be version-scoped and
  invalidated with the version; rejected reloads keep the previous version
  profile unchanged. Runtime-owned bytecode profile counters now record linked
  instruction-offset hits through nested script, method, closure, and callback
  calls, and accepted hot reload resets the counter sidecar for the new image.
  The VM now has linked-program execution for scalar, comparison, branch,
  return, budget-charged instructions, script/native/value/script-method calls,
  array/map/range/index/iterator/global/host operations, and record slot
  construction/read/write plus enum construction/slot/tag operations without
  rebuilding unlinked code; linked closure opcodes now carry linked function
  handles through closure values, and linked host-method `CallMethod` dispatch
  routes through HostAccess. All linked instruction variants now have explicit
  VM execution paths; engine runtime raw calls and normal `Runtime::call` /
  script `Runtime::call_method` paths now require the image's linked program
  for persistent and fresh heap entrypoints instead of falling back to
  `ProgramImage` execution. Engine linking now uses the definition
  registry plus installed native implementation IDs, and engine-compiled
  initial and accepted hot-reload versions carry version-owned linked layouts
  that runtime images reuse after safe-point acceptance. Standalone hot-reload
  compilation now attaches linked layouts for linkable script-only versions,
  and hot-reload behavior tests execute those linked version layouts instead
  of rebuilding unlinked programs through `ProgramVersion::to_program()`.
  Engine hot-reload linking now rebuilds linker input from version/update-owned
  function metadata instead of the `ProgramImage::to_program()` compatibility
  path, and `ProgramImage::to_program()` has been removed. No-heap raw runtime
  `run_program_runtime*` VM APIs and their diagnostic fixture callers have been
  replaced with linked-program execution, and dead managed-heap runtime wrapper
  aliases plus their helper have been deleted. The unlinked
  `run_program_with_managed_heap_and_budget` API has also been removed; its VM
  test callers now link before execution, with standard-registry facts used for
  stdlib/value methods and empty aggregate literals carrying unknown element
  shapes instead of falling back to unresolved method names. The unlinked
  `run_program_with_budget` wrapper has also been deleted after its callers
  moved to linked execution. The remaining public direct unlinked VM execution
  convenience entrypoints have been deleted, and single-function VM benchmark
  modes now link before execution while preserving linked heap-budget coverage.
  Linkable `execution_core` coverage and the compiled conformance fixture now
  run through linked bytecode after ad-hoc source record literals, enum pattern
  fields, stdlib callback receiver facts, and linked callback closures gained
  linker-ready operands/runtime ownership. Script function calls are linked
  through `ScriptFunctionHandle` tables, with mismatched call IDs rejected by
  the linker and linked execution calling by dense handle.
  Script function dispatch is being isolated behind a focused call boundary so
  later resolved-target work does not grow the main VM loop or change current
  hot-reload rename semantics. Closure creation and invocation now have a
  focused VM boundary that preserves protected roots and call-site offsets
  while materializing common capture counts through inline small storage.
  Higher-order callback dispatch now reuses the shared execution-call
  descriptor and borrows closure metadata instead of cloning the full closure
  value for each callback, and linked stdlib callback bodies receive the active
  inline-cache provider for cacheable nested operations.
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
  common path while preserving names for diagnostics and fallback. Native
  dispatch no longer has string-name fallback maps: standard and host-native
  source-name aliases install as explicit `FunctionId` bindings, reflection
  calls resolve callable descriptors to IDs, and linked bytecode keeps native
  handles plus debug names separated from runtime dispatch. Native-call
  cache-site operands are preserved from compiler output through linked
  bytecode verification and benchmark cache-site rebasing, and linked native
  dispatch now caches resolved pure, host, and borrowed-host targets behind a
  `FunctionId` guard while retaining current slow-path behavior on misses.
  Linked method
  dispatch now uses dense method handles for script, host, and value method
  paths; linked value method execution calls standard methods by `MethodId`
  only, with debug names reserved for error reporting. Runtime Option/Result
  heap values now carry standard `TypeId`/`VariantId`/payload-field identity,
  and standard method plus `try` propagation paths classify them through those
  IDs and slot reads instead of string-name fallback. Linked script enum
  construction now stores `TypeId`/`VariantId` identity in heap enum values,
  and linked enum tag checks compare those IDs while retaining names for
  diagnostics and reflection. Linked record construction now stores
  `TypeId` plus `ShapeId` identity in heap record values, while linked record
  field reads/writes continue through `FieldSlot` operands and diagnostic
  names remain side-table metadata. Engine definition registry construction now
  consumes registered host type, field, method, and native function inputs
  directly instead of rebuilding compiler identity from reflection-only
  descriptors; reflection metadata remains a separate runtime view. Linked
  method-call and record field read/write instructions now preserve cache-site
  operands from cache-site sidecars, with linked verifier and runtime image
  rebasing coverage. Linked script record field reads and writes now populate
  guarded runtime inline-cache entries keyed by `TypeId`, `ShapeId`, and
  `FieldSlot`, and guard misses fall back to the existing slot slow path before
  replacing stale entries. Linked method calls now populate runtime
  inline-cache entries keyed by `MethodDispatchHandle`, caching resolved
  script, value, or host targets before falling back to linked method-dispatch
  lookup on misses; accepted hot reloads clear those record-field and
  method-dispatch cache entries before the new image repopulates them. Native
  call cache entries now have the same accepted-hot-reload clearing coverage.
  The primitive scalar, bytes, type-hint contract, and guard-plan refactor is
  complete: source `int`/`float` hints are gone, runtime/owned/host/constant
  values share `ScalarValue` and bytes representations, type hints are
  contracts with compile-time and linked runtime guard enforcement, numeric
  operators require identical concrete scalar tags, byte strings and bytes APIs
  are covered, and final validation passes. Root host receiver index reads,
  writes, compound mutations, and removals lower for typed roots with
  configured host index capabilities, and numeric key contracts emit dynamic
  index target parts for cache-ready host access plans.

### Remaining Gaps

M20 should now be driven by close-out criteria instead of broad "continue
guarded inline-cache specialization" tasks. A remaining cache task is valid
only when it names the specific family and one missing proof:

```text
coverage: no cache entry exists for a measured hot path
correctness: hit, miss, wrong-guard, fallback, reload, or schema invalidation coverage is missing
measurement: interpreter-only, profile-only, and cache-enabled rows cannot yet be compared
decision: measured cache delta has not been classified as keep, investigate, or defer
```

Current M20 close-out gates:

- Cache-family audit: list existing cache families and mark each as complete,
  incomplete, or explicitly deferred. Do this before adding another cache
  family.
- Correctness proof: every completed family keeps generic fallback behavior and
  covers guard failures, hot reload invalidation, and schema or version
  invalidation where applicable.
- Measurement proof: cache-enabled rows must be compared against the right
  interpreter-only or profile-only baseline with `measurement_kind`,
  `delta_kind`, `measurement_summary`, and `cache_delta_summary`.
- Decision proof: slower or flat cache deltas must be assigned to a named
  follow-up, accepted as neutral overhead, or deferred to JIT/value-layout work;
  do not leave them as generic M20 work.
- Scope proof: new M20 implementation should be a small named family, not a
  cross-cutting cache expansion. Larger representation or value-layout changes
  belong to a separate milestone decision.

Lua 5.x comparable performance remains a measured target for cache-enabled
non-JIT host-boundary workloads. Scalar, array, string, function-call,
callback, and host-boundary deltas should stay separated so M20 can close
without hiding unrelated future JIT work.

### Validation

Use the relevant subset of [validation.md](validation.md) for each change.
Default full validation remains:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For remaining M20 cache-entry work, run focused correctness tests for touched
bytecode, runtime dispatch, host-boundary, and stdlib/native call paths plus
the relevant interpreter-only/profile-only/cache-enabled benchmark rows.
Preparatory fast paths must preserve ExecutionBudget, HostAccess, reflection
policy, GC roots, hot reload ownership, schema invalidation, and source-spanned
diagnostics.

## Next Up

- Audit M20 cache families and classify each as complete, incomplete, or
  deferred before starting more implementation.
- Close only named cache-family gaps with focused tests and paired benchmark
  evidence. Avoid generic "continue specialization" tasks.
- Keep the completed primitive scalar, bytes, type-hint contract, and guard-plan
  refactor as the baseline; do not reintroduce old `int`/`float` compatibility
  paths or string fallback dispatch.
- Plan M21 debugger and M22 Cranelift JIT only from stable source-span,
  frame-map, GC-root, budget, HostAccess, hot-reload, and conformance contracts.

## Update Rules

- Update this file when current focus, milestone status, available capability
  coverage, validation expectations, or remaining current gaps change.
- Do not append routine implementation details, small refactors, or every
  commit result here; those belong in commit history or focused tests.
- Keep the file quick to scan. If durable historical context becomes necessary,
  summarize it once and archive the long form under `docs/archive/`.
