# Vela crate architecture and API audit

Date: 2026-09-04

Original reviewed revision: `d22121ed0004`

Independent re-review: 2026-09-05, revision `5da258bfb9af4`. The only change
between these revisions is this report; implementation findings refer to the
same code. This edition corrects the original review in place.

Toolchain: `rustc 1.98.0`, `cargo 1.98.0`

Re-review host: `x86_64-pc-windows-msvc`.

Implementation status, 2026-09-05: P0 ranks 1-4 have been repaired with focused
regressions: bounded owned export (C-04), exact-base ordinary reload (C-02),
fresh reachability marking before every sweep slice (C-01), and explicit
reflection/exclusive-receiver authority (H-05). Finding descriptions and evidence
below preserve the reviewed baseline; remediation notes describe current code.
P1-P3 remain open, including the separate GC pause-budget follow-up.

P0 validation: focused cycle/limit, stale-base/fan-out/initializer-order,
GC mutation/root-refresh, and authority-configuration regressions pass, as do
`cargo test --workspace --no-fail-fast`, workspace formatting, Clippy with
`--all-targets -- -D warnings`, and the release `wasm32-unknown-unknown`
playground build. These gates validate the repairs, not closure of P1-P3.

## Executive assessment

Vela has a stronger correctness foundation than its size and milestone status
would normally suggest. Stable definition identities, verified MIR and bytecode,
generation-owned code, explicit host handles, deterministic collections, and
extensive conformance tests are all good architectural choices. The project does
not need a rewrite.

The re-review reproduces four correctness defects, with different exposure:

1. opt-in finite-slot incremental GC can collect newly reachable objects because
   the collector has neither a complete incremental mark state nor
   allocation/write barriers;
2. an ordinary `HotUpdate` is not bound to the program generation against which
   it was checked, so a stale update can bypass compatibility checking;
3. reflection converts stable unsigned IDs to signed integers by saturation,
   causing many distinct reflected IDs to become the same value; and
4. converting an ordinary cyclic heap value to `OwnedValue` recurses without
   cycle detection and can overflow the host stack. Acyclic shared graphs can
   also expand exponentially without an output-work or allocation budget.

C-01 is critical when finite-slot collection is selected; the default
microsecond configuration currently completes collection atomically and does
not exhibit that inter-step failure. C-02 concerns ordinary Vela reload, not a
demonstrated bypass of the separate whole-Service-generation checks. C-03 is a
high-severity reflection identity defect, not a collision in internal compiler
or runtime IDs. C-04 is critical on ordinary value export, including CLI and
playground output. These boundaries should guide release gating.

For the implementation order, see the
[prioritized remediation plan](#prioritized-remediation-plan): C-04, C-02,
C-01 containment, then H-05 lead the queue. Finding numbers identify topics,
not repair priority.

Host/reflection authority defaults also need correction for deployments that
expose those paths. A reflection resource-budget setter currently enables
reflection with all policy permissions; Engine reflection is disabled when no
reflection configuration is supplied. This is a configuration hazard, not proof
that every adapter, capability, or lease check can be bypassed.

The main architecture opportunities are removing repeated work and clarifying
ownership. Compiled artifacts retain several forms of the same program; ordinary
Vela entries rebuild VM dispatch and reflection state; runtime dependencies
reach compiler layers; and standard-library metadata has multiple hand-written
tables. Different type/schema projections also repeat some metadata, although
their layer-specific responsibilities are legitimate. Optimize demonstrated
reconstruction and retention costs before introducing broader abstractions.

The recommended direction is therefore:

- preserve the language semantics, verifiers, stable IDs, and host capability
  model;
- repair the four correctness invariants before adding more surface area;
- freeze and share one generation image containing runtime code, dispatch, and
  reflection metadata;
- keep compiler-only HIR/MIR data out of retained runtime generations;
- move the frontend toward immutable per-module data and demand-driven queries;
- reduce public APIs to a small embedding facade, with raw compiler and VM
  construction kept internal or explicitly advanced.

## Scope and method

This was a static architecture and implementation review of all 23 workspace
crates, supported by the existing tests and checked-in benchmark evidence. The
review focused on:

- crate boundaries and dependency direction;
- embedding ergonomics and the Rust/Vela boundary;
- runtime and compilation hot paths;
- hot-reload correctness;
- accidental complexity and duplicated representations;
- public API safety and ease of use.

The workspace test suite passed before the review. Targeted crate suites also
passed during the audit. A passing suite is important evidence of implementation
quality, but it does not cover the GC barrier case, stale update application,
lossy reflected IDs, or several macro and URI edge cases described below.

This document uses:

- **Critical** for a correctness or safety invariant that can fail in normal
  supported use;
- **High** for a major performance, security, scaling, or API-correctness risk;
- **Medium** for architectural debt that materially raises change cost;
- **Low** for localized cleanup.

Line references describe the reviewed revision and will naturally drift.

The independent re-review read the manifests, relevant implementations, and
existing tests across all 23 crates, then traced findings through their callers
and downstream validation. It also re-opened the primary reference documents
below. This is targeted static review plus executable probes, not exhaustive
line-by-line proof of every function or a production security certification.

### Re-review evidence and corrections

The following status overrides broader wording in the original edition.
"Reproduced" means a small external consumer demonstrated the behavior;
"source-confirmed" means the call path was inspected but no new workload
measurement was made. Proposed reorganizations are recommendations, not defects
merely because two layers have different representations.

| Finding | Re-review result |
|---|---|
| C-01 GC | Reproduced live-child loss between finite-slot steps, even when the second call supplies the current roots. A zero-microsecond request still swept 100 slots. Default collection is atomic, with an unenforced time target. |
| C-02 ordinary reload | Reproduced removal of a newly added function by a stale update that fresh compatibility checking rejects; replay was accepted. A cloned runtime consumed shared staging while the original stayed at version zero. |
| C-03 reflection IDs | Reproduced distinct `TypeId`s `2^100` and `2^100 + 1` both appearing as `i64::MAX`. Internal IDs remain distinct. |
| C-04 owned export | Reproduced `Runtime::value_to_owned` stack overflow after a script returns a self-containing array; also reproduced 13 shared heap arrays expanding into 8,191 owned arrays. |
| H-01/H-02 runtime work and retention | Source-confirmed per-call VM/registry construction and compiled-artifact MIR retention. Existing cache/profile sidecars are already shared. Historical RSS and allocation figures were not remeasured or attributed solely to MIR. |
| H-03/H-08 incremental tooling | Corrected: request snapshots already share database roots and facts are memoized per generation. Edits with a live snapshot can still deep-copy databases; HIR still rebuilds globally. Several handlers run synchronously, while completion, selected retryable queries, and formatting have actual worker lanes. |
| H-04 reflection indexes | Reproduced an old function name resolving to the new descriptor after replacing its ID. Type registration instead permits two different `TypeKey`s with the same ID; it does not use the same name-to-ID index. |
| H-05 policy configuration | Reproduced that `reflection_lookup_budget(1)` alone enables `reflect::types()`. The adapter receiver default is source-confirmed exclusive. |
| H-07 macro hygiene | Corrected: `_value`/`value` normalize to duplicate labels, but ordinary Engine construction returns `DuplicateNativeFunctionParamName`. Silent wrong execution through that supported registration path was not reproduced. |
| H-07 downstream lint policy | Reproduced E0453 with a scalar-only `#[service]` under `#![forbid(unsafe_code)]`; the unconditional generated `allow` already suffices to fail. |
| Bindgen names | Reproduced successful generation of duplicate `foo` fields from valid `Foo`/`foo` fields, followed by Rust E0124/E0062. Module, callable, and top-level type collision checks already exist. |
| Recommendations withdrawn or narrowed | Do not reverse the valid `vela_common -> vela_def` dependency, remove independent verifier proofs, treat bindgen's schema as an LSP DTO, or automatically release Host capabilities contrary to the current contract. |

The existing suites pass because these reproductions exercise missing cases or
confirm the protection supplied by another layer. No implementation or repository
test files were changed for this review.

## Overall architecture

### What is already sound

The most important vertical slice is real and coherent:

`source -> syntax -> HIR -> analysis -> MIR -> bytecode -> VM -> HostAccess`

Particularly good decisions are:

- `DefPath`-derived identity and deterministic registries;
- a lossless Rowan syntax tree;
- explicit MIR effects, safepoints, guards, and a sealed verifier;
- symbolic bytecode followed by linked handles and another verifier;
- immutable, `Arc`-owned program generations, allowing active calls to finish
  on old code;
- a 16-byte `Value` representation;
- generational `HostRef` handles and explicit `HostPath` traversal instead of
  exposing Rust references;
- generation-scoped sidecars and inline caches instead of mutating code objects;
- deterministic ordered script collections;
- unusually broad tests for aliasing, stale handles, compatibility, budgets,
  unsafe boundaries, and protocol behavior.

These are the hard parts of a scripting runtime, and they are worth preserving.

### Where the layering has become too wide

The runtime build and API dependency cone includes compiler concepts. `vela_vm`
depends on `vela_bytecode`, which in turn has normal dependencies reaching
analysis, HIR, MIR, package, registry, and stdlib; `vela_vm` also depends
directly on MIR and reflection. `vela_reflect` depends on HIR, package, and
syntax to project script metadata. Link-time dead-code elimination may remove
unused machine code, but these dependencies still couple compilation,
interfaces, build time, and artifact ownership to frontend models that should
have been compiled into a neutral runtime snapshot.

The retained artifact is a concrete example. `LinkedArtifact` contains a
`LinkedProgram`, a `ProgramImage` with unlinked code and module graph data, and
verified MIR. The portable artifact already proves that MIR is not required by
the interpreter. Checked-in measurements report roughly 650--659 MB RSS for 16
retained 200-function/lambda generations
(`docs/performance.md:417-421,456-460,502`). These are historical process
high-water RSS measurements, not incremental retained bytes or a measured
MIR-only contribution. Allocator/process overhead and shared ownership prevent
attributing that total to representation duplication alone.

The same duplication appears in smaller forms:

- `TypeRegistry`, `TypeBindingRegistry`, and `DefinitionRegistry` survive
  together in `Engine`;
- syntax hints, HIR hints, registry definitions, analysis facts, MIR contracts,
  engine hints, and language-service schema facts repeatedly translate the same
  source-level type information;
- stdlib method identity and signatures are repeated in manifest, engine, and VM
  tables;
- unlinked and linked opcode enums mirror about eighty instruction families;
- language-service source text moves through several owned `String`/`Arc<str>`
  copies.

### Recommended target shape

The target can be reached incrementally without creating another large set of
crates:

```text
platform adapters
  CLI / LSP / browser worker
             |
       small public facade
       Engine / Runtime / Schema
          /              \
compiler pipeline        generation image
syntax -> HIR shards     runtime bytecode
       -> analysis       frozen dispatch
       -> MIR            frozen reflection
       -> codegen        ABI + state layout
                              |
                        VM session + HostSession
```

The important ownership rules are:

1. compiler databases own source, CST, HIR, facts, and MIR;
2. a generation image owns only interpreter-required code and immutable runtime
   metadata;
3. per-call sessions own budgets, stacks, temporary host leases, and GC roots;
4. adapters own filesystem, network, URI, browser-worker, and protocol concerns;
5. one authoritative host-type schema supplies immutable compiler, reflection,
   binding, and tooling projections without duplicating registration authority.

## Comparison with mature implementations

This review does not recommend copying another language wholesale; Vela's hot
reload and host write-through requirements are distinct. Several mature designs
nevertheless provide useful pressure tests:

- [rust-analyzer's architecture](https://rust-analyzer.github.io/book/contributing/architecture.html)
  keeps protocol knowledge at the edge, uses immutable snapshots, and keeps
  `ItemTree` summaries stable across body edits. Vela already shares database
  roots for snapshots, but edits can trigger copy-on-write and global HIR
  reconstruction. The relevant next step is reuse across revisions.
- [rustc's query model](https://rustc-dev-guide.rust-lang.org/query.html)
  demonstrates the value of keyed, memoized derivations and dependency tracking.
  Vela's analysis fixed points and HIR closure queries often rescan complete maps
  instead.
- [Luau's performance design](https://luau.org/performance/) favors a compact
  value representation, interpreter-specialized instructions, inline caching,
  and avoiding allocation in hot loops. Vela already follows the first three,
  but recreating VM registries on every engine call and recomputing collection
  sizes defeats that work.
- [Wren's embedding API](https://wren.io/embedding/) and
  [Rhai's function registration](https://rhai.rs/book/rust/functions.html)
  provide useful comparisons for a small foreign boundary and direct native
  registration. They are usability references, not evidence of a comparative
  safety ranking. Vela must retain its own host ownership, lease, and generation
  guarantees while simplifying adapter authoring.

## Cross-cutting findings

### C-01: incremental GC is not correct across safe points

**Remediation, 2026-09-05:** finite-slot sweeping now performs a fresh atomic
mark from complete current roots before each slice. VM safe points refresh
frame, protected, and dynamic roots on every call. This conservative repair
covers allocations and changed edges without introducing partial write barriers;
it retains the sweep-slot limit but does not fix the unenforced time target or
bound marking work. The following description records the original defect.

**Status: reproduced. Severity: Critical for opt-in finite-slot collection.**

`GcConfig::max_pause_micros` defaults to 500 μs
(`crates/vela_vm/src/heap.rs:196-205`), but `GcBudget::micros` sets the sweep
slot limit to `usize::MAX` (`heap.rs:227-232`). The collector checks only the
slot limit, not elapsed time (`heap.rs:657-738`), and performs the whole mark
phase at once.

The opt-in finite-slot path is worse than an inaccurate pause promise.
Frame/protected roots are snapshotted only when a cycle starts; later safe
points pass no roots (`heap_execution.rs:172-200`). A special admission barrier
marks dynamic values (`heap_execution.rs:125-150`), but new allocations begin
unmarked (`heap.rs:744-770`), and there is no general allocation or container
write barrier. A live object allocated into a frame or linked from a previously
swept container can therefore be reclaimed by the next sweep step.

The external probe allocates a parent array and one padding object, sweeps one
slot with the parent rooted, then allocates a child and inserts it into the
parent. Completing the cycle with both parent and child explicitly supplied as
roots retains the parent but deletes the child: `step_gc_with_budget` ignores
new roots while a cycle is active. Separately, `GcBudget::micros(0)` swept all
100 unreachable probe objects. Neither reproduces Rust memory unsafety by
itself; the demonstrated failure is loss of a live script object.

The normal `HeapExecution::new` route uses `GcBudget::micros`, whose unlimited
slot count finishes the cycle in one call. Do not describe default execution as
incrementally corrupting its heap. Its separate defect is an unenforced pause
target and an unbounded-in-time atomic mark/sweep.

**Action:** immediately make collection atomic or disable the finite-slot API.
Add a regression with an allocation and a new container/frame edge between
sweep steps. Implement incremental marking only with a tri-color state,
allocation barrier, write barrier, root handling, and a real time deadline that
also accounts for marking.

### C-02: hot updates lack compare-and-swap identity

**Remediation, 2026-09-05:** updates now carry the exact checked base generation
token and version number. Both raw hot-reload application and Engine activation
validate them; Engine checks before state initialization. Replays and unrelated
same-number bases reject without publication. Runtime ownership is non-Clone;
staging handles remain cloneable. Exact-generation fan-out remains supported.
The following description records the original defect.

**Status: reproduced. Severity: Critical for ordinary precompiled/staged reload.**

Compatibility is checked against a supplied previous version in
`crates/vela_hot_reload/src/compile.rs:38-116`. `HotUpdate` then retains ABI,
changes, and artifact but no base version or checksum
(`version.rs:179-196`). Application increments the current version and installs
the update without checking its origin (`runtime.rs:96-117`).

Two updates A and B can therefore both be compiled from v0, then applied A
followed by stale B. B was never checked against A. In addition,
`HotReloadRuntime` derives `Clone` while sharing the staging mutex but copying
its current `Arc` field; one clone can consume the shared update and advance
while the other remains on the old generation (`runtime.rs:41-55,85-117`).

The probe builds A (adds `helper`) and B (changes only `main`) from v0. Checking
B afresh against A rejects the removed function; applying the already-built B
succeeds and deletes `helper`. Applying a clone of B again also succeeds.
Through shared staging, one `HotReloadRuntime` clone advances to v1 while its
sibling remains at v0 and has no pending update left to consume.

The Engine route reaches the same unchecked application after
`prepare_hot_update_state` (`runtime/reload_api.rs:196-222`), so a base check
must precede initializer/state staging as well as publication. This finding
does not establish a bypass in the unified Service controller: Service Delta
composition already checks the expected generation (`service/selection.rs:196`)
and deployment metadata carries base-generation/checksum facts. Preserve those
checks rather than replacing the Service model with ordinary callable slots.

**Action:** bind each update to an exact, validated base identity and reject
stale/replayed updates before effects. Define whether identical-base runtimes
may intentionally share an update; runtime ID, executable generation, and
checksum are alternative/complementary identity components, not automatically
four required fields. Remove split-state `Clone` behavior; the cloneable
staging-only handle already exists. An exclusive `&mut self` owner can perform
compare-and-apply without an additional atomic-pointer mechanism; concurrent
shared publication needs an atomic owner.

### C-03: reflected stable IDs are lossy

**Status: reproduced. Severity: High for the reflection identity API.**

Reflection exposes unsigned 64- and 128-bit stable IDs as signed script
integers by saturating them to `i64::MAX`
(`crates/vela_reflect/src/types.rs:45-65`,
`members.rs:42-50`, `member_records.rs:241-245`, and
`modules/records.rs:34-46,120-127`). This is not a rare overflow corner case:
roughly half of uniformly distributed 64-bit hashes exceed `i64::MAX`, and
almost every 128-bit ID does. Distinct types and members consequently collapse
to the same reflected identity.

Registering `TypeId::new(1_u128 << 100)` and the next ID under different names
produces `ReflectType.id == i64::MAX` for both. The report's original reference
to these type IDs as u64 was too broad: definition `TypeId` is 128-bit, while
some host IDs and schema hashes are 64-bit. Internal registry/linker identities
remain distinct; no compiler dispatch collision follows from this observation.

**Action:** expose an opaque ID value, a tagged pair of unsigned words, or a
canonical hexadecimal string. Round-trip tests must cover the top unsigned bit
and multiple 128-bit IDs.

### C-04: ordinary owned-value egress can recurse forever on cyclic heaps

**Status: reproduced through the public Runtime API. Severity: Critical.**

**Remediation, 2026-09-05:** ordinary export now uses a shared bounded converter
(`crates/vela_vm/src/owned_export.rs`). It rejects active-path cycles, limits
nesting to 64 heap objects and total output to 65,536 values, and charges copied
payload/backing storage against a 16 MiB cap before allocation. Each alias is
charged separately. Immutable shapes and closure artifacts remain shared.
Typed errors distinguish cycles from depth/value/byte limits. The description
below records the reviewed defect before this repair.

Vela heap graphs intentionally support aliases and cycles, but
`value_to_owned_inner` recursively follows every `HeapRef` without tracking the
active path (`crates/vela_vm/src/heap_values.rs:707-829`). `OwnedValue` cannot
represent cycles, so an ordinary host return/materialization through this path
can overflow the Rust stack. Detached-task graph egress has separate cycle-safe
handling; it does not make this converter safe.

The following source compiles and returns successfully under finite call
limits (`10_000` execution units, 1 MiB, depth 64):

```vela
fn main() {
    let values = [];
    values.push(values);
    return values;
}
```

Calling `runtime.value_to_owned(&result)` then terminated the isolated Windows
process with `STATUS_STACK_OVERFLOW` (`0xc00000fd`). CLI and playground output
also call this public converter. This is not limited to raw-heap test fixtures.

The re-review adds an acyclic case: start with an empty array and repeat
`next = [previous, previous]` twelve times. Only 13 arrays exist in the heap,
but conversion constructs 8,191 distinct owned arrays. An active-path cycle
set alone does not bound this expansion. `persistent_value_to_owned` and the
Runtime wrapper accept no egress budget, so finite script execution limits do
not cap this materialization work or its Rust allocations.

**Action:** return a typed cyclic-value/conversion-limit error, or provide a
separate explicit graph export. Bound depth, visited edges, and output bytes
for acyclic graphs too; do not merely add cycle detection. Keep owned-tree
conversion distinct from detached-task graph transport.

### H-01: each script execution call rebuilds generation-wide runtime state

**Status: source-confirmed for ordinary sync/async Vela entry.**

Sync and async runtime calls construct a fresh VM
(`crates/vela_engine/src/runtime/mod.rs:686-725,838-879`).
`Engine::vm_for_artifact` projects reflection state and installs every
function family (`engine.rs:1127-1170`); the task-reflection helper unconditionally
clones its input registry, even when the artifact has no task targets.
Checked-in boundary results show about 285 allocations and 49 KB allocated for
representative static field and collection calls
(`docs/performance.md:156-177`).

Both production call drivers invoke `runtime_vm` in
`runtime/value_support.rs`, which creates this VM. Existing
`SharedGenerationExecutionData` and `RuntimeGenerations` already share cache
and profiling ownership; they do not currently contain a frozen VM dispatch
registry. Same-session re-entry reuses the active VM, and Rust-selected Service
defaults avoid VM entry entirely. The historical 285-allocation measurement is
not proof that every call variant has that cost or that every allocation comes
from metadata construction.

**Action:** build one immutable `GenerationExecutionImage` at compile/reload
time containing linked code, std/native/method dispatch, reflected schema, and
cache layout. Extend the existing generation ownership seam rather than adding
a competing image. A call should create mutable stack/heap/budget/host-session
state and its reflection lookup counters; do not accidentally share a
per-call budget counter when freezing reflection dispatch.

### H-02: runtime artifacts retain compiler-only representations

`LinkedArtifact` retains linked code, `ProgramImage` with unlinked code and a
module graph, and verified MIR (`crates/vela_bytecode/src/artifact.rs:56-69`).
Compiled MIR supports link-time selected-plan/budget verification and advanced
inspection as well as JIT eligibility. Retaining it after linking is a distinct
ownership question: `bind_portable` creates an empty MIR bundle, demonstrating
that the interpreter can execute verified portable plans without the original
MIR. Do not remove the compiled-path proofs or runtime contract types merely
because the JIT milestone is not implemented. The artifact
checksum also clones the full linked program and hashes its Rust `Debug` output
(`artifact.rs:161-181,235-248`), which is costly and not a durable serialization
contract.

**Action:** compact the already-versioned runtime artifact. Put full MIR and
unlinked compiler data in an optional compiler cache/diagnostic sidecar. Define
a canonical fingerprint encoder over explicit fields. Split runtime code
representation from code generation at least at the module/API boundary, and
separate compiler analysis from the MIR contract/value types that VM currently
uses. Moving modules alone does not remove Cargo dependency edges.

### H-03: the frontend uses global mutable graphs for incremental work

`ModuleGraph` owns many parallel maps and global counters
(`crates/vela_hir/src/module_graph.rs:50-93`). Query helpers and dependency closure
frequently scan all bodies/modules. `vela_analysis` clones fact maps and walks
large expression sets during fixed points. However, `AnalysisFactsCache` already
memoizes graph-only and schema-backed facts with `OnceLock<Arc<AnalysisFacts>>`,
and LSP snapshots use `Arc::clone` for their database root. Remaining costs are
whole-graph rebuild/invalidation after edits and copy-on-write when an old
snapshot still holds that root (`global_state/project_state.rs:217,253-256`).

**Action:** give each module/owner an immutable shard with local dense IDs and
explicit reverse indexes. Derive global views from `Arc`-shared shards. Replace
whole-map fixed points with a dependency worklist and cache facts by
`(revision, owner)`. This is a migration, not a new framework rewrite.

### H-04: schema and registry ownership is duplicated

Runtime/compiler type facts have legitimate layer-specific detail, but name,
identity, fields, method signatures, permissions, and docs are re-encoded too
often. Raw reflection registration can silently overwrite some indexes. Engine
already shares its reflection and type-binding registries behind `Arc`; exposing
an authoring registry's mutators is not proof scripts can mutate a live schema.
`DefinitionRegistry::seal_type_bindings` records a binding checksum rather than
promising to freeze the entire definition registry.

**Action:** retain the sealed `TypeBinding` registry as host-type authority and
derive immutable indexed projections once. Definition lookup, Rust codecs,
compiler facts, reflection, and binding exports have legitimately different
jobs; counting their types does not establish overdesign. A future
`FrozenSchema` is a possible consolidation of duplicated identity/metadata, not
an approved fourth authority or a requirement to merge every projection.

### H-05: some fail-open defaults cross capability boundaries

**Remediation, 2026-09-05:** the reflection budget setter no longer enables
reflection. Explicit policy/permissions are required, and the separately stored
budget is applied independent of setter order. Custom receiver access defaults
to Shared; Exclusive requires an override. Generated scoped adapters retain
their precise authority, while MockStateAdapter explicitly admits its mutable
fixture storage. The following description records the original defaults.

**Status: budget-setter enablement reproduced; receiver default source-confirmed.**

`ScriptStateAdapter::host_receiver_access` defaults to exclusive access
(`crates/vela_host/src/adapter.rs:55-66`). A custom adapter that forgets to
override it can unintentionally authorize mutation. Separately,
`EngineBuilder::reflection_lookup_budget` uses
`unwrap_or_default` (`crates/vela_engine/src/builder.rs:281-287`), while the
default reflection policy grants all permissions. Setting only a resource
budget therefore selects a policy permitting reflection calls, private access,
and host mutation. Registered reflect visibility/callability, adapter checks,
leases, and other execution checks still apply. No reflection configuration
leaves Engine reflection disabled; this is not the default for every Engine.

**Action:** make receiver authority explicit or default to unsupported/shared.
Require an explicit reflection policy (the existing `reflection_policy` or
permission setter can provide it); resource-budget setters must not grant
permissions. A new API name is not required to fix this behavior.

### H-07: proc macro parameter hygiene and downstream lint compatibility

Macro signature normalization removes all leading underscores and gives
non-identifier patterns the same `arg` label
(`crates/vela_macros/src/signature.rs:36-52`). The async exporter derives local
identifiers from those labels. However, the `_value`/`value` probe is rejected
by standard Engine construction with `DuplicateNativeFunctionParamName` before
execution. The original unqualified claim of silent wrong execution is
withdrawn for that path. Earlier, span-local macro validation and positional
internal bindings remain useful ergonomics and defense in depth. Other
signature/adapter combinations need their own proofs.

Service expansion unconditionally emits `#[allow(unsafe_code)]`
(`crates/vela_macros/src/service.rs:255,286`). Even a scalar-only service trait
under `#![forbid(unsafe_code)]` fails with E0453. Certain non-static host
dispatch arms actually need unsafe erased reborrowing; this is a downstream
lint-compatibility defect, not evidence those operations are unsound.

**Action:** use positional hygienic internal identifiers, validate unique public
labels, and reject unsupported patterns with a span error. Avoid unconditional
lint overrides in safe expansions. For borrowed host dispatch, investigate a
safe library-owned invocation abstraction with enforced provenance/lifetime
invariants. Merely moving an unchecked generic pointer cast into a public safe
function would be unsound and is not an acceptable fix.

### H-08: language-server concurrency and URI boundaries do not match the model

Several handlers are labelled latency-sensitive or worker work but call the
synchronous dispatcher directly. Most cancellation is checked only after work,
and each actual lane is a single unbounded queue. `didChange` can update
databases and publish diagnostics synchronously. The language-service project
layer also performs filesystem operations despite the architecture contract,
and both service and server manually strip `file://` rather than correctly
handling percent-encoding, Unicode, or UNC paths.

Specifically, hover/signature-help and non-retryable worker-labelled requests
call `dispatch_snapshot_messages_typed` synchronously. Completion, completion
resolve, full semantic tokens, selected retryable workspace queries, and
formatting do use task lanes. The absence of cooperative cancellation inside
many queries must not be conflated with an absence of background execution.

**Action:** keep all I/O and URI conversion in `vela_lsp_server`, using the URL
library's file-path conversion. Use bounded/coalescing queues, a real worker
pool, and cooperative cancellation tokens inside queries. The language service
should accept immutable text/path snapshots only.

### M-01: hot-path collection accounting rescans complete maps and sets

Map/set mutations reaching `adjust_object_size_after_mutation` recompute
shallow size by scanning all entries
(`crates/vela_vm/src/script_map.rs:179-187`,
`script_set.rs:105-113`, and `heap.rs:521-565`). Repeated growing insertions on
this route have quadratic aggregate accounting work. This is a source-level
complexity finding, not a claim that every map/set operation or every fast path
is quadratic; finite limits bound the size but do not remove that cost.

**Action:** maintain capacity and payload deltas incrementally, with occasional
debug/test recomputation to verify accounting.

### M-02: host calls allocate and clone arguments in multiple layers

The VM materializes `Vec<HostValue>`; `HostAccess` converts/clones it into
another `Vec<HostCallValue>`; `PathProxy` builds another vector for each
operation and panics when adding a 257th dynamic argument
(`crates/vela_vm/src/host_access.rs:443-471`,
`crates/vela_host/src/access.rs:355-370`, and
`proxy.rs:218-225`).

These are specific `HostValue`/proxy routes, not a count for every direct typed
Host/Service call. The overflow is in the public Rust proxy builder; source
compiler path limits must be checked separately before claiming an ordinary
script can trigger that panic. Existing scoped/borrowed adapters must retain
lease and lifetime validation when reducing conversions.

**Action:** convert once into borrowed/consuming call values, use
`SmallVec` for the common proxy case, and make argument overflow fallible.

### M-03: public surface area exceeds the useful embedding surface

The engine provides a good high-level `Runtime` API, but raw VM entry structs,
cache layouts, runtime image storage generics, compiler databases, and numerous
parallel DTOs are also public. This increases documentation burden and makes
semver stabilization harder. The project currently has very little top-level
rustdoc relative to that surface.

**Action:** define supported facade modules and move implementation structures
behind `pub(crate)` where no cross-crate consumer needs them, or an explicitly
unstable compiler/advanced namespace otherwise. Enable
`missing_docs` on the facade first and add downstream compile tests for the
intended embedding path.

## Crate-by-crate review

### `vela_common`

**Assessment:** appropriately small and mostly cohesive. It is a good home for
source identity, spans, diagnostics, and deliberately universal utilities.

Strengths:

- source/span types are simple value objects;
- the crate avoids becoming a generic dumping ground;
- deterministic hashing and diagnostic primitives are reusable across stages.

Findings and recommendations:

- `SymbolInterner` is unused by production consumers
  (`crates/vela_common/src/lib.rs:152`), while the rest of the workspace owns
  many repeated strings. Either adopt it through a canonical symbol type or
  remove it; an unused interner is abstraction without leverage.
- stable IDs, shape IDs, and service IDs use separate ad-hoc hash encodings.
  Centralize a streaming, domain-separated stable-hash writer so callers cannot
  accidentally hash ambiguous byte sequences.
- diagnostic rendering scans the source prefix to count lines and allocates
  displayed line strings (`diagnostic_render.rs:146-169`). Reusing a line index
  could reduce repeated rendering cost; this function does not itself build a
  complete line-start table, as the original wording suggested.
- `vela_common` directly uses `vela_def::TypeId` and `FunctionId` in its
  interop contracts as well as re-exporting `stable_id`
  (`interop_type.rs:3-88`). This dependency direction is valid: `vela_def` is a
  leaf identity crate depending only on BLAKE3 and optional serde. The original
  recommendation to invert this edge is withdrawn. Keep higher-level host and
  compiler behavior out of both foundational crates.

### `vela_def`

**Assessment:** a strong identity layer with a few avoidable allocations and too
much representational freedom.

Strengths:

- `DefPath` uses content identity rather than allocation/order identity;
- BLAKE3-128 is an appropriate collision-resistant basis for durable definition
  IDs;
- semantic keys give compiler and runtime components a shared vocabulary.

Findings and recommendations:

- several IDs are built through temporary formatted strings
  (`crates/vela_def/src/script.rs:26-69`), and `DefPath::id` assembles temporary
  byte vectors. Feed typed components directly into a domain-separated encoder.
- `DefPath` has public string/path fields, while typed IDs already have private
  storage and explicit constructors. Validate external paths and collisions at
  ingestion/registration boundaries; do not prohibit constructing stable IDs
  needed for schemas, serialized artifacts, and collision tests merely to make
  every intermediate DTO private.
- names and path segments are commonly owned. Once a workspace-wide symbol
  policy exists, use interned/`Arc<str>` components while keeping serialized
  identity independent of process-local interning.

### `vela_package`

**Assessment:** compact, deterministic, and well tested; path normalization is
the principal correctness/API concern.

Strengths:

- deterministic module ordering, root authorization, canonical filesystem
  checks, and cycle detection are explicit;
- loader behavior is separate enough to support future virtual sources;
- package tests cover important architecture rules.

Findings and recommendations:

- `ModulePath` silently removes empty components
  (`crates/vela_package/src/identity.rs:75-102`), so `a::::b` can normalize to
  `a::b`; the external probe confirms this. Keep a permissive internal path
  representation if useful, but validate authored package/import paths at their
  input boundary with a fallible parser. This observation alone does not show
  that source import syntax or filesystem authorization accepts that spelling.
- filesystem-derived segments and language module identifiers need one canonical
  validation rule. Do not accept a disk path that could not be written as a
  module path.
- `PackageSource` owns `String` data and is deeply cloned through some compiler
  paths. Prefer immutable `Arc<str>` source snapshots and a loader/VFS trait at
  the adapter boundary.
- version parsing is intentionally loose. Before packages become externally
  resolved, define whether versions are opaque labels or semantic versions
  rather than partially supporting both.

### `vela_syntax`

**Assessment:** the Rowan foundation is mature, but expression handling layers a
second parser over the CST and can repeat both storage and work.

Strengths:

- the lossless tree is the right basis for formatting, refactoring, and robust
  editor recovery;
- lexer/parser diagnostics preserve source ranges;
- recovery and grammar tests are broad for the current language.

Findings and recommendations:

- token semantic values own strings/collections while Rowan stores the source
  text again (`crates/vela_syntax/src/lexer.rs:10-14` and
  `token.rs:12-30`). This is largely transient compile-time memory, but it
  amplifies large-file parsing and editor snapshots. Store ranges/kinds in the
  lexer and decode literal values on demand or once into an AST arena.
- interpolation is scanned again; the nested CST helper uses `SourceId(0)` and
  does not merge its lexer/parser diagnostics
  (`crates/vela_syntax/src/cst_parser/cst_expr.rs:224-231`). Audit diagnostic
  ownership across the outer lexer and parser before adding propagation: this
  local omission is source-confirmed, but it is not yet a reproduced missing
  user diagnostic or an incorrect emitted source span.
- expression parsing repeatedly rescans subranges to find operators
  (`crates/vela_syntax/src/cst_parser/cst_expr.rs:10-88,787-815`), which can
  become quadratic for long expressions. A Pratt/event parser over one token
  stream would be simpler and linear.
- literal AST construction re-lexes text. Generate typed AST accessors over the
  CST and share literal decoding rather than maintaining parallel parsing logic.

This does not require abandoning Rowan. The simplification is one lexical pass,
one expression parse, and typed views over the resulting tree.

### `vela_hir`

**Assessment:** semantically rich and testable, but the monolithic global graph
is the largest obstacle to genuinely incremental compilation.

Strengths:

- explicit IDs and side tables make relationships inspectable;
- lowering keeps source spans and ownership information needed by diagnostics;
- executable-root and dependency concepts are present rather than inferred
  ad hoc downstream.

Findings and recommendations:

- `ModuleGraph` contains many independent maps and thirteen global counters
  (`crates/vela_hir/src/module_graph.rs:50-93`). A small body edit therefore
  interacts with workspace-global allocation/order state.
- counters use saturating increment (`module_graph.rs:926-940`), which silently
  aliases IDs at exhaustion. Use checked allocation and a typed error; silent
  identity collision is never a valid recovery.
- many IDs are public wrappers around `u32` with no arena ownership encoded.
  Prefer `(owner, local index)` IDs or generational arena keys.
- common queries scan all bodies, and reverse dependency closure scans all
  modules
  (`crates/vela_hir/src/module_graph/queries.rs:194-255,564-583`). Build
  owner/body and reverse dependency indexes once.
- ordered maps are used even for dense local entities. Typed `Vec` arenas are
  simpler and faster when IDs are allocated densely.

Recommended evolution: lower each module to an immutable `HirModuleShard` with
local arenas and fingerprints; compose a workspace index from shared shards.
This matches hot reload well because unchanged modules and their IDs naturally
survive a revision.

### `vela_registry`

**Assessment:** deterministic registration and collision validation form a
coherent definition layer. Its binding-checksum seal and overall mutability
should be described separately.

Strengths:

- typed `BTreeMap` indexes by ID, path, semantic key, and primitive tag are
  deterministic;
- definition registration checks collisions before mutating indexes
  (`crates/vela_registry/src/lib.rs:94-155`);
- `RegistryCompileView` gives compiler consumers a borrowed read view.

Findings and recommendations:

- `seal_type_bindings` records a checksum and asserts against repeated sealing
  (`lib.rs:87-91`); it does not claim to freeze all definition insertion. The
  production caller in `vela_engine/src/compiler_registry.rs:62` supplies the
  already-sealed host binding checksum. A fallible repeated-seal API could
  improve low-level misuse diagnostics, but this assertion is not a reproduced
  production failure or proof that Engine's host bindings remain mutable.
- module-root, host-field, runtime-method, and native-source queries may scan all
  definitions or allocate temporary keys (`lib.rs:242-296,420-472`).
- debug names are stored as both `Vec<String>` and owned map keys
  (`lib.rs:506-542`).

A builder-to-frozen representation is a possible API improvement after the
mutation phases and consumers are mapped. More immediately, build the missing
reverse indexes and share immutable compile views. Do not introduce a second
sealing authority or change valid construction phases solely because the
checksum setter is named `seal_type_bindings`.

### `vela_reflect`

**Assessment:** the permission model and metadata are valuable, but ID
serialization and mutable registry semantics need immediate repair.

Strengths:

- permissions distinguish metadata, private access, host reads/writes, and calls;
- reflection can inspect and perform controlled operations without mutating type
  structure;
- docs, spans, effects, origin, and script/host distinctions make metadata useful
  to both tools and scripts.

Findings and recommendations:

- reflected stable IDs collapse through saturating signed conversion; see C-03.
- function/state registration overwrites ID entries while retaining old name
  mappings (`crates/vela_reflect/src/registry.rs:777-812`). Replacing function ID
  1 named `Old` with ID 1 named `New` makes `function_by_name("Old")` return the
  `New` descriptor. Type registration instead indexes `(id, name)` and accepts
  two descriptors with one ID. Define replacement versus collision semantics
  per registry, validate atomically, and avoid stale secondary indexes. The
  sealed definition/type-binding registration path has additional checks;
  this does not prove every Engine registration can create such corruption.
- trait descriptors are embedded in type descriptors and also stored in a global
  map (`registry.rs:175-190,704-717`); lookup merges/deduplicates them on demand.
  Store each trait once and reference its ID.
- the lookup budget counts API calls, not the size of `reflect::types()`'s
  complete descriptor projection. The one-lookup probe returns a schema
  successfully. Account for traversal and temporary descriptor allocations as
  well as the VM's eventual result allocation; this is a gap in work accounting,
  not evidence that every returned byte escapes the VM memory checks.
- runtime reflection depends on HIR/package/syntax projection
  (`script_types.rs:1-18`). Emit a frozen runtime reflection table during
  linking, leaving compiler projection outside the runtime crate.

### `vela_analysis`

**Assessment:** the semantics are explicit and well tested; execution is closer
to repeated batch analysis than an incremental query engine.

Strengths:

- facts are typed and separated from HIR ownership;
- executable roots, capability/effect checks, and narrowing are represented
  directly;
- the implementation favors deterministic results over hidden global state.

Findings and recommendations:

- `HirSemanticFacts` is a collection of parallel maps
  (`crates/vela_analysis/src/semantic_facts.rs:59-77`). Fixed-point passes clone
  maps and repeatedly walk expression sets (`semantic_facts.rs:124-185`).
- executable closure and registry-fact construction scan broad graph regions
  even when a single owner changed.
- recursive `TypeFact` values allocate and deep-compare unions linearly.
- `RegistryFacts` mirrors registry data using owned string keys.
- normal dependencies on package/syntax appear to serve mostly tests or
  adapters; keep the semantic core at the HIR/schema boundary.

Use a worklist keyed by owner/expression with explicit dependency edges where
profiles justify it. The language service already caches generation-level base
facts; extend reuse across changed owners/revisions instead of adding a second
whole-generation cache. Intern structural type facts only with evidence that
comparison/allocation costs justify the additional ownership mechanism.

### `vela_mir`

**Assessment:** one of the strongest crates in the repository. Its main issue is
duplicated validation/dataflow work and a wider public builder surface than
embedders need.

Strengths:

- effects, guards, safepoints, ownership, and type contracts are explicit;
- typed arenas and a sealed verifier make invalid execution input difficult to
  construct accidentally;
- verification tests cover control flow, initialization, contracts, and failure
  diagnostics comprehensively.

Findings and recommendations:

- builder-emitted liveness is independently recomputed and compared by the
  verifier; that is an intentional correctness boundary, not redundant proof to
  remove. After successful verification, `verify_owned_mir` recomputes facts,
  CFG, and sealed analyses (`crates/vela_mir/src/verifier/mod.rs:394-438`).
  Returning the verifier's independently derived analyses for sealing is a
  potential optimization; trusting builder-provided analyses is not.
- `CompileTargetSnapshot` is another broad parallel-map DTO. Prefer a view over
  frozen HIR/schema data with only MIR-specific derived facts.
- the implementation `builder` module is already private and `build_mir` is a
  public compiler seam. Cross-crate lowering inputs and model constructors are
  also needed by the backend and corruption tests. Narrow/document supported
  compiler and advanced APIs rather than making all of them `pub(crate)` and
  breaking the existing crate boundary. Production VM entry still requires a
  verified linked artifact.
- nested-function/capture lookup scans or clones broader maps than necessary.
  Add owner and capture indexes.
- JIT eligibility is public and retained even though there is no JIT. Keep the
  analysis compiler-side/optional until the JIT milestone exists; do not retain
  all MIR generations for it.

### `vela_bytecode`

**Assessment:** verification and symbolic-to-linked separation are excellent;
the crate currently combines compiler backend, runtime code format, and retained
compiler artifact responsibilities.

Strengths:

- the compiler accepts semantic/MIR input rather than parsing source;
- symbolic operands are resolved into stable linked handles;
- verification protects the interpreter from malformed control flow and
  operands;
- portable artifacts demonstrate a useful serialization boundary.

Findings and recommendations:

- `LinkedArtifact` and its dependency cone keep compiler representations in the
  runtime; see H-02.
- unlinked and linked instruction enums have parallel opcode families. A
  declarative specification could generate operand structure, exhaustive
  dispatch scaffolding, and metadata. Preserve independent semantic verifier
  checks against source/MIR and malformed-input tests; generating both producer
  and oracle from identical logic would hide common-mode bugs.
- semantic preparation clones builder/probe placements and constructs validated
  lowering inputs that are later requested again
  (`crates/vela_bytecode/src/compiler/semantic_input/mod.rs:158-174,294-308`).
  Make one owned preparation result and consume it once.
- sorted `insert_function` calls rebuild indexes repeatedly
  (`lib.rs:111-139,296-322`), yielding at least quadratic aggregate traversal
  plus ordered-index maintenance. Accumulate,
  stable-sort once, validate once, then freeze.
- linked method lookup constructs owned strings
  (`linked.rs:181-189`). Index typed owner/name IDs and support borrowed probes.

Separate runtime representation from codegen ownership first. An internal
module split helps reviewability but cannot cut normal Cargo dependencies;
removing those edges requires a crate or feature boundary. Do not create a new
codegen crate solely to satisfy a diagram without measuring build/retention
benefits and preserving portable verification.

### `vela_host`

**Assessment:** the host ownership model is a distinctive strength. The
implementation should keep that model while presenting fewer layers to
embedders.

Strengths:

- `HostSlotTable` invalidates aliases through generations before slot reuse
  (`crates/vela_host/src/slot.rs:7-16,108-150`);
- `HostRef`, `HostSlotRef`, and `HostPath` express identity/path rather than
  leaking Rust references (`path.rs:11-69`);
- reads, writes, mutations, and calls converge on `HostAccess`, making the
  capability boundary auditable;
- workspace unsafe-boundary tests constrain erased reborrowing and slice
  operations to reviewed modules.

Findings and recommendations:

- receiver authority fails open by default; see H-05.
- `ScriptStateAdapter` spans schema, interning, external state, storage,
  leases, scoped values, collections, reads, writes, mutation, and calls
  (`adapter.rs:55-284`). `ScriptHostObject` mirrors much of it. Keep one sealed
  low-level adapter boundary, but move optional capability decomposition behind
  internal/advanced APIs and give ordinary users a small derive-driven facade.
- `HostAccess` is zero-sized and obtains authority from the adapter, resolved
  target, and active lease/session. Its mutable parameter is API machinery, not
  evidence that authority is absent. Hide it behind ordinary call facades where
  possible; do not invent duplicate mutable authority state to justify a ZST.
- host argument conversion clones structural values through multiple vectors;
  see M-02.
- `HostPathParts` in `target.rs` implements empty/one/two/three/four/heap storage
  while the crate also uses `SmallVec`. Reuse could simplify code, but compare
  plan size, clone/hash cost, and allocation profiles first. Existing storage
  tests and specialized ownership may justify retaining it; no regression from
  this representation was reproduced.

The desired ordinary Rust path should remain: register a host type, pass a
borrowed host object in `CallArgs`, and let Vela write through `player.level +=
1`. Handles, leases, prepared operations, and slot generations should normally
remain implementation detail.

### `vela_stdlib`

**Assessment:** a backend-neutral semantic manifest is the right design; the
manifest is not yet the single source of truth it claims to be.

Strengths:

- standard identities derive from the same durable `DefPath` rules as user code;
- registration goes through the validated definition registry;
- consistency tests expose drift rather than allowing it silently.

Findings and recommendations:

- method metadata is authored in manifest/method files, translated to another
  engine `MethodSpec`, and repeated in per-type engine tables
  (`crates/vela_stdlib/src/manifest.rs:92-125` and
  `crates/vela_engine/src/standard/methods`).
- stable-ID helpers scan static manifests
  (`crates/vela_stdlib/src/ids.rs:6-42`), including paths reachable during
  dynamic method resolution.
- names, signatures, docs, IDs, and runtime operation identity therefore have
  multiple owners.

Generate the semantic descriptor, stable ID, documentation record, compiler
entry, and typed runtime operation tag from one declarative table. Use generated
matches/static indexes instead of manifest scans.

### `vela_stdlib_runtime`

**Assessment:** the dependency seam is reasonable, but almost the entire crate
is a manually synchronized mapping that should be generated.

Strengths:

- it prevents the semantic stdlib crate from depending on VM function pointer
  types;
- tests verify that every declared standard function has an implementation.

Findings and recommendations:

- function identity is mapped from manifest path to an enum and again from enum
  to VM function pointer (`crates/vela_stdlib_runtime/src/lib.rs:12-125` and
  `crates/vela_vm/src/stdlib.rs:20-78`).
- binding creation allocates a new vector and formatted debug names each time
  (`lib.rs:142-161`). The current per-call VM reconstruction places this on the
  execution path.
- `StdMethodRuntimeBinding` stores untyped owner/name strings, while production
  dispatch uses a separate large `StdMethodIds` mapping. Repository production
  code does not appear to consume the method binding list.

Generate this seam from the canonical stdlib table. Return a static slice or
`OnceLock` data, and use a typed operation tag for methods rather than keeping
an unused second binding model.

### `vela_vm`

**Assessment:** the interpreter has good representations and unusually strong
behavioral coverage, but its collector currently contains the audit's most
serious correctness defect.

Strengths:

- `Value` is intentionally 16 bytes and has a size regression test
  (`crates/vela_vm/src/value.rs:10-28,105-120`);
- ordered maps/sets preserve determinism and use borrowed probes;
- stable linked IDs and per-generation inline-cache sidecars are good
  interpreter architecture;
- budgets, safepoints, host access, reflection, and reload behavior have broad
  tests;
- unsafe scalar/access code is localized and audited rather than spread through
  instruction handlers.

Findings and recommendations:

- opt-in finite-step collection violates liveness and its time budget is inert;
  default safe-point collection is atomic; see C-01.
- cyclic heap materialization can recursively overflow; see C-04.
- finite-budget map/set mutation is quadratic; see M-01.
- host call conversion performs redundant allocation/cloning; see M-02.
- the public execution API has three large call structs, many lifetime
  parameters, near-duplicate run variants, and public cache-layout internals
  (`crates/vela_vm/src/lib.rs:460-634,728-758,1045-1247`).
- `HeapValue::Enum` stores owned enum and variant names on every instance in
  addition to stable identity/shape metadata (`heap.rs:61-65`). Put names in a
  shared shape/type descriptor.
- `linked_execution.rs`, `runtime_type_guards.rs`, and method-call handlers are
  legitimate file-size pressure points: each mixes multiple instruction-family
  semantics and makes local reasoning harder.

After the GC fix, collapse entry points around one internal
`ExecutionRequest`/`ExecutionSession` with a few facade conveniences. Split
instruction-family implementation modules, but do not introduce an abstraction
per opcode; generated dispatch metadata plus cohesive semantic helpers is the
simpler boundary.

### `vela_hot_reload`

**Assessment:** immutable version ownership and staged activation are strong;
the missing base-generation token undermines the central product promise.

Strengths:

- a `ProgramVersion` owns a complete immutable linked artifact and ABI through
  `Arc` (`crates/vela_hot_reload/src/version.rs:25-42`);
- package, state, function, module, and full ABI compatibility are checked before
  an update is produced;
- staging and activation are separate, so publication can occur at a safe point;
- active callers can pin old generations naturally.

Findings and recommendations:

- updates lack origin identity and cloneable runtimes can split current state;
  see C-02.
- `ProgramVersion::function` and script-method variants clone complete unlinked
  code objects into fresh `Arc` values (`version.rs:45-107`). Return a lightweight
  handle that pins/borrows the artifact.
- update comparison builds a full cloned function map when it later needs
  principally a name set (`compile.rs:63-99`).
- profile queries rebuild profile vectors, and each function profile stores
  every contiguous instruction offset only to expose membership/count
  (`profile.rs:43-69`). Cache compact range/layout data per generation.

The activation operation should compare an explicitly defined base identity:

`apply(update) succeeds only if current_base_identity == update.expected_base`.

That invariant should be tested with two updates built from one base, two
runtimes, and staged updates observed by multiple handles. Decide whether
identical-base cross-runtime fan-out is supported; mismatched bases and replay
must reject. The current whole-Service model already has its own publication
checks and remains the sole Rust hotfix model.

### `vela_engine`

**Assessment:** the best public embedding API in the workspace, backed by an
internal object graph that is too expensive to reconstruct and too easy to
misconfigure.

Strengths:

- `Runtime::call`/`call_async`, `CallArgs`, and `CallOptions` are substantially
  simpler than raw VM entry points;
- durable function/method handles validate runtime and version identity
  (`crates/vela_engine/src/runtime/mod.rs:235-307,478-506`);
- generation cache/profile data lives outside immutable code;
- reload stages host state before publishing the generation;
- Rust mutation continues to route through `ExecutionHost`/`HostAccess`.

Findings and recommendations:

- VM dispatch/reflection is rebuilt for every call; see H-01.
- setting only a reflection budget grants the default all-powerful policy; see
  H-05.
- `Engine` derives `Clone` while many native maps are direct owned fields
  (`engine.rs:42-67`). Make it a cheap `Arc<EngineInner>` after construction.
- the engine retains definition, reflection, and type-binding stores
  (`engine.rs:42-46`). Their layer-specific duties are valid; reduce repeated
  metadata construction and share immutable projections while keeping one
  authoritative host-type registration model.
- `RuntimeImage`, `OwnedImage`, `SharedImage`,
  `RuntimeImageStorage`, and `RuntimeImpl<I>` mainly encode inline versus `Arc`
  storage (`runtime/image.rs:12-73`). Consider hiding these storage policies from
  ordinary embedders. A single shared image might simplify internals, but
  equivalent performance is an unmeasured hypothesis; retain the accepted
  ownership/lifetime guarantees and benchmark both forms before removing one.
- compiler registry conversion reparses/copies type-hint strings between several
  models. Reference canonical schema/type-expression IDs instead.
- optional schema artifact support depends on the full language-service crate
  and returns its DTOs. Put neutral schema serialization beside the frozen
  schema, not behind an editor-service dependency.

The public end state should be one immutable `Engine` configuration, one
generation-pinned `Runtime`, a small `CallArgs` builder, and explicit advanced
hooks. Budgets and host borrows belong to a call session; dispatch and metadata
belong to the generation.

### `vela_macros`

**Assessment:** the derives make an otherwise sophisticated host model usable,
but macro hygiene, downstream unsafe compatibility, and default export policy
need tightening.

Strengths:

- derives turn Rust types and services into schema plus runtime bindings rather
  than relying on runtime introspection;
- compile-time diagnostics and fixtures cover many unsupported Rust shapes;
- generated stable identities integrate with the central registry model.

Findings and recommendations:

- argument normalization can collide; standard Engine construction rejects the
  demonstrated duplicate labels. Move that diagnostic closer to the authored
  parameter and use positional internal names; see H-07.
- generated service/dispatch code contains unsafe blocks annotated with
  `#[allow(unsafe_code)]`. A downstream `#![forbid(unsafe_code)]` cannot be
  overridden, so valid consumers fail to compile, even for scalar-only Services.
  Remove unnecessary overrides and design any library-owned erased-reborrow
  abstraction around enforced lifetime/provenance invariants; see H-07.
- expansion hard-codes paths such as `::vela_engine`, `::vela_host`,
  `::vela_reflect`, and `::vela_vm`. Consumers must directly depend on all
  internal crates used by that particular expansion under exact names. A simple
  export and a borrowed Service do not require the same set. Emit through one documented
  `vela_engine::__private`/facade path and resolve renamed crates with
  `proc_macro_crate` or an explicit crate option.
- `#[methods]` can expose representable private instance methods unless
  `public_only` or method-level `skip` is selected. Applying the macro and
  registering its bindings are themselves explicit opt-ins; Rust visibility is
  not the same contract as script visibility. Document this choice and consider
  a public-only default, but do not call it an established capability bypass.
- type classification uses only the final path segment, so user-defined
  `my::Vec`/`Result`-named types can be mistaken for standard containers or
  context types. Accept only known canonical paths or require an annotation.
- generated patched service adapters panic on conversion, capability, VM, or
  cancellation failures. Generate fallible request APIs and reserve panic for
  proven internal invariants.

### `vela_bindgen`

**Assessment:** deterministic schema-only generation is a clean boundary, but
name validation must precede rendering.

Strengths:

- code generation consumes schema rather than a live runtime/compiler;
- output ordering is deterministic;
- host-facing generated types make Vela interfaces discoverable from Rust.

Findings and recommendations:

- normalization is not validated consistently for record fields, variants, and
  parameters. `pub struct Item { Foo: i64, foo: i64 }` exposed by a typed public
  function successfully generates two `pub foo: i64` fields; compiling the
  generated consumer fails with E0124/E0062. This is a reproduced API defect.
- module, callable, and top-level generated type collisions are already checked
  by `collect_modules`/`validate_types`. Type validation also runs before
  rendering, so passing a fresh diagnostic vector while rendering an already
  validated type does not alone prove a lost type error. Extend the existing
  validation to field/variant/parameter namespaces and invalid raw identifiers.
- generated accessors expose long internal root-module names, which are correct
  but not pleasant as an application API.
- Rust bindgen consumes compiler-owned `vela_bytecode::RustBindingSchema`, not
  language-service DTOs. The coupling is to the broad bytecode crate, which
  brings compiler dependencies. Move the binding schema to a smaller neutral
  boundary only as part of a justified dependency split; do not duplicate it.

Build a `RustNamingPlan` that validates keywords, raw identifiers, normalization
collisions, namespaces, and stable disambiguation before rendering. Once that
plan is accepted, rendering should be infallible. Generate a nested module
facade or concise aliases for common access.

### `vela_bindgen_compile_test`

**Assessment:** a valuable end-to-end fixture rather than a reusable crate. It
should remain unpublished and be used to protect the intended consumer
experience.

Strengths:

- generated code is actually compiled and run;
- the fixture exercises registration, execution, and reload rather than merely
  snapshotting text;
- it catches dependency and macro expansion assumptions that unit tests miss.

Findings and recommendations:

- `build.rs` duplicates application exports and engine registration;
- normal and build dependency graphs repeat much of the workspace;
- it does not yet cover dependency renaming, `#![forbid(unsafe_code)]`,
  normalized-name collision, or minimal-facade consumption.

Generate schema once from a small host-schema fixture/artifact. Add compile cases
for those four consumer constraints, keeping the crate `publish = false`.

### `vela_language_service`

**Assessment:** feature coverage is impressive and database snapshots already
share immutable state. The remaining ownership problem is reuse across edits,
especially while background snapshots remain live.

Strengths:

- it has no direct LSP protocol dependency, which is the correct reusable
  boundary;
- typed editor DTOs, query contexts, fingerprints, cancellation hooks, and
  caches are already present;
- completion, hover, references, rename, diagnostics, actions, and formatting
  have broad tests.

Findings and recommendations:

- source text is copied from workspace `Arc<str>` to another `Arc`, then
  `ModuleSource::String`, then back into an `Arc`
  (`crates/vela_language_service/src/project.rs:300-366` and
  `incremental.rs:1011-1042`).
- `ParseDb` begins updates by cloning the parse record map, HIR commonly rebuilds
  the full graph, and `LanguageServiceDatabases` deep-clones under
  `Arc::make_mut`. A concurrent snapshot can turn a small edit into a graph copy.
- the project layer performs `load`, `exists`, and `canonicalize` filesystem
  operations (`project.rs:17-22,418-432,575-580`), contrary to
  `docs/architecture/lsp.md`'s stated I/O boundary.
- disk snapshot state is duplicated between workspace and LSP ownership.
- cursor recovery reparses raw strings and duplicates lexer/CST logic; lambda
  parsing uses `find`/`split`, and code actions infer fixes by parsing diagnostic
  prose/backticks.
- the public library re-exports low-level databases and implementation records,
  making future incremental changes a compatibility problem.

Use one immutable `SourceSnapshot { path, text: Arc<str>, revision }` supplied by
the adapter. Reuse CST tokens/typed AST for cursor recovery. Attach structured
repair data to diagnostics. Expose a narrow `LanguageServiceSnapshot` facade
while keeping databases internal. The HIR shard/worklist changes in H-03 should
then make edits cheaper without losing the existing cheap database-root snapshot
and per-generation analysis-fact cache.

### `vela_lsp_server`

**Assessment:** protocol typing and test breadth are good; scheduling labels
currently promise concurrency/cancellation that most handlers do not receive.

Strengths:

- protocol conversion is isolated from the language-service crate;
- stale result suppression, typed request/notification routing, and loopback TCP
  support are well tested;
- retryable/background task concepts provide a base for real scheduling.

Findings and recommendations:

- non-retryable latency-sensitive and worker dispatch functions call the synchronous
  dispatcher directly; retryable completion and several other requests use lanes
  (`crates/vela_lsp_server/src/handlers/dispatch.rs:71-123,387-421,482-507`).
- normal completion is mostly non-cancellable; checks after a result do not save
  the work.
- each real lane is one thread fed by an unbounded queue
  (`task.rs:521-526,771-784`). Rapid edits can build stale work and memory.
- `didChange` synchronously mutates databases and computes/publishes
  diagnostics (`global_state.rs:1099-1136`).
- manual `file://` stripping exists in server and language-service code
  (`paths.rs:6-27`), mishandling encoded spaces, Unicode, percent signs, Windows
  drive/UNC paths, and non-file URIs.

Move edits into a revisioned queue, coalesce superseded document work, and use a
bounded worker pool. Thread cancellation tokens into query loops. Use
`lsp_types::Url`/URL-library file conversion in the server and pass normalized
paths to the service. Add cross-platform URI round-trip tests.

### `vela_cli`

**Assessment:** a useful demo runner, not yet a stable command-line product.

Strengths:

- the canonical run path applies finite execution budgets;
- filesystem access goes through `FsSandbox`;
- deterministic time/random defaults support reproducible scripts;
- synchronous and asynchronous execution are both exercised.

Findings and recommendations:

- the interface is essentially a positional script plus `--async` and
  `--print-schema`; filesystem read/write is enabled by default.
- `--print-schema` still requires a script argument but exits before compiling
  it, so it prints only default-engine schema.
- values use internal `Debug` formatting rather than a stable text/JSON result.
- diagnostic rendering rereads only the entry source and assumes a single source
  identity, so multi-module diagnostics can be rendered against the wrong text.
- the crate has a direct language-service dependency that appears unnecessary
  for the run path.

Introduce explicit `run`, `check`, `schema`, and eventually `bindgen`
subcommands. Make host capabilities opt-in or visible in a config flag. Use one
source map for diagnostics and a versioned machine-readable output option. Drop
unused high-level dependencies.

### `vela_playground_wasm`

**Assessment:** functional and deterministic. Its 11.5 MB raw
release baseline and synchronous compile/run model justify a measured
browser-size/startup budget before the playground is treated as polished.

Strengths:

- execution uses finite budgets and controlled time/random behavior;
- the crate builds successfully for `wasm32-unknown-unknown`;
- JSON provides an accessible browser boundary.

Findings and recommendations:

- it links the full engine/compiler/bytecode/VM stack with no playground-focused
  feature profile. The reviewed release artifact was 11,536,532 bytes before
  `wasm-opt` or compression.
- a new engine is constructed for every operation. Choosing Compile and then
  Run compiles the same source twice because no artifact is cached between
  actions; a single Run action does not itself perform two compilations.
- the checked-in `site/src/components/Playground.astro:209-220` calls the
  synchronous exports on the browser thread, with no source-size or compilation
  work limit. The WASM crate could be hosted in a worker; the existing UI does
  not do so. Execution and initialization do have finite runtime budgets.
- `compile_script` constructs a Runtime and therefore evaluates script-state
  initializers before reporting success (`lib.rs:99-107`). This is broader than
  parsing/linking alone. Either describe the operation as compile-and-initialize
  or validate the artifact without initializing it.
- JSON conversion is not type preserving: unit becomes the string `"()"` and
  integer JSON text is parsed with ordinary `JSON.parse` in the UI, which loses
  precision outside JavaScript's safe-integer range. The loss is at the consumer
  numeric representation, not necessarily serde's emitted decimal text.
- compiler diagnostic projection is duplicated with CLI logic.

Provide a persistent worker-owned session keyed by source/options fingerprint.
Add source-size and compile-work limits, reuse compiled artifacts, define a
tagged/versioned JSON value schema, and add an explicit optimized-size budget in
CI. A minimal feature profile should omit server-only host, reflection, schema,
and service facilities not used by the playground.

## Rust/Vela interoperation

The fundamental model fits host-owned game-server state. Script code uses
ordinary member access and mutation, while Rust retains ownership and mediates
effects through stable handles and paths. This makes lease and generation rules
explicit without exposing Rust references to scripts. It is not a comparative
proof that safely scoped Rust closures in other embedding designs are unsound.

The current friction comes from exposing a linear-resource protocol:

- child leases must sometimes be released before parent leases;
- `host::release` must be written before an `await` when a resource is live;
- adapter implementers must understand receiver access, scope, slots, prepared
  paths, and several value representations;
- generated service patch paths may panic instead of propagating conversion,
  VM, capability, or cancellation errors.

Recommendations:

1. preserve authored `host::release` / `host::try_release`, terminal Service
   transfer, and root teardown. Improve diagnostics and examples for releasing
   children before parents and releasing retained leases before `await`;
2. make common host registration derive one schema plus one adapter and surface
   compile-time errors for unsupported fields/methods;
3. define a fallible outer-request failure channel for host-observed VM,
   conversion, and cancellation failures where compatible with the authored
   Service trait. A method returning plain `i64` cannot transparently start
   returning `Result`; preserve the authored ABI and distinguish script
   `Result` values from execution failure;
4. make all authority declarations fail closed;
5. measure and guarantee that a warmed scalar host call does not rebuild
   generation metadata or allocate proportional to the entire registry.

The original recommendation for automatic lexical/last-use releases is
withdrawn under the current contract. `docs/architecture.md:203-210` explicitly
forbids compiler-inserted releases, including last-use and scope-edge releases.
Changing that policy would require a deliberate language/interop design change,
not an audit cleanup. It is reasonable to report the usability cost without
treating the chosen lifetime semantics as an implementation bug.

## Additional repository-level findings

### Documentation and API drift

- `README.md` reports M19.5 while `docs/progress.md` tracks M20.5.
- progress history contains old artifact-format version statements alongside the
  current v5 format.
- website examples use APIs such as `register_script_host::<Player>()` that are
  not present in the reviewed engine and omit fallible runtime construction.
- the documented example package command points at a workspace package that is
  excluded.
- the README crate map omits multiple current compiler, tooling, and runtime
  crates.

Documentation drift is a usability defect for an embedding language: users
cannot infer which of several public layers is canonical. Add compile-tested
documentation examples and generate milestone/artifact version snippets from
one source.

### Workspace publication and compatibility policy

Most crates inherit publishable defaults even though many are internal
implementation layers. Workspace metadata also lacks a declared Rust version,
repository, and package description. Decide which facade/schema crates are
supported externally, set `publish = false` for fixtures/internal crates, and
declare MSRV and package metadata before the first public release.

### File-size exceptions

All currently oversized files are recorded in the repository's exception
ledger, which is much better than ignoring the problem. The ledger has grown to
dozens of entries, however. An exception records debt; it does not remove it.
Prioritize splits where a file mixes ownership domains—VM instruction families,
macro parsing versus rendering, and language-service database versus query
facade—rather than mechanically splitting by line count.

## Prioritized remediation plan

The following is the recommended repair order as of the 2026-09-05 re-review.
Ranks are global across the tables. Priority combines demonstrated consequences,
exposure through supported/default paths, and repair dependencies; it is distinct
from the severity labels above. Correctness repairs precede performance work,
and measured bottlenecks precede speculative structural cleanup. This ordering
does not change milestone status or the active roadmap.

### P0: contain process failure, invalid reload, and authority hazards

| Rank | Issue | Why this comes first | First deliverable and acceptance |
|---|---|---|---|
| 1 | **C-04: unbounded heap-to-owned export** | An ordinary script result can abort the host process through stack overflow, including CLI/playground output. Shared acyclic graphs can also expand exponentially. | Add cycle handling and depth, work, and output-allocation limits. Cyclic, deeply nested, and highly shared results must return a bounded result or structured error without aborting the process. |
| 2 | **C-02: stale ordinary `HotUpdate` acceptance** | A stale or replayed update can bypass the compatibility check and remove code added by an intervening generation. Split-state runtime clones also make staging ownership ambiguous. | Bind updates to the checked base and reject mismatches before state initializer effects or publication; resolve clone ownership. Cover stale updates, replay, clone staging, and the explicitly chosen identical-base cross-runtime policy. |
| 3 | **C-01: live-object loss during finite-slot GC** | This loses reachable script objects, but requires the opt-in finite-slot API; the default atomic path does not exhibit this inter-step defect. | Disable finite-slot stepping or use an atomic fallback first. Test allocations, changing roots, and container writes between steps before restoring incremental collection with complete barriers. |
| 4 | **H-05: reflection/receiver authority defaults** | Setting only a reflection resource limit unexpectedly enables all reflection policy permissions; custom adapters also inherit exclusive receiver access unless overridden. | Require explicit reflection authority independently of its budget and explicit receiver access where needed. Budget-only configuration must leave reflection disabled; existing visibility, capability, and lease checks must remain enforced. |

If finite-slot GC is enabled in a deployment, apply rank 3's containment
immediately alongside rank 1. Rank 2 concerns ordinary Vela reload; the audit
does not demonstrate a bypass of whole-Service-generation validation. Rank 4
is a configuration hazard, not evidence of universal host-access bypass.

### P1: repair identity, generated-code, and tool correctness

| Rank | Issue | Reason and acceptance |
|---|---|---|
| 5 | **C-03: lossy reflected stable IDs** | Distinct internal IDs become the same script-visible integer. Introduce a lossless representation and round-trip high-bit/u128 IDs before relying on public reflection identity. Internal compiler and dispatch IDs are not shown to collide. |
| 6 | **H-04: inconsistent raw reflection registration** | Replacing a function ID leaves stale name lookup; type registration permits distinct keys sharing an ID. Define collision/replacement semantics and update all affected indexes atomically. Test rejected registration leaves the registry unchanged; do not assume sealed Engine admission has the same defect. |
| 7 | **`vela_bindgen`: generated member-name collisions** | Valid Vela declarations can produce Rust that fails to compile. Validate normalized field, variant, and parameter namespaces before returning source; test `Foo`/`foo` and keyword cases in downstream consumers. Preserve existing module/callable/top-level checks. |
| 8 | **H-08: LSP file URI handling** | Percent encoding and UNC handling can select the wrong path or make a document inaccessible. Use standards-aware URI/path conversion at the adapter boundary and test spaces, Unicode, literal `%`, Windows drives, and UNC round trips. |
| 9 | **H-07: generated Service lint override** | Even scalar-only services fail under `#![forbid(unsafe_code)]` because of unconditional generated `allow(unsafe_code)`. Make that safe consumer compile; retain the enforced lifetime/provenance boundary for service signatures that actually require erased reborrowing. |
| 10 | **`vela_playground_wasm`: execution and result fidelity** | Compile-only currently constructs a Runtime and runs state initializers; UI JSON parsing loses precision for large integers. Separate compilation from initialization and preserve 64-bit result values through the browser display. Validate a compile-only initializer fixture and integer boundary round trips. |

Ranks 5-6 gate reflection identity and registration contracts, rather than all
embeddings with reflection disabled. Rank 9 must not be resolved by merely
moving unchecked reference reconstruction behind a public safe helper.

### P2: remove measured or source-confirmed scaling costs

| Rank | Issue | First deliverable and measurement |
|---|---|---|
| 11 | **H-03/H-08: incremental HIR and synchronous editor work** | Resume the active M20.5 gap: stable per-module HIR IDs/shards and reverse indexes before cross-revision fact reuse; move `did_change` diagnostics and remaining blocking handlers to appropriate workers with bounded/coalesced queues and cooperative cancellation. Measure one-file edit-to-diagnostic p95 at 128/256 modules, including edits with live snapshots. |
| 12 | **H-01: per-call VM/dispatch/reflection reconstruction** | Extend existing generation-owned execution data to share immutable dispatch and reflection metadata. Keep counters and mutable sessions call-local. Measure warmed call allocations against registry size; same-session re-entry and Rust-default Service dispatch already have different paths. |
| 13 | **H-02: retained compiler and duplicate program representations** | Measure 0/1/16 pinned generations, then retain only the execution/proof data needed after verified linking. Preserve current MIR-based verification and selected interpreter plans. Report incremental live bytes, RSS, and activation latency without attributing all process high-water memory to MIR. |
| 14 | **M-01/M-02: collection accounting and host conversion costs** | Replace full map/set accounting scans with incremental byte deltas, then remove demonstrated repeated host argument materialization. Check accounting against a full-scan oracle and measure growth scaling and allocations on the affected paths while preserving leases and validation. |
| 15 | **C-01 follow-up: unenforced GC time target** | After live-object correctness is secured, define and enforce the supported work/pause contract. A `micros(0)` request currently still sweeps all slots. Measure marking as well as sweeping; do not restore unsafe multi-step collection just to meet a timing target. |
| 16 | **`vela_playground_wasm`: main-thread work and artifact size** | Add a cached browser-worker session, avoid recompiling unchanged input, and impose source/work caps. Track browser responsiveness and optimized/compressed size separately from the current raw release WASM size. |

Rank 11 leads this tier because it is the named active milestone gap with
existing scaling evidence. Runtime-focused deployments can take rank 12 first
within P2. Existing snapshots already share database roots and analysis facts
are cached per generation; neither needs to be introduced from scratch.
Performance acceptance requires fresh measurements of the changed path, not
reuse of historical allocation/RSS figures as predicted savings.

### P3: simplify ownership, APIs, and authoring after the above repairs

| Rank | Issue | Bounded next step |
|---|---|---|
| 17 | **Stdlib tables and repeated metadata projections** | Consolidate semantic/runtime mappings where they express the same facts; reuse the sealed TypeBinding authority. Preserve layer-specific definition lookup, analysis, MIR, and export contracts rather than inventing another universal registry. |
| 18 | **M-03: public API, dependency, and file ownership** | Document the supported embedding facade, narrow APIs with no external production consumer, and split files at real ownership boundaries. Any runtime/compiler dependency reduction needs a Cargo crate/feature boundary; a module move alone does not achieve it. |
| 19 | **Syntax/parser rescanning and repeated compiler analysis** | First measure adversarial parsing and identify reusable verifier-derived results. Consolidate rescanning or redundant post-verification analysis only where proven useful; keep independently produced verifier proofs. |
| 20 | **Macro diagnostics, Host authoring, CLI/docs, and packaging** | Use positional macro locals and source-span diagnostics for duplicate labels already rejected by Engine; improve explicit-release examples, CLI/schema diagnostics, compile-tested documentation, and publication metadata. These are smaller independent tasks, not prerequisites for a broad API rewrite. |

Automatic Host release is excluded: authored release, terminal Service transfer,
and root teardown are the current contract. The macro label case belongs in P3
because supported Engine registration already rejects it; it is not a reproduced
silent miscompilation. A small independent P3 documentation fix may land earlier,
but must not displace the P0/P1 correctness work.

### Work that should remain deferred

The MVP contract excludes JIT, moving GC, suspended-frame hot migration,
runtime type mutation/monkey patching, and a custom IDE beyond the native LSP.
JIT remains an explicit later milestone. MIR already has a current role in
verification and selected interpreter plans; optional JIT inspection does not
justify every runtime retaining its complete compiler state indefinitely.
Evaluate retention separately from the proof-producing compilation pipeline.

## Suggested measurable gates

Before calling the relevant milestones complete, add gates for:

- **GC:** a live object created or linked after every incremental boundary
  survives; configured pause/work limits are measured, not merely stored.
- **Reload:** stale, mismatched-base cross-runtime, and replayed updates reject
  before state initializers/effects and program publication. Specify separately
  whether identical-base cross-runtime fan-out is supported.
- **Reflection:** every stable ID round-trips losslessly and duplicate
  registration is atomic/fallible.
- **Embedding:** the documented minimal Rust application compiles with only the
  facade dependency, with dependency renaming and
  `#![forbid(unsafe_code)]`.
- **Calls:** warmed scalar/native/host calls reuse generation-wide metadata;
  benchmark allocations are bounded by argument/result shape, not registry size.
- **Retention:** measure incremental live retained bytes and process RSS for
  0/1/16 pinned generations separately, with shared ownership and allocator
  high-water effects accounted for. Set a target after measuring the proposed
  compact runtime representation.
- **Editor:** a one-file body edit reuses unchanged parse/HIR shards; stale work
  is cancelled/coalesced; file URI round trips cover spaces, Unicode, `%`,
  Windows drives, and UNC.
- **WASM:** compile/run uses a worker, has source/work caps, preserves 64-bit
  values, and stays within a tracked optimized/compressed size budget.

## Validation and limitations

Commands completed successfully during the original 2026-09-04 review:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast
cargo test --workspace

cargo test -p vela_common -p vela_syntax -p vela_def -p vela_hir \
  -p vela_mir -p vela_bytecode -p vela_package -p vela_analysis \
  --no-fail-fast

cargo test -p vela_registry -p vela_host -p vela_reflect \
  -p vela_hot_reload -p vela_stdlib -p vela_stdlib_runtime

cargo test -p vela_vm -p vela_engine

cargo test -p vela_macros -p vela_bindgen -p vela_bindgen_compile_test \
  -p vela_language_service -p vela_lsp_server -p vela_cli \
  -p vela_playground_wasm

cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
```

The independent 2026-09-05 re-review reran these commands successfully:

```text
cargo test -p vela_vm -p vela_hot_reload -p vela_reflect
cargo test --workspace --no-fail-fast
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p vela_playground_wasm --target wasm32-unknown-unknown --release
```

Clippy exited successfully; Windows linking emitted the informational
`linker_messages` warning about creation of the proc-macro import library.
The raw release WASM artifact was again 11,536,532 bytes, before wasm-opt,
compression, or browser startup measurements.

Eleven behavioral probes ran in a temporary external Cargo consumer with local
path dependencies and the repository's lockfile as the dependency baseline.
They asserted observed behavior, including existing defects and the Engine's
duplicate-parameter protection; passing probes do **not** mean fixes landed.
The report preserves the important inputs/outcomes in its findings:

- finite-slot allocation/root loss and zero-microsecond full sweep;
- stale/replayed ordinary reload and shared-staging clone divergence;
- two high-bit reflected IDs colliding;
- duplicate type-ID acceptance and stale function-name indexing;
- permissive `ModulePath` normalization;
- reflection enabled by a budget setter;
- duplicate macro parameters rejected by Engine;
- acyclic alias expansion from 13 heap arrays to 8,191 owned arrays;
- successful bindgen output containing colliding record fields.

Two deliberately failing downstream compile probes confirmed E0453 for the
scalar-only Service under `forbid(unsafe_code)` and E0124/E0062 for the generated
`Foo`/`foo` record. Two isolated processes, one using the public heap converter
and one the ordinary script/Runtime path, terminated with
`STATUS_STACK_OVERFLOW` on cyclic export. These expected failures are separate
from the passing repository suite. Reproduction code and logs were kept outside
the repository; only this audit document is changed.

The re-review did not run Miri, sanitizers, fuzz campaigns, a new interpreter
performance matrix, or a live-editor latency experiment. Source inspection and
existing test passes do not establish soundness of every unsafe boundary.
Long-duration benchmarks were not rerun. Performance conclusions
therefore distinguish source-proven complexity from checked-in measurements:
per-call reconstruction and quadratic accounting are visible in code, while
absolute latency/RSS figures come from `docs/performance.md` or the audit build.
No production workload profile was available, so the remediation order favors
correctness and removal of obviously generation-proportional work over
speculative micro-optimization.

## Final verdict

Vela's core ideas are credible: verified compilation stages, immutable hot
generations, compact values, and host-owned mutation through capability handles
form a solid language architecture. The project is more advanced than its
public API and documentation currently communicate.

The next improvement should not be another subsystem. It should be subtraction:
repair the GC/reload/reflection invariants, share immutable execution metadata
and schema projections, reduce retained compiler state, and make one documented
Rust embedding path fast and hard to misuse. Preserve independent verification,
explicit Host release, and the single Service-generation model. These changes
can preserve the project's strongest engineering while making the implementation
easier to reason about, benchmark, and evolve.
