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

`baseline` accepts optional workload-name substring filters after `--`, for
example `cargo bench -p vela_vm --bench baseline -- --quick host_field`.
`external_compare` accepts the same kind of workload-name substring filters,
for example `cargo bench -p vela_vm --bench external_compare -- --quick string`.
It also accepts profiling/regression parameters:

```text
--runtime <substring-or-comma-list>
--iterations <count>
--repeats <count>
--warmup <count>
```

For example, a Vela-only scalar run suitable for local profiling can be
captured with:

```bash
cargo bench -p vela_vm --bench external_compare -- \
  --runtime vela --iterations 500000 --repeats 1 --warmup 1 scalar
```

It is a mixed-mode language comparison harness:

```text
runtime=vela    mode=internal_hot_loop
runtime=lua54   mode=embedded_hot_loop
runtime=rhai    mode=embedded_hot_loop
runtime=node    mode=process_hot_loop
runtime=python3 mode=process_hot_loop
```

Vela compiles and links with the standard registry outside the timed section.
Lua 5.4 runs through vendored `mlua`, and Rhai runs through the `rhai` crate,
also loading scripts outside the timed section. Node.js and Python 3 remain
optional process-backed rows; missing commands are reported as `status=missing`
instead of failing the benchmark. Each workload entrypoint receives
`iterations` and performs its hot loop inside the runtime. Compare rows within
the same `mode` first; `embedded_hot_loop` is closer to Vela's in-process hot
path than `process_hot_loop`, while process rows still include process startup
and script loading costs that are only amortized by the iteration count.
When a cache family has an explicit non-cache `_hot_offsets` row, `cache_delta`
pairs the cache-enabled row against that base to isolate cache overhead from
bytecode-profiler overhead. Host-boundary `_hot_offsets` rows run in
`host_access_profile_only` mode so interpreter-only host measurements still
carry bytecode profile counters without enabling inline caches.
Method-dispatch aggregate rows use the `method_dispatch` prefix so filtered
quick runs include interpreter, profile-only, and cache-enabled rows together.
Cache-enabled benchmark rows use the same `Cell`-backed storage shape as the
engine runtime for copyable global-read, host-access, record-field, and
resolved method-dispatch entries. Dynamic method-dispatch and native-call
entries use cloneable target storage.

Tracked workload groups:

```text
scalar/range dispatch
script/native function calls
array, map, set, string, Option, and Result stdlib methods
callbacks, direct closure calls, and higher-order collection methods
cache-enabled stdlib/native call, method-dispatch aggregate/detail, script record-field aggregate/detail, range-method detail, collection lookup/view/aggregation/combination/mutation/materialization, string/bytes method and string-transform detail, Option/Result helper, callback collection/detail, host-boundary aggregate/detail, plus linked script-call and direct-closure profile rows with bytecode profile counters
dynamic method dispatch rows for monomorphic string receivers, monomorphic script receivers, polymorphic standard/script receivers, and deliberate guard-miss pressure, plus the existing static CallMethodId method-dispatch rows for comparison
record and enum construction and field access
managed heap allocation and materialization
declared host globals, host field reads, nested path reads/writes, RMW mutations, dynamic key access, and method calls
reflection reads, writes, and calls
hot reload compile/apply/reject workflow
GC pacing and pause-budget scenarios
domain-demo workflows from examples/src/bin game-server examples
external_compare mixed-mode pure-language rows for scalar branch loops, range iteration, function calls, array scanning, map lookup/update, set lookup/mutation, string methods, closure/callback-style calls, recursive countdown, nested collection allocation, object field/method access, string split/join construction, float math loops, and array transform/sort across Vela, embedded Lua 5.4, embedded Rhai, optional process-backed Node.js, and optional process-backed Python 3
```

Every durable benchmark report should include:

```text
commit, rustc, cargo, target, profile
runtime options: heap, cache, debugger, JIT
warmup, repeats, iterations, and input size
min, mean, median, p95, checksum
measurement_kind, cache_sets, cache_hits, cache-family set/hit counters,
including `cache_dynamic_method_sets` and `cache_dynamic_method_hits`,
profile_hits, and cache_delta rows with mode, base_mode, checksum_match,
delta_kind, delta_band, base_profile_hits, and profile_hits_match when the
harness emits paired cache-enabled rows; measurement_kind/delta_kind separate
true cache-hit rows from profile-only hot-offset rows, delta_band uses a 1%
mean-ratio tolerance to classify faster/slower/flat pairs, and native-call
cache-enabled rows count resolved target-cache population; measurement_summary
rows should count interpreter, profile-only, cache, and cache-no-activity rows
for the run, and cache_delta_summary rows should count paired-row outcomes and
mismatches, including kind-specific faster/slower/flat counts so true cache-hit
deltas can be separated from profile-only hot-offset deltas
external runtime versions when used
external comparison mode (`internal_hot_loop`, `embedded_hot_loop`, or
`process_hot_loop`) when used
```

## Perf Optimization Loop

Performance work must follow a measurement-first loop:

```text
capture stable baseline -> profile hotspot -> implement one focused change ->
capture candidate -> compare against baseline -> commit only with evidence
```

Use `tools/perf/capture_external_compare.py` to retain raw key=value output
with commit, branch, rustc, cargo, command, and timestamp metadata:

```bash
tools/perf/capture_external_compare.py \
  --name scalar-vela-before \
  --baseline external_compare_scalar_vela_macos_aarch64 \
  -- \
  --runtime vela --iterations 500000 --repeats 5 --warmup 2 scalar
```

Routine local captures go under `perf-results/`, which is git-ignored.
Checked-in guardrail captures belong under `perf-baselines/` and should be
small, intentional, and tied to a documented regression check.

After a candidate change, capture another run and compare:

```bash
tools/perf/capture_external_compare.py \
  --name scalar-vela-after \
  -- \
  --runtime vela --iterations 500000 --repeats 5 --warmup 2 scalar

tools/perf/compare_keyvalue_bench.py \
  --baseline perf-baselines/external_compare_scalar_vela_macos_aarch64.txt \
  --candidate perf-results/external_compare/<candidate-file>.txt \
  --runtime vela \
  --bench scalar_branch_loop \
  --max-regression-percent 5
```

The comparison script checks matching `runtime/mode/bench` rows, verifies
checksums by default, and fails when the candidate `per_iter_mean_ns` exceeds
the baseline by more than the configured percentage.

Use profiling before changing interpreter internals. On macOS, the repo helper
uses `sample` against a long-running Vela-only `external_compare` process:

```bash
tools/perf/profile_external_compare.sh \
  --runtime vela --iterations 500000 --repeats 1 --warmup 1 scalar
```

The helper writes text profiles under `perf-results/profiles/`. If a GUI call
tree is needed, use Instruments or `xctrace` with the same long-running
command. Do not accept broad interpreter changes from benchmark deltas alone;
the profile should identify the dominant stack first.

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

The last recorded external quick comparison below predates the mixed-mode
`external_compare` harness. Keep it as historical context until a full
non-quick mixed-mode capture replaces it.

External quick comparison per-iteration means:

| Runtime | scalar_branch_loop | function_calls | array_scan | string_methods |
|---|---:|---:|---:|---:|
| vela | 34439 | 97006 | 264744 | 183821 |
| lua5 | 11977 | 18387 | 84890 | 116015 |
| luajit | 8059 | 9394 | 13160 | 16549 |
| node | 77891 | 80074 | 83395 | 87523 |

### Focused M20 Guardrail: Collection Mutation Budgets

The collection-growth budget change was accepted with a focused mutation
benchmark checkpoint against its parent commit:

```text
before=e4d939b6
after=f17ede9f
rustc=1.96.0 (ac68faa20 2026-05-25)
cargo=1.96.0 (30a34c682 2026-05-25)
target=macos/aarch64
profile=release
command=cargo bench -p vela_vm --bench baseline -- mutation
warmup=10, repeats=7, iterations=100
```

Checksums matched for every compared row. The accepted cost is concentrated in
array/map mutation paths, where in-place heap growth now performs collection
limit checks, memory-budget charging, allocator reserve checks, and heap-size
accounting. Set interpreter rows were flat to slightly faster in this sample.

| Benchmark | Mode | Mean Before ns | Mean After ns | Delta | P95 After ns |
|---|---|---:|---:|---:|---:|
| managed_heap_set_mutation | managed_heap | 29194738 | 28914904 | -0.96% | 29408250 |
| set_mutation | inline | 29191339 | 28827351 | -1.25% | 29395458 |
| set_mutation_cache_hot_offsets | cache_enabled | 28876434 | 32845779 | +13.75% | 37833416 |
| managed_heap_array_mutation | managed_heap | 87970535 | 90379106 | +2.74% | 91181291 |
| array_mutation | inline | 87498553 | 89817803 | +2.65% | 90207334 |
| array_mutation_cache_hot_offsets | cache_enabled | 87810910 | 89517976 | +1.94% | 89907167 |
| managed_heap_map_mutation | managed_heap | 52848244 | 54131410 | +2.43% | 54425792 |
| map_mutation | inline | 52686250 | 54207744 | +2.89% | 54423333 |
| map_mutation_cache_hot_offsets | cache_enabled | 52352470 | 54491898 | +4.09% | 56054042 |

The `set_mutation_cache_hot_offsets` row should be rechecked before treating it
as a cache-regression target: the paired profile-only row also showed a large
tail sample in this capture. Future budget or collection-mutation changes must
rerun the same filtered benchmark before landing; a regression above roughly
5% on array/map interpreter or cache-enabled rows needs either an explicit
safety rationale or a follow-up performance task.

## Current Conclusions

M19 is complete enough for M19.5. The interpreter/heap phase delivered measured
improvements in GC pacing, direct heap aggregate construction, argument
materialization/storage, borrowed receiver views, collection/string/Option/
Result helpers, scalar equality/constant loads, peephole lowering, range-loop
lowering, small record/enum fields, and short array construction.

The Lua 5.x target is not met across all microbenchmarks. Remaining gaps are
cache-shaped, and M20 measurement should stay tied to cache-ready operands:

- script record field slot reads and writes now have shape/slot-ready operands
  and detail benchmark rows
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
collection, script-call, native-call, script record-field aggregate/detail,
method-dispatch aggregate/detail, range-method detail,
collection lookup/view/aggregation/combination/mutation/materialization,
string/bytes method plus string-transform detail, Option/Result helper, callback
collection/detail, direct-closure, and host-boundary aggregate/detail rows with
warmed inline caches and bytecode profile counters. The record-field detail
rows cover triplet, quad, quint, and sextet shapes; the host-boundary detail
rows cover declared global read/write, field read/write, nested path read/write,
RMW mutation, dynamic key access, and host method calls; the method-dispatch detail rows cover script
inherent and trait/default method calls. Direct script-call and direct-closure
rows are profile-only when their linked operands already avoid runtime lookup
and no inline-cache family exists. Baseline `bench=` rows now emit
`measurement_kind` and paired rows emit `delta_kind`, so M20 reports must
separate interpreter-only, profile-only, and cache-enabled results by those
fields instead of row names alone.

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
