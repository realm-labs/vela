# Performance

This document tracks how Vela performance should be measured and optimized.
It is not a substitute for conformance tests: correctness, host-boundary safety,
hot reload semantics, budgets, and diagnostics remain required for every fast
path.

## Current Position

Vela already runs compiled bytecode. The temporary local comparison harness used
during early exploration loaded or compiled scripts ahead of time, warmed them
up, then repeatedly called the same function. That means the large gap to
LuaJIT and Node.js in scalar microbenchmarks is not primarily because Vela was
interpreting source text directly.

The current known cost centers are:

```text
VM instruction dispatch
managed heap allocation and result materialization
dynamic Value operations
dynamic stdlib method dispatch
record, Option, and Result helper paths
string allocation and copying
missing inline caches, specialization, and JIT
```

`PatchTx` is not expected to be the dominant cost in pure script
microbenchmarks that do not touch host state. It matters for host-heavy gameplay
benchmarks and should be measured separately.

Temporary experiments may live under ignored paths such as
`target/perf_compare_demo/`. Official benchmark sources, baselines, and reports
should live in tracked benchmark or fixture locations once their shape is
stable.

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
| Vela baseline | Current release-mode behavior before an optimization. |
| Lua 5.x | Primary non-JIT comparison target for post-MVP interpreter work. |
| LuaJIT / Node.js | Upper-reference points for hot scalar loops and future JIT decisions. |
| Rhai | Rust-embedded dynamic scripting reference point. |

## Benchmark Groups

Official benchmarks should separate compile/load time from repeated function
execution. Function-call benchmarks should load scripts, perform warmup calls,
then measure repeated calls to the same already-loaded function.

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
whether managed heap, caches, or JIT are enabled
external runtime versions when comparing other languages
```

## Optimization Order

Optimization should follow the roadmap in [goal.md](goal.md) and the contract
in [architecture.md](architecture.md):

1. Establish M18 measurement baselines.
2. Optimize the M19 interpreter and managed heap path without changing
   semantics.
3. Add M20 inline caches and specialization with guarded slow-path fallback.
4. Evaluate M21 optional Cranelift JIT only after non-JIT targets and
   conformance are stable.
5. Harden M22 release targets and regression thresholds.

Optimized paths must never bypass:

```text
ExecutionBudget
memory budget and GC roots
PatchTx and ScriptStateAdapter
permissions and reflection policy
hot reload ProgramVersion ownership
source-spanned diagnostics where errors can still occur
```
