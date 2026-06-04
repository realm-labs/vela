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
stdlib_collections          array, map, set, Option, and stdlib method dispatch
host_patch_tx               HostRef reads, nested HostPath writes, PatchTx overlay
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
