# Rust/Vela Unified Service Model Hard-Switch Plan

> Track: one Rust/Vela TypeBinding registry and service contract, generated
> Rust fallback and Vela partial implementation, generation-coherent
> publication, host-reference and collection interop, and deletion of
> callable-level replacement
>
> Status: S0-S2 accepted; S3 implementation is active
>
> Switch policy: pre-release hard switch; no public compatibility layer and no
> second Rust hot-replacement model
>
> Supersedes: the optional `#[replaceable]` / `#[override]` slot model in
> [the archived interop plan](archive/rust-vela-interop-model-plan-superseded-2026-07-23.md)

## 0. Objective

Vela exists to supply the runtime dynamism that Rust game-server business
logic cannot provide by itself. The integration therefore optimizes for one
authoring experience:

```text
Rust defines an ordinary service trait and its default implementation.
The service framework macro generates all Vela boundary machinery.
Business callers always call the service contract.
A Vela package may implement only the faulty methods.
One validated service generation is atomically published for future calls.
```

Business code must not contain a per-method patch branch, dense replacement
slot, target string, runtime lookup, manual `CallArgs`, `HostRef`, or handwritten
Vela adapter. Whether a method currently runs Rust or Vela is a deployment
choice, not a caller concern.

This is also a simplification plan. Vela will not keep both service replacement
and callable-level replacement. The current `ReplaceableSlotId`,
`InterceptSlotIndex`, `DispatchController`, `DispatchRoot`, `#[replaceable]`,
and `#[override]` model is frozen immediately and removed by the hard-switch
batch. Handler, rule, event, and function hotfixes do not receive parallel slot
types; a host entry that must be hotfixable is expressed as a service method.

Vela's own `ProgramVersion` / `CodeObject` hot reload remains the mechanism for
changing Vela code safely. It is not a second Rust integration model. Ordinary
exported Rust functions and methods also remain callable from Vela, but they
are not independently replaceable. A Rust operation that must be hotfixable
must belong to a service contract.

The service model is viable only after Vela has one complete Rust type
interaction model. Every Rust type reachable from a service signature must
retain a registered type identity, methods, constructors, protocols, and
ownership capabilities when it crosses into Vela. Service dispatch must not
land before that prerequisite and leave patches unable to express the Rust
logic they replace.

## 1. Normative Service Contract

The stable technical contract is
[Rust/Vela Unified Service Model](architecture/rust-vela-service-model.md).
It owns the non-negotiable generation model, authoring shape, runtime objects,
TypeBinding and collection interop, macro responsibilities, and hard-switch
deletion boundary. This document owns only execution order, phase gates,
acceptance, performance, validation, and completion.

## 2. Phased Execution

The phase definitions and gates below are stable contracts. Current phase
status and remaining gaps live only in [progress.md](progress.md); this plan
must not accumulate per-commit implementation chronology.

Execution discipline:

- choose one independently verifiable behavior inside the active phase;
- add focused success and failure-path proof before committing it;
- keep local implementation commits coherent, but integrate a short sequence
  only after its focused checks pass;
- update this plan only when the model, phase boundary, deliverable, gate, or
  completion definition changes;
- update `progress.md` only when focus, phase status, validation expectations,
  or the named remaining gaps change;
- run the repository-wide validation gate once at phase acceptance, record the
  commands and result in the checkpoint commit or acceptance report, and do
  not infer phase acceptance from focused tests alone; and
- fold unpublished immediate fixups into the triggering change before shared
  integration.

The implementation may use a short-lived internal construction sequence, but
no accepted checkpoint may advertise both callable replacement and service
replacement as supported authoring models.

### S0 — Freeze, inventory, and executable fixtures

Deliverables:

- mark callable-level replacement superseded in all active docs;
- inventory every old type, macro, parser attribute, linker field, Engine API,
  example, test, and benchmark;
- extract a representative domain-neutral fixture with at least two services,
  one handler service, a mutable host actor, value DTOs, Array/Map arguments,
  nested service calls, Result, and async coverage;
- record direct Rust trait dispatch and current Vela boundary baselines; and
- freeze allocation, latency, and throughput rows for HostRef alias copy,
  static field/path read-write, registered method call, shared/exclusive
  argument-set preflight, nested reborrow, borrowed return/release, and
  host-backed bulk collection operations.

Gate:

```text
the fixture runs entirely through Rust defaults
no new feature is added to #[replaceable] or #[override]
the deletion inventory has an owner and replacement for every hit
```

### S1 — Delete callable-level replacement

Deliverables:

- preserve general call/lease/reentry facts under neutral modules;
- remove the old IDs, dispatch module surface, Engine registration, macro,
  parser/linker override path, examples, tests, and benchmark rows;
- update ordinary interop docs so Rust exports are callable but not
  replaceable; and
- leave the workspace green before adding a new public service API.

Gate:

```text
production-source audit has zero old replacement identifiers
ordinary Rust/Vela calls and NativeCallContext re-entry remain green
there is no public Rust hot-replacement mechanism at this checkpoint
```

Deleting first is intentional. It prevents the new design from becoming an
optional layer beside an already accepted competing model.

### S2 — Unified Rust type binding foundation

Deliverables:

- implement `InteropTypeId`, `TypeBinding`, `TypeAbiFingerprint`, and one sealed
  registry snapshot;
- implement `Value` versus `Host` storage policy and owned/shared/exclusive/
  construct receiver capabilities;
- expose constructors, static and instance methods, fields, indexes,
  iteration, protocols, effects, lifetime, and escape metadata;
- make the script value carry a compact copyable HostRef handle backed by dense
  root-local host-slot and borrow-group tables, with one metadata/lease entry
  shared by every alias;
- prepare dense field/method IDs and typed adapter thunks from the sealed
  TypeBinding snapshot, and use an allocation-free inline request set for
  common-arity atomic host-argument preflight;
- provide manual custom-type registration plus `Value`, `Host`, and `methods`
  derive generation; and
- make compile, Runtime, reflection, and LSP use the same binding facts.

Gate:

```text
one custom Value type and one custom Host type construct and call Rust methods
T, &T, and &mut T share one identity but enforce receiver capabilities
external unannotatable types register through the same TypeBinding API
Rust host objects remain outside the script GC
copying View/MutView allocates nothing and creates no new lease/refcount
common-arity alias preflight allocates nothing and remains atomic
```

Status: **Accepted.** The checkpoint proves one sealed registry and ABI,
manual and derive-generated Value/Host registration, exact receiver
capabilities, compact generational `HostRef` slots, prepared field/method and
lease plans, and allocation-free common-arity alias preflight. Rust host
objects remain outside the script GC, and copied aliases share one metadata and
lease entry. Detailed implementation chronology remains in Git; current status
is tracked in [progress.md](progress.md).

### S3 — Standard Rust types, views, and collection protocols

Deliverables:

- bind supported primitives, String/bytes, Option/Result/tuples, Vec/slices/
  arrays, BTreeMap/HashMap, and BTreeSet/HashSet;
- implement owned, View, and MutView representations with exact fixed/growable
  mutation capability;
- implement Sequence/Iterable/MapLike/SetLike once, including filter,
  group_by, fold, collect, sorting, and budget rules;
- synthesize recursive concrete bindings from type facts without handwritten
  per-instantiation method registration;
- pass views across nested Rust/Vela calls through scoped reborrow;
- preserve or generate prepared HostTargetPlan/index/method operations so
  linked static access materializes no HostPath/segment vector and performs no
  name/reflection lookup; and
- expose prepared bulk collection operations for host-backed iteration,
  grouping, filtering, collection, and mutation where they reduce repeated
  boundary setup without changing write-through semantics.

Gate:

```text
owned/shared/exclusive standard type matrix passes in both directions
BTreeMap and HashMap share MapLike behavior while preserving registered ABI
filter/group_by/collect work on owned values and borrowed views
mutating views write through immediately and shared views reject mutation
implicit mutable copy-in/copy-out is rejected
linked static host paths and methods use prepared dense operations
bulk host-backed operations preserve budgets, identity, and immediate writes
```

Status: **Active.** The accepted foundation already covers recursive concrete
standard bindings; exact owned, shared, fixed, and growable representation
facts; generated sync/async borrowed collection adapters; scoped retained
reborrow; prepared field/index/key plans; deterministic budgeted collection
projections; and immediate write-through for the implemented Array, Map, and
Set mutations.

The remaining exit work is deliberately expressed as capability gaps, not a
method-by-method chronology:

- complex-element borrowed views with exact identity, lease, escape, and nested
  reborrow proof;
- remaining element/key methods and live or resumable traversal behavior;
- remaining transactional bulk mutations with conversion, budget, and stale
  snapshot failure before mutation;
- richer user-defined collection adapters through the same protocol surface;
- prepared element-method, grouping, filtering, and traversal operations with
  no successful-path name lookup, reflection walk, or HostPath materialization;
  and
- the complete owned/shared/exclusive matrix and phase-wide validation gate.

Current details and the next selected gap live in
[progress.md](progress.md). Do not append completed method lists here.

### S4 — Service contract and Rust-only generation

Deliverables:

- implement `#[vela::service]` and `#[vela::service_set]` schema generation on
  the sealed TypeBinding registry;
- reject service signatures whose transitive types lack complete bindings;
- generate object-safe sync/async dispatch and Rust default composites;
- implement whole-set generation identity, `ArcSwap` publication, safe-point
  pinning, and same-controller rollback validation;
- migrate the fixture's callers to the generated service set; and
- keep the generated Rust-default branch as a direct Rust call that creates no
  HostRef, performs no VM entry, and allocates nothing after root pinning.

Gate:

```text
business trait implementations and call sites contain no patch logic
all methods execute Rust defaults through one pinned generation
the complete transitive parameter/return type closure is validated
old and new roots retain their exact generations across activation/rollback
the Rust-default service branch pays no cross-language HostRef conversion
```

### S5 — Partial Vela service and cross-service vertical slice

Deliverables:

- import service schemas into Vela and link sparse `#[service_impl]` blocks;
- stage one method while adjacent methods remain Rust, then stage a second
  exact-base Delta while the first Vela implementation remains active;
- implement `base` and `services.other.method(...)` on the pinned generation;
- preserve registered constructors, Rust methods, custom types, container
  protocols, HostRef reborrow, borrowed returns, and type identity throughout
  the Rust/Vela/Rust chain;
- use the active root-local lease/provenance entry for nested reborrow without
  global owner reacquisition, business-ID resolution, or per-alias metadata;
- reject ABI/effect/identity/duplicate/unknown-method candidates atomically;
- reject stale-base activation without changing active state; and
- explicitly restore one inherited Vela method to its Rust default.

Gate:

```text
Rust default -> activate one Vela method -> adjacent Rust -> rollback
Delta over active Vela -> inherit old Vela method -> replace another -> activate
explicit RustDefault removes an inherited Vela implementation
stale-base candidate cannot overwrite a concurrent activation
Vela constructs and calls a registered custom Rust type inside the patch
Vela service -> Rust service -> patched Vela service uses one generation
collection views retain identity and writes are immediately visible
Vela failure propagates with no Rust fallback retry
nested reborrow retains complete alias preflight without global lookup/allocation
```

This is the first accepted end-to-end service hotfix slice because it proves
that a patch can express realistic Rust-side logic, not only scalar arithmetic.

### S6 — Async, handlers, deployment, and tooling

Deliverables:

- support authored async service methods with generated object-safe adapters;
- retain pinned service/artifact generations and leases across suspension;
- prove cancel, dropped-future, panic-unwind, and no-runtime-mutex behavior;
- model handlers/rules/events exclusively as service contracts;
- expose stage/activate/rollback diagnostics, service metadata, and TypeBinding
  constructors, methods, views, and protocols to CLI/LSP;
- expose immutable Snapshot/Delta bundle build/load metadata, exact base and
  artifact checksums, dry-run staging reports, and stale activation/rollback
  diagnostics to the deployment surface; and
- replace old examples and benchmark rows with service-generation examples.

Gate:

```text
one actor may remain pending while another completes on shared code
actors keep isolated Runtime state
cancellation/drop/unwind releases Runtime borrows and host leases
there is no handler/rule/event-specific replacement API
```

### S7 — Host-framework integration and final acceptance

Deliverables:

- integrate the generated model into a representative domain-neutral
  service/handler call chain without patch-aware business code;
- patch one small method fragment, call Rust base, call another Rust service,
  and call another patched Vela service;
- publish two successive Deltas over existing Vela logic, prove inherited
  selections survive, then fold the resulting desired state into a Snapshot;
- exercise Player/actor mutable references, DTOs, nested Array/Map values,
  registered constructors and Rust methods, View/MutView iteration and
  grouping, business Result, and async handling;
- measure Rust-default and active-Vela service paths plus every frozen S0
  HostRef/lease/path/bulk-operation row, recording allocations and checksums;
  and
- complete structural, documentation, example, fuzz/build, and workspace gates.

Gate:

```text
the host pins once per actor/request safe point
the service caller is identical before and after activation
one atomic generation contains the complete multi-service patch
old in-flight roots finish on old code and new roots enter new code
rollback is publication only and never retries host effects
all old replacement production identifiers remain absent
```

## 3. Acceptance Matrix

### Authoring

- A Rust author writes no Vela adapter and no per-method patch annotation.
- A Vela author implements one method without copying adjacent Rust methods.
- A caller does not branch, use a target string, or construct a Runtime value.
- Direct concrete Rust calls are documented as intentional bypasses.
- Macro UI tests reject every unsupported service signature with a focused
  diagnostic.

### Type interaction

- Standard-library and custom Rust types use the same `TypeBinding` API.
- `T`, `&T`, and `&mut T` retain one type and method identity with exact
  owned/shared/exclusive receiver checks.
- Registered constructors create either typed Vela values or host-owned
  objects according to explicit storage policy.
- Vela can call registered shared, mutating, consuming, and static Rust methods
  without handwritten boundary wrappers.
- Manual external-type registration and derive-generated registration produce
  the same ABI and registry facts.
- Missing type, constructor, method, protocol, conversion, or view support
  rejects a service candidate before activation.

### Dispatch and generation

- Rust-only generation, single partial patch, multiple methods in one service,
  and multiple services in one package all work.
- Activation and rollback publish exactly one whole-set pointer.
- A root pinned before activation never observes a new method selection.
- Nested calls cannot mix generations.
- Cross-controller and cross-schema candidates/rollback tokens are rejected.
- Missing Vela methods resolve to Rust at stage time.
- Vela failures never trigger automatic Rust execution.
- Snapshot omission selects Rust while Delta omission inherits the exact base;
  explicit `RustDefault` removes an inherited Vela implementation.
- A Delta names its exact base generation/artifact, activation and rollback use
  conditional publication, and stale operations change no active state.
- Vela function/module changes and service selections publish in one candidate;
  a flattened generation performs no lookup through patch ancestry.

### Values and collections

- Scalars, String/bytes, records, enums, Option, Result, tuples, and nested
  owned collections round-trip.
- Shared/exclusive host references preserve identity and alias rules.
- Borrowed returns preserve origin/freeze and cannot escape.
- Owned, shared, exclusive-fixed, and exclusive-growable Array cases pass.
- Owned/shared/exclusive Map and Set cases pass with stable keys.
- BTreeMap, HashMap, BTreeSet, and HashSet implement the shared MapLike/SetLike
  protocols without per-concrete method copies.
- Iteration, grouping, collection, sorting, and mutation charge budgets.
- A host-backed collection passes through Vela to Rust and another Vela
  service without materialization or identity loss.

### HostRef hot path

- View/MutView aliases copy one compact handle and share one root-local slot and
  `BorrowLeaseId`; alias copy performs no allocation, `Arc` clone, atomic
  refcount update, or lease acquisition.
- Statically linked host fields, paths, indexes, and methods use prepared dense
  plans/thunks without runtime names, reflection walks, owned HostPath
  materialization, or segment-vector allocation on success.
- Common-arity host-argument sets are preflighted atomically with inline
  storage; conflicting aliases still fail before any Rust reference exists.
- Same-session nested service calls reborrow from current provenance without a
  global lock, owner lookup, business-ID resolution, or HostRef rematerialization.
- Rust-default service selection creates no HostRef and does not enter Vela.
- Prepared collection bulk operations retain budgets, permissions, identity,
  alias rules, generation pinning, and immediate host write-through.

### Execution

- Sync and async methods use the same service model.
- Nested calls inherit artifact, state, heap, budgets, capabilities, effects,
  tracing, cancellation, and HostAccess.
- No service object, target, controller, or generation owns a mutable Runtime.
- Panic, error, cancellation, unpolled future drop, and rollback release every
  scoped borrow and lease.

### Structural

- No active production code contains the deleted callable-slot types/macros.
- No compatibility alias or dual syntax remains.
- No service method uses runtime string lookup on the hot path.
- No per-concrete standard collection implementation is handwritten.
- Rust host state remains outside the script GC.

## 4. Performance Contract

Correctness and authoring coherence come first, but the Rust-default path must
remain suitable for a high-frequency game server.

The accepted Rust-default service call performs:

```text
one already-pinned service-generation access
one generated service/dyn dispatch
one prelinked per-method Rust/Vela selection
ordinary Rust default call
```

It performs no allocation, serialization, Runtime lock, source lookup, string
lookup, hash lookup, reflection walk, collection materialization, or Vela VM
entry when the method resolves to Rust. Pinning is charged once per host root,
not once per nested service call.

Benchmarks compare:

- direct concrete Rust call;
- ordinary Rust trait-object call;
- generated Rust-default service call;
- generated Vela-active service call;
- nested same-generation Rust/Vela chains;
- owned versus host-backed collection calls;
- HostRef alias copy and borrowed-return release;
- static field/path read-write and registered method calls;
- one-, two-, and representative multi-argument shared/exclusive preflight;
- first-level versus nested same-session reborrow;
- per-element versus prepared bulk host-backed collection operations; and
- activation/staging cost outside the request hot path.

No fixed percentage is accepted before the S0 baseline exists. S2-S3 record
value, host-object, method, constructor, and collection-view boundary costs;
S4 records a service-dispatch budget justified by representative calls. S7
must explain every slower HostRef row and cannot close with accidental
per-alias allocation/refcount/lease work, successful static-path name or
reflection lookup, common-arity preflight allocation, or HostRef conversion on
the Rust-default branch. A regression cannot be hidden by weakening generation
coherence, lease safety, type identity, HostAccess policy, or error semantics.

## 5. Validation Commands

Every implementation checkpoint runs the focused crate/macro/UI/example tests
for its changed area plus the repository baseline:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Later phases also run:

```bash
cargo clippy --manifest-path examples/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path examples/Cargo.toml --no-fail-fast
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins
```

The hard-switch audit excludes historical archives, this execution plan, and
the normative model's explicit deletion contract:

```bash
rg -n 'ReplaceableSlot|InterceptSlot|DispatchRoot|DispatchController|DispatchAuthority|register_replaceable_slots|vela_replaceable|#\[override' \
  crates examples tests fuzz docs \
  --glob '!docs/archive/**' \
  --glob '!docs/rust-vela-service-hard-switch-plan.md' \
  --glob '!docs/architecture/rust-vela-service-model.md'
```

At S1 and every later accepted checkpoint, the command must return no
production/API/example matches. Active migration notes may name deleted terms
only when they clearly state that the model is unavailable.

## 6. Explicitly Deferred

The hard switch does not include:

- general script-language generics;
- arbitrary Rust trait reflection or automatic exposure of every trait;
- hot mutation of Rust service trait structure;
- monkey patching of existing Rust or Vela types;
- automatic hotfixing of direct concrete Rust calls;
- transparent mutable copy-in/copy-out for script collections;
- cross-root persistent borrowed container views;
- multi-origin borrowed returns, projection-granular lease domains, and
  durable host handles beyond the conservative root-scoped borrow model;
- async frame migration between service generations;
- a service-owned or process-global Runtime;
- JIT, moving GC, or script-level shared-memory concurrency; or
- more replacement abstractions for handlers, rules, events, providers, or
  individual functions.

## 7. Completion Definition

This plan is complete only when S0-S7 and the full acceptance matrix are green,
the host-framework integration demonstrates the intended authoring form, Vela
can construct and operate on registered standard and custom Rust types across
owned/shared/exclusive representations, and the old callable-level replacement
model is absent from all production paths.

The final architecture should be explainable in one sentence:

```text
Rust registers complete type behavior and supplies a default service
generation; Vela uses those types in sparse method implementations; the host
atomically publishes one complete generation through the same safe boundary.
```
