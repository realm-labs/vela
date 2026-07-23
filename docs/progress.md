# Progress

This file records current implementation truth, the active milestone, and the
remaining gaps. It is not a changelog.

Detailed progress before the 2026-06-01 compaction lives in
[archive/progress-full-2026-06-01.md](archive/progress-full-2026-06-01.md).
Completed execution plans live under [archive](archive/); newer implementation
history belongs in Git.

## Current Focus

The active architecture focus is the
[Rust/Vela unified service hard switch](rust-vela-service-hard-switch-plan.md).
Rust hotfixing will use one generated model: Rust service traits and defaults,
sparse Vela service implementations, and one atomically published complete
service generation. Handler, rule, event, provider, and free-function hotfixes
do not receive separate replacement paths. S1 has deleted the former
callable-level replacement implementation without compatibility aliases. A
sealed TypeBinding registry, compact root-local HostRef
handles and borrow groups, prepared host plans/thunks, allocation-free
common-arity preflight, standard Rust type and View/MutView protocols,
constructors/methods, and user-defined type registration are now explicit
S2-S3 prerequisites. The Rust-default service branch must bypass HostRef and
VM conversion. Deployment uses complete Snapshots or exact-base Deltas that
flatten inherited Vela code into one candidate and activate/rollback with
conditional publication. S0 is accepted: the callable-slot deletion inventory,
domain-neutral Rust-default migration fixture, and dedicated direct-Rust/
HostRef/path/preflight/reborrow/borrowed-return/bulk-operation latency,
throughput, allocation, and checksum baselines are recorded. S1 and S2 are
accepted; S3 is the active implementation checkpoint. S2 delivered:
manual `TypeBinding` registration now seals stable interop identity, explicit
Value/Host storage, receiver capabilities, ABI fingerprints, and one registry
checksum into Engine, reflection, compiler-analysis, and LSP-facing facts. A
typed `ValueCodec` can now be registered manually for an unannotatable external
Rust type and round-trip that value through real Vela execution without serde
or JSON. Per-method receiver requirements now share the same reflection,
compiler, LSP, ABI, generated-metadata, and Runtime facts; an exclusive method
rejects a call-scoped Rust `&T` before authored code runs and accepts `&mut T`.
Value constructors are now registered on `TypeBinding`, reuse the ordinary
native-function execution path, derive `construct`, participate in the binding
ABI, project their stable IDs through reflection/compiler facts, and execute
from Vela through a qualified `host::Type::new` call.
Host factories now use `host_constructor_fn` to transfer exact Rust objects
into actor-local Runtime-owned storage. Vela receives only a `HostRef`; the
object remains outside script GC, supports HostAccess and shared/exclusive
leases, and survives across Runtime calls until its Runtime is dropped. The
owned arena stores object/type metadata in a dense generational
`HostSlotTable`; expanded internal roots derive from that slot and reject
mismatched type or stale-generation identities.
The first generated authoring path is also live: `#[derive(Value)]` emits
qualified named-struct or enum schemas, stable field/variant facts, direct
structural codecs, and the same `TypeBinding` consumed by
`register_rust_type`; no handwritten Vela adapter or serde/JSON conversion is
involved. Registered nominal values retain identity across entry arguments and
sync/async native results, so Vela enum `match` works on Rust-produced values.
`#[derive(ScriptHost)]` now also emits the base Host `TypeBinding`, allowing the
same typed registration path without a handwritten host binding adapter.
`#[script_methods]` now composes its sync, async-direct, and async-context
method thunks into that binding, and `register_script_host::<T>()` consumes the
single completed registration instead of installing schema and methods in
parallel.
Generated host-argument preflight and the corresponding acquired lease-guard
set now both stay inline for up to eight leases. Wider service boundaries still
spill, and failed multi-lease acquisition drops the partially acquired inline
set before returning, preserving atomic rollback.
Generated `ScriptHost` adapters now consume their resolved dense field and
method slots for root-level field reads/writes/mutations and synchronous root
method calls. Those successful accesses no longer repeat stable field/method ID
dispatch. Nested method resolution and execution now advance an offset through
the original linked `HostTargetPlan` instead of cloning suffix plans. Generated
adapters also cache up to four resolved schema-local field slots inline and
execute ordinary nested field reads, writes, mutations, and method calls
through typed slot thunks without stable field-ID redispatch or leaf
re-resolution. Deeper or non-preparable paths retain the validated generic
traversal. Collection length/empty queries, snapshots, and batch mutations now
use the same prepared field chain to reach a leaf collection adapter, while
index/key segments without an adapter-local slot retain validated generic
traversal. Indexed removal through generated host-field prefixes reaches the
collection adapter via that fallback. `Vec<T>` index-shaped plans now resolve
to an `AdapterLocal` slot, preserving the prepared generated-field prefix
through the sequence adapter. `BTreeMap<K, V>` and `HashMap<K, V>` do the same
for key-shaped suffixes, as do `BTreeSet<K>` and `HashSet<K>` membership paths.
Fixed arrays now preserve prepared generated-field prefixes through
index-shaped suffixes as well, and borrowed slices classify index-shaped
targets through the same adapter-local contract. Prepared accesses now carry
an inline mixed chain of generated field slots and adapter-local steps;
`Vec<T>` consumes its index step and executes a terminal generated element
field read, write, or compound mutation through dense field thunks without a
live-element lookup during resolution; fixed arrays and borrowed slices now do
the same. `BTreeMap<K, V>` and `HashMap<K, V>` consume typed key steps and
execute the same dense value-field operations while preserving key conversion
and missing-entry validation. Element method calls remain open.
Default/manual adapters preserve that cursor during read-modify-write and use
it when distinguishing a nested leaf from a missing target.
Direct call arguments now keep an inline dense host-slot index separate from
their mixed positional/named value entries. Every copied alias resolves its one
binding/lease metadata entry in O(1) from the execution-assigned object range,
including exact type and generation validation. Preassigned same-session
reborrows use their child binding rather than reacquiring the already leased
parent object.
The compact table key is now a pointer-free, 8-byte `HostSlotRef` containing
only a `u32` slot and `u32` generation. One reusable dense `HostSlotTable`
owns inline metadata for the common eight-slot case, rejects stale aliases,
and advances a slot generation before reuse. Production direct-host arguments
use that table instead of an ad hoc slot vector. `Value::HostRef` now carries
only `HostSlotRef`; expanded canonical `HostRef` values stay behind the active
host adapter and never enter VM values, script GC, or persistent script state.
Reflection resolves the handle through that adapter before producing its
controlled boundary value. Runtime owns the canonical slot namespace so
Runtime-owned and extern identities can survive calls, while direct and scoped
entries are generation-invalidated at their call-tree boundary. Nested
re-entry shares the namespace, recursive value/native/async/reflection
conversions require an active resolver, and detached conversions fail closed.
Early scoped release retires the live slot while retaining a call-local
diagnostic tombstone so existing `ExpiredBorrowedHostRef` behavior is
preserved. Live scoped-return object/type/access metadata now uses its own
dense generational `HostSlotTable`; its internal roots encode slot and
generation in a private range and validate exact type identity. A scoped
root's `BorrowLeaseId` derives from that same slot/generation, so copied aliases
share one release group and a reused slot receives a different identity.
Runtime extern-state object/type/activation metadata also uses dense
generational slots; durable `StateId` and staged-name maps remain boundary
indexes, staged roots remain inactive until commit, and replacement or
reclamation generation-invalidates the old root. Transient lease provenance
stays in the inline active-call proof, prepared adapters stay in sealed
registration/link plans, and service-generation pins stay in the root
execution session; none is copied into HostRef aliases.
The first S3 standard binding family is also live: concrete
`BTreeMap<K, V>` and `HashMap<K, V>` bindings synthesize stable recursive
key/value facts, share the Vela `MapLike` surface and owned Map codec, and keep
distinct Rust ABI identities. Their keys now require the explicit
`VelaValueKeyBoundary` proof, and `Vec<u8>` consistently reports the Bytes
boundary fact. Owned `Vec<T>` now binds as a growable Sequence/Iterable Array,
and `BTreeSet<T>`/`HashSet<T>` share SetLike/Iterable behavior with distinct
concrete ABI identities. Concrete Rust `Option<T>` and `Result<T, E>` bindings
now specialize their recursive payload ABI while round-tripping through the
existing dynamic Vela Option/Result values and standard methods. Rust unit,
bool, char, exact-width numeric scalars, and String now also have concrete ABI
bindings over their native Vela value representations. Rust tuples of arity
two through four now have ordered concrete bindings, an exact reflected Tuple
kind, and element facts that survive both reflection and compiler-registry
projection. `RustValueType` and `register_rust_value_closure::<T>()` now
recursively install concrete standard containers, shared leaves, and nested
`#[derive(Value)]` field/variant types from one owned root; exact duplicates
are idempotent while conflicting manual bindings remain seal errors.
Standard `Vec` (including the owned-Bytes `Vec<u8>` specialization), map, and
set bindings now advertise shared View and exact growable MutView capabilities
on that same identity. Those
representation facts participate in the type ABI and project consistently
through reflection and compiler registries. Concrete `[T; N]` bindings include
`N` in stable identity and expose shared plus exact fixed MutView capability;
generated `&[T; N]`/`&mut [T; N]` adapters reborrow through HostRef, permit
indexed replacement, and withhold structural mutation.
Concrete borrowed `[T]` bindings now use a distinct stable slice identity and
the same shared/fixed Array view surface. Generated sync/async free-function
and method adapters preserve the original DST reference without copying,
including slices returned from one service and reborrowed into another.
Borrowed slice recovery now uses one private lifetime-aware erased-borrow
module in `vela_host`; the obsolete visitor/support API and `better_any` are
removed. Mutable downcast consumes its exclusive token, all other host modules
forbid unsafe, and a syntax-aware architecture audit restricts unsafe Rust to
the reviewed slice-erasure and C ABI boundary files.
The focused acceptance matrix covers shared/exclusive and wrong-type recovery,
empty and zero-sized slices, HostRef alias conflicts, retained returns, nested
native re-entry, real async suspension/completion and cancellation, authored
error/panic cleanup, and old-generation completion across staged reload.
Borrowed `Vec<u8>`, `[u8; N]`, and `[u8]` now use HostRef-backed
`ArrayView<u8>`/`ArrayMut<u8>` contracts with exact growable/fixed capability;
direct and retained views reborrow without copying and mutate the Rust bytes
immediately, while the owned `Vec<u8>` representation remains Vela `Bytes`.
Callable contracts can now carry an exact binding-use proof containing the
concrete `InteropTypeId`, `TypeAbiFingerprint`, and owned/Host/View/MutView
representation. Engine sealing rejects unregistered, stale, unsupported, or
boundary-mode-incompatible proofs, so an `ArrayMut` surface cannot conceal a
fixed/growable or concrete Rust ABI mismatch. Export and method macros now map
borrowed standard `Vec`, map, and set signatures to exact View/MutView facts,
emit those proofs, and register the concrete binding closure automatically.
Restricted `ArrayView`/`ArrayMut`, `MapView`/`MapMut`, and
`SetView`/`SetMut` hints now project as distinct analysis facts without adding
general Vela generics. Hidden fixed/growable mutation capability survives
native metadata, host-method return reflection/compiler projection, exported
language-service schema, and callable/type-binding ABI fingerprinting.
Standard method facts reuse the owned collection
read/iteration/transform surface while withholding structural mutation from
shared and fixed views; growable exclusive views retain it. Linked calls keep
borrowed collection contracts distinct from script-owned Array/Map/Set values.
Generated sync and async free-function and method adapters now lease real
borrowed standard collections without materialization. Shared and exclusive
collection references returned from one Rust export retain their owner lease
and exact binding identity when passed into another Rust export, including
immediate mutable write-through. Vela now executes `len` and `is_empty` on
direct or retained HostRef-backed views through a domain-neutral read-only
collection protocol and HostAccess, including shared references. Array
positional and typed Map indexing also read and write through HostAccess for
direct and retained views; shared writes fail without changing Rust state. The
standard Array surface now includes `get(index) -> Option<T>` for owned,
shared, and exclusive representations. HostRef-backed `get`, `first`, and
`last` reuse a live length query plus at most one indexed HostAccess read,
return `Option::None` for an absent index or empty view, and work for direct
and retained borrows without snapshotting or length-proportional budget cost.
The dynamic key boundary preserves bool, char, exact-width signed/unsigned
integers, String, Bytes, and HostRef identity, so a `BTreeMap<i32, V>` is
indexed by an actual `i32` rather than a serialized path string. Standard key
implementations and the public `ScriptHostKey` conversion contract share this
model. Standard membership now exposes baseline Map `contains_key` and Set
`contains` on owned, shared, and exclusive collections while preserving `has`
for both families. Those names and read-only `MapView.get/get_or` reuse the same
resolved keyed HostAccess path without materializing the collection. A distinct
missing-entry error ensures only absent keys become `false`, `Option::None`, or
the caller fallback; other host errors propagate. Read-only Array
`contains/index_of` consume one bounded values projection, charge one execution
unit per projected element, compare exact `ValueKey` identities, and preserve
false/`Option::None` behavior for empty or absent values without materializing
a script Array. Read-only Array `distinct/reverse/slice/join` consume the same
single precharged values projection and reuse the owned transform algorithms
directly. They return ordinary owned Array/String results without creating a
temporary receiver Array or mutating the Rust backing collection. Complex
Array `sort/min/max` also consume one completely precharged projection, then
run the existing resumable ordering state machine after the HostAccess lease
ends. The state retains only Vela values, roots projected heap values across
nested comparison calls, and returns an owned Array or Option without mutating
the Rust collection. Untyped dynamic HostRef receivers now discover only
standard methods backed by an implemented HostRef execution route. Resolution
uses the canonical HostRef, sealed collection-view capability, and live
shared/exclusive access mode; structural mutators require an exclusive
growable view, while shared/fixed receivers and unimplemented collection
methods remain `UnknownMethod`. These access-sensitive HostRef resolutions are
not entered into the ordinary dynamic StandardValue inline cache. Complex
borrowed element views, remaining element/key methods, live/resumable
iteration, remaining bulk mutation, user-defined collection adapters, and
prepared index plans remain open. Growable `MapMut.set` and missing-key index
assignment now insert
scalar/String/Bytes leaves through the keyed HostAccess write, while
`MapMut.remove` uses a keyed HostAccess remove and returns the prior value as
`Option<V>`. `SetMut.add/insert/remove` write membership through the same path
and retain standard changed/not-changed results; `insert` is the baseline name
while `add` remains available. Growable `ArrayMut.remove_at`
reads and removes through one indexed HostAccess path, returns the prior value
as `Option<T>`, preserves missing-index behavior, and works on retained
method-return views without materialization. `ArrayMut.pop` shares the live
length/edge query used by `last`, removes the resolved final element through
the same path, and returns `Option::None` without mutation when empty.
Growable `ArrayMut.push` converts its element before mutation, precharges one
execution unit, and submits one stack-backed `ExtendSequence` item through
HostAccess, so conversion or budget failure leaves the Rust Vec unchanged.
`ArrayMut.insert` queries the live length under the exclusive lease, rejects
sparse indexes with the ordinary Vela bounds error, then converts and
precharges one element before one `InsertSequence` mutation; insertion at the
current length appends and every failure remains non-mutating.
Borrowed Array `iter/values`, Map
`keys/values/entries/iter`, and Set `values/iter` now capture deterministic
bounded boundary projections under the active lease and feed the existing Vela
Iterator pipeline, including `filter/count/collect`; complex element handles
and per-resume live host generation checks remain open. Direct callback methods
on borrowed Array, Map, and Set views now reuse that projection to create one
budgeted temporary script collection and enter the existing resumable callback
state machine, so Array `filter/group_by`, Map `filter/map_values`, and Set
`filter/map` retain owned collection semantics without exposing Vela method
IDs to host adapters. Borrowed Set `union`, `intersection`, `difference`,
`symmetric_difference`, `is_subset`, `is_superset`, and `is_disjoint` now
consume one completely precharged values projection, reuse the owned/cached
Set algebra algorithms against an owned Set operand, and return a detached
owned Set or bool without mutating the Rust backing collection. Static and
untyped dynamic receivers share this route; empty Sets and non-Set operands
preserve the owned method semantics. Borrowed Map `merge` similarly consumes
one completely precharged entries projection and reuses the owned/cached merge
payload against an owned Map operand. The right operand replaces duplicate
keys, the result is a detached owned Map, and static or untyped dynamic calls
never mutate the Rust backing Map. Growable borrowed
collection `clear` now precharges its size and performs one semantic
`HostCollectionMutation::Clear` through HostAccess for standard Vec, Map, and
Set host objects; budget failure occurs before mutation and adapters never see
Vela method IDs. Growable borrowed collection `extend` now converts one owned
Vela Array/Map/Set into an exact borrowed mutation batch, charges one execution
unit per input, and performs one HostAccess mutation. Standard Vec, Map, and
Set adapters validate the complete batch before changing Rust state, so a
conversion or budget failure cannot partially extend the host collection.
The host protocol now also has transactional retain primitives for the later
resumable callback surface. `RetainSequence` requires one decision for every
element and rejects a changed sequence length, while `RetainKeys` converts the
complete expected/retained key sets and verifies the current Map or Set key
snapshot before changing Rust state. Standard Vec, BTreeMap, HashMap,
BTreeSet, and HashSet adapters implement these mutations without partial
writes on conversion, shape, or stale-snapshot failure. Array, Map, and Set
`retain` are now public resumable callback methods for owned collections and
growable exclusive HostRef views. Callback decisions are accumulated before
one mutation; callback errors, final traversal budget failure, and stale
sequence/key snapshots do not partially retain. Host-backed completion
reacquires the original alias through HostAccess and the domain-neutral retain
protocol rather than mutating the temporary projection. Static and dynamic
dispatch share the standard method identity, retained child views write
through their parent lease, and shared or fixed-length views withhold the
method. Scalar, String, Bytes, and HostRef boundary leaves are supported;
matching borrowed Array/Map/Set sources can now extend a growable HostRef
target without owned materialization. The source is snapshotted through its
active lease before one target batch, source and mutation traversals are
precharged, and same-alias extension reads the complete old snapshot before
write-through. Complex element handles, write-through filter, and prepared
live grouping/traversal remain open.

Ordinary Rust/Vela exports, exact lease adapters, owner-frozen borrowed
returns, generated typed bindings, and `NativeCallContext` sync/async re-entry
remain accepted foundations. The historical optional-replacement proof is
recorded in the
[reconciliation acceptance report](archive/rust-vela-interop-actor-runtime-reconciliation-acceptance-2026-07-17.md),
but it no longer defines the target authoring model.

The Actor Runtime/cache plan is accepted through Batch F. Batch A's baseline is recorded in the
[Batch A report](archive/actor-runtime-cache-batch-a-baseline-2026-07-18.md),
and the deletion-first profile/metadata/cache hard switch in Batches B-C is
accepted in the
[ownership-cut report](archive/actor-runtime-cache-batches-b-c-acceptance-2026-07-18.md).
Actors retain no instruction-counter arrays or cache-site vectors. One exact
Engine deployment shares weakly registered generation execution data; caches
use one typed synchronized slot per linked site, profiling is default-off
aggregate atomic data, and immutable state/ABI/method facts remain with linked
generation metadata. Batch D's repeated shared-versus-isolated dynamic-method
measurement found no material lane-local benefit and closed with no execution
lane in the
[lane-gate report](archive/actor-runtime-cache-batch-d-lane-gate-2026-07-18.md).
Batch E's reload, old-generation lifetime, Actor isolation, and
cancellation/panic/reclamation proofs are accepted in the
[lifetime report](archive/actor-runtime-cache-batch-e-lifetime-2026-07-18.md).
Batch F's stable memory, concurrency, cache/profile, callback, reload, host,
async, interop, examples, workspace, documentation, benchmark-build, fuzz,
site, and safe-Rust gates close M20 in the
[final acceptance report](archive/actor-runtime-cache-acceptance-2026-07-18.md).
The completed execution plan is archived beside that report.

The explicit state-storage hard switch is accepted through Batch G. Exact
qualified embedding types, linked nominal canonicalization, graph-preserving
budgeted reload staging, external-owner generation reclamation, and nested
initializer-call fingerprints have focused and workspace-wide proof. The Actor
authority prerequisite and cache ownership/lifetime cuts are closed. M20.5 is
queued behind the reprioritized service hard switch.

The executor-neutral async implementation from Batches A-D is landed: Vela has
one explicit frame driver, scoped `Send` Runtime/native futures, direct typed
host leases, same-session NativeCallContext reentry, generation-pinned reload,
and one `call`/`call_async` target surface for functions, bound methods, and
providers. The 2026-07-13 baseline, zero-hit audit, and original acceptance
result remain recorded under [archive](archive/).

Post-implementation review on 2026-07-14 reopened final acceptance through
Batch E in [async-execution-model-plan.md](async-execution-model-plan.md).
Batch E is complete: dynamic reentry roots, exact shared/exclusive leases,
script-addressable `is_async`, focused VM session/resume/reentry ownership, and
one provider resolver are implemented without compatibility paths. Full
features, examples, benches, Rust docs, fuzz build, site gates, audits, and
performance/memory comparison passed; the result is recorded in
[the Batch E acceptance report](archive/async-execution-batch-e-acceptance-2026-07-14.md).
M20 cache close-out and the M20.5 LSP follow-up remain valid after the accepted
state-storage hard switch.

## Milestone Snapshot

| Milestone | Status | Current note |
|---|---|---|
| M0-M7 | Complete | The source-to-VM-to-HostAccess-to-hot-reload vertical slice, execution budgets, managed heap, and GC roots are validated. |
| M8-M18 | Complete enough | HIR, executable language surface, script metadata, host bridge, reflection, stdlib, embedding, reload, diagnostics, examples, and benchmark foundations satisfy their checkpoints. |
| M19 | Complete enough | The non-JIT interpreter and heap optimization checkpoint is closed; remaining measured costs belong to cache, value-layout, or later backend work. |
| M19.5 | Complete enough | Primitive scalars, bytes, type contracts, guard plans, linked bytecode, runtime profile ownership, and HostTargetPlan/HostAccess preparation are validated. |
| M20 | Complete enough | Actor Runtime/cache Batches A-F are accepted with shared generation execution data, no eager Actor vectors, and no execution lane. |
| M20.5 | Queued | Resume the concrete editor-visible follow-up after the service hard switch. |
| Rust/Vela service interop | S2 accepted; S3 active | The unified TypeBinding and compact HostRef/preflight gate is green; S3 closes standard view, collection-protocol, and prepared-operation gaps. |
| M21 | Not started | Debugger runtime hooks and DAP integration. |
| M22 | Not started | Cranelift JIT after interpreter, cache, debugger, and conformance contracts stabilize. |
| M23 | Not started | Release hardening, public documentation, validation gates, and performance targets. |

## Current Baseline

### Language And Runtime

- `.vela` source uses lossless Rowan syntax, Heavy HIR, analysis facts,
  verified MIR, linked bytecode, and one production interpreter route.
- Functions, closures, records, enums, traits, pattern matching, loops,
  iterators, tuples, unit, structured type hints, Option/Result propagation,
  value-keyed maps/sets, and controlled reflection execute through tested
  runtime paths.
- Execution-unit, memory, call-depth, and collection-growth budgets are
  enforced. Script heap objects use non-moving managed storage and GC roots;
  Rust host state is never placed under script GC.
- `LinkedArtifact` is the sole production executable generation.
  `ProgramVersion` and linked closures retain generation ownership across hot
  reload; no unlinked compatibility interpreter remains.
- Callable asyncness and explicit await/resume control flow are preserved from
  source through verified MIR and linked execution. Sync execution, awaited
  sync targets, and real Rust future suspension use the same explicit
  `ExecutionSession` frame driver. The outer future is scoped and `Send`, and
  registered async futures may borrow invocation state without being `'static`.

### Host Boundary And Embedding

- Scripts mutate Rust-owned state only through `HostRef`, `HostPath`,
  `PathProxy`, `HostTargetPlan`, and call-scoped `HostAccess`; scripts never
  receive real Rust `&mut T` references.
- Nested reads, writes, compound mutations, removals, indexed paths, host
  methods, permission checks, generation checks, and source-spanned failures
  are covered.
- Reflection can inspect registered metadata and perform permissioned
  reads/writes/calls, but cannot mutate runtime type structure or monkey patch
  types.
- Engine registration, native functions, derive macros, capability profiles,
  package graphs, service-provider discovery/selection, serde snapshots,
  runtime value handles, hot reload, and the initial C ABI surface are
  available.
- `PackageId + ModulePath` is the sole script module identity. Package/provider
  compilation and reload use sealed package/HIR snapshots and linked artifact
  metadata rather than parallel package-unaware paths.
- Ordinary Rust/Vela integrations use one deterministic compiler-owned binding
  schema and build-time generated typed Rust surface. Runtime strings and
  boundary wrapper values remain low-level dynamic escape hatches, not the
  primary call workflow.
- The former callable-level replacement implementation is absent. Until the
  generated service boundary lands, Rust/Vela integration exposes ordinary
  exports and generated typed bindings but no Rust-logic hotfix API.

### Standard Library, Tooling, And Proof

- Arrays, maps, sets, strings, bytes, iterators, Option/Result, math, context,
  deterministic time, controlled random, opt-in stdio, and sandboxed filesystem
  helpers have runtime and analysis coverage.
- The native LSP uses editor-neutral queries in `vela_language_service`, typed
  `lsp_server::Message` transport and projection in `vela_lsp_server`, and thin
  VS Code/Zed integrations. It covers diagnostics, completion, signature help,
  hover/navigation, symbols, semantic tokens, references, rename, code
  actions, formatting, inlay hints, file watching, cancellation, and schema
  reload.
- Runnable examples, conformance and diagnostic fixtures, a parser fuzz target,
  benchmark harnesses, and the documentation site provide end-to-end proof.
- Current performance rules and baseline summaries live in
  [performance.md](performance.md); detailed historical measurements live in
  [archive/performance-full-2026-06-06.md](archive/performance-full-2026-06-06.md).

## Active Gaps

### Rust/Vela Service Hard Switch

The plan is fixed, but implementation remains open. S0 froze the boundary
baselines, S1 deleted callable-level replacement, and S2 accepted one
TypeBinding registry plus compact root-local HostRef/borrow tables, prepared
typed thunks, and allocation-free common-arity preflight. S3 completes
prepared host paths and standard collection View/MutView protocols, including
bulk operations. Only then do S4-S7 add a zero-HostRef Rust-default generation,
sparse Vela implementations, root-local same-generation reborrow,
successive exact-base Delta/Snapshot deployment, async/handler integration, and
final measured host-framework acceptance.

The existing ordinary interop, HostRef/HostAccess lease safety, Actor-owned
Runtime, generated bindings, same-session re-entry, staging, activation,
rollback, and no-retry semantics are reusable constraints rather than a reason
to keep the old slot API.

S0 is accepted in the
[baseline report](archive/service-hard-switch-s0-baseline-2026-07-23.md).
S1 is accepted. The deleted model has no aliases, annotations, dispatch APIs,
examples, benchmark rows, or schema metadata. Neutral ABI, lease, re-entry,
borrowed-return, generation-pinning, and no-retry behavior remains under its
ordinary module ownership. S2 is accepted with the sealed binding
identity/storage/ABI
substrate, manual external-type entrypoint, typed structural Value codec path,
and type-owned Value plus Host constructor registration backed by actor-local
Runtime storage. Structural `Value` derive generation is implemented for named
structs plus unit/named-field enums, and `ScriptHost` emits the base Host
binding. Owned standard bindings now cover concrete `Vec<T>`, `Vec<u8>`,
`BTreeMap<K, V>`, `HashMap<K, V>`, `BTreeSet<T>`, `HashSet<T>`, `Option<T>`,
`Result<T, E>`, tuples of arity two through four, unit, bool, char, exact-width
numeric, and String identities. Collections expose their common
Sequence/MapLike/SetLike surfaces, while Option/Result reuse the standard Vela
dynamic enum behavior. Method-thunk composition is complete for generated
ScriptHost registrations. Owned Value roots now recursively register their
concrete standard and `derive(Value)` dependency closure with exact duplicate
handling. Common-arity host-argument preflight now uses
generated request arrays and an eight-entry inline result set; the
shared/exclusive boundary rows allocate zero times and still reject the
complete conflict set before lease acquisition. Generated host functions and
methods now reuse registration-time prepared parameter plans instead of
rebuilding contracts and request metadata on each call. Active native-reborrow
provenance now keeps the common eight exact root/mode/object-address proofs
inline, so ordinary nested host calls do not allocate a provenance vector.
Root execution-host lease guards and grouped scoped child/activity sets now use
the same eight-entry inline threshold while retaining acquire-all-or-clean-up
behavior on conflict.
S3 is active. Its remaining gaps are complex-element borrowed views, remaining
element/key and live/resumable collection operations, borrowed-source and
write-through bulk mutations, richer adapters, and prepared
element/grouping/traversal operations.
Service-signature traversal and service-generation pinning belong to S4-S6. A
shorter owned-host reclamation policy remains post-S2 follow-up.
Runtime receiver enforcement is live; compile-time
View/MutView enforcement awaits receiver-capable expression and service-
signature facts.

### State Storage Acceptance

No state-storage correctness gap remains from Batches A-G. Runtime embedding
resolves canonical qualified types exactly, recursively validates and stamps
linked record/enum identities, and preserves those identities through
`set_state` and `update_state`. Reload copies added-state graphs with one shared
transaction budget while preserving aliases and cycles. Old generations are
pinned only by owners reachable outside inactive state roots and reclaim at an
ordinary safe point. Initializer reports traverse only their reachable script
call graph, including nested closure and parameter-default executables, with
recursive termination. The accepted contract and proof matrix live in
[state-storage-model-plan.md](state-storage-model-plan.md).

### Async Post-Review Closure

Batch E closed `ASYNC-ROOT-1`, `ASYNC-LEASE-1`, `ASYNC-REFLECT-1`,
`ASYNC-VM-MOD-1`, `ASYNC-PROVIDER-1`, and `ASYNC-DOC-1`. No async correctness
or final-acceptance gap remains. Named post-M20 performance follow-up
`ASYNC-LEASE-PERF-1` may profile the measured owned-guard exclusive lease cost
without weakening exact state, safe Rust, scoped `Send`, or RAII.

Do not solve these with a permanent-root leak, an exclusive lease labeled
shared, reflection aliases, navigation-only source splits, duplicated drivers,
or provider-specific public execution methods.

### M20 Cache Close-Out

The ownership, memory, concurrency, profiling, reload, and cache execution
batches are complete in the archived
[execution plan](archive/actor-runtime-cache-execution-plan.md). Gate I,
Batches A-F, and state-storage Batch G are accepted. The final ownership table,
measurements, proof matrix, and validation live in the
[acceptance report](archive/actor-runtime-cache-acceptance-2026-07-18.md).

Existing cache or measured families include declared state, script record
fields, host access, native calls, linked method dispatch, dynamic method
dispatch, stdlib value methods, callbacks, strings/bytes, Option/Result, and
selected array/map/set paths.

A future cache task is valid only when it names one concrete regression or
new measured gap:

- coverage: a measured hot path has no cache entry;
- correctness: hit, miss, wrong-guard, fallback, reload, schema, or version
  invalidation proof is missing;
- measurement: interpreter-only, profile-only, and cache-enabled rows cannot be
  compared;
- decision: a flat or slower result has not been accepted, assigned to a named
  follow-up, or deferred.

The accepted contract remains:

- Preserve the published cache-family ownership classification before adding
  another family.
- Do not restore eager per-Actor full-program metadata, migration flags,
  adapters, dual owners, or dual read/write paths.
- Preserve generic fallback behavior, budgets, GC roots, HostAccess policy,
  reflection permissions, hot-reload ownership, schema invalidation, and
  source-spanned diagnostics.
- Compare cache rows against the correct baseline using `measurement_kind`,
  `delta_kind`, `measurement_summary`, and `cache_delta_summary`.
- Keep scalar, collection, string, call/callback, and host-boundary results
  separate. Lua 5.x remains the non-JIT comparison target for representative
  host-boundary workloads.
- Move representation-wide, value-layout, or backend changes to an explicit
  later milestone instead of expanding M20.

The completed executable-generation contract is recorded in
[archive/mir-executable-generation-architecture-plan.md](archive/mir-executable-generation-architecture-plan.md).
Its accepted scalar interpreter cost belongs to a named M20
instruction-selection follow-up or M22, not to a second execution route.

### Parameterized Container Contracts

The current implementation includes nested `Array<T>`, `Map<K, V>`, `Set<T>`,
`Iterator<T>`, Option, and Result facts; recursive runtime guards; budgeted deep
checks; value-keyed map/set storage; compiler-owned mutator checks; macro
inference; serde/reflection preservation; hot-reload ABI comparison; contract
stamps and invalidation; and lazy iterator item guards.

The remaining checkpoint is an explicit acceptance audit against
[container-type-hints-plan.md](container-type-hints-plan.md) and
[value-keyed-map-set-plan.md](value-keyed-map-set-plan.md). Do not reopen
string-only map keys or vector-scan set semantics. Object equality/order is
complete enough for M20: user comparison traits remain separate from
`ValueKey` container identity/equivalence.

### M20.5 LSP Follow-Up

The clean query/context/result/projection boundary, typed main loop, GlobalState
ownership, lifecycle handling, incremental overlays, workspace/schema reload,
authoring-core completion model, formatting, semantic highlighting, and
protocol coverage are the baseline.

Remaining work must name a concrete editor-visible failure or missing protocol
proof. Known follow-up areas are broader method/schema call-site
classification and suppression of future hint families across dynamic `Any`
boundaries. Do not restore raw JSON-RPC handlers, feature-local semantic
scanners, runtime execution, live host-state reads, or editor-owned analysis.

### Deferred Tracks

- M21 debugger/DAP work waits for stable source spans, frame maps, GC roots,
  budgets, HostAccess, reload, tooling, and conformance contracts.
- M22 Cranelift JIT waits for M20/M21 close-out and must consume the verified
  MIR/linked-artifact contract.
- Typed scalar superinstructions remain deferred until profile evidence and
  temporary-register liveness support a specific fused lowering.
- Persistent host iterator handles remain deferred until their lifetime model
  is explicit.

## Validation

The 2026-07-17 Rust/Vela interop reconciliation gates remain historical proof
for ordinary interop and reusable Actor Runtime/re-entry safety. They are not
acceptance for the new service-generation model. Each S0-S7 checkpoint must add
the focused proof required by the hard-switch plan.

State-storage Batch G's exact resolution, nominal canonicalization,
graph-preserving staging, external-owner reclamation, and nested initializer
fingerprint regressions and full validation gates pass. State storage remains
accepted. The async Batch E acceptance remains recorded for 2026-07-14.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo bench --workspace --all-features --no-run
cargo doc --workspace --all-features --no-deps
cargo check --manifest-path fuzz/Cargo.toml --bins
cargo bench -p vela_vm --bench baseline -- vm_state_read_write --quick
```

The Miri component is unavailable on the installed stable Rust 1.97.1
`x86_64-pc-windows-msvc` toolchain, so the erased-slice boundary has not been
claimed as Miri-validated. Focused erased-borrow, lease/reentry, returned-slice,
and async adapter tests plus the unsafe-boundary source audit are green.
The compact `Value::HostRef` hard-switch checkpoint passes formatting,
workspace Clippy with warnings denied, the full workspace test suite, the
unsafe-boundary audit, and the active-file size architecture audit. Focused
proof covers recursive compact-slot conversion, canonical aliases and stale
generations, Runtime-owned HostRefs across calls, direct/scoped root cleanup,
early-release diagnostics, async resume, returned borrows, and generated
same-session mutable re-entry without parent reacquisition.
Documentation placeholder,
syntax-highlighting, Astro diagnostics, and static-site build gates also pass.

Use the relevant subset of [validation.md](validation.md) for each change.
M20 work also requires focused correctness tests for the touched bytecode,
runtime dispatch, host, or stdlib path and the matching
interpreter-only/profile-only/cache-enabled benchmark rows.

## Next Up

1. Execute S1 deletion before accepting a new public service replacement API.
2. Implement the unified TypeBinding foundation and standard Rust collection
   views in S2-S3 before service dispatch.
3. Resume the M20.5 editor-visible follow-up after the service hard switch.
4. Keep persistence, snapshots, replication, cross-Runtime sharing, structural
   state migration, async-frame migration, and initializer dependency reads as
   explicit non-goals.

## Update Rules

- Update this file only when the current focus, milestone status, supported
  baseline, validation expectation, or remaining gaps change.
- Do not append per-commit notes, benchmark logs, implementation chronology, or
  rejected candidates.
- Keep active status concise. Put durable historical detail in
  [archive](archive/) only when Git history is insufficient.
