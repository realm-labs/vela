# Performance

This document tracks how Vela performance should be measured and optimized.
It is not a substitute for conformance tests: correctness, host-boundary safety,
hot reload semantics, budgets, and diagnostics remain required for every fast
path.

## Measurement Principles

Performance measurements should distinguish source loading, compilation,
warmup, and repeated execution. Function-call benchmarks should load and compile
scripts before timing, then measure repeated calls to an already-loaded
function.

Precompiled `.vbc` bytecode artifacts should be measured as a
load/startup/reload optimization. They can reduce parser, HIR, analysis, and
bytecode generation costs, but they should not be counted as an execution-speed
optimization for an already-loaded function.

Benchmark suites should track these cost centers separately:

```text
VM instruction dispatch
managed heap allocation and result materialization
dynamic Value operations
dynamic stdlib method dispatch
record, Option, and Result helper paths
string allocation and copying
missing inline caches, specialization, and JIT
```

Pure script microbenchmarks and host-heavy gameplay benchmarks should be
reported separately. `PatchTx` cost belongs in host-boundary benchmarks, not in
scalar VM dispatch conclusions.

Only tracked benchmark sources, baselines, and reports define the official
benchmark surface.

## Tracked Harnesses

The first tracked M18 harness lives in `crates/vela_vm/benches/baseline.rs` and
can be run with:

```bash
cargo bench -p vela_vm --bench baseline
```

Hot reload compile/apply and ABI rejection timing lives in
`crates/vela_engine/benches/hot_reload.rs` and can be run with:

```bash
cargo bench -p vela_engine --bench hot_reload
```

External reference comparisons live in
`crates/vela_vm/benches/external_compare.rs` and can be run with:

```bash
cargo bench -p vela_vm --bench external_compare
```

For quick validation during implementation:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_engine --bench hot_reload -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
```

The baseline harness intentionally has no external benchmarking dependency yet.
It separates script compilation from repeated execution timing, runs in Cargo's
bench/release profile, and reports one line per workload with:

```text
benchmark name
execution mode
min, mean, median, and p95 nanoseconds
checksum
repeat, iteration, warmup, profile, OS, and architecture parameters
```

Current tracked workload groups:

```text
scalar_branch_loop          VM dispatch, arithmetic, branches, range for-in
script_call_small_args      script function calls with one- and two-argument calls
stdlib_collections          array, map, set, Option, and stdlib method dispatch
host_patch_tx               HostRef reads, nested HostPath writes, PatchTx overlay
gameplay_monster_kill       demo monster kill workflow with HostPath, PatchTx, stdlib callbacks, and host methods
managed_heap_materialization records, enums, strings, Option helpers, heap mode
gc_pacing                   safe-point GC under managed heap allocation pressure
hot_reload_accept           compatible update compile/apply and post-apply call
hot_reload_abi_reject       rejected event ABI update and report generation
external_compare            Vela plus available Lua 5.x, LuaJIT, Node, and Rhai
```

The external comparison harness records missing runtimes explicitly instead of
failing the benchmark. On each machine it reports the executable version for
available runtimes, the process-backed timing mode used for external tools, and
the same repeat, iteration, warmup, profile, OS, architecture, and checksum
fields as the internal baseline harnesses.

## Baseline Captures

### 2026-06-04 Quick Baseline

This quick baseline was captured before M19 optimization work. It is intended
as an implementation checkpoint, not a release-quality benchmark report.

Environment:

```text
commit=47a6589
rustc=1.96.0 (ac68faa20 2026-05-25)
cargo=1.96.0 (30a34c682 2026-05-25)
host=x86_64-pc-windows-msvc
target=windows/x86_64
profile=release
```

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_engine --bench hot_reload -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
```

Parameters:

```text
vela_vm baseline repeats=2 iterations=8 warmup=2
vela_engine hot_reload repeats=2 iterations=8 warmup=2
vela_vm external_compare repeats=2 iterations=8 warmup=1
```

Internal VM baseline:

| Benchmark | Mode | Min ns | Mean ns | Median ns | P95 ns | Checksum |
|---|---|---:|---:|---:|---:|---:|
| scalar_branch_loop | inline | 611300 | 616200 | 621100 | 621100 | 5382776514408301204 |
| stdlib_collections | inline | 320200 | 330400 | 340600 | 340600 | 13147904610567772544 |
| host_patch_tx | host_patch_tx | 21600 | 21700 | 21800 | 21800 | 9734661366414131234 |
| managed_heap_materialization | managed_heap | 612000 | 729550 | 847100 | 847100 | 11773534860610571856 |
| gc_pacing | gc_pacing | 24138300 | 24831400 | 25524500 | 25524500 | 16625037316567583116 |

Hot reload baseline:

| Benchmark | Mode | Min ns | Mean ns | Median ns | P95 ns | Checksum |
|---|---|---:|---:|---:|---:|---:|
| hot_reload_accept | compile_apply | 1169700 | 1311550 | 1453400 | 1453400 | 6588661877666281699 |
| hot_reload_abi_reject | compile_reject | 767700 | 790750 | 813800 | 813800 | 6965985632367789055 |

External comparison baseline:

| Runtime | Version/status | Mode | Min ns | Mean ns | Median ns | P95 ns | Checksum |
|---|---|---|---:|---:|---:|---:|---:|
| vela | 0.1.0 | internal | 576600 | 579600 | 582600 | 582600 | 14532497248610255407 |
| lua5 | missing: `lua`, `lua5.4`, `lua5.3` | n/a | n/a | n/a | n/a | n/a | n/a |
| luajit | missing: `luajit` | n/a | n/a | n/a | n/a | n/a | n/a |
| node | v24.15.0 | process | 38736800 | 40990950 | 43245100 | 43245100 | 5650647607070153539 |
| rhai | missing: `rhai-run` | n/a | n/a | n/a | n/a | n/a | n/a |

Initial bottleneck notes:

```text
gc_pacing is the slowest quick workload by a wide margin and should be
investigated before GC pacing optimization.
managed_heap_materialization is slower and more variable than inline scalar and
stdlib workloads, so heap allocation/materialization is an early M19 candidate.
hot_reload_accept is costlier than ABI rejection because it compiles, applies,
and performs a post-update call; keep reload measurements separate from steady
function execution.
external Node timing uses process mode and includes process startup overhead, so
it is useful for version/availability tracking but not a direct VM dispatch
comparison.
Lua 5.x, LuaJIT, and Rhai were unavailable on this machine, so the primary
non-JIT external comparison target remains unmeasured for this capture.
```

### 2026-06-04 Full Baseline

This full/default baseline was captured before M19 optimization work using the
tracked harness default parameters.

Environment:

```text
commit=cd64022
rustc=1.96.0 (ac68faa20 2026-05-25)
cargo=1.96.0 (30a34c682 2026-05-25)
host=x86_64-pc-windows-msvc
target=windows/x86_64
profile=release
```

Commands:

```bash
cargo bench -p vela_vm --bench baseline
cargo bench -p vela_engine --bench hot_reload
cargo bench -p vela_vm --bench external_compare
```

Parameters:

```text
vela_vm baseline repeats=7 iterations=100 warmup=10
vela_engine hot_reload repeats=7 iterations=100 warmup=10
vela_vm external_compare repeats=5 iterations=100 warmup=3
```

Internal VM baseline:

| Benchmark | Mode | Min ns | Mean ns | Median ns | P95 ns | Checksum |
|---|---|---:|---:|---:|---:|---:|
| scalar_branch_loop | inline | 7141400 | 7278714 | 7219300 | 7603600 | 14794452088437409837 |
| stdlib_collections | inline | 3301600 | 3479714 | 3409000 | 3818900 | 8455524478326472193 |
| host_patch_tx | host_patch_tx | 300700 | 301585 | 301200 | 303200 | 2706371544431107761 |
| managed_heap_materialization | managed_heap | 6992400 | 7122628 | 7148300 | 7250600 | 1965056817950502848 |
| gc_pacing | gc_pacing | 1097458400 | 1119594114 | 1108094000 | 1162844900 | 10923073775105338595 |

Hot reload baseline:

| Benchmark | Mode | Min ns | Mean ns | Median ns | P95 ns | Checksum |
|---|---|---:|---:|---:|---:|---:|
| hot_reload_accept | compile_apply | 12637000 | 13063714 | 12992000 | 13580200 | 16819348956461335541 |
| hot_reload_abi_reject | compile_reject | 9215500 | 10452328 | 9303500 | 12234700 | 8095282285294424121 |

External comparison baseline:

| Runtime | Version/status | Mode | Min ns | Mean ns | Median ns | P95 ns | Checksum |
|---|---|---|---:|---:|---:|---:|---:|
| vela | 0.1.0 | internal | 7091400 | 7758440 | 7225800 | 9519800 | 310942833354159201 |
| lua5 | missing: `lua`, `lua5.4`, `lua5.3` | n/a | n/a | n/a | n/a | n/a | n/a |
| luajit | missing: `luajit` | n/a | n/a | n/a | n/a | n/a | n/a |
| node | v24.15.0 | process | 37929300 | 40226140 | 39597500 | 43048100 | 8356183458656122754 |
| rhai | missing: `rhai-run` | n/a | n/a | n/a | n/a | n/a | n/a |

Full-baseline bottleneck notes:

```text
gc_pacing is still dominant at roughly 1.12 seconds mean for 700 total
iterations. M19 should inspect safe-point sweep work, seeded garbage setup,
allocation pressure, and GC step accounting before changing general VM dispatch.
managed_heap_materialization and scalar_branch_loop are close in mean time on
this machine, which points to heap/materialization costs and scalar dispatch
both needing measurement-preserving optimization.
stdlib_collections is materially faster than scalar_branch_loop in this
workload mix, so broad stdlib rewrites should wait for narrower evidence.
host_patch_tx is much cheaper than the script-heavy workloads in this harness;
do not optimize PatchTx first unless a host-heavy gameplay benchmark exposes a
different profile.
hot_reload_accept remains slower than ABI rejection and should stay measured as
a compile/update workflow, not as steady-state execution.
Lua 5.x, LuaJIT, and Rhai were unavailable on this machine. The external
comparison harness still records the missing commands and Node.js version, but
the Lua-comparable M19 target requires a later capture on a machine with Lua
5.x installed.
```

### 2026-06-04 M19 GC Root Buffer Checkpoint

This checkpoint optimized safe-point GC root collection without changing the GC
algorithm, safe-point cadence, budget accounting, or benchmark checksums. Before
each safe-point GC step, `HeapExecution` now reuses one root buffer and appends
current frame roots directly instead of allocating temporary root vectors.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| gc_pacing | 21782500 | 6617300 | 16625037316567583116 | 16625037316567583116 |

Default baseline comparison against the pre-M19 full baseline:

| Benchmark | Pre-M19 mean ns | After mean ns | Pre-M19 checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_materialization | 7122628 | 1805528 | 1965056817950502848 | 1965056817950502848 |
| gc_pacing | 1119594114 | 86437328 | 10923073775105338595 | 10923073775105338595 |

Checkpoint notes:

```text
The large managed_heap_materialization improvement is expected because the same
safe-point root path is active in managed heap execution.
Checksums stayed stable for both quick and default runs, so the optimization is
measurement-preserving for these harnesses.
GC pacing remains the largest default VM workload after this change, but it is
now much closer to the other tracked groups. Further M19 work should inspect
heap materialization, allocation pressure, and scalar dispatch before adding
larger GC policy changes.
```

### 2026-06-04 M19 GC Continuation Checkpoint

This checkpoint avoids frame-root scanning on incremental GC continuation
steps. `ScriptHeap::step_gc_with_budget` uses roots only when starting a
collection and performs sweep-only work while a collection is already in
progress, so `HeapExecution` now gathers roots only for the collection-start
step.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| gc_pacing | 6491400 | 5326100 | 16625037316567583116 | 16625037316567583116 |

Default baseline comparison against the previous M19 checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| gc_pacing | 86437328 | 78924471 | 10923073775105338595 | 10923073775105338595 |

Checkpoint notes:

```text
The optimization preserves the incremental GC algorithm: roots still determine
the mark set at collection start, and continuation steps keep sweeping the
already-started collection.
Checksums stayed stable for quick and default runs.
GC pacing is still the largest tracked VM workload, but remaining work should
now balance GC allocation pressure against scalar dispatch and broader
gameplay-style benchmark coverage.
```

### 2026-06-04 M19 Gameplay Baseline Checkpoint

This checkpoint adds a tracked gameplay-style host workload to the VM baseline
harness. `gameplay_monster_kill` compiles the real
`examples/game_server_demo/scripts/monster_kill_reward.vela` source and runs it
through a `MockStateAdapter`, `HostPath` reads and writes, `PatchTx` apply,
stdlib `filter` callback dispatch, and host method patches. Compilation remains
outside the timed loop.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Initial gameplay baseline:

| Run | Benchmark | Mode | Min ns | Mean ns | Median ns | P95 ns | Checksum |
|---|---|---|---:|---:|---:|---:|---:|
| quick | gameplay_monster_kill | gameplay_host | 189000 | 198100 | 207200 | 207200 | 11641737387043360531 |
| default | gameplay_monster_kill | gameplay_host | 2094500 | 2159685 | 2121000 | 2329600 | 5386942582173291744 |

Checkpoint notes:

```text
The new workload gives M19 a host-heavy gameplay timing target before adding
inline caches or broader specialization.
This benchmark exercises PatchTx and stdlib callback behavior together; keep it
separate from scalar VM dispatch conclusions.
Future M19 optimization reports should include this workload when touching host
paths, callbacks, collection methods, or PatchTx-heavy execution.
```

### 2026-06-04 M19 Numeric Dispatch Checkpoint

This checkpoint replaces the generic closure-based bytecode numeric helpers
with named add/sub/mul and comparison operations. VM error kinds and
source-spanned diagnostics are preserved, while integer comparisons now stay in
the integer domain instead of converting through `f64`.

Commands:

```bash
git worktree add --detach ../vela-bench-head HEAD
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session. The before run used a
detached worktree at `ccebc40`; the after run used the numeric-dispatch working
tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| scalar_branch_loop | 589250 | 567550 | 5382776514408301204 | 5382776514408301204 |
| gameplay_monster_kill | 193250 | 181200 | 11641737387043360531 | 11641737387043360531 |
| managed_heap_materialization | 168650 | 150750 | 11773534860610571856 | 11773534860610571856 |
| gc_pacing | 5099800 | 5084650 | 16625037316567583116 | 16625037316567583116 |

Default before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| scalar_branch_loop | 7438914 | 7421900 | 14794452088437409837 | 14794452088437409837 |
| stdlib_collections | 3472385 | 3367914 | 8455524478326472193 | 8455524478326472193 |
| gameplay_monster_kill | 2341614 | 2223314 | 5386942582173291744 | 5386942582173291744 |
| managed_heap_materialization | 2061700 | 1855300 | 1965056817950502848 | 1965056817950502848 |
| gc_pacing | 115115285 | 64910728 | 10923073775105338595 | 10923073775105338595 |

Checkpoint notes:

```text
Checksums stayed stable for every reported workload.
The biggest default improvement appears in gc_pacing because its inner loop is
numeric-heavy even though the benchmark is still categorized as a GC workload.
Scalar dispatch remains a broader target: register access, branch dispatch, and
instruction-loop structure are still unoptimized beyond these named numeric
operations.
Heap allocation pressure remains the clearest next M19 target after this
checkpoint.
```

### 2026-06-04 M19 GC Mark Stack Checkpoint

This checkpoint reduces GC mark-phase allocation pressure. `ScriptHeap` now
keeps a reusable mark stack and extends it from the current roots at collection
start instead of allocating a fresh `Vec<GcRef>` for every `mark_from_roots`
call. Marking, sweeping, GC roots, execution-budget memory accounting, and
checksums are unchanged.

Commands:

```bash
git worktree add --detach ../vela-heap-bench-head HEAD
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session. The before run used a
detached worktree at `57e1b4d`; the after run used the mark-stack working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| gc_pacing | 5255300 | 4487850 | 16625037316567583116 | 16625037316567583116 |

Default before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| gc_pacing | 106587671 | 65015785 | 10923073775105338595 | 10923073775105338595 |

Checkpoint notes:

```text
This is a narrow GC allocation-pressure optimization; it does not change the
heap object model or expose script-owned state outside the existing GC.
The reusable mark stack is runtime bookkeeping and remains outside script memory
budget charging, matching the previous temporary root-stack allocation behavior.
Remaining M19 heap work should focus on script value allocation/materialization
and temporary collection construction outside the GC mark stack.
```

### 2026-06-04 M19 Heap Aggregate Construction Checkpoint

This checkpoint removes temporary `Value` aggregate construction in managed heap
mode for bytecode array, map, record, and enum literals. When a heap execution
is active, `MakeArray`, `MakeMap`, `MakeRecord`, and `MakeEnum` now convert
source registers directly into `HeapSlot` collections before allocating the
heap object. Non-heap execution still constructs the same `Value` aggregates.

Commands:

```bash
git worktree add --detach ../vela-aggregate-bench-head HEAD
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session. The before run used a
detached worktree at `74c234c`; the after run used the aggregate-construction
working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| gameplay_monster_kill | 207200 | 201100 | 11641737387043360531 | 11641737387043360531 |
| managed_heap_materialization | 184400 | 151400 | 11773534860610571856 | 11773534860610571856 |
| gc_pacing | 4956250 | 6647750 | 16625037316567583116 | 16625037316567583116 |

Default before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| gameplay_monster_kill | 2287300 | 2135071 | 5386942582173291744 | 5386942582173291744 |
| managed_heap_materialization | 1829471 | 1709614 | 1965056817950502848 | 1965056817950502848 |
| gc_pacing | 70608685 | 58571514 | 10923073775105338595 | 10923073775105338595 |

Checkpoint notes:

```text
Checksums stayed stable for every reported workload.
Quick gc_pacing was noisy in this session, but the default run improved and the
change directly affects the temporary array construction inside that workload.
This removes one heap-mode temporary aggregate layer; remaining materialization
pressure is now more likely in native/stdlib boundaries, returned heap object
materialization, string construction, callbacks, and mutable collection methods.
```

### 2026-06-04 M19 Callback Root Guard Checkpoint

This checkpoint reduces callback and method-call allocation pressure outside
managed heap execution. VM method dispatch now collects caller heap roots only
when a heap exists, and callback invocation skips temporary root-vector
construction when there is no heap to protect. Managed heap execution keeps the
same caller-root and protected-root behavior.

Commands:

```bash
git worktree add --detach ../vela-callback-bench-head HEAD
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session. The before run used a
detached worktree at `83f6f6f`; the after run used the callback-root working
tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 270850 | 227100 | 13147904610567772544 | 13147904610567772544 |

Default before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 3133314 | 2694171 | 8455524478326472193 | 8455524478326472193 |

Checkpoint notes:

```text
The improvement is intentionally scoped to non-heap stdlib method and callback
dispatch. Heap-mode callback paths still collect and protect roots before
executing callbacks.
Checksums stayed stable for the reported workload.
Remaining callback work should focus on argument vector construction and closure
call overhead, not no-heap GC root protection.
```

### 2026-06-04 M19 GroupBy Protected-Value Guard Checkpoint

This checkpoint removes another no-heap callback allocation from
`array.group_by`. Grouping callbacks still protect previously-built groups
when managed heap execution is active, but inline/no-heap execution now skips
building the protected-value clone vector that only feeds heap root protection.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
git worktree add --detach ../vela-groupby-bench-head HEAD
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 244050 | 217050 | 13147904610567772544 | 13147904610567772544 |

Default before/after from the same working session. The before run used a
detached worktree at `b6e15c3`; the after run used the group-by protected-value
working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 2612585 | 2474571 | 8455524478326472193 | 8455524478326472193 |

Checkpoint notes:

```text
The optimization is scoped to no-heap array.group_by callback dispatch.
Managed heap execution still collects the previously-built groups as protected
roots before executing each callback.
Remaining callback work should focus on map/sort protected-value construction,
map callback argument vectors, and closure call overhead.
```

### 2026-06-04 M19 Map/Sort Callback Allocation Checkpoint

This checkpoint adds a targeted `callback_collections` benchmark and reduces
allocation pressure in map and sort callbacks. Map higher-order methods now pass
zero-, one-, and two-argument callbacks from stack-local slices instead of
allocating a callback argument `Vec` per entry. No-heap `map.map_values`,
`map.filter`, and `array.sort_by` callback dispatch also skips protected-value
clone vectors that are only needed for managed heap root protection.

Managed heap execution keeps protected-value root collection for callbacks that
can allocate while previously-built map or sort results remain live.

Commands:

```bash
git worktree add --detach ../vela-callback-protected-before HEAD
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

The before worktree used `0bba76b` with only the new benchmark workload applied;
the after run used the map/sort callback allocation working tree.

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 12008950 | 6794900 | 3270490998308601835 | 3270490998308601835 |

Default before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 82308542 | 75103300 | 12210731834836948394 | 12210731834836948394 |

Checkpoint notes:

```text
The new callback_collections benchmark exercises map_values, map filter, and
array sort_by callbacks over a repeated map workload.
Checksums stayed stable for the reported workload.
Remaining callback work should focus on closure invocation overhead and broader
stdlib/native boundary materialization.
```

### 2026-06-04 M19 Native Argument Materialization Checkpoint

This checkpoint reduces managed-heap native-call boundary work. `CallNative`
now materializes argument registers directly into the native argument vector
instead of first cloning register values into a temporary `Vec<Value>` and then
materializing that second pass. Native calls still receive fully materialized
script-owned `Value` arguments, preserving the heap and host boundary contract.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
git worktree add --detach ../vela-native-materialize-before HEAD
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_materialization | 139450 | 137200 | 11773534860610571856 | 11773534860610571856 |

Default before/after from the same working session. The before run used a
detached worktree at `0cf817f`; the after run used the native argument
materialization working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_materialization | 1697428 | 1672942 | 1965056817950502848 | 1965056817950502848 |

Checkpoint notes:

```text
The optimization is intentionally narrow: it removes one temporary vector from
native argument materialization while preserving the materialized Value boundary
for native calls.
Remaining materialization work is still likely in native return storage,
stdlib heap receiver conversion, host conversion, and returned heap objects.
```

### 2026-06-04 M19 Returned Heap Object Storage Checkpoint

This checkpoint reduces managed-heap return storage work. VM return and method
results that are already owned `Value` aggregates now move strings,
arrays, maps, sets, records, and enums into heap objects directly instead of
converting through borrowed heap-slot helpers that clone the same aggregate data
before allocation. Nested heap-slot conversion still preserves the same
budget charging, GC root, and script-owned heap boundary behavior.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
git worktree add --detach ../vela-owned-return-before HEAD
cargo bench -p vela_vm --bench baseline
```

Quick warmed before/after from the same working session. The before run used a
detached worktree at `4a901f8`; the after run used the owned return storage
working tree after the benchmark binary was already compiled.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_materialization | 138400 | 131200 | 11773534860610571856 | 11773534860610571856 |

Default before/after from the same working session. The before run used the
same detached worktree at `4a901f8`; the after run used the owned return
storage working tree. Other workloads were noisy during the default runs, so
the target workload and stable checksum are the useful signal.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_materialization | 1688600 | 1576128 | 1965056817950502848 | 1965056817950502848 |

Checkpoint notes:

```text
The optimization is scoped to storing owned return/method-result aggregates in
managed heap mode. Borrowed heap-slot conversion remains available for frame
registers, host values, and other paths that do not own the source Value.
Remaining materialization work is more likely in stdlib heap receiver
conversion, host conversion, string operations, and callback invocation.
```

### 2026-06-04 M19 Array Lookup Receiver Checkpoint

This checkpoint reduces array stdlib receiver work for `first`, `last`,
`contains`, and `index_of`. These methods now read or scan `Value::Array`
receivers directly and iterate heap array slots one element at a time instead
of first cloning or materializing the whole receiver into a temporary
`Vec<Value>`. Heap array receivers still materialize individual slots only when
the method needs to return or compare that element.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
git worktree add --detach ../vela-array-lookup-before HEAD
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 218950 | 205700 | 13147904610567772544 | 13147904610567772544 |

Default before/after from the same working session. The before run used a
warmed detached worktree at `e4ab551`; the after run used the array lookup
working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 2378228 | 2350500 | 8455524478326472193 | 8455524478326472193 |

Checkpoint notes:

```text
The benchmark signal is strongest in the quick stdlib_collections workload,
which calls first and last after array transforms. The same direct path covers
managed heap arrays and avoids full receiver materialization for contains and
index_of, but broader stdlib heap receiver work remains in map, set, transform,
ordering, and callback-heavy methods.
```

### 2026-06-04 M19 Map Callback Entry Checkpoint

This checkpoint reduces no-heap map callback overhead. `map.map_values`,
`map.filter`, `map.find`, `map.any`, `map.all`, and `map.count` now iterate
`Value::Map` receivers directly instead of first cloning the whole receiver
into a temporary entry vector. Managed-heap map receivers keep the existing
materialized-entry path so callback execution can borrow `HeapExecution`
mutably without holding an immutable heap-map borrow across the callback.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
git worktree add --detach ../vela-map-callback-before HEAD
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 6412800 | 5747250 | 3270490998308601835 | 3270490998308601835 |

Default before/after from the same working session. The before run used a
warmed detached worktree at `ac9fa81`; the after run used the map callback
working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 76368914 | 68746600 | 12210731834836948394 | 12210731834836948394 |

Checkpoint notes:

```text
The optimization is scoped to no-heap Value::Map higher-order callbacks and
preserves managed-heap callback root behavior. Remaining callback work should
focus on heap-mode protected values, closure invocation overhead, and set/array
callback receiver materialization.
```

### 2026-06-04 M19 Array Sort Callback Receiver Checkpoint

This checkpoint reduces no-heap `array.sort_by` callback overhead. The method
now iterates `Value::Array` receivers directly instead of first cloning the
whole receiver through `array_values`. Managed-heap receivers keep the existing
materialized-entry path so already-collected sort entries remain protected as
callback roots.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
git worktree add --detach ../vela-array-receiver-before HEAD
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 5804150 | 5781000 | 3270490998308601835 | 3270490998308601835 |

Default before/after from warmed runs in the same working session. The before
run used a detached worktree at `8498ea5`; the after run used the
`array.sort_by` working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 77584071 | 67049900 | 12210731834836948394 | 12210731834836948394 |

Checkpoint notes:

```text
The optimization is scoped to no-heap array.sort_by receiver iteration and
preserves managed-heap callback root behavior. Broader array aggregation,
plain ordering, set callbacks, heap-mode protected values, and closure
invocation overhead remain separate measured targets.
```

### 2026-06-04 M19 Set Callback Benchmark Coverage Checkpoint

This measurement checkpoint expands the `callback_collections` workload to
cover set `filter`, `map`, `find`, `any`, `all`, and `count` callbacks in the
same repeated collection loop that already covers map callbacks and
`array.sort_by`. It does not accept a runtime fast path by itself; a direct
no-heap set receiver experiment was rejected in this working session because
it did not improve the warmed default benchmark.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
git worktree add --detach ../vela-set-callback-before HEAD
cargo bench -p vela_vm --bench baseline
```

Expanded benchmark baseline:

| Benchmark | Quick mean ns | Quick checksum | Default mean ns | Default checksum |
|---|---:|---:|---:|---:|
| callback_collections | 10642800 | 13737855412215224532 | 130130614 | 3465184824986257422 |

Checkpoint notes:

```text
The callback_collections benchmark now includes set callback semantics and
can be used for future set callback receiver materialization work. Remaining
set work should be accepted only with benchmark evidence from this expanded
surface or a more targeted set callback workload.
```

### 2026-06-04 M19 Array Higher-Order Callback Benchmark Coverage Checkpoint

This measurement checkpoint expands the `callback_collections` workload to
cover array `map`, `filter`, `find`, `any`, `all`, and `count` callbacks in the
same repeated collection loop that already covers map callbacks, set callbacks,
and `array.sort_by`. It does not accept a runtime fast path by itself; a direct
no-heap array higher-order receiver experiment was rejected in this working
session because the quick benchmark worsened from `14962650` ns to `15213150`
ns while preserving the checksum.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Expanded benchmark baseline:

| Benchmark | Quick mean ns | Quick checksum | Default mean ns | Default checksum |
|---|---:|---:|---:|---:|
| callback_collections | 14870500 | 6661976061914330346 | 185455500 | 4123773336162002392 |

Checkpoint notes:

```text
The callback_collections benchmark now includes array higher-order callback
semantics and can be used for future array callback receiver materialization
work. Remaining array higher-order callback work should be accepted only with
benchmark evidence from this expanded surface or a targeted array callback
workload.
```

### 2026-06-04 M19 Host Conversion Benchmark Coverage Checkpoint

This measurement checkpoint expands the `host_patch_tx` workload beyond integer
host path reads and numeric patch operations. The benchmark now reads a host
array, pushes a script string through `PatchTx`, observes the overlay length in
script code, applies the transaction, and includes the applied host array length
in the checksum. This gives future host conversion work a focused benchmark
surface for `HostValue::Array`, `HostValue::String`, `Value::Array`, and
`Value::String` conversion in addition to numeric patches.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Expanded benchmark baseline:

| Benchmark | Quick mean ns | Quick checksum | Default mean ns | Default checksum |
|---|---:|---:|---:|---:|
| host_patch_tx | 51000 | 8875875486420011969 | 710442 | 1944703388338173655 |

Checkpoint notes:

```text
The host_patch_tx benchmark now covers array/string host conversion and
transaction apply verification, not only integer read/modify/write paths. Future
host conversion optimizations should preserve this checksum and report
before/after results against this expanded workload or a more targeted host
conversion benchmark.
```

### 2026-06-04 M19 Read-Only Method Receiver Fast Path Checkpoint

This checkpoint reduces non-mutating method dispatch overhead by trying a
borrowed receiver path before cloning the receiver for the existing mutable
method fallback. The fast path covers string methods, callback methods, and
read-only stdlib collection/Option/Result methods. Mutating methods such as
`push`, `pop`, `insert`, `extend`, `set`, `add`, `remove`, and `clear` still
use the existing mutable receiver path, and the optimized path still reaches
the normal instruction-end GC safe point.

Commands:

```bash
git worktree add --detach ../vela-readonly-method-before HEAD
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from warmed runs in the same working session. The before run
used a detached worktree at `ab57a95`; the after run used the read-only method
receiver working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 205750 | 163200 | 13147904610567772544 | 13147904610567772544 |
| callback_collections | 14815850 | 12799750 | 6661976061914330346 | 6661976061914330346 |
| host_patch_tx | 51650 | 48900 | 8875875486420011969 | 8875875486420011969 |

Default before/after from warmed runs in the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 2404842 | 1988657 | 8455524478326472193 | 8455524478326472193 |
| callback_collections | 185523200 | 166372700 | 4123773336162002392 | 4123773336162002392 |
| host_patch_tx | 715028 | 666242 | 1944703388338173655 | 1944703388338173655 |

Checkpoint notes:

```text
Checksums stayed stable for every reported workload.
The optimization is scoped to non-mutating method dispatch; mutating methods and
script impl methods keep the existing mutable receiver behavior.
Remaining callback work should focus on callback invocation overhead, heap-mode
receiver materialization, and set/array callback receiver materialization that
still clones or materializes collection entries.
```

### 2026-06-04 M19 Method Argument Materialization Checkpoint

This checkpoint reduces interpreter method-call overhead by avoiding a heap
`Vec<Value>` allocation for one- and two-argument `CallMethod` and
`CallMethodId` dispatch. Zero-argument method calls still pass the existing
empty slice directly, and method calls with three or more arguments keep the
existing vector-backed path. The optimization changes only argument
materialization for method dispatch; receiver mutation, heap storage of return
values, GC safe points, budgets, and source-spanned errors use the existing
paths.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from warmed runs in the same working session. The before run
used commit `adbafb5`; the after run used the method-argument working tree.

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 172950 | 164250 | 13147904610567772544 | 13147904610567772544 |
| callback_collections | 13037350 | 12446050 | 6661976061914330346 | 6661976061914330346 |
| host_patch_tx | 49250 | 48750 | 8875875486420011969 | 8875875486420011969 |
| gameplay_monster_kill | 168600 | 171250 | 11641737387043360531 | 11641737387043360531 |
| managed_heap_materialization | 131350 | 121000 | 11773534860610571856 | 11773534860610571856 |

Default after-run comparison against the previous read-only method receiver
checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| stdlib_collections | 1988657 | 1981728 | 8455524478326472193 | 8455524478326472193 |
| callback_collections | 166372700 | 154637371 | 4123773336162002392 | 4123773336162002392 |
| host_patch_tx | 666242 | 655728 | 1944703388338173655 | 1944703388338173655 |

Checkpoint notes:

```text
Checksums stayed stable for every reported workload. The strongest signal is in
callback-heavy method dispatch, where one- and two-argument calls are common.
Remaining callback work should focus on callback invocation overhead and
heap-mode receiver/materialization costs rather than generic method argument
allocation.
```

### 2026-06-04 M19 Managed-Heap Callback Benchmark Coverage Checkpoint

This measurement checkpoint adds `managed_heap_callback_collections`, a
managed-heap version of the existing callback-heavy collections workload. The
source is shared with `callback_collections`, so the two benchmark rows compare
the same map/set/array callback behavior with and without managed heap
execution. Matching checksums verify that both modes produce the same script
result.

Commands:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Expanded benchmark baseline:

| Benchmark | Mode | Quick mean ns | Quick checksum | Default mean ns | Default checksum |
|---|---|---:|---:|---:|---:|
| callback_collections | inline | 12428900 | 6661976061914330346 | 153822685 | 4123773336162002392 |
| managed_heap_callback_collections | managed_heap | 19018650 | 6661976061914330346 | 242331900 | 4123773336162002392 |

Checkpoint notes:

```text
The managed-heap callback benchmark now gives M19 a direct timing surface for
heap-mode callback root protection, receiver materialization, callback return
storage, and heap value conversion costs. Future heap-mode callback
optimizations should preserve the matching checksum and report before/after
results against this workload.
```

### 2026-06-04 M19 Heap Callback Root Buffer Checkpoint

This checkpoint removes per-callback temporary root-vector allocation in
managed heap callback dispatch. `HeapExecution` now appends caller roots,
callback arguments, and protected callback values directly into its existing
protected-root buffer, then truncates that buffer after the nested call. Nested
script function and closure calls also append frame roots directly into the
same buffer instead of first building a temporary frame-root vector.

Commands:

```bash
cargo test -p vela_vm
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_callback_collections | 19384000 | 19242050 | 6661976061914330346 | 6661976061914330346 |

Default after-run comparison against the managed-heap callback benchmark
coverage checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 153822685 | 157089685 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 242331900 | 228158371 | 4123773336162002392 | 4123773336162002392 |

Checkpoint notes:

```text
Checksums stayed stable for the callback workload in both modes. The intended
signal is the managed-heap callback row, where root protection no longer builds
a fresh Vec<GcRef> for each callback invocation. The inline callback row is
included as a guardrail; its default timing moved against the change within the
same benchmark surface even though the edited path is heap-only for callbacks.
Remaining callback work should focus on receiver materialization and invocation
overhead that still shows up in both callback modes.
```

### 2026-06-04 M19 Heap Map Callback Protection Checkpoint

This checkpoint adds a targeted `managed_heap_map_callbacks` benchmark and
removes per-iteration protected-value `Vec<Value>` allocation from heap-mode
map `map_values()` and `filter()` callbacks. The callback dispatcher can now
protect an iterator of existing `Value` references, so partial mapped or
filtered map results are traced directly into the existing `HeapExecution`
protected-root buffer.

Commands:

```bash
cargo test -p vela_vm map_
cargo bench -p vela_vm --bench baseline
```

Default before/after for the targeted benchmark:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_map_callbacks | 161074557 | 145166342 | 8330170948568223460 | 8330170948568223460 |

Default guardrail rows from the same before/final-after runs:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 131643214 | 136827514 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 214569042 | 210889400 | 4123773336162002392 | 4123773336162002392 |

Checkpoint notes:

```text
Checksums stayed stable. The targeted benchmark isolates heap-mode map
callbacks that accumulate partial map results while invoking callbacks. The
generic callback helper now accepts borrowed protected values, preserving GC
root behavior without allocating a temporary protected-value vector each
iteration.
```

### 2026-06-04 M19 Call Default Allocation Checkpoint

This checkpoint removes a per-call allocation from VM function and closure
entry. `execute_body` now reads `CodeObject::param_defaults` directly and
treats missing default flags as `false` instead of cloning and resizing a
temporary defaults vector for every call. This especially affects callback-heavy
workloads because each collection callback enters a closure frame.

Commands:

```bash
cargo test -p vela_vm program_execution
cargo test -p vela_vm array_methods
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick after-run comparison against the latest documented callback quick
checkpoints:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 12428900 | 11750950 | 6661976061914330346 | 6661976061914330346 |
| managed_heap_callback_collections | 19242050 | 17631400 | 6661976061914330346 | 6661976061914330346 |

Default after-run comparison against the heap callback root-buffer checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 157089685 | 141971342 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 228158371 | 213228385 | 4123773336162002392 | 4123773336162002392 |

Checkpoint notes:

```text
Checksums stayed stable for the callback workload in both modes. The change
preserves default-parameter semantics, including missing param_defaults entries
being treated as no default, while avoiding one temporary Vec allocation on
each script function or closure call.
```

### 2026-06-04 M19 Array Higher-Order Receiver Checkpoint

This checkpoint adds no-heap receiver fast paths for array `map`, `filter`,
`find`, `any`, `all`, and `count`. When the receiver is already a
`Value::Array` and managed heap execution is not active, these methods now
iterate the receiver directly instead of cloning the full array through
`array_values` before invoking callbacks. Managed heap execution keeps the
existing materializing path so heap-root protection semantics stay unchanged.

Commands:

```bash
cargo test -p vela_vm array_methods::tests
cargo test -p vela_vm program_execution
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick after-run comparison against the call default allocation checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 11750950 | 11115800 | 6661976061914330346 | 6661976061914330346 |
| managed_heap_callback_collections | 17631400 | 17106100 | 6661976061914330346 | 6661976061914330346 |

Default after-run comparison against the call default allocation checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 141971342 | 138982100 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 213228385 | 212849257 | 4123773336162002392 | 4123773336162002392 |

Checkpoint notes:

```text
Checksums stayed stable for the callback workload in both modes. The targeted
change is the inline no-heap array callback path; managed-heap callback numbers
are retained as guardrails because their receiver materialization path is
unchanged.
```

### 2026-06-04 M19 Set Higher-Order Receiver Checkpoint

This checkpoint adds no-heap receiver fast paths for set `map`, `filter`,
`find`, `any`, `all`, and `count`. When the receiver is already a `Value::Set`
and managed heap execution is not active, these methods now iterate the
receiver directly instead of cloning the full set through `set_values` before
invoking callbacks. Managed heap execution keeps the existing materializing
path so heap-root protection semantics stay unchanged.

Commands:

```bash
cargo test -p vela_vm set_methods
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick after-run comparison against the array higher-order receiver checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 11115800 | 11152600 | 6661976061914330346 | 6661976061914330346 |
| managed_heap_callback_collections | 17106100 | 17546600 | 6661976061914330346 | 6661976061914330346 |

Default after-run comparison against the array higher-order receiver checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 138982100 | 134873314 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 212849257 | 218051128 | 4123773336162002392 | 4123773336162002392 |

Checkpoint notes:

```text
Checksums stayed stable for the callback workload in both modes. The targeted
change is the inline no-heap set callback path, where the default benchmark
shows the strongest signal. Managed-heap callback numbers are retained as
guardrails because their receiver materialization path is unchanged.
```

### 2026-06-04 M19 Managed Heap Array Sum Receiver Checkpoint

This checkpoint adds a targeted `managed_heap_array_sum` benchmark and removes
receiver materialization from plain array `sum()` calls. When `sum()` has no
callback, the VM now iterates inline `Value::Array` values directly and reads
managed-heap array numeric `HeapSlot` values directly instead of cloning the
full receiver through `array_values`. Callback-based `sum(|value| ...)` keeps
the existing materializing callback path so callback argument and heap-root
semantics stay unchanged.

Commands:

```bash
cargo test -p vela_vm array_sum
cargo test -p vela_vm managed_heap_execution_runs_array_group_by_method
cargo bench -p vela_vm --bench baseline
```

Default before/after for the targeted benchmark:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_array_sum | 12156971 | 10685242 | 3176850815018688896 | 3176850815018688896 |

Default guardrail rows from the same before/after runs:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 132143585 | 134236785 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 210769728 | 211583471 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_materialization | 1488357 | 1478057 | 1965056817950502848 | 1965056817950502848 |

Checkpoint notes:

```text
Checksums stayed stable. The targeted benchmark isolates repeated managed-heap
plain array sums, where avoiding receiver materialization removes a Vec<Value>
build per sum call. Callback-based sums still route through the callback path,
and non-targeted callback rows remain within normal benchmark noise.
```

### 2026-06-04 M19 Managed Heap Array Extrema Receiver Checkpoint

This checkpoint adds a targeted `managed_heap_array_extrema` benchmark and
removes receiver materialization from array `min()` and `max()` calls. Inline
`Value::Array` receivers now scan by reference, and managed-heap array receivers
scan `HeapSlot` values directly before wrapping the winning value in the
existing Option result shape. Mixed scalar domains and string comparison keep
the same error and comparison behavior as the previous materializing path.

Commands:

```bash
cargo test -p vela_vm array_extrema
cargo fmt --all -- --check
cargo bench -p vela_vm --bench baseline
```

Default before/after for the targeted benchmark:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_array_extrema | 85392400 | 55802914 | 323503183347530798 | 323503183347530798 |

Default guardrail rows from the same before/after runs:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 135736257 | 133785871 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 215251414 | 213857671 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_array_sum | 10760428 | 10736600 | 3176850815018688896 | 3176850815018688896 |
| managed_heap_materialization | 1514685 | 1457114 | 1965056817950502848 | 1965056817950502848 |

Checkpoint notes:

```text
Checksums stayed stable. The targeted benchmark isolates repeated managed-heap
array extrema calls, where direct HeapSlot scanning removes a Vec<Value> build
per min/max call. Result payloads still use the same Option wrapper and heap
reference materialization path as other heap-mode method returns.
```

### 2026-06-04 M19 Managed Heap Array Sort Receiver Checkpoint

This checkpoint adds a targeted `managed_heap_array_sort` benchmark and removes
receiver materialization from managed-heap array `sort()` calls. Heap-mode sort
now builds sort entries directly from array `HeapSlot` values, preserving the
existing stable tie-breaker and scalar-domain checks, then returns the sorted
values through the same array result path.

Commands:

```bash
cargo test -p vela_vm array_sort
cargo fmt --all -- --check
cargo bench -p vela_vm --bench baseline
```

Default before/after for the targeted benchmark:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_array_sort | 17870271 | 14702914 | 49647096020964123 | 49647096020964123 |

Default guardrail rows from the same before/after runs:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 133621785 | 136428014 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 214439871 | 216786200 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_array_extrema | 79037914 | 77153600 | 323503183347530798 | 323503183347530798 |
| managed_heap_materialization | 1453114 | 1439814 | 1965056817950502848 | 1965056817950502848 |

Checkpoint notes:

```text
Checksums stayed stable. The targeted benchmark isolates repeated managed-heap
numeric array sorts, where direct HeapSlot key construction avoids cloning the
full receiver through Vec<Value> before sorting. Callback-based sort_by keeps
its existing callback and root-protection path.
```

### 2026-06-04 M19 Managed Heap Array Slice Receiver Checkpoint

This checkpoint adds a targeted `managed_heap_array_slice` benchmark and
removes full receiver materialization from managed-heap array `slice()` calls.
Heap-mode slice now validates against the heap array length and materializes
only the requested `start..end` range, preserving the existing index error and
type-mismatch behavior for invalid ranges and receivers.

Commands:

```bash
cargo test -p vela_vm array_slice
cargo fmt --all -- --check
cargo bench -p vela_vm --bench baseline
```

Default before/after for the targeted benchmark:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_array_slice | 21918357 | 20340285 | 4447774498174460210 | 4447774498174460210 |

Default guardrail rows from the same before/after runs:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 132857457 | 137827014 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 212066571 | 224178685 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_array_sort | 14843442 | 17499171 | 49647096020964123 | 49647096020964123 |
| managed_heap_materialization | 1452571 | 1451714 | 1965056817950502848 | 1965056817950502848 |

Checkpoint notes:

```text
Checksums stayed stable. The target benchmark isolates repeated partial slices
of managed-heap arrays, where converting only the requested range avoids a
full Vec<Value> receiver build before copying the subrange. Several
non-targeted guardrails were noisy in the after run and are retained only as
checksum/behavior checks, not claimed as performance wins or regressions.
```

### 2026-06-04 M19 Managed Heap Array Join Receiver Checkpoint

This checkpoint adds a targeted `managed_heap_array_join` benchmark and removes
full receiver materialization from managed-heap array `join()` calls. Heap-mode
join now scans string heap slots directly and builds the output string with a
precomputed capacity instead of first cloning the receiver through
`Vec<Value>`.

Commands:

```bash
cargo test -p vela_vm array_join
cargo fmt --all -- --check
cargo bench -p vela_vm --bench baseline
```

Default before/after for the targeted benchmark:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_array_join | 32650571 | 26664214 | 11392497872150165547 | 11392497872150165547 |

Default guardrail rows from the same before/after runs:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 136272471 | 136369885 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 216481385 | 217583757 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_array_slice | 20331542 | 19804757 | 4447774498174460210 | 4447774498174460210 |
| managed_heap_materialization | 1547242 | 1476971 | 1965056817950502848 | 1965056817950502848 |

Checkpoint notes:

```text
Checksums stayed stable. The target benchmark isolates repeated managed-heap
string array joins, where direct heap-slot string reads avoid the temporary
receiver Vec<Value>. Non-targeted guardrails are kept as checksum and behavior
checks only.
```

### 2026-06-04 M19 Managed Heap Array Reverse Receiver Checkpoint

This checkpoint adds a targeted `managed_heap_array_reverse` benchmark and
removes full receiver materialization from managed-heap array `reverse()`
calls. Heap-mode reverse now walks array heap slots in reverse order and
materializes only the returned array, instead of first cloning the receiver
through `Vec<Value>` and then reversing that temporary vector.

Commands:

```bash
cargo test -p vela_vm array_reverse
cargo fmt --all -- --check
cargo bench -p vela_vm --bench baseline
```

Default before/after for the targeted benchmark:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_array_reverse | 41384914 | 40754542 | 6904157696146865977 | 6904157696146865977 |

Default guardrail rows from the same before/after runs:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| callback_collections | 131548857 | 133303685 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_callback_collections | 212675257 | 211428200 | 4123773336162002392 | 4123773336162002392 |
| managed_heap_array_slice | 19775042 | 19991771 | 4447774498174460210 | 4447774498174460210 |
| managed_heap_array_join | 26578500 | 26775885 | 11392497872150165547 | 11392497872150165547 |

Checkpoint notes:

```text
Checksums stayed stable. The target benchmark shows a small improvement on
this machine; the optimization is primarily a materialization cleanup that
removes the extra temporary reverse pass for managed-heap receivers.
Non-targeted guardrails are kept as checksum and behavior checks only.
```

### 2026-06-04 M19 Managed Heap Array Distinct Benchmark Coverage Checkpoint

This measurement checkpoint adds `managed_heap_array_distinct`, a managed-heap
benchmark covering `array.distinct()` over inline numeric slots, string heap
refs, and nested array heap refs. It gives the remaining transform-method heap
receiver materialization path a direct benchmark surface.

Commands:

```bash
cargo test -p vela_vm array_distinct
cargo bench -p vela_vm --bench baseline
```

New benchmark baseline:

| Benchmark | Mode | Default mean ns | Default checksum |
|---|---|---:|---:|
| managed_heap_array_distinct | managed_heap | 73399514 | 4824218642054093469 |

Checkpoint notes:

```text
Checksums stayed stable. A direct heap-slot distinct fast path was measured
but not accepted because the mixed benchmark regressed versus the existing
generic materialized equality path. Future distinct optimization needs either
cached materialized comparison values or a narrower benchmark-proven scalar
path that does not penalize heap-ref arrays.
```

### 2026-06-04 M19 Scalar Dispatch Mix Benchmark Coverage Checkpoint

This measurement checkpoint adds `scalar_dispatch_mix`, an inline benchmark
that exercises integer arithmetic, modulo, float arithmetic and comparisons,
boolean short-circuiting, string equality/inequality, branch control, and loop
exit behavior in one scalar-heavy workload. It complements
`scalar_branch_loop`, which remains focused on integer dispatch and branch
control.

Commands:

```bash
cargo test -p vela_vm --bench baseline --no-run
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

New benchmark baseline:

| Benchmark | Mode | Quick mean ns | Quick checksum | Default mean ns | Default checksum |
|---|---|---:|---:|---:|---:|
| scalar_dispatch_mix | inline | 1449200 | 15308784822820424249 | 18350600 | 18355421299335186739 |

Checkpoint notes:

```text
The scalar_dispatch_mix workload gives M19 a broader scalar dispatch surface
before additional interpreter work on mixed int/float/bool/string operations.
The benchmark is measurement-only; no VM runtime behavior changed.
```

### 2026-06-04 M19 Scalar Equality Fast Path Checkpoint

This checkpoint avoids materializing values for direct scalar equality and
inequality checks. `values_equal` now compares `null`, bool, int, float, and
string pairs directly before falling back to the existing materializing path
for heap refs and aggregates. This keeps aggregate and heap-reference equality
semantics unchanged while avoiding string clones in common scalar dispatch
paths.

Commands:

```bash
cargo test -p vela_vm execution_core
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick before/after from warmed runs in the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| scalar_branch_loop | 550750 | 526400 | 5382776514408301204 | 5382776514408301204 |
| scalar_dispatch_mix | 1468400 | 1245750 | 15308784822820424249 | 15308784822820424249 |

Default after-run comparison against the scalar dispatch mix coverage
checkpoint:

| Benchmark | Previous mean ns | After mean ns | Previous checksum | After checksum |
|---|---:|---:|---:|---:|
| scalar_branch_loop | 6963514 | 6615942 | 14794452088437409837 | 14794452088437409837 |
| scalar_dispatch_mix | 18350600 | 15448514 | 18355421299335186739 | 18355421299335186739 |

Checkpoint notes:

```text
Checksums stayed stable for both scalar workloads. The strongest signal is in
scalar_dispatch_mix, where string equality and inequality no longer clone both
sides through materialization. Aggregate equality, heap-ref equality, and
source-spanned fallback errors continue through the previous materializing path.
```

### 2026-06-04 M19 Truthy Bytecode Checkpoint

This checkpoint adds a `Truthy` bytecode instruction for boolean-result
coercion in logical `&&` and `||` chains. The compiler previously emitted two
`Not` instructions for this conversion; it now emits one `Truthy` instruction
that preserves the same dynamic truthiness semantics while reducing dispatch
work in scalar short-circuit paths.

Commands:

```bash
cargo test -p vela_bytecode logical
cargo test -p vela_vm execution_core
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench baseline
```

Quick/default after-run comparison against the scalar equality checkpoint:

| Benchmark | Previous quick mean ns | After quick mean ns | Previous default mean ns | After default mean ns | Checksum |
|---|---:|---:|---:|---:|---:|
| scalar_dispatch_mix | 1245750 | 1205550 | 15448514 | 15096228 | quick 15308784822820424249 / default 18355421299335186739 |

Checkpoint notes:

```text
Checksums stayed stable. The targeted win is in scalar_dispatch_mix, where
short-circuit boolean result coercion appears inside the hot loop. Non-targeted
benchmarks stayed within normal run-to-run noise and keep the same checksums.
The VM still charges one instruction per executed bytecode instruction, so the
optimization reduces both dispatch count and budget consumption for the same
source-level logical expression.
```

### 2026-06-04 M19 Option/Result Helper Tag Checkpoint

This checkpoint adds a `managed_heap_option_result_helpers` benchmark for
repeated heap-mode Option/Result helper-method chains. Option/Result method
dispatch now carries `Some`, `None`, `Ok`, and `Err` as a compact copyable tag
instead of cloning the enum variant name into a temporary `String` for every
helper call. Payload reads, callback calls, wrong-shape errors, and managed heap
materialization behavior still use the existing paths.

Commands:

```bash
cargo test -p vela_vm option_result
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_option_result_helpers | 52994750 | 52187100 | 1812806599834733941 | 1812806599834733941 |

Checkpoint notes:

```text
The focused helper benchmark kept the same checksum and improved modestly after
removing per-call variant-name allocation. Neighboring managed-heap callback and
array benchmarks stayed within normal quick-run noise. This is a narrow helper
dispatch cleanup, not a broader enum payload materialization change.
```

### 2026-06-04 M19 Native Call Argument Storage Checkpoint

This checkpoint gives bytecode native calls stack-backed argument storage for
zero-, one-, and two-argument calls instead of always materializing native call
arguments into a temporary `Vec<Value>`. Wider native calls keep the existing
vector-backed path, and native functions still receive the same `&[Value]`
interface.

Commands:

```bash
cargo test -p vela_vm managed_heap_execution_runs_option_result_helper_methods
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Checksum |
|---|---:|---:|---:|
| managed_heap_option_result_helpers | 55650150 | 51879550 | 1812806599834733941 |

Checkpoint notes:

```text
The focused Option/Result helper workload improved by about 6.8% because it
uses many small native/helper calls. Checksums stayed stable, and the slow path
for wider native calls remains unchanged.
```

### 2026-06-04 M19 Script Call Argument Storage Checkpoint

This checkpoint adds a focused `script_call_small_args` benchmark for repeated
one- and two-argument script function calls through a compiled `Program`.
Script function, closure, and method call argument packing now uses
stack-backed storage for one- and two-argument calls before falling back to the
existing `Vec<Value>` path for wider calls.

Commands:

```bash
cargo test -p vela_vm runs_compiled_script_function_calls
cargo test -p vela_vm runs_immediate_lambda_calls_and_block_returns
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Checksum |
|---|---:|---:|---:|
| script_call_small_args | 1982200 | 1696050 | 17951189677707400592 |

Checkpoint notes:

```text
The focused script-call workload improved by about 14.4% with the checksum
unchanged. The optimization keeps the same `&[Value]` call interface and does
not change call-depth budgeting, frame root collection, or hot-reload code
object ownership.
```

### 2026-06-04 M19 Managed Heap Host Conversion Benchmark Checkpoint

This checkpoint adds a focused `managed_heap_host_conversion` benchmark for
host execution with managed heap enabled. The workload writes map, record, and
enum aggregates through `PatchTx`, applies the patches to the mock host, and
verifies the final host aggregate shapes through the benchmark checksum.

Commands:

```bash
cargo test -p vela_vm managed_heap_host_execution
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick baseline from the same working session:

| Benchmark | Mean ns | Checksum |
|---|---:|---:|
| managed_heap_host_conversion | 2584850 | 2738613165024392619 |

Checkpoint notes:

```text
This gives M19 a measured host-managed-heap conversion surface separate from
the broader host_patch_tx row. A direct heap-slot-to-HostValue conversion path
was measured but not accepted because repeated quick runs did not show a
consistent win, so the runtime path stayed unchanged.
```

### 2026-06-04 M19 Managed Heap Set Lookup Checkpoint

This checkpoint adds a focused `managed_heap_set_lookup` benchmark for repeated
heap-mode `set.has()` calls over string and integer sets. The accepted runtime
change makes `set.has()` scan existing set storage directly instead of cloning
or materializing the full receiver before checking membership.

Commands:

```bash
cargo test -p vela_vm set_has
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Before checksum | After checksum |
|---|---:|---:|---:|---:|
| managed_heap_set_lookup | 7938100 | 6987650 | 17198566150566951166 | 17198566150566951166 |

Checkpoint notes:

```text
The focused lookup benchmark kept the same checksum and improved after avoiding
temporary receiver vectors and heap-reference wrapper values in set.has().
Other quick benchmark rows stayed within normal run-to-run noise.
```

### 2026-06-04 M19 Managed Heap Array Lookup Benchmark Checkpoint

This checkpoint adds a focused `managed_heap_array_lookup` benchmark for
repeated heap-mode `array.contains()` and `array.index_of()` calls over string
and integer arrays. It also adds a focused managed-heap scalar lookup test.

Commands:

```bash
cargo test -p vela_vm managed_heap_execution_runs_array_contains_method
cargo test -p vela_vm managed_heap_execution_runs_array_index_of_method
cargo test -p vela_vm managed_heap_execution_runs_array_scalar_lookup_methods
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick baseline from the same working session:

| Benchmark | Mean ns | Checksum |
|---|---:|---:|
| managed_heap_array_lookup | 9624100 | 17198566150566951166 |

Checkpoint notes:

```text
This gives M19 a focused array lookup timing surface separate from broader
array transform and callback rows. A direct heap-slot comparison helper was
measured but not accepted because quick runs regressed the focused benchmark,
so the runtime path stayed unchanged.
```

### 2026-06-04 M19 Managed Heap Map Lookup Benchmark Checkpoint

This checkpoint adds a focused `managed_heap_map_lookup` benchmark for repeated
heap-mode `map.has()`, `map.get()`, and `map.get_or()` calls over string and
integer map values. It also adds a focused managed-heap map lookup test.

Commands:

```bash
cargo test -p vela_vm managed_heap_execution_runs_map_lookup_methods
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick baseline from the same working session:

| Benchmark | Mean ns | Checksum |
|---|---:|---:|
| managed_heap_map_lookup | 10804250 | 13501942729849410472 |

Checkpoint notes:

```text
This gives M19 a focused map lookup timing surface separate from callback-heavy
map rows and broader collection workloads. No runtime optimization was accepted
in this checkpoint.
```

### 2026-06-04 M19 Managed Heap Map Lookup Key Borrow Checkpoint

This checkpoint keeps map lookup keys borrowed for immediate `map.has()`,
`map.get()`, and `map.get_or()` access instead of allocating an owned `String`
before probing `BTreeMap<String, _>` storage. Mutating map methods still own
keys when inserting into map storage.

Commands:

```bash
cargo test -p vela_vm managed_heap_execution_runs_map_lookup_methods
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Checksum |
|---|---:|---:|---:|
| managed_heap_map_lookup | 12152350 | 10382200 | 13501942729849410472 |

Checkpoint notes:

```text
The optimization removes repeated key-string allocation from read-only map
lookups while preserving the existing string key type checks and heap string
reads. The focused quick benchmark improved by about 14.6% with the checksum
unchanged.
```

### 2026-06-04 M19 String Len ASCII Fast Path Checkpoint

This checkpoint makes string `.len()` count ASCII strings with byte length
before falling back to Unicode scalar counting for non-ASCII strings. The
runtime behavior remains character-count based for script-visible semantics,
including managed-heap strings.

Commands:

```bash
cargo test -p vela_vm string_len_counts_unicode_characters
cargo test -p vela_vm string_utility_methods
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Checksum |
|---|---:|---:|---:|
| callback_collections | 11433000 | 11052650 | 6661976061914330346 |
| managed_heap_map_callbacks | 13549700 | 12286450 | 2601892725534891372 |
| managed_heap_option_result_helpers | 55354550 | 52248650 | 1812806599834733941 |

Checkpoint notes:

```text
ASCII map keys, labels, and helper strings are common in callback-heavy
gameplay code. The fast path avoids repeated UTF-8 decoding for those strings
while preserving Unicode character counts through the fallback path and focused
non-heap/managed-heap regression tests.
```

### 2026-06-04 M19 Negated Equality Peephole Checkpoint

This checkpoint lowers `!(lhs == rhs)` and `!(lhs != rhs)` directly to the
inverse equality bytecode instead of emitting an equality instruction followed
by `Not`. Ordering comparisons are intentionally not inverted here because
`!(a < b)` is not equivalent to `a >= b` for NaN float values.

Commands:

```bash
cargo test -p vela_bytecode compiler_inverts_negated_equality_without_not_instruction
cargo test -p vela_vm runs_compiled_scalar_equality_source
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick before/after from the same working session:

| Benchmark | Before mean ns | After mean ns | Checksum |
|---|---:|---:|---:|
| scalar_dispatch_mix | 1205750 | 1169850 | 15308784822820424249 |

Checkpoint notes:

```text
The scalar-dispatch benchmark includes a hot `!(label != "tick")` branch. The
peephole removes one dispatch from that expression while preserving dynamic
equality semantics and source-spanned slow-path errors through the existing
equality bytecode.
```

### 2026-06-04 M19 Range Iteration Benchmark Checkpoint

This checkpoint adds a focused `range_iteration` benchmark for nested exclusive
range loops plus an inclusive range loop. It gives M19 for-in loop and iterator
state work a direct timing surface separate from the broader scalar branch and
dispatch rows.

Commands:

```bash
cargo test -p vela_vm runs_compiled_range_for_in_source
cargo bench -p vela_vm --bench baseline -- --quick
```

Quick baseline from the same working session:

| Benchmark | Mean ns | Checksum |
|---|---:|---:|
| range_iteration | 1339750 | 11386712117419000375 |

Checkpoint notes:

```text
This is benchmark coverage only. A direct in-place iterator mutation experiment
was measured in the same session but was not accepted because it regressed the
scalar range-loop rows, so the runtime path stayed unchanged.
```

## Targets

The post-MVP non-JIT target is:

```text
optimized bytecode interpreter performance comparable to Lua 5.x on
representative gameplay workloads
```

This target is workload-based, not a promise that every scalar microbenchmark
matches Lua. Host integration, PatchTx safety, reflection policy, and hot reload
checks are part of Vela's runtime model and must remain enabled for gameplay
benchmarks.

Reference tiers:

| Tier | Purpose |
|---|---|
| Vela baseline | Release-mode behavior before a given optimization. |
| Lua 5.x | Primary non-JIT comparison target for post-MVP interpreter work. |
| LuaJIT / Node.js | Upper-reference points for hot scalar loops and future JIT decisions. |
| Rhai | Rust-embedded dynamic scripting reference point. |

## Benchmark Groups

Official benchmarks should use the measurement rules above and should never
mix compile/load time into repeated function execution results unless the case
is explicitly labeled as a cold-start or reload benchmark.

Required groups:

```text
scalar arithmetic and branch loops
script function calls and callbacks
array, map, set, and string stdlib operations
record and enum field access
Option and Result helper chains
managed heap allocation and materialization
host field reads, writes, RMW patches, and host method calls
reflection reads, writes, and calls
hot reload compile/update/apply workflow
GC pacing and pause-budget scenarios
gameplay workflows from examples/game_server_demo
```

Every benchmark should report:

```text
runtime options
Rust profile and target triple
Vela commit or ProgramVersion build identity
warmup, iteration, repeat, and input-size parameters
min, mean, median, p95, and checksum
whether managed heap, debugger hooks, caches, or JIT are enabled
external runtime versions when comparing other languages
```

## Optimization Order

Optimization should follow the roadmap in [goal.md](goal.md) and the contract
in [architecture.md](architecture.md):

1. Establish M18 measurement baselines.
2. Optimize the M19 interpreter and managed heap path without changing
   semantics.
3. Add M20 inline caches and specialization with guarded slow-path fallback.
4. Add M21 debugger runtime and DAP contracts before optimized backends rely on
   frame metadata.
5. Implement M22 Cranelift JIT after non-JIT targets, inline caches, debugger
   contracts, and conformance are stable.
6. Harden M23 release targets and regression thresholds.

Optimized paths must never bypass:

```text
ExecutionBudget
memory budget and GC roots
debugger breakpoints, stepping, frame maps, and safe suspension points when enabled
PatchTx and ScriptStateAdapter
permissions and reflection policy
hot reload ProgramVersion ownership
source-spanned diagnostics where errors can still occur
```

JIT benchmark reports must separate interpreter-only, cache-enabled, and
JIT-enabled runs. Cranelift optimization work must not trade away breakpoint
accuracy, single-step behavior, stack/frame inspection, GC root reporting,
budget checks, PatchTx routing, or hot-reload invalidation.
