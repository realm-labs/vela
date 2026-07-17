# Actor Runtime Cache Batch D Lane Gate — 2026-07-18

## Result

Batch D is accepted with no `WorkerExecutionSidecars` or other execution-lane
owner. No measured family showed repeatable material shared-generation
contention or cross-Actor polymorphic thrashing that lane-local storage would
fix. Execution setup therefore has no lane identity, Actor migration remains
unconstrained, and generation-shared slots plus the canonical generic path are
the complete runtime contract.

## Harness

The frozen `actor_concurrency` command now includes a named dynamic-method
contention row. One linked call site is shared by all Runtimes. Even workers use
only a `String` receiver and odd workers use only a script `Label` receiver, so
each worker is monomorphic while the shared site sees cross-worker
polymorphism. The shared mode uses one Engine deployment's execution data. The
comparison mode uses the same immutable artifact with independently registered
execution data per worker, providing the lane-local cache counterfactual
without adding production lane plumbing.

Commands:

```bash
cargo bench -p vela_engine --bench actor_concurrency
cargo bench -p vela_vm --bench baseline -- dynamic_method_polymorphic dynamic_method_cache_miss dynamic_string_method_monomorphic dynamic_script_method_monomorphic
```

The Engine command was run three times with stable sampling: 2,000 outer calls
per worker, 32 dynamic sends per call, and worker counts 1, 2, and 10.

## Shared-versus-isolated result

Positive delta favors shared generation execution data.

| Workers | Stable deltas across three runs | Median | Decision |
| ---: | --- | ---: | --- |
| 1 | -0.890%, +1.294%, +1.443% | +1.294% | variance; no contention |
| 2 | -0.743%, -3.153%, -3.035% | -3.035% | small; not material or scaling |
| 10 | +5.221%, +1.697%, +2.408% | +2.408% | shared is not degraded |

All shared and isolated checksums matched. The ordinary same-generation
override rows also completed concurrently at 1/2/10 workers, and the pending
Actor overlap proof remained correct. Those state-only override rows execute
no mutable cache site, so their reported cache-lock wait remains structurally
zero; the new dynamic row reports the direct shared-versus-isolated throughput
signal instead of claiming instrumented lock-wait time.

## Intrinsic polymorphism result

The stable VM rows compared cache-enabled execution with their profile-only
counterparts. Monomorphic String dispatch was 1.36% faster, monomorphic script
dispatch was 1.75% slower, deliberate polymorphism was 5.43% slower, and
deliberate guard-miss pressure was 9.51% slower. Checksums and profile counts
matched.

The slower polymorphic rows alternate receivers inside one Runtime at the same
site. An execution lane cannot make that site monomorphic, and the cross-Actor
counterfactual above shows no repeatable shared-owner penalty. These rows may
support a future dynamic-cache policy experiment, but they do not justify
lane-local ownership.

## Acceptance

- D1: stable shared-generation contention and polymorphism measurements exist.
- D2: no named family crossed the retained-lane evidence threshold.
- D3: conditional lane-identity plumbing is not applicable and was not added.
- D4: generation-shared storage and generic fallback require no stable lane.
- D5: Batch D closes with no execution-lane implementation.
