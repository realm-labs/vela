# Actor-Owned Runtime And Cache Model Hard-Switch Plan

> Status: in progress. Batch A is accepted in the
> [baseline report](archive/actor-runtime-cache-batch-a-baseline-2026-07-18.md).
> Batches B and C are accepted in the
> [ownership-cut report](archive/actor-runtime-cache-batches-b-c-acceptance-2026-07-18.md).
> Batch D closed without an execution lane in the
> [lane-gate report](archive/actor-runtime-cache-batch-d-lane-gate-2026-07-18.md).
> Batch E is the active lifetime/correctness proof. The Rust/Vela Actor
> Runtime authority reconciliation closed Gate I, and state-storage Batch G
> remains accepted.
>
> Execution order: begin Batch A as a new checkpoint after the
> [interop reconciliation report](archive/rust-vela-interop-actor-runtime-reconciliation-acceptance-2026-07-17.md),
> then run Batches A-F in this document.
>
> Last updated: 2026-07-18.

## 0. Codex Goal

Use this prompt only after state-storage Batch G remains accepted and the
Rust/Vela Actor Runtime authority reconciliation has published an accepted
report:

```text
/goal Execute docs/actor-runtime-cache-execution-plan.md end to end as a
breaking, deletion-first hard switch. Treat docs/goal.md as the product
roadmap, docs/architecture.md and docs/architecture/*.md as the architecture
contract, docs/decisions.md as durable design decisions, docs/progress.md as
the rolling status source, docs/performance.md as the durable benchmark source,
and this plan as the complete implementation and acceptance contract.

This is one persistent, multi-turn goal. Continue across turns and context
compactions until Batches A-F, every acceptance-matrix row, every
never-complete condition, the final acceptance report, documentation updates,
validation, and coherent commits are complete. Finishing the inventory,
disabling profiling, moving one cache family, making a focused test pass, or
publishing a generation owner while old Runtime sidecars still execute is
progress only and is not a valid stopping condition.

Before implementation, verify from current code, tests, docs/progress.md, and
the acceptance reports that state-storage Batch G remains accepted and
`I-RECON-1..6` in docs/rust-vela-interop-model-plan.md are closed. In
particular, prove override execution borrows the current Actor Runtime from the
exclusive Actor turn, nested calls re-enter the active ExecutionSession, and
neither the Actor Runtime nor an override target is wrapped in
Arc<Mutex<Runtime>>. If either prerequisite is open, keep this plan queued,
report the exact prerequisite, and do not begin Batch A or mutate cache/profile
ownership. Do not duplicate or bypass the prerequisite plans inside this goal.

Once the gates are closed, preserve the fixed architecture throughout:

1. One Actor owns one logical Vela Runtime, its Vela state, heap, roots, extern
   bindings, retained values, HostRef leases, suspended session, and adopted
   generation. One Actor remains sequential; independent Actors must execute
   the same generation and override concurrently.
2. DeploymentGeneration owns immutable code, verified MIR, schemas, callable
   and type metadata, source maps, layouts, and statically resolved targets.
   Actor business state and script heap values never enter shared generation or
   execution-lane storage.
3. Cache/profile ownership follows identity: link exact facts immutably; place
   generation-stable facts in generation-qualified shared execution data; use
   Actor-local sparse/lazy storage only for truly Actor-dependent facts; add an
   explicit execution-lane sidecar only after repeatable contention or
   polymorphic-thrashing evidence.
4. Full per-instruction profiling is disabled by default. An ordinary Actor
   Runtime must not allocate instruction-counter arrays or complete cache
   vectors sized to the shared program.
5. Hot reload publishes a new immutable generation and new
   generation-qualified execution data. Old frames, closures, suspended calls,
   and pinned roots retain their original code and metadata until their owners
   are gone.
6. Cache miss, guard failure, unsupported caching, and cache-disabled
   measurement use the one canonical generic VM operation. This permanent
   correctness path preserves budgets, GC roots, HostAccess, effects,
   reflection policy, diagnostics, cancellation, and return values; it is not
   a legacy compatibility path.

Execute the batches in order:

- Batch A: read-only ownership inventory plus bounded Actor memory,
  concurrency, pending-async, cache/profile, and lock-wait baselines.
- Batch B: hard-switch profiling and shareable state-name/schema metadata to
  the permanent generation owner; delete displaced per-Runtime counters,
  accessors, construction, and Actor-local immutable copies in the same
  verified ownership cuts.
- Batch C: finish every cache-family identity proof, then hard-switch all
  production cache consumers to the final generation execution-data view in
  one coherent implementation batch; delete per-Runtime InlineCaches, full
  vectors, old RuntimeSidecars cache delegation, accessors, and old/new
  selection plumbing before committing the cut.
- Batch D: introduce only measured, retained execution-lane optimizations. If
  no family satisfies the evidence gate, complete the batch with no
  WorkerExecutionSidecars implementation.
- Batch E: close reload, old-generation lifetime, cancellation, panic,
  multi-Actor isolation, and reclamation proofs.
- Batch F: rerun stable memory, throughput, latency, contention, profile,
  cache, reload, host, callback, interop, workspace, example, docs, bench-build,
  fuzz-build, and relevant safe-Rust/Miri gates; publish the final ownership
  table and acceptance report.

This is not a compatibility-preserving migration. Do not add legacy aliases,
wrapper types, adapter traits, forwarding methods, migration feature flags,
environment switches, OldOrNew modes, dual reads or writes, shadow counters,
mirrored cache population, old-owner fallback, parallel root/reentry/provider/
callback/override wiring, or temporary locking, serialization, and cloning to
bridge the cut. Do not commit a checkpoint in which the final owner is active
while the displaced owner remains a functioning production path. Intermediate
local compile failures are allowed while an ownership cut is in progress; keep
and continue that dirty hard-switch work across turns rather than resetting it
or adding compatibility code to make an intermediate shape green.

At the start of every turn, follow AGENTS.md: read docs/goal.md,
docs/architecture.md, docs/progress.md, this complete plan, and the current git
diff; inspect the active batch and run or inspect its most relevant failing
test or benchmark. Work on the smallest verifiable piece that completes the
active hard-switch checkpoint without creating a transitional production
architecture. Preserve unrelated user changes. Update docs/decisions.md when a
classification becomes implemented truth, docs/progress.md when the active
checkpoint changes, and docs/performance.md only for durable baseline or exit
conclusions. Archive raw measurements and acceptance detail instead of
inflating current status docs.

Use Conventional Commits for coherent verified checkpoints. Batch A
measurement may be committed independently. Once a Batch B or Batch C
ownership cut begins, do not commit its half-migrated state; update all
producers, consumers, tests, examples, and benchmarks and delete the displaced
production owner before that cut's breaking checkpoint commit. Run focused
tests during the cut as useful, then run the plan's full validation gates at
the required acceptance boundary.

Never mark this goal complete while a prerequisite is unaccepted, an unchecked
batch or acceptance row remains, independent Actors serialize through a shared
Runtime/cache lock, a pending Actor blocks another Actor using the same code,
default Actor construction allocates full-program cache/profile arrays, a
cache/profile identity can be read against the wrong generation, Actor state
enters shared storage, old generation execution data leaks, a migration-only
compatibility surface remains, an obsolete owner is deferred to a later
cleanup, required measurements or validations are missing, the final
acceptance report is unpublished, the completed work is uncommitted, or the
final worktree is dirty.

Do not report blocked merely because the hard switch is broad or temporarily
does not compile. Report blocked only when an external decision or prerequisite
prevents meaningful repository-local progress after the available alternatives
have been exhausted.
```

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

## 2. Hard-Switch Contract

This migration is one pre-release breaking internal hard switch. It does not
preserve the current per-Runtime cache/profile ownership as a compatibility
surface.

Every ownership cut must land as one coherent verified checkpoint:

```text
define the final owner and final internal API
update every production producer and consumer
update tests, examples, and benchmarks to the final contract
delete the displaced fields, types, accessors, and construction paths
restore a green build and commit the completed cut
```

The local working tree may be temporarily uncompilable while the cut is being
made. That is preferable to committing transitional production architecture.
No intermediate compatibility checkpoint is required.

Prohibited migration mechanisms:

- legacy aliases, wrapper types, adapter traits, or forwarding methods whose
  only purpose is keeping the old cache/profile API compiling;
- feature flags, environment switches, or `OldOrNew` modes selecting the old
  versus new ownership model;
- dual reads, dual writes, shadow counters, mirrored cache population, or a
  fallback from the new owner to the displaced owner;
- keeping per-Runtime vectors alive until a later cleanup batch after the final
  generation owner is active;
- parallel root-call, re-entry, provider, callback, or override wiring for old
  and new execution metadata;
- temporary serialization or cloning introduced only to bridge the cut.

Permanent semantic fallback is different from migration compatibility. A cache
miss, guard failure, unsupported cache family, or intentionally cache-disabled
benchmark continues through the canonical generic VM operation. That generic
path is the correctness definition and remains after migration; it never reads
the old cache owner. Likewise, hot-reload ABI/schema compatibility remains a
product contract, not an excuse to retain obsolete internal storage APIs.

Stable benchmark modes may keep cache-disabled and profile-disabled execution
when those modes remain intentional product/measurement surfaces. They must
select `None` or the final owner, never resurrect the old Runtime sidecars.
Before/after comparisons use Git checkpoints and recorded artifacts rather than
a production migration toggle.

## 3. Prerequisite Order

### Gate S: state-storage acceptance

State-storage Batch G is accepted. Its exact type resolution, nominal
canonicalization, graph preservation, external-owner reclamation, and nested
initializer fingerprint proofs remain the prerequisite baseline; cache
movement must not weaken or bypass them.

### Gate I: Rust/Vela Actor Runtime authority reconciliation

Gate I is closed by the
[interop reconciliation report](archive/rust-vela-interop-actor-runtime-reconciliation-acceptance-2026-07-17.md).
`DispatchRoot` owns immutable generation selection only, and replacement
borrows the current Actor turn's `&mut SharedRuntime` through a scoped
invocation. The former `SharedDispatchRuntime` alias, Runtime-bearing roots,
and lock-based entry paths are deleted. Nested replacement re-enters the
active Actor execution session.

This ordering is mandatory because same-session re-entry determines which
generation, cache view, profile sink, budget, heap, state, and cancellation
context a nested override must observe. Moving caches first would risk
optimizing the former target-owned Runtime that Gate I removed.

Gate I proves:

- unrelated Actors can concurrently execute the same override without a
  package-global Runtime lock;
- one override suspended in `await` does not block another Actor using the same
  linked override;
- nested Rust/Vela/replaceable calls use one Actor Runtime and one session;
- the override observes the current Actor's Vela `state`, not package-global
  mutable state;
- remaining budgets, leases, cancellation, tracing, artifact, and dispatch
  generation are inherited.

Batch A is now ready as a separate checkpoint. Cache/profile work must not
resurrect, preserve, optimize, or route around the removed lock-based
replacement authority.

## 4. Current Implementation Audit

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
  boundaries, which may be changed directly into the final injection contract
  rather than wrapped in a compatibility adapter or duplicated execution loop.

The existing shared-image state test proves that two Runtimes can share an
immutable image while keeping Vela state isolated. Existing profile tests prove
the current per-Runtime counter semantics; those tests must be updated to prove
the selected opt-in aggregation contract rather than retained as accidental
product requirements.

## 5. Target Ownership Model

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

## 6. Cache And Profile Classification Rules

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

## 7. Synchronization And Throughput Contract

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

## 8. Execution Batches

### Batch A: ownership inventory and baselines

- [x] A1. Inventory every field in `RuntimeState`, `RuntimeSidecars`,
  `GenerationRuntimeState`, `InlineCaches`, and `RuntimeBytecodeProfile`.
  Record semantic owner, identity inputs, mutation frequency, invalidation
  event, entry size, and current allocation shape.
- [x] A2. Inventory every `VmInlineCaches` and `VmBytecodeProfiler` call site and
  identify which operations are already statically linked.
- [x] A3. Add an Actor Runtime memory harness for 1, 100, and 10,000 Runtimes
  sharing small and large artifacts. Report construction time, retained bytes,
  peak RSS, cache-site count, instruction count, state-schema count, and Actor
  state bytes separately. Run large rows in a bounded subprocess with an
  explicit RSS/time ceiling so a capacity failure is reported instead of
  exhausting the validation host.
- [x] A4. Add a concurrent Actor throughput harness for 1, 2, and available-core
  worker counts. Include the same override, a long-pending override, cache-cold
  and cache-hot execution, P50/P95/P99 latency, throughput, allocation count,
  and observable lock-wait time. The pending case uses a bounded test latch and
  deterministic release; it is not an unbounded future.
- [x] A5. Capture interpreter-only, current-cache, and current-profile baselines
  without changing semantics. Archive detailed raw data; keep only durable
  conclusions in `docs/performance.md`.

Batch A changes measurement only. It must not move ownership or introduce an
execution lane.

### Batch B: opt-in profiling and empty Actor footprint

- [x] B1. Finalize the permanent generation execution-data owner and its profile
  configuration/sink contract before editing production ownership. Profiling is
  disabled by default; enabled profiling is generation-qualified aggregate or
  explicit execution-lane data with defined snapshot, reset, saturation,
  reload, and old-generation retention semantics.
- [x] B2. Introduce that final owner and hard-switch every root-call, re-entry,
  provider, callback, and override profile consumer to `None` or its profile
  sink. In the same checkpoint, remove eager `RuntimeBytecodeProfile`
  construction from `GenerationRuntimeState` and delete displaced accessors and
  per-Runtime counter storage. The generation owner is retained as final
  architecture and is not temporary cache-migration scaffolding.
- [x] B3. Update existing profile tests directly to the final disabled/aggregate
  contract. Do not keep isolated per-Runtime counters as an alternate mode or
  compatibility fixture.
- [x] B4. When identity analysis proves immutable state-name/schema lookup data
  shareable, hard-switch every lookup consumer to generation metadata and
  delete the Actor-local copy in the same checkpoint. Retain only Actor values
  and heap state locally.
- [x] B5. Add structural allocation tests proving the default Actor footprint is
  independent of instruction count and does not eagerly materialize cache
  storage.

### Batch C: immutable linking and generation-shared caches

- [x] C1. Finish the immutable/shared/Actor-local classification and identity
  proof for every existing cache family before production ownership changes.
  Remove mutable caches for facts that can be represented as verified immutable
  linked operands.
- [x] C2. Add final cache storage to the permanent generation execution-data
  owner introduced by Batch B. Replace current `Cell`/`RefCell` storage with
  proven `Send + Sync` entries whose synchronization is local to the relevant
  site or cold population path. Do not introduce another cache migration owner.
- [x] C3. Hard-switch declared state, record field, linked method, native call,
  host access, and dynamic method cache consumers to the final generation
  execution-data view in one coherent implementation batch. Root calls,
  re-entry, providers, callbacks, and overrides must change together.
- [x] C4. In that same checkpoint, delete the per-Runtime `InlineCaches` owner,
  its full vectors, old `RuntimeSidecars` cache delegation, displaced accessors,
  and any old/new selection plumbing. No production cache family may remain on
  the former authority.
- [x] C5. Update every family test directly to the final owner while preserving
  hit, miss, wrong guard, concurrent first population, generic fallback, schema
  rejection, and hot-reload behavior. Do not add adapter fixtures for the old
  trait shape.

### Batch D: measured execution-lane specialization

- [x] D1. Run the shared-generation contention and polymorphism benchmarks
  before adding lane-local data.
- [x] D2. Add an execution-lane sidecar only for a named family whose shared
  design shows repeatable material contention or cache thrashing and whose
  lane-local candidate improves the stable benchmark without excessive memory.
- [x] D3. Pass lane identity explicitly through host execution setup. Actor
  migration between lanes must be correct and must not move Actor semantic
  state into the lane.
- [x] D4. Keep the generation-shared or generic path available when no stable
  lane identity exists. Do not require a host to pin Actors to workers.
- [x] D5. If no family meets the evidence threshold, close Batch D with no
  `WorkerExecutionSidecars` implementation. Any lane representation that is
  accepted is a retained final optimization, not a temporary bridge that a
  later cleanup batch removes.

D2-D4 were conditional gates. No family met D2, so no lane identity or storage
was added; the generation-shared and generic paths remain the complete runtime
contract.

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

## 9. Acceptance Matrix

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

## 10. Never-Complete Conditions

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
  async, or cache contention measurements;
- a migration-only compatibility alias, adapter, feature flag, dual read/write,
  shadow counter, mirrored cache, or old/new execution-metadata path remains;
- an accepted checkpoint leaves obsolete ownership in production for a later
  cleanup batch.

## 11. Expected Implementation Map

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

## 12. Validation

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
