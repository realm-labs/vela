# Async Execution Batch E Acceptance — 2026-07-14

This report closes the post-review Batch E in
`docs/async-execution-model-plan.md`. The historical Batch A-D result remains
in `async-execution-acceptance-2026-07-13.md`.

## Environment And Comparison

```text
host: Windows x86_64
rustc: 1.96.1 (31fca3adb 2026-06-26)
cargo: 1.96.1 (356927216 2026-06-26)
profile: release
baseline: d7c52017 (before E1)
candidate: final Batch E tree
command: cargo bench -p vela_engine --bench async_execution -- --stable
sampling: 3 samples, 100,000 operations per ordinary row, median reported
```

The deep-call row is one sample of 1,000 complete 10,000-frame calls.

The detached baseline received only the identical benchmark reporting changes;
its runtime implementation remained at `d7c52017`. All compared checksums
matched. The July 13 macOS/Rust 1.97 measurements are not compared numerically
with this Windows capture.

| Workload | Before ns/call | Batch E ns/call | Delta |
|---|---:|---:|---:|
| sync entry | 3,239.1 | 2,994.8 | -7.5% |
| ready async entry | 3,838.5 | 3,447.5 | -10.2% |
| pending/wake/resume | 4,927.7 | 4,328.2 | -12.2% |
| deep call depth 10,000 | 11,184,905.2 | 11,515,076.4 | +3.0% |
| exclusive mutable lease | 5,681.8 | 7,009.1 | +23.4% |
| provider sync | 8,566.2 | 7,135.8 | -16.7% |
| provider async | 11,156.4 | 8,172.6 | -26.7% |

The exclusive lease regression is accepted because the old mutable-origin slot
could not represent a true shared lease. The replacement uses safe-Rust owned
read/write guards whose state agrees with the requested capability and whose
RAII lifetime covers suspension, cancellation, reentry, and rollback. Named
follow-up `ASYNC-LEASE-PERF-1` will profile direct lease acquisition/restoration
under M20 and may reduce lookup or guard-management overhead only while
preserving the exact shared/exclusive state machine.

New paired rows measured the repaired boundaries. A true shared lease measured
6,932.2 ns/call versus 7,009.1 ns/call for exclusive acquisition (-1.1%). A
scalar reentry measured 11,625.0 ns/call; reentry returning and releasing a
dynamically rooted record measured 15,253.9 ns/call (+31.2%). The latter cost is
confined to the correctness boundary and includes tracing the returned value,
lazy registry creation, root insertion, weak-token construction, and removal.
Ordinary executions allocate no dynamic-root registry.

## Suspended Memory

The release binaries held each memory shape live for 250 ms while PowerShell
sampled Windows peak working set. Each shape contains 2,000 runtimes.

| Shape | Before bytes | Batch E bytes | Delta |
|---|---:|---:|---:|
| idle | 32,186,368 | 32,063,488 | -0.4% |
| shallow suspended | 50,905,088 | 50,700,288 | -0.4% |
| suspended plus 16 frames | 104,935,424 | 104,923,136 | flat |

The idle-to-shallow delta is about 9,359 bytes per pending invocation before
and 9,318 bytes after. Additional suspended frames are about 1,688 and 1,694
bytes per frame respectively (+0.36%). The public future header remains 16
bytes. These are process working-set observations, not allocator guarantees.

## Validation

The final tree passed:

- focused VM, engine, host, macros, reflection, analysis, provider, reentry,
  reload, root-liveness, and lease tests;
- `cargo fmt --all -- --check`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test --workspace --all-features --no-fail-fast`, including rustdoc
  compile-fail tests;
- example-workspace all-target/all-feature clippy and tests, all 31 runnable
  example checks, plus direct `async_basic` and `async_stateful_reentry` runs;
- `cargo bench --workspace --all-features --no-run`;
- `cargo doc --workspace --all-features --no-deps`;
- `cargo check --manifest-path fuzz/Cargo.toml --bins`;
- documentation placeholder and Vela-highlighting checks, Astro diagnostics,
  and the 147-page static site build.

Miri is unavailable for the installed `stable-x86_64-pc-windows-msvc`
toolchain. No Miri result is claimed; the safe-Rust compile tests plus focused
lease/reentry/cancellation/Runtime-reuse tests remain the executable proof.

## Audit Classification

Every Section 17.6 forbidden-path audit returned zero active hits except
`execute_linked_call(`. Its 24 hits are one non-recursive driver definition,
six public VM root adapters, and 17 tests. The driver and exhaustive opcode loop
never call it recursively.

The Batch E audits also prove:

- no script-visible reflection record retains field `async`;
- no session/resume/reentry policy remains in `linked_execution.rs`;
- no provider-specific Runtime execution method exists;
- `resolve_provider_call` alone owns provider handle, method dispatch,
  asyncness, receiver shape, parameter, and default resolution;
- all 12 Rust files above 1,200 lines are listed in the reviewed exception
  table; `linked_execution.rs` is 2,417 lines of driver/dispatch/root glue and
  the expanded async benchmark remains 331 lines.

Batch E therefore closes `ASYNC-ROOT-1`, `ASYNC-LEASE-1`,
`ASYNC-REFLECT-1`, `ASYNC-VM-MOD-1`, `ASYNC-PROVIDER-1`, and
`ASYNC-DOC-1` without a compatibility alias, duplicate driver, permanent root,
exclusive-as-shared lease, unsafe ownership, or provider-specific call API.
