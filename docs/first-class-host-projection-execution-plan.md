# First-Class Host Projection And Borrow Diagnostics Execution Plan

> **Status:** proposed execution contract; no implementation batch is accepted
> yet
>
> **Track:** cross-cutting Host ergonomics and runtime-safety follow-up
>
> **Migration:** pre-release hard switch from the first implementation batch.
> Replace internal MIR, bytecode, runtime values, Host adapters, generated
> bindings, callers, fixtures, and tests as one contract. Do not add aliases,
> compatibility branches, dual execution paths, fallback to obsolete scoped
> projection behavior, or adapters whose only purpose is keeping old validation
> green. When executable semantics or encoding changes, advance the portable
> artifact format and emit only the current format. The loader recognizes only
> its current format; every other format is simply unknown and follows the same
> generic unsupported-format path before decoding, linking, or activation. Do
> not enumerate old versions or add version-specific rejection, translation, or
> migration logic.

## Hard-Switch Rules

This plan has no compatibility phase. Each migrated surface removes its
superseded producer and consumer in the same verified checkpoint. A later
batch may build on an earlier batch, but it must not keep old and new semantics
selectable through a feature flag, runtime fallback, legacy artifact decoder,
hidden adapter, or test-only path.

Existing tests and fixtures are safety evidence, not a requirement to preserve
obsolete pre-release behavior. When a test expects ordinary fields, collection
elements, or standard iterators to create scoped children, replace that
expectation with the accepted projection contract while retaining equivalent
coverage for permissions, alias conflicts, stale roots, budgets, partial
progress, and genuine Rust borrows. Do not delete or weaken a safety assertion
merely to make the suite pass, and do not add compatibility code merely to keep
an obsolete assertion unchanged.

Temporary compile breaks are allowed only inside an uncommitted worktree.
Every commit must contain one coherent new-contract slice with its affected
callers and tests migrated, focused validation passing, and no compatibility
shim for the replaced slice.

## Persistent Goal

```text
/goal Implement the First-Class Host Projection And Borrow Diagnostics
Execution Plan in docs/first-class-host-projection-execution-plan.md.

Treat docs/goal.md as the product roadmap, docs/architecture.md and
docs/architecture/host-and-registration.md as the technical contract, and
docs/progress.md as current milestone state. Execute batches P0-P7 in order and
stop only when their gates and the complete acceptance matrix pass.

Make ordinary Host field, index, key, collection, and element access produce
first-class typed path projections instead of retained Rust borrows. Reuse the
existing HostRef, HostTargetPlan, PathProxy, prepared HostAccess, generated
ScriptHostFieldAccess, and inline-cache infrastructure. Vela must not expose or
store a real Rust reference, implement a Rust-style borrow checker, infer
last-use release, or place Rust host state under the script GC.

Keep explicit host::release and host::try_release only for genuine retained
scoped capabilities, including non-pathable Rust &T/&mut T returns and custom
iterators that actually retain Rust borrows. A projection rooted at a scoped
HostRef inherits that root's access and becomes stale when the root is
released, but creates no child lease and does not itself require release.

Allow an exact typed Host projection to satisfy supported synchronous generated
Rust &T/&mut T parameters by resolving and borrowing its path only for that
native invocation. Validate root identity, generation, target type, access,
permissions, and the complete argument conflict set before authored Rust runs.
For one canonical root, acquire the root once and use generated struct-field or
standard-container split projection to admit every path set proven disjoint,
including multiple mutable paths. Fail closed for overlapping or unprovable
alias sets, async projected borrows, path escape, forged/stale paths, and paths
invalidated by structural mutation. Do not add a general
HostRef-to-Rust-reference conversion.

Replace standard Host collection iterator leases with root-plus-path cursors
and frozen traversal metadata where no Rust borrow must be retained. Preserve
documented live-read and structural-mutation behavior, budgets, source spans,
and hot-reload generation pinning. Keep genuinely borrowed Rust iterator
returns on the scoped-resource path.

In the same track, make HostObjectBusy, BorrowStillInUse, expired-root,
missing-projection, iterator-invalidation, and await-blocking diagnostics
identify the attempted operation, requested access, source location, producer
origin, parent/child chain, and required child-before-parent release order.
Tooling must suggest release only for genuine scoped resources, never ordinary
projections.

For every batch, run focused tests, preserve representative Host benchmarks,
update docs/decisions.md only for accepted durable decisions, update
docs/progress.md only when milestone truth changes, and commit one coherent
verified checkpoint using Conventional Commits. Do not complete the goal until
focused, workspace, artifact, async, Service, iterator, diagnostic,
performance, and documentation gates pass.

Apply the hard switch from the first implementation checkpoint. Validation
must prove only the accepted projection contract and the intentionally retained
genuine-borrow contract. Never preserve obsolete ordinary scoped-projection
behavior solely because an existing test, fixture, example, benchmark, or
artifact expects it.
```

## 0. Objective And Exit Outcome

The current implementation already has the low-level path engine:

```text
typed HIR Host placement
  -> MIR HostRead/HostWrite/HostMutate/HostCall
  -> linked HostTargetPlan plus dynamic arguments
  -> prepared HostAccess and inline caches
  -> generated ScriptHostFieldAccess traversal
```

It also has a retained-borrow fallback:

```text
complex field or element read
  -> borrow_resolved_host_shared/exclusive
  -> retained scoped child HostRef
  -> parent remains busy
  -> authored host::release in child-before-parent order
```

That fallback currently leaks Rust borrowing mechanics into ordinary source:

```vela
let owner = actor.owner;
let items = owner.items;
let values = items.values;
values.clear();
host::release(values);
host::release(items);
host::release(owner);
```

The accepted ordinary form is:

```vela
let values = actor.owner.items.values;
values.clear();
repair::normalize(values);
```

with this runtime shape:

```text
ordinary root HostRef + typed target plan + owned dynamic arguments
  = first-class typed Host projection

read/write/query/mutate/call/iterate
  -> resolve the projection
  -> acquire invocation-local Rust access
  -> execute
  -> release access before returning to Vela
```

Rust borrowed returns remain explicit resources:

```vela
let selected = lookup::selected_mut(actor); // scoped HostRef<&mut T>
let stats = selected.stats;                  // projection rooted at selected
stats.normalize();                           // no child scoped borrow
host::release(selected);                     // releases the real Rust borrow
```

Completion requires:

- ordinary fields, indexes, collection views, and elements create no retained
  scoped child HostRefs;
- projections remain typed through analysis, MIR verification, bytecode,
  runtime guards, reflection metadata, and tooling;
- projection operations and supported synchronous Rust calls preserve
  immediate write-through;
- standard collection iteration retains no Rust borrow merely for traversal;
- true Rust borrowed returns retain explicit-release safety; and
- Host borrowing failures report actionable source and ownership information.

## 1. Normative Semantic Model

### 1.1 Roots And Projections

A root `HostRef` identifies one canonical Host object in the active root
execution. A projection identifies a location reachable from that root:

```rust
struct HostProjection {
    root: HostRef,
    target: HostTargetPlan,
    args: Vec<HostPathArgOwned>,
    access: ProjectionCapability,
}
```

This shape is illustrative. The implementation should evolve the existing
`PathProxy`, not add a parallel public path system.

A projection stores no Rust pointer, reference, or lease guard. Copying one
copies path identity only. Shared roots produce read-only projections;
exclusive roots may produce writable projections. Field metadata,
permissions, effects, and protocol facts may restrict access further but never
widen it.

For a registered Host-shaped target, assigning a field to a local creates a
projection rather than performing a complex read:

```vela
let inventory = actor.state.inventory;
```

Terminal scalar and detached owned-value expressions still read immediately:

```vela
let count = actor.state.inventory.count;
```

Sealed TypeBinding and field facts decide projection versus owned read. A
failed scalar read followed by a runtime scoped-borrow fallback is not the
ordinary semantic route.

### 1.2 Composition And Liveness

Member and index operations extend the target:

```text
projection(actor, state.inventory)
  .field(items)
  .key(item_id)
  .field(count)
```

Static fields and constant indexes/keys remain compiled plan parts. Dynamic
arguments retain exact `HostCollectionKey` identity and are not converted to
diagnostic strings. Composition must preserve prepared traversal and inline
cache eligibility.

A projection is valid only while its canonical root is live in the same root
execution and generation. Root release, expiry, replacement, or cross-root use
makes the next projection operation fail with a source-spanned stale-root
diagnostic. Releasing a genuine scoped root does not wait for projections;
projections retain no borrow and simply become stale.

`host::release` and `host::try_release` accept only genuine retained scoped
capabilities. They never release a projection's ancestor. A statically known
projection release is a source error; a dynamic `Any` case fails closed at
runtime. There is no compatibility behavior that releases an ancestor.

### 1.3 Genuine Rust Borrowed Returns

A Rust function or method returning `&T`, `&mut T`, `Option<&T>`, or
`Result<&T, E>` may return a location that cannot be represented by a registered
path. A successful payload remains a scoped HostRef backed by the real borrow
and existing provenance checks.

Further fields, indexes, methods, and collection operations below that root
use projections. They create no retained descendant unless a later Rust call
itself returns another non-pathable borrow.

```text
Rust &mut T return                    -> scoped HostRef, explicit release
field/index below returned T          -> projection, no release
later Rust &mut U return              -> new scoped HostRef, explicit release
```

Child-before-parent release applies only to genuine retained descendants.

### 1.4 Passing A Projection To Rust

Supported synchronous generated callables may accept exact projections:

```vela
let values = actor.state.inventory.values;
repair::normalize(values);
```

```rust
#[vela]
fn normalize(values: &mut HashMap<ItemId, Item>) {
    // The reference exists only inside this invocation.
}
```

Before authored Rust executes, the generated boundary must:

1. resolve every root and generation;
2. validate sealed target type and exact path shape;
3. validate access, permissions, capabilities, and effects;
4. canonicalize the complete argument set;
5. reject overlapping or unsupported mutable arguments;
6. acquire root leases atomically;
7. project admitted paths through generated typed traversal;
8. invoke Rust under an invocation-local callback; and
9. drop every projected reference and lease before returning to Vela.

No public `FromScriptArg` or low-level helper may provide a general dynamic
path-to-reference conversion. This authority belongs only to generated,
registered call boundaries with exact parameter types.

Requests are grouped by canonical root. Each root is acquired exactly once at
the strongest access required by the group, then a generated projection-group
operation splits that root inside one invocation-local callback. It must not
acquire the same exclusive root independently for each argument.

The conflict rules are:

```text
shared + shared:
  overlap is allowed

mutable + mutable:
  allowed only when generated code or a registered container protocol proves
  the paths disjoint

shared + mutable:
  allowed only when the paths are proven disjoint

exact equality or a prefix relationship involving mutable access:
  always rejected
```

Disjointness proof is structural rather than textual:

- `#[derive(ScriptHost)]` generates safe splitting for different stored struct
  fields and recursively groups requests that enter the same field;
- fixed arrays, slices, and Vec-like bindings split distinct checked indexes
  through their safe sequence adapter;
- Map bindings split distinct checked keys only when the concrete adapter
  provides a safe multi-value mutable projection operation;
- enum variant fields split only within the validated active variant;
- computed properties, opaque adapters, Set elements, equal dynamic keys, and
  custom containers without an explicit split capability fail closed; and
- unequal diagnostic path strings alone never prove non-aliasing.

The generated projection group may reuse the existing erased lease-group and
higher-ranked callback infrastructure internally, but it is invocation-local.
It does not intern child HostRefs, enter the scoped-resource table, or require
authored release. No raw pointer may be used to manufacture disjointness.

Async projected Rust borrows remain rejected until a separate dependent-future
proof is designed and accepted. Existing async root Host and Service leases
remain unchanged. Rejection must occur during compilation or schema sealing,
before authored Rust begins.

### 1.5 Collections And Iterators

Standard Host Array, Map, and Set iterators rooted at a projection store:

```text
root projection
frozen Array extent or deterministic Map/Set key snapshot
cursor position
prepared element target/access
```

They retain no Rust collection borrow between polls. Each poll performs one
prepared live read and releases access before yielding. Complex elements yield
typed element projections instead of scoped child HostRefs.

Preserve current traversal semantics:

- later value replacement is visible;
- structural growth is outside the frozen traversal;
- removal of a pending key/index reports an error;
- Array shrink that invalidates a pending index reports an error;
- bulk `clear`, `extend`, and `retain` keep existing preflight and budget
  behavior; and
- `for`, transforms, callbacks, and terminal operations keep exact budgets.

An arbitrary Rust iterator may genuinely retain a Rust borrow. Such iterators
remain scoped resources with explicit release and await blocking. Tooling and
diagnostics must distinguish them from standard path cursors.

## 2. Diagnostic Contract

Every retained scoped binding records enough provenance to explain its
lifetime:

```text
canonical HostRef and access kind
producer callable/operation and source span
parent retained HostRef, when present
borrow group identity
registered type and display path
resource category: borrowed return, borrowed iterator, or other retained view
```

Variable names should be included when stable authored-binding metadata is
available. Diagnostics must remain useful without them.

Projection errors report the attempted operation, requested access, complete
diagnostic HostPath, source span and call stack, root type/identity/generation,
and missing, stale, or conflicting segment.

`HostObjectBusy` must show both sides:

```text
HostObjectBusy at scripts/repair.vela:18:5
cannot acquire exclusive access to actor.state.resources
requested by: resources.clear()
blocked by: scoped mutable Host `selected`
created at scripts/repair.vela:9:20 by inventory::selected_mut(actor)
root: ActorState#42 generation 7
release `selected` before this operation
```

It must not suggest release when the blocker is invocation-local Rust work that
the script cannot close.

`BorrowStillInUse` must show genuine descendants and required order:

```text
BorrowStillInUse at scripts/repair.vela:24:5
cannot release `owner` while 2 retained descendants are active
release in this order:
  1. cursor, created at line 16 by rows.borrowing_iter()
  2. selected, created at line 12 by owner.selected_mut()
  3. owner
```

Projection descendants are not listed because they retain no borrow.

Pre-await diagnostics continue to inspect the complete scoped-resource table,
group blockers by parent, and report child-before-parent order. Standard path
cursors do not block await merely because they contain a path. A genuine
scoped root still blocks await and cursors rooted at it become stale after its
release.

Language service and LSP must:

- mark projections non-detachable but not releasable;
- mark genuine scoped borrowed returns/iterators releasable;
- offer release actions only for genuine resources;
- show root, access, and target type in hover/inlay information;
- preserve source spans through path composition; and
- reject discarded or unnameable genuine scoped producers before execution.

No diagnostic or code action may implement whole-function borrow analysis or
insert an inferred release.

## 3. Current Repository Anchors

- `crates/vela_host/src/path.rs`: `HostRef`, `HostSlotRef`, diagnostic paths.
- `crates/vela_host/src/proxy.rs`: current root-plus-target `PathProxy`.
- `crates/vela_host/src/target.rs` and `resolved.rs`: target plans, dynamic
  arguments, prepared steps, and access operations.
- `crates/vela_host/src/access.rs`: current scalar-read to scoped-read fallback.
- `crates/vela_host/src/adapter.rs`: lease and scoped-projection hooks.
- `crates/vela_host/src/object.rs` and collection modules: recursive traversal
  and collection protocols.
- `crates/vela_macros/src/script_host/emission.rs` and
  `emission/scoped_borrow.rs`: generated prepared traversal and child borrows.
- `crates/vela_mir/src/builder/host.rs`: exact Host placement lowering and the
  current lost-prefix rejection.
- `crates/vela_mir/src/operations.rs` and verifier modules: executable Host
  operations and effects.
- `crates/vela_bytecode`: linked plans, cache sites, verification, artifacts.
- `crates/vela_vm/src/host_collection_projection.rs`: Host collection
  snapshots, iterators, and scoped iterator retention.
- `crates/vela_vm/src/indexing.rs`, `iteration/source.rs`, dynamic method
  resolution, record fields, and guards: current opaque `PathProxy` cases.
- `crates/vela_engine/src/runtime/execution_host.rs`, `scoped_access.rs`, and
  `scoped_projection.rs`: root/scoped leases, retained children, release, and
  conflict reporting.
- `crates/vela_engine/src/runtime/call_args.rs`: exact root argument leases and
  invocation-local typed provenance.
- `crates/vela_macros/src/export` and `service`: generated Rust parameter
  restoration.
- `crates/vela_language_service`: release actions, diagnostics, hovers, inlays.

## 4. Implementation Batches

### P0 - Freeze Semantics, Inventory, And Baselines

Deliverables:

- inventory every producer of scoped child fields, collection elements, and
  iterators;
- inventory every operation that accepts `HostRef` but rejects `PathProxy`;
- freeze fixtures for nested fields, genuine borrowed returns, collection
  iteration, single- and same-root multi-projection Rust calls, and target
  diagnostics;
- record allocation, execution-unit, and wall-time baselines for direct and
  nested Host access, iteration, and borrowed returns; and
- choose the compact projection representation after measuring current
  heap-allocated `PathProxy` cost.

P0 fixtures that reproduce obsolete explicit-release behavior are comparison
baselines only. They are replaced by new-contract fixtures when their owning
surface migrates and never become compatibility acceptance tests.

Gate:

```text
inventories name every retained producer and PathProxy rejection
fixtures reproduce the current ordinary explicit-release burden
diagnostic fixtures preserve the insufficient baseline for comparison
performance baselines use one stable toolchain and protocol
```

### P1 - Add First-Class Typed Projection Values

Deliverables:

- evolve `PathProxy` into an ordinary typed runtime projection value;
- add verified MIR/bytecode construction and composition where a runtime value
  is required;
- preserve target type and shared/exclusive capability through locals,
  branches, admitted containers, and arguments;
- capture dynamic keys/indexes without string conversion;
- distinguish projection facts from scoped-resource facts; and
- advance the portable artifact format at the first incompatible semantic or
  encoding change, emit only that current format, and keep one current-format
  decoder with a generic unknown-format outcome for every other identifier.

Gate:

```text
let child = root.field preserves an exact typed path through a local
projection aliases create no lease or scoped-resource entry
dynamic keys preserve canonical identity
verifiers reject forged and ill-typed projections
the loader recognizes exactly the current artifact format
every other format identifier follows the same unknown-format path
no old-format registry, decoder, translator, or targeted rejection branch exists
```

### P2 - Route Ordinary Host Operations Through Projections

Deliverables:

- compose fields, variants, indexes, and keys from a root or projection;
- route reads, writes, mutation, removal, queries, collection mutation, and
  methods through the combined target;
- preserve prepared access, caches, permissions, generations, budgets, and
  source spans;
- make complex generated fields produce projections, not scoped children;
- resolve projections rooted at scoped borrowed returns per operation; and
- reject release of a projection without releasing its root;
- remove each superseded ordinary scoped-read producer and fallback when its
  projection operation lands rather than retaining a dual path for later.

Gate:

```text
ordinary nested access requires no authored release
write-through and partial-progress semantics are unchanged
shared/exclusive capability remains correct
released roots make derived projections stale
common generated field chains never reach read_scoped_host
no migrated operation can select obsolete scoped-projection behavior
```

### P3 - Replace Standard Collection Child Borrows With Paths

Deliverables:

- operate Array/Map/Set protocols on projection receivers;
- return scalars by value and complex elements as projections;
- implement standard iterators as path cursors with frozen traversal metadata;
- route `for`, iteration methods, callbacks, transforms, and terminals through
  prepared live reads;
- preserve structural mutation and missing-pending-element behavior;
- retain scoped handling only for custom iterators that actually borrow; and
- remove standard collection child-borrow and iterator-lease paths in the same
  checkpoints that introduce their path-cursor replacements.

Gate:

```text
standard iteration retains no Rust borrow
complex element mutation writes through its key/index path
replacement/growth/removal semantics match the contract
custom borrowed iterators still enforce release and await safety
budgets remain exact
no standard collection compatibility iterator remains
```

### P4 - Bind Projections To Supported Synchronous Rust Parameters

Deliverables:

- admit exact projections for generated synchronous Host `&T`/`&mut T`
  parameters in free functions, methods, and Services;
- group all arguments by canonical root and acquire each root exactly once at
  the strongest required access;
- add invocation-local generated reborrow callbacks without a public general
  conversion API;
- build a structural path conflict trie and reject exact/prefix mutable
  overlap before Rust executes;
- generate safe recursive splitting for distinct stored struct fields;
- add safe distinct-index splitting for fixed arrays, slices, and Vec-like
  bindings;
- add safe distinct-key splitting for standard Map bindings and an explicit
  opt-in split capability for custom containers;
- support mixed shared/mutable same-root arguments only when every mutable
  relationship is proven disjoint;
- preserve the direct-root fast path; and
- reject async projected borrows, opaque/computed paths, overlapping paths, and
  unsupported custom-container splits before authored Rust begins.

Gate:

```text
nested shared and exclusive projections reach natural Rust parameters
multiple disjoint same-root mutable fields reach one Rust invocation
distinct sequence indexes and supported Map keys split safely
Rust sees exact original field/container identity
type/generation/access/permission/alias failures precede Rust execution
references and projections cannot escape the callback
one canonical root group acquires one root lease rather than one per argument
direct root behavior and performance remain intact
```

### P5 - Make Diagnostics Actionable

Deliverables:

- attach producer span, category, parent, access, and target to every retained
  scoped binding;
- enrich busy, still-in-use, expired-root, stale/missing projection, iterator,
  and await errors;
- report requested and blocking sides plus release order;
- preserve provenance through aliases, Service calls, methods, and iterators;
- update LSP actions, hover, inlay, and rendering for the projection/resource
  distinction; and
- remove obsolete ordinary-projection diagnostics and release actions instead
  of preserving alternate rendering for old behavior.

Gate:

```text
target errors include source and attempted Host path
busy/release/await errors include producer and retained chain
release suggestions appear only for genuine resources
diagnostics remain useful without variable names
no Rust pointer or reflection permission is required
```

### P6 - Audit The Hard Switch And Remove Residual Scoped-Projection Paths

Deliverables:

- audit that earlier batches already deleted scoped child creation used only by
  ordinary fields, standard complex elements, and standard path cursors, then
  delete any residual producer as a correctness defect;
- retain infrastructure for genuine borrowed returns, slices, custom borrowed
  iterators, and other non-pathable resources;
- audit that tests and docs were migrated with their owning surfaces and no
  ordinary path still requires release;
- audit reflection and dynamic `Any` paths against recreating ordinary scoped
  children; and
- split oversized modules by projection, lease, iterator, and diagnostic
  responsibility where required.

Gate:

```text
no ordinary-field retained child producer remains
every retained producer corresponds to a documented real Rust borrow
reflection/dynamic dispatch preserve the same split
no legacy compatibility or implicit ancestor release remains
no feature flag, fallback, legacy decoder, or test-only entry can restore old behavior
```

### P7 - Acceptance, Performance, And Documentation

Deliverables:

- run focused and workspace validation;
- run stable interleaved benchmarks against P0;
- update architecture, interop usage, decisions, and examples;
- update progress only when active focus or status changes;
- confirm portable artifacts use the current format and the loader contains no
  old-version registry, decoder, translator, or targeted rejection branch; and
- write an archived acceptance report with the matrix and measurements.

Gate:

```text
correctness, lifetime, async, Service, collection, reflection, LSP, and
artifact tests pass
performance gates pass or the design is revised
active docs contain no ordinary-field release guidance
the report names every intentionally retained scoped category
validation contains no legacy-mode run and no compatibility-only assertion
```

## 5. Required Acceptance Matrix

### 5.1 Projection Semantics

| ID | Proof |
|---|---|
| HP-01 | A nested Host field assigned to a local is a projection, not a scoped child. |
| HP-02 | Projection aliases create no lease and require no release. |
| HP-03 | Scalar reads observe current Host state. |
| HP-04 | Writes and compound mutations write through immediately. |
| HP-05 | Shared roots reject mutation; exclusive roots permit registered mutation. |
| HP-06 | Dynamic integer, String, Bytes, and HostRef keys preserve identity. |
| HP-07 | Released/expired roots make projections stale. |
| HP-08 | Releasing a projection never releases its ancestor. |
| HP-09 | Projections cannot detach, persist, or cross roots/tasks. |
| HP-10 | Reflection cannot bypass field permissions through a projection. |

### 5.2 Genuine Borrowed Returns

| ID | Proof |
|---|---|
| BR-01 | Rust `&T` return remains a shared scoped HostRef. |
| BR-02 | Rust `&mut T` return remains an exclusive scoped HostRef. |
| BR-03 | Fields below a borrowed return are projections. |
| BR-04 | A later borrowed Rust return creates a genuine retained descendant. |
| BR-05 | Parent release rejects a live genuine descendant. |
| BR-06 | Root teardown handles success, error, panic, cancellation, and drop. |

### 5.3 Rust Invocation

| ID | Proof |
|---|---|
| RI-01 | A shared nested projection reaches synchronous Rust `&T`. |
| RI-02 | An exclusive nested projection reaches synchronous Rust `&mut T`. |
| RI-03 | Rust sees exact original field/container identity. |
| RI-04 | Wrong type/generation/access/permission fails before Rust. |
| RI-05 | Overlapping mutable paths fail before Rust. |
| RI-06 | Different generated struct fields support multiple same-root mutable projections. |
| RI-07 | Distinct sequence indexes and supported Map keys split safely at runtime. |
| RI-08 | Custom/opaque paths without split capability fail before Rust. |
| RI-09 | Async projected borrowed parameters fail before Rust. |
| RI-10 | Direct root Host parameters retain their fast path. |

### 5.4 Collections And Iterators

| ID | Proof |
|---|---|
| CI-01 | Array iteration works from a projection receiver. |
| CI-02 | Map keys/values/entries/direct `for` preserve frozen traversal. |
| CI-03 | Set values/direct `for` preserve frozen traversal. |
| CI-04 | Complex elements are writable projections. |
| CI-05 | Replacement after iterator creation is visible. |
| CI-06 | Growth is outside frozen traversal. |
| CI-07 | Pending removal/shrink reports key/index and source. |
| CI-08 | Standard cursors do not enter the scoped-resource table. |
| CI-09 | Custom borrowed Rust iterators still require release. |
| CI-10 | Callback/terminal operations preserve budgets and partial progress. |

### 5.5 Diagnostics And Tooling

| ID | Proof |
|---|---|
| DG-01 | `HostObjectBusy` reports attempted path/access/source and blocker origin. |
| DG-02 | `BorrowStillInUse` reports genuine descendants and release order. |
| DG-03 | Await blocking reports every retained resource grouped by parent. |
| DG-04 | Stale projection reports root and attempted operation. |
| DG-05 | Iterator invalidation reports frozen key/index and cause when known. |
| DG-06 | Aliases preserve one resource-group identity. |
| DG-07 | LSP release actions appear only for genuine resources. |
| DG-08 | Diagnostics remain useful without local-variable names. |

### 5.6 Artifacts, Reload, And Isolation

| ID | Proof |
|---|---|
| AR-01 | Current artifacts encode the projection instruction/value contract exactly. |
| AR-02 | The loader recognizes only the current format; every other identifier follows one generic unknown-format path with no old-version-specific logic. |
| AR-03 | Active frames retain exact plans across reload. |
| AR-04 | Projections cannot cross Runtime/root/session boundaries. |
| AR-05 | Detached tasks reject nested projections with an exact value path. |
| AR-06 | Service generations cannot mix plans or schema epochs. |

## 6. Performance Contract

P0 freezes same-toolchain baselines for:

```text
direct scalar Host read/write
three- and five-segment nested read/write
static and dynamic Map access
Map iteration over scalar and complex values
projection construction and alias copy
projection-to-Rust shared/exclusive calls
same-root calls with two and three disjoint mutable projections
genuine borrowed return create/use/release
```

Retention gates:

- direct scalar Host operations regress by no more than 5%;
- nested Host access and standard collection iteration regress by no more than
  10%, and nested access should improve when retained allocation disappears;
- projection alias copy performs no lease, lock, or reference-count operation;
- one same-root projection group acquires one root lease regardless of argument
  count, and conflict checking stays allocation-free for the common inline
  argument count;
- common static projections do not allocate an unbounded `Vec` and `Box` per
  field segment;
- inline-cache hits remain generation/schema correct; and
- Actor/Runtime memory guardrails regress by no more than 5% without an
  explicit measured trade-off.

If heap `PathProxy` allocation misses these gates, P1 must use a compact
root-local projection slot or another measured representation while retaining
`HostTargetPlan` and typed dynamic arguments. Strings and raw pointers remain
forbidden in the hot operational path.

## 7. Validation

Run affected packages first:

```bash
cargo test -p vela_host
cargo test -p vela_mir
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
cargo test -p vela_macros
cargo test -p vela_language_service
cargo test -p vela_lsp_server
```

Interop batches additionally run generated macro fixtures, Service
source/reload suites, async cancellation/drop suites, collection-view suites,
and portable artifact tests identified in P0.

Validation runs only the new contract after a surface migrates. Update obsolete
tests, fixtures, snapshots, examples, and benchmarks in the same commit as that
surface. A green legacy mode, compatibility feature, legacy decoder, or
old/new test matrix is not an acceptance result for this plan.

The phase-closing gate is:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run relevant benchmarks under `docs/performance.md` and run architecture
file-size, unsafe-boundary, generated-path, and documentation-link audits.

## 8. Non-Goals

This plan does not add:

- a Rust-style Vela borrow checker or last-use analysis;
- implicit compiler-generated release;
- automatic ancestor release when a projection dies;
- Rust references, raw pointers, or lease guards in script values, GC,
  reflection, persistent state, or artifacts;
- a general dynamic PathProxy-to-reference conversion;
- multiple mutable projections whose disjointness is not proven by generated
  field splitting or a registered container split protocol;
- async projected Rust borrows without a separately accepted proof;
- durable or cross-root projections;
- silent skipping of removed entries in frozen traversal;
- a second Host path/adapter system;
- compatibility aliases for ordinary-field scoped children;
- dual old/new execution modes, legacy feature flags, old-version artifact
  registries/decoders, targeted old-format rejection branches, or fallback to
  obsolete scoped-projection behavior; or
- code retained solely to satisfy tests, fixtures, snapshots, examples,
  benchmarks, or artifacts that assert the superseded contract.

## 9. Completion Checklist

- [ ] P0 inventory and baselines are frozen.
- [ ] P1 typed projections pass verifier and artifact gates.
- [ ] P2 ordinary Host operations create no scoped child borrows.
- [ ] P3 standard collection elements and iterators use paths.
- [ ] P4 synchronous Rust parameters accept exact projections.
- [ ] P5 borrow and projection diagnostics are actionable.
- [ ] P6 obsolete ordinary scoped-projection machinery is removed.
- [ ] P7 workspace, performance, docs, and acceptance gates pass.
- [ ] Every migrated surface hard-switches callers and tests in the same
      checkpoint with no compatibility shim or dual path.
- [ ] HP-01 through HP-10 pass.
- [ ] BR-01 through BR-06 pass.
- [ ] RI-01 through RI-10 pass.
- [ ] CI-01 through CI-10 pass.
- [ ] DG-01 through DG-08 pass.
- [ ] AR-01 through AR-06 pass.
- [ ] Active docs describe the accepted model.
- [ ] An archived acceptance report records final proof and every retained
  scoped category.
