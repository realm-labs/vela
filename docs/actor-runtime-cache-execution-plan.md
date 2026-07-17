# Actor-Owned Runtime And Cache Model Execution Plan

> Status: queued behind state-storage Batch G and the Rust/Vela replaceable
> post-review closure.
>
> Execution order: finish state-storage Batch G, complete `F-REVIEW-1..7` and
> `G-REVIEW-1..2` in
> [rust-vela-interop-model-plan.md](rust-vela-interop-model-plan.md), then run
> Batches A-F in this document.
>
> Last updated: 2026-07-17.

## 1. Objective

Make the one-Actor/one-logical-Runtime model scale to production actor counts
without introducing a process-global Runtime lock or eagerly duplicating
full-program execution metadata in every Actor.

The completed model must provide:

```text
one Actor owns one logical Vela Runtime and its script-visible mutable state
many Actor Runtimes execute independently across host workers
immutable code, schemas, metadata, and layouts are shared by deployment generation
cache/profile ownership follows the identity on which each fact depends
hot reload publishes a new generation instead of rebasing old execution metadata
default Actor memory does not scale with all cache sites or bytecode instructions
```

This is an execution and storage-ownership plan. It does not add script-level
threads, concurrent execution inside one Actor, async-frame migration, JIT, or
a second interpreter route.

## 2. Prerequisite Order

### Gate S: state-storage acceptance

State-storage Batch G remains the current project prerequisite. Cache movement
must not hide or work around unresolved state identity, graph preservation, or
generation lifetime defects.

### Gate I: Rust/Vela replaceable post-review acceptance

Complete the existing interop review before changing cache ownership. In
particular, override execution must already use the current Actor Runtime and
the active `ExecutionSession`; target-owned `Mutex<Runtime>` execution must be
gone.

This ordering is mandatory because same-session re-entry determines which
generation, cache view, profile sink, budget, heap, state, and cancellation
context a nested override must observe. Moving caches first would risk
optimizing the provisional target-owned Runtime that Gate I removes.

Gate I must prove at least:

- unrelated Actors can concurrently execute the same override without a
  package-global Runtime lock;
- one override suspended in `await` does not block another Actor using the same
  linked override;
- nested Rust/Vela/replaceable calls use one Actor Runtime and one session;
- the override observes the current Actor's Vela `state`, not package-global
  mutable state;
- remaining budgets, leases, cancellation, tracing, artifact, and dispatch
  generation are inherited.

Only after Gate I has a replacement acceptance report does Batch A become
active.

## 3. Current Implementation Audit

The implementation already has a useful separation between `RuntimeImage` and
`RuntimeState`, but its mutable sidecars are currently too coarse for one
Runtime per Actor:

- `RuntimeState` owns Actor VM/extern state and a generation-keyed
  `RuntimeSidecars` map;
- every `GenerationRuntimeState` eagerly constructs one `InlineCaches` and one
  `RuntimeBytecodeProfile`;
- `InlineCaches` allocates six vectors sized from the generation's maximum
  cache-site ID;
- `RuntimeBytecodeProfile` allocates one `u64` counter for every instruction in
  every linked function;
- `RuntimeVmStateStore` correctly owns the Actor's heap and values, but also
  rebuilds a state-name lookup map whose shareability must be audited;
- the VM already receives caches and profiling through optional trait-object
  boundaries, which is the migration seam rather than a reason to add a second
  execution loop.

The existing shared-image state test proves that two Runtimes can share an
immutable image while keeping Vela state isolated. Existing profile tests prove
the current per-Runtime counter semantics; those tests must be updated to prove
the selected opt-in aggregation contract rather than retained as accidental
product requirements.

## 4. Target Ownership Model

| Owner | Required contents | Must not contain by default |
| --- | --- | --- |
| `ActorRuntimeState` | Vela `state`, heap, roots, extern bindings, retained `VelaValue` handles, HostRef leases, suspended session, adopted generation | full-program cache arrays, full instruction counters, another Actor's state |
| `ExecutionSession` | frames, continuations, remaining budgets, HostAccess, capabilities, effect ceiling, cancellation, tracing, root dispatch generation | process-global mutable execution state, fresh nested-call budgets |
| `DeploymentGeneration` | linked code, verified MIR, schemas, callable/type metadata, source maps, cache/profile layouts, statically resolved targets | Actor business state, mutable host objects |
| generation execution data | generation-qualified shareable cache slots and optional aggregate hotness/profile data | unqualified targets, Actor-owned script values |
| execution-lane sidecar | only measured polymorphic or write-heavy cache families that contend when generation-shared | correctness state, Actor state, implicit OS-thread affinity |

`DeploymentGeneration` is immutable after publication. Mutable generation
execution data, when used, is a separate generation-qualified object and may
outlive publication only while an old frame, closure, Actor turn, or retained
generation still owns it.

An execution lane is an explicit host scheduling identity, not `thread_local!`
state. A `Send` Runtime future may migrate between executor workers, so OS
thread identity cannot determine correctness or select an Actor's semantic
state.

## 5. Cache And Profile Classification Rules

Every existing and proposed family must be classified from its dependency
proof before its storage is changed.

Use this decision order:

1. If linking can resolve the fact exactly, store it in immutable linked code
   or generation metadata and remove the mutable cache.
2. If the fact depends only on stable generation/schema/type identity, prefer a
   generation-shared write-once or synchronized slot.
3. If a shared slot is correct but measured contention or polymorphic thrashing
   is material, add an explicit execution-lane representation for that family.
4. Use Actor-local storage only when the cached result genuinely depends on
   Actor identity or Actor-owned mutable state; keep it sparse or lazy.
5. If none of the identity and invalidation proofs are complete, retain the
   generic fallback and do not publish the cache as accepted.

Initial hypotheses are not acceptance decisions:

| Family | Initial target | Required proof |
| --- | --- | --- |
| declared state slot | immutable linked operand or generation-shared | slot layout is identical for every Actor adopting the generation |
| script record field | generation-shared | shape identity and field slot are generation-stable across Actor heaps |
| linked method dispatch | immutable or generation-shared | receiver/type guard and target belong to the same generation |
| dynamic method dispatch | shared monomorphic first; lane-local only if measured | receiver identity guard, fallback equivalence, polymorphic behavior |
| native call | immutable when fully linked; otherwise generation-shared | registry/callable generation identity and reload invalidation |
| host access | generation-shared only when adapter-independent | root type, target plan, schema epoch, and adapter semantics; per-call permissions are always revalidated and never cached as a grant |
| bytecode instruction profile | opt-in aggregate, never eager per Actor | counter ownership, overflow, reset/snapshot, reload generation, measurement overhead |
| tier hotness/selection | generation-shared or explicit lane | future backend consumes the same verified generation and publication policy |

Cache hits are optimizations only. Misses, empty slots, races during first
population, and guard failures must execute the same generic path and preserve
budgets, GC roots, HostAccess, reflection policy, diagnostics, effects, and
return values.

## 6. Synchronization And Throughput Contract

The design forbids:

- a process-global or package-global Runtime mutex;
- holding a shared cache-population lock across script execution, native calls,
  HostAccess, or `await`;
- a single lock covering every cache site or every deployment generation;
- using worker or OS-thread affinity as a correctness condition;
- publishing a partially initialized target that the generic fallback cannot
  safely replace;
- clearing shared slots in place so an old generation can observe new targets.

Generation-shared cache implementations may use per-site atomics, write-once
publication, or a narrowly synchronized cold path. The exact representation is
chosen per family after measuring entry size, read-hit cost, population cost,
false sharing, contention, and polymorphic replacement behavior. The plan does
not mandate atomics for facts that can be linked statically.

The throughput contract is:

```text
one Actor remains sequential
different Actors can execute the same generation and override concurrently
one suspended Actor releases its executor worker but retains its own Runtime turn
no shared Runtime lock limits throughput to one override call at a time
shared metadata contention is measured and localized by cache family
```

## 7. Execution Batches

### Batch A: ownership inventory and baselines

- [ ] A1. Inventory every field in `RuntimeState`, `RuntimeSidecars`,
  `GenerationRuntimeState`, `InlineCaches`, and `RuntimeBytecodeProfile`.
  Record semantic owner, identity inputs, mutation frequency, invalidation
  event, entry size, and current allocation shape.
- [ ] A2. Inventory every `VmInlineCaches` and `VmBytecodeProfiler` call site and
  identify which operations are already statically linked.
- [ ] A3. Add an Actor Runtime memory harness for 1, 100, and 10,000 Runtimes
  sharing small and large artifacts. Report construction time, retained bytes,
  peak RSS, cache-site count, instruction count, state-schema count, and Actor
  state bytes separately. Run large rows in a bounded subprocess with an
  explicit RSS/time ceiling so a capacity failure is reported instead of
  exhausting the validation host.
- [ ] A4. Add a concurrent Actor throughput harness for 1, 2, and available-core
  worker counts. Include the same override, a long-pending override, cache-cold
  and cache-hot execution, P50/P95/P99 latency, throughput, allocation count,
  and observable lock-wait time. The pending case uses a bounded test latch and
  deterministic release; it is not an unbounded future.
- [ ] A5. Capture interpreter-only, current-cache, and current-profile baselines
  without changing semantics. Archive detailed raw data; keep only durable
  conclusions in `docs/performance.md`.

Batch A changes measurement only. It must not move ownership or introduce an
execution lane.

### Batch B: opt-in profiling and empty Actor footprint

- [ ] B1. Make full per-instruction profiling disabled by default so an ordinary
  Actor Runtime allocates no instruction-counter arrays.
- [ ] B2. Define an explicit profile sink/configuration selected at Runtime or
  deployment setup. Disabled execution must pass `None` through the existing VM
  boundary and avoid profile branches or allocations beyond the existing
  option check.
- [ ] B3. Implement generation-qualified aggregate or execution-lane profiling
  only for enabled runs. Define snapshot, reset, saturation, reload, and old
  generation retention semantics.
- [ ] B4. Move immutable state-name/schema lookup data out of Actor-local stores
  when identity analysis proves it safe; retain only Actor values and heap state
  locally.
- [ ] B5. Add structural allocation tests proving the default Actor footprint is
  independent of instruction count and does not eagerly materialize cache
  storage.

### Batch C: immutable linking and generation-shared caches

- [ ] C1. Remove mutable caches for facts that can be represented as verified
  immutable linked operands.
- [ ] C2. Introduce one generation-qualified execution-data owner separate from
  `ActorRuntimeState`. Do not copy it into each Runtime. Replace current
  `Cell`/`RefCell` storage for migrated shared families with a proven `Send +
  Sync` representation whose synchronization is local to the relevant site or
  cold population path.
- [ ] C3. Migrate one cache family at a time in this order unless Batch A data
  justifies a different order: declared state, record fields, linked method,
  native call, host access, dynamic method.
- [ ] C4. For every migrated family, preserve hit, miss, wrong guard, concurrent
  first population, generic fallback, schema rejection, and hot-reload tests.
- [ ] C5. Remove obsolete per-Runtime vectors and compatibility accessors as
  soon as their final family migrates; do not retain two cache authorities.

### Batch D: measured execution-lane specialization

- [ ] D1. Run the shared-generation contention and polymorphism benchmarks
  before adding lane-local data.
- [ ] D2. Add an execution-lane sidecar only for a named family whose shared
  design shows repeatable material contention or cache thrashing and whose
  lane-local candidate improves the stable benchmark without excessive memory.
- [ ] D3. Pass lane identity explicitly through host execution setup. Actor
  migration between lanes must be correct and must not move Actor semantic
  state into the lane.
- [ ] D4. Keep the generation-shared or generic path available when no stable
  lane identity exists. Do not require a host to pin Actors to workers.
- [ ] D5. If no family meets the evidence threshold, close Batch D with no
  `WorkerExecutionSidecars` implementation.

### Batch E: reload, lifetime, and multi-Actor correctness

- [ ] E1. Publish fresh immutable generation metadata and fresh
  generation-qualified execution data on accepted reload. Never clear or
  rebase old cache slots for new code.
- [ ] E2. Prove old frames, closures, suspended calls, and pinned dispatch roots
  continue using their original generation and cache/profile view.
- [ ] E3. Reclaim old execution data only after every generation owner is gone;
  do not leak generations or require a second reload for collection.
- [ ] E4. Prove two Actors sharing one generation keep Vela state, heaps, roots,
  leases, retained values, and suspended sessions isolated while safely sharing
  eligible execution metadata.
- [ ] E5. Prove cancellation, panic, failed cache population, and dropped futures
  leave no held cache lock, Actor lease, or permanent generation owner.

### Batch F: performance acceptance and close-out

- [ ] F1. Rerun 1/100/10,000 Actor memory scaling for small and large artifacts
  with profiling disabled and enabled.
- [ ] F2. Rerun concurrent same-generation and same-override throughput for
  1/2/available-core workers, including one long-pending Actor.
- [ ] F3. Rerun interpreter-only, profile-only, cache-enabled, reload, host
  access, callback, and interop benchmark groups with stable sampling.
- [ ] F4. Publish a cache-family ownership table with final immutable/shared/
  lane/Actor-local classification and the evidence for each non-static family.
- [ ] F5. Run complete workspace, examples, documentation, benchmark-build,
  fuzz-build, and relevant Miri/unsafe-code gates.
- [ ] F6. Publish an acceptance report, update `docs/progress.md` and
  `docs/performance.md`, and archive this plan only after every never-complete
  condition is false.

## 8. Acceptance Matrix

| Area | Required proof |
| --- | --- |
| Actor semantics | one Actor/Runtime is sequential; state, heap, roots, leases, and suspended execution are isolated |
| cross-Actor concurrency | Actors concurrently execute the same code and override; a pending Actor does not block another through a Runtime lock |
| default memory | no eager per-instruction profile arrays or full cache vectors in each Actor Runtime |
| cache correctness | hit, miss, race, wrong guard, fallback, schema change, reload, cancellation, and panic preserve semantics |
| generation ownership | every dense cache/profile identity is interpreted only with its owner generation |
| profiling | disabled by default; enabled ownership and overhead are explicit and measured |
| lane behavior | optional, explicit, migration-safe, and justified by repeatable contention evidence |
| hot reload | new roots adopt new metadata; old roots keep old code and metadata until release |
| performance | per-family stable rows report throughput, latency, allocation, memory, and contention without hiding regressions in aggregates |

Memory acceptance is structural, not only an RSS observation. With profiling
disabled, increasing linked instruction count alone must not add per-Actor
instruction counters. Increasing cache-site count alone must not allocate a
complete cache vector in every Actor. RSS benchmarks confirm the structure and
detect allocator or hidden-owner regressions.

Concurrency acceptance is also structural. Tests must show overlapping calls
for independent Actors and must audit that no package-global Runtime lock is
held. A noisy throughput improvement alone is not sufficient proof.

## 9. Never-Complete Conditions

Do not declare this plan complete while any of the following remains true:

- an override target owns `Arc<Mutex<Runtime>>` or another independently
  executing mutable Runtime;
- independent Actors serialize through one Runtime or package-global cache
  lock;
- default Runtime construction allocates counters for every instruction;
- default Runtime construction allocates all cache families for every cache
  site;
- profile or cache entries can be interpreted against a different generation;
- cache hits bypass budgets, HostAccess, GC roots, effects, reflection policy,
  diagnostics, cancellation, or generic fallback semantics;
- lane-local state is required for correctness or selected implicitly from OS
  thread identity;
- Actor Vela state, heap objects, leases, roots, or suspended sessions enter
  shared generation or lane storage;
- reload clears shared slots in place or leaks old execution data permanently;
- the final report omits multi-Actor memory, concurrent same-override, pending
  async, or cache contention measurements.

## 10. Expected Implementation Map

| Area | Expected responsibility |
| --- | --- |
| `vela_bytecode` linker/artifact | immutable linked operands, cache/profile layouts, generation identity |
| `vela_hot_reload` | immutable generation publication, compatibility, old-generation ownership |
| `vela_vm` | optional cache/profile traits, generic fallback equivalence, no storage-policy ownership |
| `vela_engine::runtime` | Actor state, session wiring, selected generation execution data, explicit profile/lane configuration |
| `vela_engine::dispatch` | current-Actor same-session override execution; no target-owned Runtime |
| benchmark harnesses | Actor memory slope, concurrent throughput/latency, contention, profile/cache comparison |

Do not introduce a second VM loop, a cache-aware alternate Runtime API, or an
interop-only execution-data store. Root calls, nested re-entry, providers,
callbacks, and overrides use the same storage selection and linked execution
path.

## 11. Validation

Focused commands will evolve with the harness names, but final acceptance must
include at least:

```bash
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

Batch A must record the exact future Actor memory and concurrency benchmark
commands before implementation begins. Batch F must rerun those same commands
with the same workload shape and stable sampling settings.
