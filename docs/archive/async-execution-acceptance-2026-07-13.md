# Async Execution Performance And Memory Acceptance — 2026-07-13

This report compares the completed executor-neutral async implementation with
the pre-change checkpoint in
`async-execution-baseline-2026-07-13.md`. Both captures use macOS/aarch64,
Rust/Cargo 1.97.0, and release builds on the same machine.

## Synchronous Comparison

The original quick commands were rerun without changing their parameters or
checksums.

| Workload | Pre-change mean | Current mean | Delta |
|---|---:|---:|---:|
| scalar branch loop | 113,562 ns | 309,520 ns | +172.6% |
| budgeted scalar branch loop | 114,708 ns | 267,417 ns | +133.1% |
| small-argument script calls | 1,010,667 ns | 1,646,396 ns | +62.9% |
| wide-argument script calls | 1,014,979 ns | 1,479,604 ns | +45.8% |
| collection callbacks | 12,287,791 ns | 13,360,541 ns | +8.7% |
| cached collection callbacks | 9,341,583 ns | 11,624,208 ns | +24.4% |
| call-heavy, 2 executables | 335,875 ns | 431,125 ns | +28.4% |
| call-heavy, 201 executables | 741,417 ns | 804,375 ns | +8.5% |
| hot-reload accepted update | 24,106,958 ns | 23,146,417 ns | -4.0% |
| hot-reload ABI rejection | 19,020,083 ns | 17,141,812 ns | -9.9% |

The scalar and script-call regressions are material. They are accepted for this
milestone because the hard switch replaces Rust-recursive calls and scattered
callback/provider execution with the correctness-owned `ExecutionSession`,
explicit frames, semantic budget state, GC roots, cancellation, and resumable
continuations. Restoring recursion or a parallel fast driver would violate the
architecture.

Named follow-up `ASYNC-PERF-1` must profile `scalar_branch_loop`,
`script_call_small_args`, and the cached callback row, then reduce
session/frame dispatch cost through compact reusable frame storage and/or a
verified leaf/block dispatch specialization. It must retain the single driver,
semantic charge points, full root maps, and non-recursive calls. This follow-up
belongs to the post-async performance queue and is not permission to weaken the
accepted runtime contract.

The explicit frame-stack depth test still completes 10,000 recursive script
calls. The async acceptance bench measured that workload at about 3.78 ms per
outer call in a quick capture.

## Async And Provider Comparison

`cargo bench -p vela_engine --bench async_execution -- --quick` reported:

| Workload | ns/call | Relative observation |
|---|---:|---|
| sync Runtime entry | 2,172.4 | comparison base |
| ready async Runtime entry | 2,346.4 | +8.0% vs sync |
| one pending/wake/resume | 2,344.1 | indistinguishable from ready at quick-run resolution |
| ready async mutable lease | 2,726.7 | +25.5% vs sync |
| provider sync target | 3,559.2 | provider base |
| provider async target | 3,760.2 | +5.6% vs provider sync |

Each row ran 100 warmups and 1,000 measured operations with matching scalar
results. The pending native deterministically returns `Pending` once and wakes
before completing. The lease row acquires/restores an exclusive typed host
receiver on every call. Provider rows use the same `ProviderMethodTarget` and
Runtime call pair.

## Suspended Memory

The benchmark creates 2,000 runtimes sharing one artifact. Direct execution of
the release benchmark binary under `/usr/bin/time -l` reported:

| Shape | Maximum RSS |
|---|---:|
| idle runtimes | 34,537,472 bytes |
| one suspended entry + pending native per runtime | 55,263,232 bytes |
| suspended entry plus 16 additional script frames per runtime | 120,946,688 bytes |

The shallow suspended delta is 20,725,760 bytes, about 10,363 bytes per pending
invocation including its session, one script frame, pending native state, and
boxed future. The deep-minus-shallow delta is 65,683,456 bytes, about 2,053
bytes per additional suspended script frame. Subtracting one frame gives a
rough 8.3 KiB session/pending-native remainder. These are process-RSS estimates,
not allocator-layout guarantees; page rounding and shared process state remain
in the sample. The public `RuntimeCallFuture` header itself is 16 bytes in this
build (32,000 bytes for 2,000 headers).

Correctness tests separately enforce memory budgets on async result
materialization and prove suspended/nested GC roots. No Rust future is stored in
the script heap.

## Acceptance

All checksums matched. Async overhead is bounded and executor-neutral, memory
growth is linear in pending invocations and frames, and the material sync
regression has an explicit architectural justification plus named optimization
follow-up. The Batch D performance/memory gate is accepted without recursion,
duplicate drivers, unsafe leases, or bytecode-inferred await.

## Final Validation

The completed implementation passed the following gates on 2026-07-13:

- focused tests for `vela_syntax`, `vela_hir`, `vela_analysis`, `vela_mir`,
  `vela_bytecode`, `vela_vm`, `vela_host`, `vela_engine`, `vela_hot_reload`,
  `vela_reflect`, `vela_macros`, `vela_registry`, `vela_language_service`,
  `vela_lsp_server`, `vela_cli`, and `vela_c_api`;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test --workspace --all-features`, including compile-fail rustdoc tests;
- `cargo test --manifest-path examples/Cargo.toml --all-features`, all 31
  runnable example checks, and direct runs of `async_basic` and
  `async_stateful_reentry`;
- `cargo bench --workspace --all-features --no-run`, including the async
  acceptance benchmark;
- `cargo doc --workspace --all-features --no-deps`;
- the documentation site's placeholder, highlighting, Astro check, and static
  build gates.

`cargo miri --version` reported that the `miri` component is unavailable for
the installed `stable-aarch64-apple-darwin` toolchain. No Miri result is claimed;
the focused safe-Rust ownership, cancellation, lease restoration, nested
reentry, and Runtime reuse tests remain the executable proof on this machine.

The file-size audit compared async-track Rust files with the pre-track base.
The newly over-threshold mixed files were split: runtime unit tests moved out of
`runtime/mod.rs`, and async provider tests moved out of the package test root,
leaving both roots below 1,200 lines. Remaining over-threshold files predate the
track or are focused test/verification modules; `linked_execution.rs` retains
its documented opcode-dispatch exception and delegates async ownership to the
focused session/reentry modules.
