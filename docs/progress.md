# Progress

This file is the rolling implementation status for the current milestone. It
records what is true now and what remains to close next; it is not a changelog.

Detailed historical progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Later history should be read from git unless a durable milestone summary needs
to be archived.

## Current Focus

M0-M18 are complete enough as a runnable prototype, embedding surface,
production hot-reload workflow, diagnostics/tooling foundation, and runnable
game-server/conformance proof with measured performance baselines. Current work
is centered on M19 non-JIT interpreter and heap optimization:

```text
preserve all runtime, host, reflection, GC, and hot-reload semantics
optimize only against recorded benchmark bottlenecks
separate before/after benchmark evidence for each accepted change
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
| M18 | Complete enough | Quick and full/default baseline captures exist with environment metadata and checksums. |
| M19 | Partial | Safe-point and mark-stack GC pacing optimizations, direct heap aggregate construction, native/method argument materialization cleanup, small and wider script/native/method argument storage, read-only method root guards, owned return aggregate storage, borrowed runtime view receiver classification, array lookup/sort/slice/reverse/join/read-only/higher-order/sum/extrema, map lookup key borrowing and merge receiver fast path, string-length ASCII fast paths, and set lookup/combination/higher-order receiver fast paths, callback root/protected-value guards and heap root-buffer reuse, stack-local/no-heap map callback entries, heap map callback protection reuse, expanded script-call/range-iteration/map/map-lookup/map-merge/map-extend/map-find-entry/record-triplet/record-quad/record-quint/record-sextet/set/set-lookup/set-combination/array/array-lookup/array-extend/array-distinct/array-group-by/host-conversion/managed-heap-host-conversion/managed-heap-callback/Option-Result/scalar-dispatch benchmarks, numeric dispatch fast paths, scalar and heap-string equality fast paths, scalar constant load fast path, truthy bytecode lowering, negated equality peephole lowering, range-loop bytecode lowering, Option/Result helper tag and fixed-field construction fast paths, stdlib Option fixed-field construction, call-entry default allocation removal, and small script-field construction fast paths through six fields exist; remaining heap materialization pressure and scalar dispatch optimizations remain candidates. |
| M20 | Not started | Inline caches and specialization follow M19 interpreter and heap work. |
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
- M18 full/default benchmark output is recorded in
  [performance.md](performance.md) with environment metadata, checksums,
  external runtime availability, and measured bottleneck notes.
- The external comparison benchmark now covers scalar branch loops, script
  function calls, array scans, and string method workloads across Vela and
  available Lua 5.x, LuaJIT, Node.js, and Rhai runtimes.
- The first M19 GC pacing optimization is recorded in
  [performance.md](performance.md): safe-point GC root collection now reuses a
  `HeapExecution` buffer and appends frame roots directly while preserving
  benchmark checksums.
- A second M19 GC pacing checkpoint is recorded in
  [performance.md](performance.md): incremental GC continuation steps now skip
  frame-root scanning because the heap consumes roots only when starting a
  collection.
- A third M19 GC pacing checkpoint is recorded in
  [performance.md](performance.md): the heap now reuses a mark stack instead of
  allocating a temporary root stack for each mark phase.
- An M19 heap aggregate construction checkpoint is recorded in
  [performance.md](performance.md): managed heap execution now builds array,
  map, record, and enum heap slots directly from frame registers instead of
  first constructing temporary `Value` aggregates.
- An M19 callback root guard checkpoint is recorded in
  [performance.md](performance.md): non-heap method and callback dispatch now
  skips temporary GC root vectors that are only needed when a heap is active.
- An M19 group-by protected-value guard checkpoint is recorded in
  [performance.md](performance.md): no-heap `array.group_by` callbacks skip
  cloning previously-built groups that are only needed for heap root
  protection.
- An M19 map/sort callback allocation checkpoint is recorded in
  [performance.md](performance.md): the `callback_collections` benchmark now
  covers `map_values`, map `filter`, and `sort_by`, and no-heap map/sort
  callbacks skip protected-value clone vectors while map callbacks pass
  zero-, one-, and two-argument slices without allocating a per-entry `Vec`.
- An M19 native argument materialization checkpoint is recorded in
  [performance.md](performance.md): managed-heap native calls now materialize
  argument registers directly into the native argument vector instead of first
  cloning register values into a temporary `Vec`.
- An M19 native call argument storage checkpoint is recorded in
  [performance.md](performance.md): zero- through four-argument native calls
  now use stack-backed argument storage instead of allocating a temporary
  `Vec<Value>`, while five or more arguments keep the vector-backed path.
- An M19 script call argument storage checkpoint is recorded in
  [performance.md](performance.md): `script_call_small_args` now measures
  repeated one- and two-argument script function calls, and script function,
  closure, and method call argument packing uses stack-backed storage for zero-
  through four-value temporary argument slices before falling back to the
  vector-backed path.
- An M19 returned heap object storage checkpoint is recorded in
  [performance.md](performance.md): owned return and method-result aggregates
  now move strings, collections, records, and enums directly into managed heap
  objects instead of cloning through borrowed heap-slot conversion.
- An M19 array lookup receiver checkpoint is recorded in
  [performance.md](performance.md): array `first`, `last`, `contains`, and
  `index_of` now avoid cloning or materializing the full receiver before
  reading or scanning elements.
- An M19 map callback entry checkpoint is recorded in
  [performance.md](performance.md): no-heap map higher-order callbacks now
  iterate borrowed map receiver entries directly instead of cloning the receiver into
  a temporary entry vector before callback dispatch.
- An M19 array sort callback receiver checkpoint is recorded in
  [performance.md](performance.md): no-heap `array.sort_by` now iterates
  borrowed array receiver values directly instead of cloning the receiver into a
  temporary vector before callback dispatch.
- An M19 set callback benchmark coverage checkpoint is recorded in
  [performance.md](performance.md): `callback_collections` now also exercises
  set `filter`, `map`, `find`, `any`, `all`, and `count`, giving set
  higher-order receiver materialization a measured benchmark surface.
- An M19 array higher-order callback benchmark coverage checkpoint is recorded
  in [performance.md](performance.md): `callback_collections` now also
  exercises array `map`, `filter`, `find`, `any`, `all`, and `count`, giving
  array higher-order receiver materialization a measured benchmark surface.
- An M19 array higher-order receiver checkpoint is recorded in
  [performance.md](performance.md): no-heap array `map`, `filter`, `find`,
  `any`, `all`, and `count` now iterate borrowed array receiver values directly
  instead of cloning the full receiver before callback dispatch.
- An M19 set higher-order receiver checkpoint is recorded in
  [performance.md](performance.md): no-heap set `map`, `filter`, `find`,
  `any`, `all`, and `count` now iterate borrowed set receiver values directly
  instead of cloning the full receiver before callback dispatch.
- An M19 managed-heap array sum receiver checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_sum` now measures
  repeated heap-mode plain array sums, and callback-free `array.sum()` reads
  managed-heap numeric slots directly instead of cloning the full receiver
  before aggregation.
- An M19 managed-heap array extrema receiver checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_extrema` now measures
  repeated heap-mode array `min()`/`max()` calls, and extrema scan inline array
  values and managed-heap slots directly instead of cloning the full receiver
  before comparison.
- An M19 managed-heap array sort receiver checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_sort` now measures
  repeated heap-mode array `sort()` calls, and heap-mode sort builds keys
  directly from heap slots instead of cloning the full receiver before sorting.
- An M19 managed-heap array slice receiver checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_slice` now measures
  repeated heap-mode array `slice()` calls, and heap-mode slice materializes
  only the requested range instead of cloning the full receiver before slicing.
- An M19 managed-heap array join receiver checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_join` now measures
  repeated heap-mode array `join()` calls, and heap-mode join reads string
  heap slots directly instead of cloning the full receiver before joining.
- An M19 managed-heap array reverse receiver checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_reverse` now measures
  repeated heap-mode array `reverse()` calls, and heap-mode reverse walks heap
  slots in reverse order instead of cloning and then reversing the full
  receiver.
- An M19 managed-heap array distinct benchmark coverage checkpoint is recorded
  in [performance.md](performance.md): `managed_heap_array_distinct` now
  measures repeated heap-mode array `distinct()` calls over inline numeric
  slots, string heap refs, and nested array heap refs; a direct heap-slot fast
  path was measured but not accepted because it regressed the mixed benchmark.
- An M19 host conversion benchmark coverage checkpoint is recorded in
  [performance.md](performance.md): `host_patch_tx` now also exercises host
  array reads, script string pushes through `PatchTx`, overlay length reads,
  and post-apply host array verification.
- An M19 managed-heap host conversion benchmark coverage checkpoint is recorded
  in [performance.md](performance.md): `managed_heap_host_conversion` now
  measures host execution with managed heap enabled while writing map, record,
  and enum aggregates through `PatchTx`; a direct heap-slot conversion path was
  measured but not accepted because quick runs were flat to slower.
- An M19 managed-heap set lookup checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_set_lookup` now measures
  repeated heap-mode `set.has()` calls, and membership checks scan existing set
  storage directly instead of materializing the full receiver first.
- An M19 managed-heap set combination checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_set_combination` now measures
  repeated heap-mode set combination and predicate methods, and those methods
  iterate borrowed set slots instead of cloning receiver sets first.
- An M19 managed-heap array lookup benchmark coverage checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_lookup` now measures
  repeated heap-mode `array.contains()` and `array.index_of()` calls; a direct
  heap-slot comparison path was measured but not accepted because it regressed
  the focused quick benchmark.
- An M19 managed-heap map lookup checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_map_lookup` now measures
  repeated heap-mode `map.has()`, `map.get()`, and `map.get_or()` calls over
  string and integer map values, and read-only map lookups borrow key strings
  instead of allocating owned lookup keys.
- An M19 managed-heap map merge checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_map_merge` now measures
  repeated heap-mode `map.merge()` calls, and merge iterates borrowed heap map
  slots instead of cloning receiver maps into temporary entry vectors first.
- An M19 managed-heap extend coverage checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_array_extend` and
  `managed_heap_map_extend` now measure repeated heap-mode mutating collection
  extension; direct temporary-vector cleanup candidates were measured but not
  accepted because quick runs were flat to slower.
- An M19 read-only method receiver checkpoint is recorded in
  [performance.md](performance.md): non-mutating string, callback, and stdlib
  method dispatch now tries a borrowed receiver fast path before falling back to
  the existing mutable receiver path.
- A gameplay-style M19 benchmark is recorded in [performance.md](performance.md):
  `gameplay_monster_kill` runs the real demo monster-kill script through
  HostPath reads/writes, PatchTx apply, stdlib callbacks, and host method
  patches.
- A numeric-dispatch M19 checkpoint is recorded in [performance.md](performance.md):
  bytecode add/sub/mul and numeric comparisons now use named integer/float
  operations, preserving checksums and source-spanned errors while avoiding
  float rounding for integer comparisons.
- An M19 scalar constant load checkpoint is recorded in
  [performance.md](performance.md): VM `LoadConst` now handles scalar constants
  directly on the dispatch path while string, array, and map constants keep the
  existing heap-aware conversion behavior.
- An M19 method argument materialization checkpoint is recorded in
  [performance.md](performance.md): one- and two-argument method calls now use
  stack-backed argument storage instead of allocating a temporary `Vec<Value>`,
  while later call-argument storage work extends stack-backed temporary
  argument slices through four values.
- An M19 managed-heap callback benchmark coverage checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_callback_collections` now
  runs the same callback-heavy map/set/array source as `callback_collections`
  through managed heap execution with matching checksums, giving heap-mode
  callback costs a direct benchmark surface.
- An M19 heap callback root-buffer checkpoint is recorded in
  [performance.md](performance.md): managed heap callback dispatch now appends
  caller roots, callback args, and protected values into the existing
  `HeapExecution` protected-root buffer instead of allocating a temporary
  `Vec<GcRef>` for each callback.
- An M19 heap map callback protection checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_map_callbacks` now measures
  repeated heap-mode map `map_values()` and `filter()` callbacks, and partial
  map results are protected through borrowed value iterators instead of a
  temporary protected-value `Vec`.
- An M19 managed-heap array group-by benchmark coverage checkpoint is recorded
  in [performance.md](performance.md): `managed_heap_array_group_by` now
  measures repeated heap-mode `array.group_by()` calls over script string
  arrays; a heap-slot snapshot fast path was measured but not accepted because
  the focused quick benchmark was flat to slower.
- An M19 call-entry default allocation checkpoint is recorded in
  [performance.md](performance.md): script function and closure calls now read
  parameter default flags directly from `CodeObject` instead of allocating a
  normalized defaults vector for every call, reducing callback-heavy
  invocation overhead.
- An M19 small script-field construction checkpoint is recorded in
  [performance.md](performance.md): managed-heap record/enum construction now
  uses zero-, one-, and two-field `ScriptFields` fast paths before falling back
  to the general sorted-field path, and `managed_heap_map_find_entries` now
  covers heap-mode `map.find()` returning `MapEntry` records.
- An M19 three-field script-field construction checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_record_triplets` now measures
  heap-mode three-field record/enum materialization, and unique three-field
  `ScriptFields` construction uses a sorted-slot fast path with duplicate
  inputs falling back to the general path.
- An M19 four-field script-field construction checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_record_quads` now measures
  heap-mode four-field record/enum materialization, and unique four-field
  `ScriptFields` construction uses the same sorted-slot fast path with
  duplicate inputs falling back to the general path.
- An M19 five-field script-field construction checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_record_quints` now measures
  heap-mode five-field record/enum materialization, and unique five-field
  `ScriptFields` construction uses the same sorted-slot fast path with
  duplicate inputs falling back to the general path.
- An M19 six-field script-field construction checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_record_sextets` now measures
  heap-mode six-field record/enum materialization, and unique six-field
  `ScriptFields` construction uses the same sorted-slot fast path with
  duplicate inputs falling back to the general path.
- An M19 runtime view architecture checkpoint is recorded in
  [performance.md](performance.md): read-only string, array, map, set, enum,
  and length-style receiver classification now flows through a crate-internal
  borrowed view layer, while script-visible collection layouts, managed heap
  objects, native/host interfaces, GC, and hot reload ABI remain unchanged.
- An M19 scalar dispatch benchmark coverage checkpoint is recorded in
  [performance.md](performance.md): `scalar_dispatch_mix` now exercises mixed
  integer, float, boolean, string comparison, branch, and loop behavior as a
  broader scalar interpreter measurement surface.
- An M19 scalar equality checkpoint is recorded in
  [performance.md](performance.md): direct `null`, bool, int, float, and string
  equality checks now avoid value materialization and string cloning while
  aggregate and heap-reference equality keep the previous fallback path.
- An M19 heap string equality checkpoint is recorded in
  [performance.md](performance.md): compact heap-ref string equality now
  compares borrowed heap string slots directly before falling back to
  materialized aggregate equality for non-string heap refs.
- An M19 truthy bytecode checkpoint is recorded in
  [performance.md](performance.md): logical `&&` and `||` result coercion now
  lowers to one `Truthy` instruction instead of a `Not`/`Not` pair, reducing
  scalar short-circuit dispatch work while preserving dynamic truthiness
  semantics.
- An M19 Option/Result helper tag checkpoint is recorded in
  [performance.md](performance.md): `managed_heap_option_result_helpers` now
  measures heap-mode helper-method chains, and Option/Result method dispatch
  branches on compact copied variant tags instead of cloning variant names into
  temporary strings.
- An M19 Option/Result fixed-field construction checkpoint is recorded in
  [performance.md](performance.md): fixed empty and single-field `ScriptFields`
  constructors preserve shape IDs while Option/Result constructors avoid the
  general pair-sorting path for `None`, `Some`, `Ok`, and `Err`.
- An M19 stdlib Option fixed-field construction checkpoint is recorded in
  [performance.md](performance.md): array/std-common Option results keep their
  existing shape-owner strings while skipping temporary field vectors and
  pair-sorted field construction.
- An M19 string length checkpoint is recorded in
  [performance.md](performance.md): script string `.len()` now uses byte length
  for ASCII strings and preserves Unicode character counts through the existing
  fallback path for non-ASCII strings.
- An M19 negated equality peephole checkpoint is recorded in
  [performance.md](performance.md): `!(lhs == rhs)` and `!(lhs != rhs)` now
  lower directly to inverse equality bytecode while preserving NaN-sensitive
  ordering comparison semantics.
- An M19 range iteration benchmark coverage checkpoint is recorded in
  [performance.md](performance.md): `range_iteration` now measures nested
  exclusive range loops and an inclusive range loop as a focused for-in
  iterator timing surface.
- An M19 range loop bytecode checkpoint is recorded in
  [performance.md](performance.md): direct `for value in start..end` and
  `start..=end` loops now use range cursor registers instead of a generic
  iterator state register, while other iterable categories keep the existing
  iterator path.
- An M19 read-only method root checkpoint is recorded in
  [performance.md](performance.md): heap-mode string and read-only stdlib
  methods now dispatch before collecting caller frame roots, while callback and
  script-defined method paths keep the existing protected-root behavior.
- An M19 wide call argument storage checkpoint is recorded in
  [performance.md](performance.md): `script_call_wide_args` and
  `native_call_wide_args` now measure three- and four-argument calls, and
  temporary call argument storage now uses hand-written stack-array enum
  variants through four values before falling back to `Vec<Value>`.

### Remaining Gaps

- M19: continue optimizing the non-JIT interpreter and managed heap path only
  with before/after benchmark evidence, focusing next on remaining stdlib heap
  receiver materialization, measured host conversion deltas, callback
  invocation overhead, scalar dispatch optimizations, and gameplay-host
  benchmark deltas.
- M20+: keep inline-cache and specialization work behind M19 benchmarked
  interpreter/heap improvements.

### Validation

Use the relevant subset of [validation.md](validation.md) for each change.
Default full validation remains:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For current M19 work, run focused correctness tests for touched runtime areas
plus the relevant benchmark before and after each optimization. Optimized paths
must preserve ExecutionBudget, PatchTx, reflection policy, GC roots, hot reload
ownership, and source-spanned diagnostics.

## Next Up

- Choose the next narrow measured M19 optimization target from the updated
  checkpoint notes, with remaining stdlib heap receiver materialization, host
  conversion deltas, callback invocation overhead, and scalar dispatch
  currently the clearest candidates; include the gameplay-host benchmark when
  relevant.
- Keep benchmark evidence ahead of M19/M20 optimization work.
- Plan M21 debugger and M22 Cranelift JIT only from stable source-span,
  frame-map, GC-root, budget, PatchTx, hot-reload, and conformance contracts.

## Update Rules

- Update this file when current focus, milestone status, available capability
  coverage, validation expectations, or remaining current gaps change.
- Do not append routine implementation details, small refactors, or every
  commit result here; those belong in commit history or focused tests.
- Keep the file quick to scan. If durable historical context becomes necessary,
  summarize it once and archive the long form under `docs/archive/`.
