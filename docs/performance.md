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
callbacks, direct closure calls, and higher-order collection methods
cache-enabled stdlib, script/native call, method-dispatch, callback collection, and host-boundary rows with warmed inline caches and bytecode profile counters
record and enum construction and field access
managed heap allocation and materialization
host field reads, nested path reads/writes, RMW mutations, dynamic key access, and method calls
reflection reads, writes, and calls
hot reload compile/apply/reject workflow
GC pacing and pause-budget scenarios
domain-demo workflows from examples/src/bin game-server examples
external_compare against available Lua 5.x, LuaJIT, Node.js, and Rhai
```

Every durable benchmark report should include:

```text
commit, rustc, cargo, target, profile
runtime options: heap, cache, debugger, JIT
warmup, repeats, iterations, and input size
min, mean, median, p95, checksum
cache_sets and profile_hits when the harness emits cache-enabled rows
external runtime versions when used
```

## Current Baseline

Latest M19.5 prep checkpoint:

```text
commit=7097d6b6
rustc=1.96.0 (ac68faa20 2026-05-25)
cargo=1.96.0 (30a34c682 2026-05-25)
target=macos/aarch64
profile=release
runtime=linked interpreter, cache-disabled baseline
warmup=10, repeats=7, iterations=100
```

Representative default `baseline` means:

| Benchmark | Mode | Mean ns | P95 ns |
|---|---|---:|---:|
| scalar_branch_loop | inline | 4468880 | 5594041 |
| scalar_dispatch_mix | inline | 6217892 | 6281917 |
| script_call_small_args | script_program | 8731958 | 8955292 |
| native_call_wide_args | inline | 5194095 | 5246166 |
| stdlib_collections | inline | 20019208 | 20191125 |
| callback_collections | inline | 1340609249 | 1354016666 |
| direct_closure_calls | inline | 12818005 | 13057083 |
| managed_heap_callback_collections | managed_heap | 1339119952 | 1341697958 |
| managed_heap_direct_closure_calls | managed_heap | 12785095 | 12963500 |
| managed_heap_option_result_helpers | managed_heap | 8737640660 | 8775230083 |
| host_access | host_access | 209940 | 212459 |
| host_field_read_write | host_access | 1102642 | 1139542 |
| host_nested_read_write | host_access | 1309750 | 1376375 |
| host_dynamic_key_access | host_access | 2792363 | 2845875 |
| managed_heap_host_read_conversion | host_managed_heap_read_conversion | 2353339 | 2464083 |
| gameplay_monster_kill | gameplay_host | 872970 | 942958 |
| managed_heap_record_quads | managed_heap | 28406773 | 41063791 |
| gc_pacing | gc_pacing | 24438286 | 24635792 |

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
- host field/path reads, writes, and RMW operations now have `HostTargetPlan`
  operands and resolved access boundaries ready for M20 caches
- method and stdlib dispatch need ID or resolved-target lookup
- native/stdlib calls need lower materialization through borrowed Value views
- callback invocation and hot closure calls need lower root/materialization cost
- hot bytecode offset profiling needs versioned ownership before specialization
- cache invalidation must stay tied to hot reload and schema ABI changes

M19.5 reports interpreter-only before/after rows for each prep family. The
baseline harness now splits callback rows into collection callbacks and direct
closure calls with default baseline data, includes cache-enabled stdlib
collection, script-call, native-call, script record-field, method-dispatch,
callback collection, and direct-closure rows with warmed inline caches and
bytecode profile counters, and splits host-boundary rows into field
read/write, nested path read/write, RMW mutation, dynamic key access, and host
method calls. M20 reports must separate interpreter-only and cache-enabled
results.

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
