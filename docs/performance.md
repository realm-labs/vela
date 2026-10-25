# Performance

This is the live performance contract for the reduced synchronous interpreter.
A–E removal implementation and software gates are complete; see
[current progress](progress.md) and the
[acceptance report](archive/runtime-simplification-acceptance.md). Benchmark
compilation passed, not performance acceptance; strict/native Input/Render
coverage remains incomplete. Historical timings are not fresh post-removal
measurements.

## Rules

- Correctness, finite budgets, roots, native permissions, reflection policy,
  source-spanned diagnostics and exact-generation reload ownership precede speed.
- Separate loaded repeated execution from parsing, HIR/MIR compilation, linking,
  artifact loading, Runtime construction and reload compile/apply/reject work.
- Report scalar VM, script heap/collections, owned native boundary, reflection,
  reload, shared-generation memory and editor-analysis rows separately.
- Require matching checksums and focused before/after evidence. Retain root,
  error/effect ordering and logical budget accounting on optimized paths.
- No JIT target, borrowed-host/path target, Service dispatch baseline, async
  suspension or detached-task throughput requirement remains.

## Harnesses

These targets are declared by the current Cargo manifests:

```bash
cargo bench -p vela_vm --bench baseline -- --quick
cargo bench -p vela_vm --bench external_compare -- --quick
cargo bench -p vela_vm --bench verified_mir_compile
cargo bench -p vela_vm --bench scalar_block_allocations
cargo bench -p vela_engine --bench hot_reload -- --quick
cargo bench -p vela_engine --bench interop -- --quick
cargo bench -p vela_engine --bench generation_memory -- top-level
cargo bench -p vela_engine --bench generation_memory -- lambda
cargo bench -p vela_engine --bench generation_memory -- shared-runtime
cargo bench -p vela_engine --bench generation_memory -- retained-generations
cargo bench -p vela_engine --bench generation_memory -- call-heavy
cargo bench -p vela_engine --bench actor_memory -- memory
cargo bench -p vela_engine --bench actor_memory -- allocations
cargo bench -p vela_engine --bench actor_concurrency -- --quick
cargo bench -p vela_language_service --bench incremental_latency
```

`baseline` covers scalar/range execution, ordinary calls/closures, script
records/enums, VM state, collection/iterator/string/bytes operations, guards,
reflection and interpreter cache/profile variants. Workload-name substring
filters select rows after `--`; for example:

```bash
cargo bench -p vela_vm --bench baseline -- --quick scalar
cargo bench -p vela_vm --bench external_compare -- \
  --runtime vela --iterations 500000 --repeats 5 --warmup 2 scalar
```

`external_compare` separates Vela internal hot loops, embedded Lua 5.4/Rhai and
optional process-backed Node.js/Python rows. Compare within the same measurement
mode first; process rows include amortized startup/loading. Missing optional
commands are not proof of comparative performance.

`verified_mir_compile` measures compiler/verification overhead separately from
execution. `scalar_block_allocations` checks that repeated scalar block/loop
entries do not add incremental allocations after warmup.

`interop` now measures direct Rust scalar calls, Vela-to-Rust scalar calls,
schema-backed generated Rust-to-Vela roots and existing synchronous
Vela/Rust/Vela nested calls. Compilation and Runtime construction are outside
the call loops. `--quick` uses 1,000 iterations, default 10,000, and `--stable`
100,000 with three samples. It has no borrowed receiver or object-path rows.

`generation_memory` separates top-level/lambda artifacts, sharing across
Runtimes, retained generations and call-heavy shapes. `actor_memory` and
`actor_concurrency` measure independent host-owned synchronous Runtimes sharing
immutable code, cold/hot calls and dynamic-cache contention. Their names do not
introduce script actors, tasks or scheduling. Keep allocation calibration out
of concurrent throughput runs.

`incremental_latency` measures editor buffer changes, database updates,
diagnostics, completion, hover and snapshot latency. Shape is selected by
`VELA_LS_BENCH_MODULES`, `VELA_LS_BENCH_FUNCTIONS`,
`VELA_LS_BENCH_ITERATIONS` and `VELA_LS_BENCH_WARMUP`. This is native tooling
work, not script execution.

## Perf Optimization Loop

1. Freeze a retained workload and checksum; record the parent revision and machine.
2. Separate compilation, ordinary interpreter, profile-only and cache-enabled rows.
3. Identify the owning cost, make a focused change and run semantic/rejection tests.
4. Compare repeated measurements on the same quiet machine and inputs.
5. Report non-target guardrails, memory/allocation changes and remaining gaps.

Cache-hit rows must actually exercise cache activity. Where the harness emits
paired `cache_delta` rows, retain `measurement_kind`, `delta_kind`, hit/set
counts, checksum and profile-hit matches; do not infer a cache win from a name.
Statically resolved calls/slots need no mutable cache just to appear in a report.

## Current Baseline

The current executable baseline is the post-D retained harness set above, not
the old host-boundary suite. A fresh consolidated post-removal timing baseline
is not claimed by the [acceptance report](archive/runtime-simplification-acceptance.md):
its benchmark compilation gate is build proof, not timing/performance acceptance.
[Current progress](progress.md) records remaining measured-performance work and
separate strict/native Input/Render gaps.

Historical scalar interpreter results remain useful context: the final
verified-MIR interpreter report measured 37.634% improvement for
`scalar_branch_loop`, 75.796% for `range_iteration` and 32.215% geometric-mean
improvement over its frozen five-row baseline. These are measurements against
that report's revision/machine/workloads, not claims about today's full suite.
See the [historical final report](archive/verified-mir-interpreter-final-acceptance-2026-08-28.md).

Older broad measurements live in
[performance history](archive/performance-full-2026-06-06.md) and
[shared-generation acceptance history](archive/actor-runtime-cache-acceptance-2026-07-18.md).
Removed host/Service/async rows in those reports are history only.

## Targets

Optimize representative retained interpreter workloads, using embedded Lua 5.x
and Rhai as comparison points and optional process runtimes as explicitly labeled
context. There is no blanket claim of Lua parity. Preserve interpreter-used MIR,
root/budget analyses, linked operands, caches, superinstructions, scalar blocks,
range-loop physical plans and exact-generation profile layouts.

A new optimization threshold must be scoped, measured and approved before
acceptance; do not carry old removed-feature thresholds forward or relax a
threshold to fit a result. Debugger source hooks remain planned and must not
change ordinary execution semantics.

## Update Policy

Durable reports include revision, Rust/Cargo versions, target/OS/CPU/profile,
runtime limits and cache/profile options, input size, warmup/repeats/iterations,
latency distribution, allocations/bytes and checksums. Include external runtime
versions and measurement modes for comparisons. Raw captures and historical
milestone logs belong in Git or clearly labeled archives, not active requirements.

Build/check commands and repository gates live in [validation.md](validation.md).
