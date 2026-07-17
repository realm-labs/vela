# Actor Runtime Cache Batches B-C Acceptance — 2026-07-18

## Result

Batches B and C are accepted as one coherent deletion-first ownership cut.
The former per-Runtime profile arrays, six dense cache vectors,
`RuntimeSidecars`, duplicated state-name/schema maps, and deep-cloned
`ProgramVersion` ABI are absent. There is no compatibility mode, dual owner,
old/new cache switch, mirrored population, or Runtime cache lock.

## Final ownership

| Family | Final owner | Identity and invalidation proof |
| --- | --- | --- |
| VM/extern state declaration | immutable linked `StateSlot` and `ProgramImage` schema/name indexes | exact artifact generation; no mutable cache |
| script/host method target | immutable `LinkedMethodDispatch` | exact linked handle and generation; no mutable target cache |
| record field | generation execution data | script type ID, shape ID, linked field slot; fresh generation on reload |
| host access | generation execution data | Engine registry, root type, target plan/root, operation, schema epoch; guard miss or epoch change repopulates |
| native call | generation execution data | exact Engine deployment plus native function ID; Engine clones share implementation identity |
| standard/callback method | generation execution data | linked method ID plus runtime receiver/callback guard; polymorphic miss repopulates |
| dynamic method | generation execution data | method name plus standard/script/host receiver guard and schema epoch |
| instruction profile | optional generation execution data | exact generation/function/offset; aggregate saturating atomics, disabled by default |
| Actor state/heap/roots/leases/session | Actor Runtime | never stored in shared cache or profile data |

`GenerationInlineCaches` uses one cache-kind-qualified slot per linked site,
not six family vectors. Mutable slots are individual `RwLock<Option<T>>`
values. This keeps synchronization local to the accessed site. Cache miss,
wrong guard, schema refresh, unsupported cache use, and disabled-cache
benchmarks continue through the canonical generic VM operation.

## Profile contract

- Disabled Runtime construction allocates no instruction counters.
- `RuntimeBuilder::with_bytecode_profiling()` enables profiling for the exact
  Engine deployment, including already-live and future reload generations.
- Every Runtime on that Engine/generation contributes to the same aggregate.
- Counters saturate at `u64::MAX`.
- Snapshots identify their executable generation.
- Reset is a relaxed aggregate reset, not a stop-the-world linearization point.
- Accepted reload receives fresh zeroed counters while old owners retain the
  old generation's counters.

## Structural and memory evidence

- Separate owned Runtime images built from the same Engine and artifact retain
  one pointer-equal execution-data object.
- Actor generation entries contain state-ID sets, a weak artifact observation,
  and one shared execution-data handle, with no cache/profile arrays.
- `ProgramVersion` clones share the linked artifact, verified MIR, and ABI.
- The quick 100-Actor large-artifact row (512 sites, 2,309 instructions) fell
  from about 14.0 MiB retained RSS in the intermediate cache-only cut to about
  1.1 MiB after ABI sharing; the small row was about 1.4 MiB. Stable 10,000
  Actor exit measurements remain Batch F evidence.

## Correctness evidence

- 25 focused Engine cache tests pass, including wrong guards, schema epochs,
  reload, immutable linked state/method facts, and eight concurrent owned
  Runtimes racing first native-cache population.
- Five profile tests pass for default-off allocation, aggregate sharing,
  saturation, reset, and fresh reload counters.
- All 855 `vela_vm` unit tests pass after removing redundant generic method
  cache population.
- All 559 `vela_engine` unit tests plus integration tests and doctests pass.
- All 96 `vela_hot_reload` tests pass with shared `ProgramVersion` ABI data.

## Batch D entry

No execution-lane sidecar has been introduced. Batch D must first rerun stable
shared-generation contention and polymorphism measurements. If no named family
shows repeatable material contention or thrashing, the accepted result is no
lane implementation.
