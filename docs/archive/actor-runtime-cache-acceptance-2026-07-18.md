# Actor Runtime/Cache Acceptance — 2026-07-18

## Result

M20's Actor Runtime/cache hard switch is accepted through Batches A-F at
checkpoint `ee97e3567`. One Actor owns one logical Runtime and all mutable
script-visible state. Immutable generation data and generation-qualified
execution metadata are shared without a process-global Runtime lock, eager
per-Actor cache vectors, or default instruction-counter arrays. No execution
lane was retained because the repeated contention gate did not justify one.

The displaced per-Runtime `RuntimeSidecars`, six dense cache vectors,
instruction profiles, immutable state/schema copies, and deep-cloned ABI data
are deleted. There is no migration flag, dual owner, forwarding compatibility
API, mirrored population, or old-owner fallback.

## Final ownership table

| Family | Final classification and owner | Identity / invalidation |
| --- | --- | --- |
| VM/extern state declaration | immutable `StateSlot` plus `ProgramImage` indexes | exact linked generation; no mutable cache |
| script/host method target | immutable `LinkedMethodDispatch` | exact linked handle and generation; no mutable cache |
| record field | generation-shared per-site slot | script type, shape, and linked field slot; fresh generation on reload |
| host access | generation-shared per-site slot | Engine deployment, root type/plan/operation, and schema epoch |
| native call | generation-shared per-site slot | exact Engine deployment and native function ID |
| standard/callback method | generation-shared per-site slot | linked method ID and runtime receiver/callback guard |
| dynamic method | generation-shared per-site slot | method name, receiver guard, and schema epoch |
| instruction profile | optional generation-shared aggregate | exact generation/function/offset; lazy saturating atomics, default off |
| execution lane | absent | three stable shared-versus-isolated runs showed no repeatable material penalty |
| state, heap, roots, externs, leases, retained values, suspended session | Actor Runtime | Actor identity; never enters shared execution data |

Each mutable cache site is one typed `RwLock<Option<T>>`; generic resolution,
host work, callbacks, and script execution occur after its short-lived guard is
dropped. Cache miss, wrong guard, schema refresh, unsupported caching, and
cache-disabled measurement use the canonical generic VM operation.

## Memory acceptance

The frozen stable memory harness used 1/100/10,000 Actors, small (2 cache sites,
9 instructions) and large (512 sites, 2,309 instructions) artifacts, and both
profile modes. Retained RSS is process-level allocator evidence; allocation
counts are construction traffic, not retained object size.

| Artifact | Profile | Actors | Construction | Retained RSS | Allocated / deallocated bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| small | off | 1 | 0.071 ms | 491,520 B | 21,239 / 9,046 |
| small | off | 100 | 0.422 ms | 1,261,568 B | 1,845,308 / 904,600 |
| small | off | 10,000 | 41.297 ms | 99,287,040 B | 187,213,616 / 90,460,000 |
| small | on | 1 | 0.085 ms | 491,520 B | 21,535 / 9,078 |
| small | on | 100 | 0.575 ms | 1,310,720 B | 1,845,604 / 904,632 |
| small | on | 10,000 | 39.369 ms | 99,336,192 B | 187,213,912 / 90,460,032 |
| large | off | 1 | 0.304 ms | 442,368 B | 335,677 / 233,724 |
| large | off | 100 | 22.861 ms | 819,200 B | 24,402,868 / 23,372,400 |
| large | off | 10,000 | 2,272.421 ms | 96,796,672 B | 2,434,083,376 / 2,337,240,000 |
| large | on | 1 | 0.325 ms | 540,672 B | 372,485 / 241,980 |
| large | on | 100 | 22.632 ms | 868,352 B | 24,439,676 / 23,380,656 |
| large | on | 10,000 | 2,277.688 ms | 96,321,536 B | 2,434,120,184 / 2,337,248,256 |

The pre-switch large 10,000-Actor row exceeded the 1,536 MiB ceiling and was
killed. The accepted row retains about 93 MiB, and enabling the one shared
aggregate profile does not add a per-Actor instruction-count slope. Structural
tests separately prove default construction has no full cache/profile arrays.

## Concurrency and allocation acceptance

An Actor held pending on the same immutable override generation while another
completed in 36.083 us with results 42 and 41. Same-generation override rows:

| Mode | Workers | Throughput/s | P50 / P95 / P99 |
| --- | ---: | ---: | --- |
| hot | 1 | 162,694 | 5.750 / 7.209 / 8.375 us |
| hot | 2 | 293,102 | 6.416 / 8.042 / 9.833 us |
| hot | 10 | 683,926 | 12.417 / 24.542 / 27.917 us |
| cold | 1 | 165,955 | 5.166 / 5.292 / 6.125 us |
| cold | 2 | 273,115 | 6.209 / 7.042 / 7.625 us |
| cold | 10 | 522,822 | 13.250 / 25.042 / 27.083 us |

The final shared-versus-isolated dynamic-site deltas were -8.559%, -2.727%,
and +8.286% at 1/2/10 workers; one worker has no possible shared contention.
Together with the three-run Batch D medians (+1.294%, -3.035%, +2.408%), this
does not establish repeatable material degradation. No lane sidecar was added.

The final separate stable allocation calibration recorded 395,008 events and
95,355,784 allocated bytes for 5,000 hot calls, and 10,120 events and 2,441,520
allocated bytes for 128 cold calls. Checksums matched.

## Stable performance acceptance

All paired VM checksums and profile counts matched. Cache deltas versus the
correct profile-only/hot-offset rows were: native call -14.532%, linked method
+0.241%, script method +0.279%, dynamic polymorphic +6.565%, dynamic miss
+9.233%, trait +0.935%, dynamic String -5.225%, dynamic script +1.717%, host
field +0.394%, host state approximately flat, and record field +0.062%.
Intrinsic polymorphism/miss pressure remains a cache-policy follow-up, not an
ownership or lane justification.

Stable callback groups were all faster: collections -11.655%, array -20.764%,
set -20.238%, map -2.099%, and Option/Result -10.868%. Stable hot-reload means
were 222.214 ms for accepted candidates and 214.920 ms for rejected candidates.
Stable interop retained a 9.612 us Vela-to-Rust scalar boundary, 10.336 us
shared-host, 10.358 us exclusive-host, 9.591 us Rust-to-Vela, 12.067 us round
trip, 2.6 ns empty override, 2.555 us override hit, and 3.988 us
stage/apply/first-call row. Stable async rows preserved correct checksums and
bounded sync/ready/pending costs at 1.926/2.009/2.259 us.

## Acceptance matrix

| Area | Accepted proof |
| --- | --- |
| Actor semantics | Actor-local state, heaps, roots, externs, leases, values, and suspended sessions; one sequential turn owner |
| cross-Actor concurrency | overlapping same-generation/same-override calls and deterministic pending-Actor completion without a Runtime lock |
| default memory | structural allocation tests plus successful 10,000-Actor large rows with profile off/on |
| cache correctness | hit, miss, race, wrong guard, fallback, schema/reload, cancellation, failed population, and panic coverage |
| generation ownership | exact generation selection for caches/profiles and retained old closures/frames/suspensions/roots |
| profiling | default-off; one lazy generation aggregate; stable off/on memory rows and exact reload lifetime |
| lane behavior | absent after the retained three-run contention evidence gate |
| hot reload | fresh new execution data; old data retained only by real owners and reclaimed at the next safe point |
| performance | stable per-family VM, callback, reload, host, async, interop, memory, concurrency, and allocation rows |

Every never-complete condition in the execution plan is false.

## Validation

The acceptance checkpoint passed:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo bench --workspace --all-features --no-run
cargo doc --workspace --all-features --no-deps
cargo check --manifest-path fuzz/Cargo.toml --bins
npm --prefix site run test:syntax
npm --prefix site run test:docs
npm --prefix site run build
```

The workspace uses Rust/Cargo 1.97.0 on macOS arm64. Workspace lint policy
forbids unsafe Rust except in the dedicated C ABI crate; the runtime, VM,
hot-reload, and bytecode scope contains no unsafe Rust. Miri was checked but is
not available for the installed stable aarch64-apple-darwin toolchain.
