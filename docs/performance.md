# Performance

This document is the current performance contract. Detailed historical M18/M19
benchmark notes were archived to
[archive/performance-full-2026-06-06.md](archive/performance-full-2026-06-06.md).

## Rules

- Correctness, budgets, GC roots, HostAccess routing, reflection policy, hot
  reload ownership, and source-spanned diagnostics take priority over speed.
- Measure loaded repeated execution separately from parsing, HIR, compilation,
  bytecode loading, hot reload apply, and cold start.
- Pure VM, managed-heap, host-boundary, reflection, hot-reload, and domain-demo
  workloads must be reported separately.
- Accepted optimizations need focused before/after evidence and stable
  checksums. Rejected candidates belong in commit/PR notes unless they change
  milestone direction.
- Do not append routine benchmark logs here. Keep only current baselines,
  milestone exit summaries, target thresholds, and durable measurement rules.

## Harnesses

```bash
cargo bench -p vela_vm --bench baseline
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
cargo bench -p vela_engine --bench hot_reload -- --quick
```

Tracked workload groups:

```text
scalar/range dispatch
script/native function calls
array, map, set, string, Option, and Result stdlib methods
callbacks and higher-order collection methods
record and enum construction and field access
managed heap allocation and materialization
host field reads, writes, RMW mutations, and method calls
reflection reads, writes, and calls
hot reload compile/apply/reject workflow
GC pacing and pause-budget scenarios
domain-demo workflows from examples/game_server_demo
external_compare against available Lua 5.x, LuaJIT, Node.js, and Rhai
```

Every durable benchmark report should include:

```text
commit, rustc, cargo, target, profile
runtime options: heap, cache, debugger, JIT
warmup, repeats, iterations, and input size
min, mean, median, p95, checksum
external runtime versions when used
```

## Current Baseline

Latest M19 exit checkpoint:

```text
commit=10f03bf
rustc=1.96.0 (ac68faa20 2026-05-25)
cargo=1.96.0 (30a34c682 2026-05-25)
target=macos/aarch64
profile=release
```

Representative default `baseline` means:

| Benchmark | Mode | Mean ns | P95 ns |
|---|---|---:|---:|
| scalar_branch_loop | inline | 3490422 | 3898208 |
| scalar_dispatch_mix | inline | 7359768 | 8184292 |
| script_call_small_args | script_program | 9726273 | 9833666 |
| stdlib_collections | inline | 1042910 | 1217416 |
| callback_collections | inline | 78264411 | 81619084 |
| managed_heap_option_result_helpers | managed_heap | 233396131 | 235194542 |
| host_access | host_access | 350410 | 396791 |
| managed_heap_host_read_conversion | host_managed_heap_read_conversion | 13977702 | 14445541 |
| gameplay_monster_kill | gameplay_host | 943922 | 981417 |
| gc_pacing | gc_pacing | 29055851 | 29898250 |

External quick comparison per-iteration means:

| Runtime | scalar_branch_loop | function_calls | array_scan | string_methods |
|---|---:|---:|---:|---:|
| vela | 34439 | 97006 | 264744 | 183821 |
| lua5 | 11977 | 18387 | 84890 | 116015 |
| luajit | 8059 | 9394 | 13160 | 16549 |
| node | 77891 | 80074 | 83395 | 87523 |

## Current Conclusions

M19 is complete enough for M19.5. The interpreter/heap phase delivered measured
improvements in GC pacing, direct heap aggregate construction, argument
materialization/storage, borrowed receiver views, collection/string/Option/
Result helpers, scalar equality/constant loads, peephole lowering, range-loop
lowering, small record/enum fields, and short array construction.

The Lua 5.x target is not met across all microbenchmarks. Remaining gaps are
cache-shaped, but M20 should wait until the hot operands are cache-ready:

- script record field slot reads and writes need shape/slot-ready operands
- host field/path reads, writes, and RMW operations need reusable path keys
- method and stdlib dispatch need ID or resolved-target lookup
- native/stdlib calls need lower materialization through borrowed Value views
- callback invocation and hot closure calls need lower root/materialization cost
- hot bytecode offset profiling needs versioned ownership before specialization
- cache invalidation must stay tied to hot reload and schema ABI changes

M19.5 reports interpreter-only before/after rows for each prep family. M20
reports must separate interpreter-only and cache-enabled results.

## Targets

Primary post-MVP target:

```text
optimized non-JIT bytecode interpreter performance comparable to Lua 5.x on
representative host-boundary workloads
```

Reference tiers:

| Tier | Purpose |
|---|---|
| Vela interpreter | Correctness-preserving baseline before caches. |
| Vela prep-enabled | M19.5 ID/slot/target/path-key prep before caches. |
| Vela cache-enabled | M20 inline-cache and specialization target. |
| Lua 5.x | Primary non-JIT comparison target. |
| LuaJIT / Node.js | Upper-reference points for future JIT decisions. |
| Rhai | Rust-embedded scripting reference point. |

## Update Policy

Update this file only when one of these changes:

- current baseline checkpoint
- accepted target threshold
- benchmark harness or workload group
- milestone exit conclusion
- durable rule for measuring or reporting performance

Long before/after tables, failed micro-candidates, and routine benchmark output
belong in commit messages, PR notes, or `docs/archive/` if they need to be
preserved.
