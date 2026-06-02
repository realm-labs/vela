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
