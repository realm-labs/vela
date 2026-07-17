# Actor Runtime/Cache Batch A Baseline — 2026-07-18

This report closes Batch A of the
[Actor Runtime/cache hard-switch plan](../actor-runtime-cache-execution-plan.md).
It records the pre-switch ownership inventory and the commands that Batch F
must rerun with the same workload shapes. Measurements used parent checkpoint
`66577eac4` plus the measurement-only harness in this Batch A checkpoint, Rust
and Cargo 1.97.0, the optimized bench profile, macOS arm64, and 10 available
workers.

No cache, profile, state-layout, Runtime, or execution ownership changed in
Batch A. The only pre-existing harness correction makes extern-state seeding
conditional for host-access workloads that actually declare extern state.

## Prerequisite confirmation

State-storage Batch G and interop `I-RECON-1..6` remain accepted. Current code
and focused reruns prove:

- `DispatchRoot` owns immutable `DispatchGeneration` plus `CallOptions` only;
- `DispatchInvocation<'turn>` borrows the Actor turn's `&mut SharedRuntime`;
- nested replacement re-enters the active `ExecutionSession`;
- exact production scans find no `SharedDispatchRuntime`,
  `Arc<Mutex<Runtime>>`, or `Mutex<Runtime>` execution boundary;
- the 35 engine state tests, 15 hot-reload state tests, three VM owned-contract
  tests, pending-Actor isolation test, and nested-reentry test pass.

## Frozen baseline commands

Batch F must rerun these commands without changing their stable workload shape:

```bash
cargo bench -p vela_engine --bench actor_memory -- memory
cargo bench -p vela_engine --bench actor_memory -- allocations
cargo bench -p vela_engine --bench actor_concurrency
cargo bench -p vela_vm --bench baseline -- native_call_wide_args method_dispatch dynamic_method
cargo bench -p vela_vm --bench baseline -- record_fields host_field_read_write host_state_read_write dynamic_string_method dynamic_script_method
```

The memory parent uses a 1,536 MiB RSS ceiling, a 90-second child ceiling, and
5 ms RSS sampling. A row exceeding either limit is killed and reported as a
capacity failure. `--quick` omits the 10,000-Actor rows and reduces concurrency
samples; it is a harness smoke test, not the acceptance baseline.

Allocation calibration uses `stats_alloc` in its own executable because a
global counting allocator measurably contends under multi-worker load. The
concurrency executable therefore uses the system allocator and cites the
separate allocation calibration instead of perturbing throughput.

## A1: current ownership and allocation inventory

| Storage | Current semantic owner and identity | Mutation / invalidation | Current allocation shape |
|---|---|---|---|
| `RuntimeState::id` | Actor Runtime identity | assigned once | one `u64` per Runtime |
| `RuntimeState::extern_states` | Actor host bindings | bind/stage/reload/reclaim | binding and pending maps plus per-Actor name/type layout maps |
| `RuntimeState::vm_states` | Actor heap, persistent Vela state, retained values | script/host writes, GC, reload | heap, state-value map, retained-root map, and per-Actor state-name map |
| `RuntimeState::sidecars` | currently Actor-local generation execution metadata | reload inserts; safe points prune | one ordered generation map per Runtime |
| `RuntimeSidecars::active_generation` | Actor's adopted dense generation | accepted reload | one generation ID |
| `RuntimeSidecars::generations` | currently Actor-local retained-generation records | construction/reload/prune | one `GenerationRuntimeState` per live adopted generation |
| generation artifact/state sets | weak generation owner plus old-generation state liveness | constructed once; removed on prune | one `Weak<LinkedArtifact>` and two `BTreeSet<StateId>` values per Runtime generation |
| `InlineCaches` | currently Actor-local, indexed by generation-global `CacheSiteId` | read on hits; overwritten/populated on misses; discarded with sidecar | six full vectors sized to `max(CacheSiteId)+1` for every Runtime generation |
| `RuntimeBytecodeProfile` | currently Actor-local, indexed by debug-name and instruction offset | incremented on every executed instruction; discarded with sidecar | one `u64` per linked instruction plus one vector header per function and a debug-name index map |
| VM/extern state-name and expected-type maps | immutable state schema copied into Actor stores | rebuilt on reload | multiple `BTreeMap` copies even though `ProgramImage` already owns generation state name/ID slot maps |

The aarch64 release build reports these full-vector element sizes:

| Cache/profile family | Bytes per dense entry |
|---|---:|
| declared state slot | 16 |
| host access | 80 |
| script record field | 48 |
| linked method dispatch | 48 |
| dynamic method dispatch | 80 |
| native call | 48 |
| bytecode profile counter | 8 |

The six cache families therefore reserve 320 bytes per cache-site index per
Actor generation before vector headers and allocator rounding. The large
fixture has 512 cache sites and 2,309 instructions, so its unavoidable current
dense payload is at least 163,840 cache bytes plus 18,472 profile-counter bytes
per Actor, excluding function-vector headers, maps, state sets, and Actor data.

## A2: consumers and static-link audit

All production root sync/async calls, initialization calls, re-entry calls,
provider calls, callbacks, and replacement calls eventually pass
`RuntimeSidecars` as both `VmInlineCaches` and `VmBytecodeProfiler`. Root and
re-entry execution always pass `Some`, so ordinary Runtime construction and
ordinary execution have no disabled-profile path today.

| Family | VM consumer | Current fact and identity conclusion |
|---|---|---|
| declared VM/extern state | `host_access::{load_linked_state, load_linked_cached_extern_state}` | the dense `StateSlot` is already a linked instruction operand; the mutable cache republishes that exact generation-local operand and is redundant |
| record field | `field_access` linked get/set paths | the linked operand already carries `FieldSlot`; runtime type/shape guards remain necessary, but the guard/result depends on linked nominal generation identity rather than Actor identity |
| linked method dispatch | `script_method_calls::linked_method_dispatch_target` | `MethodDispatchHandle` already indexes immutable `LinkedMethodDispatch`; the cache copies an immutable linked target and is redundant |
| dynamic method dispatch | `script_method_calls::linked_dynamic_method_dispatch_target` | target depends on runtime receiver guard plus the generation's method/type metadata; polymorphic replacement behavior must be measured before choosing shared versus lane storage |
| native call | `native_function_calls::resolve_cached_native_call_target` | linked `FunctionId` is exact, while the callable `Arc` is resolved from the VM registry; sharing requires generation/registry identity qualification |
| host access | `host_access` target-plan resolution | linked code carries `HostTargetPlan`; resolved access depends on root type, operation, schema epoch, and adapter-independent semantics, while permissions/effects remain per-call checks and can never be cached as grants |
| bytecode profile | the linked dispatch loop and nested frame preparation | every executed instruction records `(DebugNameId, InstructionOffset)` into the selected generation view; current engine selection is always the Actor sidecar |

`VmInlineCaches::for_generation` and `VmBytecodeProfiler::for_generation` are
the old sidecar selectors used by frames that retain their original artifact.
The hard switch must preserve that generation qualification without retaining
the Actor-local owner or wrapping it in a forwarding compatibility surface.

## A3: Actor memory baseline

| Artifact | Actors | Cache sites | Instructions | Construction | Retained RSS delta | Allocated / deallocated bytes | Result |
|---|---:|---:|---:|---:|---:|---:|---|
| small | 1 | 2 | 9 | 0.357 ms | 0.50 MiB | 28,285 / 9,286 | accepted |
| small | 100 | 2 | 9 | 0.573 ms | 1.80 MiB | 2,600,020 / 928,600 | accepted |
| small | 10,000 | 2 | 9 | 60.503 ms | 176.1 MiB | 263,012,560 / 92,860,000 | accepted |
| large | 1 | 512 | 2,309 | 0.419 ms | 0.63 MiB | 564,391 / 242,172 | accepted |
| large | 100 | 512 | 2,309 | 31.811 ms | 32.0 MiB | 56,210,620 / 24,217,200 | accepted |
| large | 10,000 | 512 | 2,309 | n/a | >1,536 MiB sampled peak | n/a | killed at RSS ceiling after 1.508 s |

Each fixture has one scalar state schema. Its logical script state payload is
8 bytes per Actor (80,000 bytes at 10,000 Actors), reported separately from
Runtime metadata. The large-row capacity failure is therefore execution
metadata amplification, not Actor business state.

## A4: concurrent Actor baseline

One Actor remained pending on the same async override while an independent
Actor completed in 26.125 us; the pending Actor then completed after the
deterministic latch release. Both results were correct and no Runtime/cache
lock was present or observed.

| Mode | Workers | Calls | Throughput/s | P50 | P95 | P99 |
|---|---:|---:|---:|---:|---:|---:|
| cache hot | 1 | 5,000 | 243,688 | 3.958 us | 4.208 us | 4.458 us |
| cache hot | 2 | 10,000 | 189,542 | 7.833 us | 19.041 us | 21.291 us |
| cache hot | 10 | 50,000 | 591,769 | 10.833 us | 25.916 us | 45.542 us |
| cache cold | 1 | 128 | 201,919 | 3.959 us | 4.459 us | 4.959 us |
| cache cold | 2 | 256 | 105,551 | 17.125 us | 18.959 us | 34.125 us |
| cache cold | 10 | 1,280 | 465,638 | 10.667 us | 28.750 us | 61.291 us |

The separate single-worker allocation calibration recorded 395,008 allocation
events for 5,000 hot calls (79.00/call) and 10,120 for 128 cold calls
(79.06/call), including benchmark thread/sample overhead. It is a consistent
before/after signal, not a claim that every event originates in the VM.
Observable shared Runtime/cache lock-wait is zero by structural audit: current
caches are Actor-local and replacement borrows the Actor Runtime directly.

## A5: cache/profile baseline

All paired checksums and profile-hit counts matched. Stable cache deltas against
the corresponding profile-only rows were:

| Family/workload | Cache delta | Baseline conclusion |
|---|---:|---|
| native wide call | -14.4% | retained candidate; target lookup avoidance is material |
| linked method aggregate | +15.9% | noisy/slower; immutable linked lookup should replace the mutable cache |
| script method aggregate | -14.4% | aggregate benefit mixes record and method families |
| dynamic polymorphic | +4.6% | shared monomorphic replacement risks thrashing; evidence required |
| dynamic guard-miss pressure | +10.0% | current cache is counterproductive under miss pressure |
| trait method aggregate | +1.0% | flat |
| dynamic string monomorphic | +4.8% | slower |
| dynamic script monomorphic | +3.7% | slower |
| host field read/write | +2.4% | slower |
| extern-state read/write | +7.8% | slower; state-slot component is already linked |
| record fields | +0.8% | flat |

Profiling is currently inseparable from engine cache-enabled execution and its
counter arrays are allocated eagerly in every Actor. The paired rows prove the
measurement surface can distinguish interpreter, profile-only, and cache
modes; Batch B must make ordinary engine profiling `None` by default and move
opt-in counters to generation-qualified execution data.

## Batch A conclusion

The baseline confirms the plan's ownership diagnosis. The current design is
correct and concurrent because mutable execution metadata is Actor-local, but
its dense cache/profile footprint cannot construct 10,000 Actors for the large
artifact under the explicit 1,536 MiB ceiling. Batch B may begin with the
frozen commands above and without changing the Gate S/Gate I authority model.
