# Vela crate architecture and API audit

Date: 2026-09-04

Reviewed revision: `d22121ed0004`

Toolchain: `rustc 1.98.0`, `cargo 1.98.0`

## Executive assessment

Vela has a stronger correctness foundation than its size and milestone status
would normally suggest. Stable definition identities, verified MIR and bytecode,
generation-owned code, explicit host handles, deterministic collections, and
extensive conformance tests are all good architectural choices. The project does
not need a rewrite.

It is not yet safe to describe the runtime as production-ready, however. Four
correctness defects should block that claim:

1. finite-slot incremental GC can collect newly reachable objects because the
   collector has neither a complete incremental mark state nor allocation/write
   barriers;
2. a hot update is not bound to the program generation against which it was
   checked, so a stale update can bypass compatibility checking; and
3. reflection converts stable unsigned IDs to signed integers by saturation,
   causing many distinct IDs to become the same value; and
4. converting an ordinary cyclic heap value to `OwnedValue` recurses without
   cycle detection and can overflow the host stack.

Fail-open host/reflection authority defaults are also release blockers for
deployments that expose those paths.

The dominant architectural problem after those defects is duplication, not a
missing abstraction. Runtime artifacts retain several forms of the same program;
the engine rebuilds VM dispatch and reflection state for each call; compiler and
runtime layers share more dependencies than they need; type/schema information
exists in several parallel models; and standard-library metadata is maintained
in several hand-written tables. These choices make a carefully designed system
look and behave more complicated than necessary.

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

The retained artifact is the clearest example. `LinkedArtifact` contains a
`LinkedProgram`, a `ProgramImage` with unlinked code and module graph data, and
verified MIR. The portable artifact already proves that MIR is not required by
the interpreter. Checked-in measurements report roughly 650--659 MB RSS for 16
retained 200-function/lambda generations
(`docs/performance.md:417-421,456-460,502`). Some of that is allocator and
process overhead, but the representation overlap is structurally expensive.

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
5. one neutral frozen schema feeds compiler validation, reflection, bindgen, and
   tooling views.

## Comparison with mature implementations

This review does not recommend copying another language wholesale; Vela's hot
reload and host write-through requirements are distinct. Several mature designs
nevertheless provide useful pressure tests:

- [rust-analyzer's architecture](https://rust-analyzer.github.io/book/contributing/architecture.html)
  keeps protocol knowledge at the edge, uses immutable snapshots, and makes
  per-file syntax/`ItemTree` data stable across unrelated body edits. Vela's
  language service currently clones mutable database roots and rebuilds global
  HIR structures, so its advertised incremental model is less incremental than
  its API suggests.
- [rustc's query model](https://rustc-dev-guide.rust-lang.org/query.html)
  demonstrates the value of keyed, memoized derivations and dependency tracking.
  Vela's analysis fixed points and HIR closure queries often rescan complete maps
  instead.
- [Luau's performance design](https://luau.org/performance/) favors a compact
  value representation, interpreter-specialized instructions, inline caching,
  and avoiding allocation in hot loops. Vela already follows the first three,
  but recreating VM registries on every engine call and recomputing collection
  sizes defeats that work.
- [Wren's embedding API](https://wren.io/embedding/) keeps the foreign boundary
  small and session-oriented. [Rhai's embedding API](https://rhai.rs/book/start/features.html)
  similarly makes common registration and calls direct. Vela's `HostRef` model
  is safer for mutable game-server state than either project's simplest path,
  but `ScriptStateAdapter`, `HostAccess`, proxy argument preparation, leases,
  and explicit release expose too much of the machinery to ordinary embedders.

## Cross-cutting findings

### C-01: incremental GC is not correct across safe points

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

**Action:** immediately make collection atomic or disable the finite-slot API.
Add a regression with an allocation and a new container/frame edge between
sweep steps. Implement incremental marking only with a tri-color state,
allocation barrier, write barrier, root handling, and a real time deadline that
also accounts for marking.

### C-02: hot updates lack compare-and-swap identity

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

**Action:** store base `ProgramVersionId`, runtime identity, executable
generation, and artifact checksum in every update, and reject a mismatch
atomically. Remove `Clone` from the runtime; expose a cloneable staging handle,
or deliberately share current state through one atomic generation owner.

### C-03: reflected stable IDs are lossy

Reflection exposes unsigned 64- and 128-bit stable IDs as signed script
integers by saturating them to `i64::MAX`
(`crates/vela_reflect/src/types.rs:45-65`,
`members.rs:42-50`, `member_records.rs:241-245`, and
`modules/records.rs:34-46,120-127`). This is not a rare overflow corner case:
roughly half of uniformly distributed 64-bit hashes exceed `i64::MAX`, and
almost every 128-bit ID does. Distinct types and members consequently collapse
to the same reflected identity.

**Action:** expose an opaque ID value, a tagged pair of unsigned words, or a
canonical hexadecimal string. Round-trip tests must cover the top unsigned bit
and multiple 128-bit IDs.

### C-04: ordinary owned-value egress can recurse forever on cyclic heaps

Vela heap graphs intentionally support aliases and cycles, but
`value_to_owned_inner` recursively follows every `HeapRef` without tracking the
active path (`crates/vela_vm/src/heap_values.rs:707-829`). `OwnedValue` cannot
represent cycles, so an ordinary host return/materialization through this path
can overflow the Rust stack. Detached-task graph egress has separate cycle-safe
handling; it does not make this converter safe.

**Action:** return a typed non-detachable/cyclic-value error, or introduce an
explicit graph representation. Never recursively materialize without an active
node set and depth/budget accounting.

### H-01: each script execution call rebuilds generation-wide runtime state

Sync and async runtime calls construct a fresh VM
(`crates/vela_engine/src/runtime/mod.rs:686-725,838-879`).
`Engine::vm_for_artifact` clones/enriches reflection state and installs every
function family (`engine.rs:1127-1170`); task reflection may clone it again.
Checked-in boundary results show about 285 allocations and 49 KB allocated for
representative static field and collection calls
(`docs/performance.md:156-177`).

**Action:** build one immutable `GenerationExecutionImage` at compile/reload
time containing linked code, std/native/method dispatch, reflected schema, and
cache layout. A call should create only mutable stack/heap/budget/host-session
state.

### H-02: runtime artifacts retain compiler-only representations

`LinkedArtifact` retains linked code, `ProgramImage` with unlinked code and a
module graph, and verified MIR (`crates/vela_bytecode/src/artifact.rs:56-69`).
MIR is retained beyond interpreter needs for JIT eligibility and source-side
validation even though the JIT milestone is not implemented. The artifact
checksum also clones the full linked program and hashes its Rust `Debug` output
(`artifact.rs:161-181,235-248`), which is costly and not a durable serialization
contract.

**Action:** make the runtime artifact compact and versioned. Put MIR and
unlinked compiler data in an optional compiler cache/diagnostic sidecar. Define
a canonical fingerprint encoder over explicit fields. Split runtime code
representation from code generation at least at the module/API boundary, and
remove normal VM dependencies on HIR/MIR.

### H-03: the frontend uses global mutable graphs for incremental work

`ModuleGraph` owns many parallel maps and global counters
(`crates/vela_hir/src/module_graph.rs:50-93`). Query helpers and dependency closure
frequently scan all bodies/modules. `vela_analysis` clones fact maps and walks
large expression sets during fixed points. The language service then clones
database roots and still performs full HIR rebuilds for common changes.

**Action:** give each module/owner an immutable shard with local dense IDs and
explicit reverse indexes. Derive global views from `Arc`-shared shards. Replace
whole-map fixed points with a dependency worklist and cache facts by
`(revision, owner)`. This is a migration, not a new framework rewrite.

### H-04: schema and registry ownership is duplicated

Runtime/compiler type facts have legitimate layer-specific detail, but name,
identity, fields, method signatures, permissions, and docs are re-encoded too
often. Public registry mutation can silently overwrite some reflection indexes,
and registry sealing uses assertions rather than changing the type of the
object.

**Action:** make an immutable, indexed `FrozenSchema` the sealed output and
replacement for the overlapping definition/type registry state—not a fourth
schema model. Compiler facts and MIR contracts remain separate, but refer to
schema IDs instead of copying names and descriptors. Reflection and bindgen
consume frozen projections; the engine shares them.

### H-05: some fail-open defaults cross capability boundaries

`ScriptStateAdapter::host_receiver_access` defaults to exclusive access
(`crates/vela_host/src/adapter.rs:55-66`). A custom adapter that forgets to
override it can unintentionally authorize mutation. Separately,
`EngineBuilder::reflection_lookup_budget` uses
`unwrap_or_default` (`crates/vela_engine/src/builder.rs:281-287`), while the
default reflection policy grants all permissions. Setting only a resource
budget therefore enables reflection calls, private access, and host mutation.

**Action:** make receiver authority explicit or default to unsupported/shared.
Require `enable_reflection(policy)`; budget setters must never grant
permissions.

### H-07: proc macros can change meaning or break safety-policy compatibility

Macro signature normalization removes all leading underscores and gives
non-identifier patterns the same `arg` label
(`crates/vela_macros/src/signature.rs:36-52`). Parameters such as `_value` and
`value` can silently refer to the same generated binding; multiple destructuring
patterns can produce invalid or duplicate generated bindings. Service/dispatch
expansion also emits unsafe code with
`#[allow(unsafe_code)]`; this fails in consumers using
`#![forbid(unsafe_code)]`.

**Action:** use positional hygienic internal identifiers, validate unique public
labels, and reject unsupported patterns with a span error. Keep erased-pointer
unsafe operations behind a safe library function so generated downstream code
contains no unsafe block.

### H-08: language-server concurrency and URI boundaries do not match the model

Several handlers are labelled latency-sensitive or worker work but call the
synchronous dispatcher directly. Most cancellation is checked only after work,
and each actual lane is a single unbounded queue. `didChange` can update
databases and publish diagnostics synchronously. The language-service project
layer also performs filesystem operations despite the architecture contract,
and both service and server manually strip `file://` rather than correctly
handling percent-encoding, Unicode, or UNC paths.

**Action:** keep all I/O and URI conversion in `vela_lsp_server`, using the URL
library's file-path conversion. Use bounded/coalescing queues, a real worker
pool, and cooperative cancellation tokens inside queries. The language service
should accept immutable text/path snapshots only.

### M-01: hot-path collection accounting rescans complete maps and sets

Every tracked map/set mutation recomputes shallow size by scanning all entries
(`crates/vela_vm/src/script_map.rs:179-187`,
`script_set.rs:105-113`, and `heap.rs:521-565`). With finite limits, repeated
insertion is quadratic.

**Action:** maintain capacity and payload deltas incrementally, with occasional
debug/test recomputation to verify accounting.

### M-02: host calls allocate and clone arguments in multiple layers

The VM materializes `Vec<HostValue>`; `HostAccess` converts/clones it into
another `Vec<HostCallValue>`; `PathProxy` builds another vector for each
operation and panics when adding a 257th dynamic argument
(`crates/vela_vm/src/host_access.rs:443-471`,
`crates/vela_host/src/access.rs:355-370`, and
`proxy.rs:218-225`).

**Action:** convert once into borrowed/consuming call values, use
`SmallVec` for the common proxy case, and make argument overflow fallible.

### M-03: public surface area exceeds the useful embedding surface

The engine provides a good high-level `Runtime` API, but raw VM entry structs,
cache layouts, runtime image storage generics, compiler databases, and numerous
parallel DTOs are also public. This increases documentation burden and makes
semver stabilization harder. The project currently has very little top-level
rustdoc relative to that surface.

**Action:** define supported facade modules and move implementation structures
behind `pub(crate)` or an explicitly unstable `advanced` namespace. Enable
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
- diagnostic rendering rebuilds line starts and allocates line strings
  (`diagnostic_render.rs:146-169`). A shared `LineIndex` keyed by source revision
  would reduce repeated CLI/LSP work.
- `vela_common` directly uses `vela_def::TypeId` and `FunctionId` in its
  interop contracts as well as re-exporting `stable_id`
  (`interop_type.rs:3-88`). That makes “common” depend on a higher-level identity
  crate. Move the minimal ID primitives/hash macro into the lower layer, or
  rename/narrow this crate so its position in the dependency graph is explicit.

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
- public fields allow values that were not produced by canonical construction.
  Make durable identity fields private and expose checked constructors/accessors.
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
  `a::b`. Construction should be fallible and validate every segment as a Vela
  identifier.
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
- interpolation is scanned again; nested lexing uses a synthetic `SourceId(0)`
  and drops nested diagnostics
  (`crates/vela_syntax/src/cst_parser/cst_expr.rs:224-231`). Preserve the
  original source/range mapping and merge diagnostics.
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

**Assessment:** registration invariants are stronger than most early language
projects, but “sealed” is not yet an immutable representation.

Strengths:

- typed `BTreeMap` indexes by ID, path, semantic key, and primitive tag are
  deterministic;
- definition registration checks collisions before mutating indexes
  (`crates/vela_registry/src/lib.rs:94-155`);
- `RegistryCompileView` gives compiler consumers a borrowed read view.

Findings and recommendations:

- `seal_type_bindings` is an assertion on a still-mutable object
  (`lib.rs:87-91`). Invalid builder order becomes a process panic and the type
  system does not enforce frozen state.
- module-root, host-field, runtime-method, and native-source queries may scan all
  definitions or allocate temporary keys (`lib.rs:242-296,420-472`).
- debug names are stored as both `Vec<String>` and owned map keys
  (`lib.rs:506-542`).

Replace this with
`DefinitionRegistryBuilder -> Result<Arc<FrozenDefinitionRegistry>, Error>`.
Construct all reverse indexes at freeze time, intern names, and make every
post-freeze operation read-only and non-panicking.

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
- registration methods overwrite primary/secondary indexes
  (`crates/vela_reflect/src/registry.rs:725-812`). Inserting an existing ID under
  another name can leave stale name mappings. All registration must validate
  atomically and return a typed collision error.
- trait descriptors are embedded in type descriptors and also stored in a global
  map (`registry.rs:175-190,704-717`); lookup merges/deduplicates them on demand.
  Store each trait once and reference its ID.
- lookup budgets charge one unit for an API call such as `reflect::types()` even
  if it allocates and returns the complete schema. Charge returned records/bytes
  or page enumeration.
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

Use a worklist keyed by owner/expression with explicit dependency edges. Intern
structural type facts and canonicalize unions. Cache a generation-level base
fact set, then compute small executable deltas instead of rebuilding equivalent
maps per root.

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

- liveness is computed during construction, recomputed by verification, and
  compared; sealing also rebuilds control-flow/fact information
  (`crates/vela_mir/src/verifier/mod.rs:394-438` and liveness modules).
  Consolidate into one verifier result consumed by sealing.
- `CompileTargetSnapshot` is another broad parallel-map DTO. Prefer a view over
  frozen HIR/schema data with only MIR-specific derived facts.
- public raw builders allow many invalid intermediate states that only the
  compiler should create. Keep them `pub(crate)` and expose sealed MIR to other
  crates.
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
- unlinked and linked instruction enums duplicate approximately eighty opcode
  families and their maintenance ladders. Generate both representations,
  verifier dispatch, and metadata from one declarative opcode specification.
- semantic preparation clones builder/probe placements and constructs validated
  lowering inputs that are later requested again
  (`crates/vela_bytecode/src/compiler/semantic_input/mod.rs:158-174,294-308`).
  Make one owned preparation result and consume it once.
- sorted `insert_function` calls rebuild indexes repeatedly
  (`lib.rs:111-139,296-322`), yielding quadratic construction. Accumulate,
  stable-sort once, validate once, then freeze.
- linked method lookup constructs owned strings
  (`linked.rs:181-189`). Index typed owner/name IDs and support borrowed probes.

Keep `vela_bytecode` as the compact runtime representation and move codegen and
compiler-only artifact assembly into an internal backend module or a future
`vela_codegen` crate only if the dependency graph cannot otherwise be cut.

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
- `HostAccess` is zero-sized, yet APIs require a mutable reference to it. If it
  represents authority/session state, make that state real; otherwise hide it
  behind the session facade.
- host argument conversion clones structural values through multiple vectors;
  see M-02.
- `HostPathParts` implements custom small storage while the crate already uses
  `SmallVec`. Reusing one tested representation would remove substantial code.

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

- finite-step collection is unsafe and its time budget is inert; see C-01.
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

The activation operation should be a clear compare-and-swap:

`apply(update) succeeds iff runtime_id and current_version equal update.base`.

That invariant should be tested with two updates built from one base, two
runtimes, and staged updates observed by multiple handles.

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
- the engine retains three overlapping registry/schema stores
  (`engine.rs:42-46`). Share views over one frozen schema.
- `RuntimeImage`, `OwnedImage`, `SharedImage`,
  `RuntimeImageStorage`, and `RuntimeImpl<I>` mainly encode inline versus `Arc`
  storage (`runtime/image.rs:12-73`). A single `Arc<GenerationImage>` is easier
  to teach and normally just as efficient; retain a distinct owned fast path
  only if profiling demonstrates material allocation/refcount cost.
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

- argument normalization can collide and alter bindings; see H-07.
- generated service/dispatch code contains unsafe blocks annotated with
  `#[allow(unsafe_code)]`. A downstream `#![forbid(unsafe_code)]` cannot be
  overridden, so valid consumers fail to compile. Keep unsafe erased
  reborrowing behind a safe function in `vela_host`.
- expansion hard-codes paths such as `::vela_engine`, `::vela_host`,
  `::vela_reflect`, and `::vela_vm`. Consumers must directly depend on all
  internal crates under exact names. Emit through one documented
  `vela_engine::__private`/facade path and resolve renamed crates with
  `proc_macro_crate` or an explicit crate option.
- `#[methods]` can expose every representable instance method, including private
  methods, unless configured otherwise. That conflicts with the documented
  explicit-registration capability model. Default to explicit/public-only
  export and require an annotation for additional methods.
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
  parameters. The renderer substitutes fallback identifiers and can discard
  diagnostics (`crates/vela_bindgen/src/rust.rs:416-515`).
- distinct source names can normalize to the same Rust identifier, producing
  duplicate items after generation reports success.
- generated accessors expose long internal root-module names, which are correct
  but not pleasant as an application API.
- schema DTO ownership currently pulls toward language-service types instead of
  a neutral frozen schema.

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

**Assessment:** feature coverage is impressive, but the internal ownership model
does not yet deliver the cheap immutable snapshots suggested by the public API.

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
- `ParseDb` begins updates by cloning the full record map, HIR commonly rebuilds
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
then make snapshots cheap rather than cosmetically immutable.

### `vela_lsp_server`

**Assessment:** protocol typing and test breadth are good; scheduling labels
currently promise concurrency/cancellation that most handlers do not receive.

Strengths:

- protocol conversion is isolated from the language-service crate;
- stale result suppression, typed request/notification routing, and loopback TCP
  support are well tested;
- retryable/background task concepts provide a base for real scheduling.

Findings and recommendations:

- latency-sensitive and worker dispatch functions call the synchronous
  dispatcher directly
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

**Assessment:** functional and deterministic. Its 11.5 MB unoptimized raw
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
- a new engine is constructed for every operation, and the “compile then run”
  flow compiles the same source twice.
- compilation is synchronous on the browser thread, with no source-size limit,
  deadline, or cancellation.
- JSON conversion is not type preserving: unit becomes the string `"()"` and
  64-bit integers can exceed JavaScript's exact numeric range.
- compiler diagnostic projection is duplicated with CLI logic.

Provide a persistent worker-owned session keyed by source/options fingerprint.
Add source-size and compile-work limits, reuse compiled artifacts, define a
tagged/versioned JSON value schema, and add an explicit optimized-size budget in
CI. A minimal feature profile should omit server-only host, reflection, schema,
and service facilities not used by the playground.

## Rust/Vela interoperation

The fundamental model is good for game-server scripting. Script code uses
ordinary member access and mutation, while Rust retains ownership and mediates
all effects through stable handles and paths. This is safer than registering an
arbitrary Rust closure over a real `&mut T` and is consistent with the product's
hot-reload goals.

The current friction comes from exposing a linear-resource protocol:

- child leases must sometimes be released before parent leases;
- `host::release` must be written before an `await` when a resource is live;
- adapter implementers must understand receiver access, scope, slots, prepared
  paths, and several value representations;
- generated service patch paths may panic instead of propagating conversion,
  VM, capability, or cancellation errors.

Recommendations:

1. prefer lexical lifetime analysis that inserts safe releases automatically;
   if that cannot cover the common case, add a scoped `host::with` library
   combinator before considering new language syntax. Keep explicit
   `host::release` as the advanced escape hatch;
2. make common host registration derive one schema plus one adapter and surface
   compile-time errors for unsupported fields/methods;
3. expose `try_with_request`/fallible patch APIs rather than panicking inside
   generated adapters;
4. make all authority declarations fail closed;
5. measure and guarantee that a warmed scalar host call does not rebuild
   generation metadata or allocate proportional to the entire registry.

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

### P0: restore correctness invariants

1. Disable multi-step sweeping or conservatively run atomic GC; add allocation,
   frame-root, and container-write regressions between requested steps.
2. Bind `HotUpdate` to runtime/base generation/checksum and make activation an
   atomic compare-and-swap; remove split-brain `Clone` semantics.
3. Replace all reflected stable-ID saturation with a lossless opaque format.
4. Detect cycles and enforce depth/work budgets during heap-to-owned conversion.
5. Make host receiver and reflection configuration fail closed.
6. Fix macro argument hygiene and remove generated unsafe blocks from consumer
   crates.

These changes should precede new syntax, JIT preparation, or more reflection
surface.

### P1: remove generation-wide work from request/call paths

1. Build and share one frozen execution/reflection/dispatch image per program
   generation.
2. Make `Engine` and frozen registries cheap `Arc` handles; create only
   per-call mutable sessions.
3. Keep MIR and duplicate unlinked code outside the retained runtime artifact.
4. Replace map/set full-size scans and double host-argument conversions with
   incremental/borrowed paths.
5. Move filesystem/URI concerns fully into the LSP adapter and make worker lanes,
   cancellation, and coalescing real.
6. Generate stdlib semantic/runtime mappings from one table.

Success criteria should include retained-generation RSS, warmed allocations per
host call, reload activation latency, and edit-to-diagnostic p95.

### P2: make incrementality and APIs structurally simpler

1. Introduce immutable per-module HIR shards, local IDs, reverse indexes, and
   owner-keyed analysis worklists.
2. Consolidate type/schema identity into a frozen neutral schema while keeping
   analysis facts and MIR contracts layer-specific.
3. Replace expression rescanning/re-lexing with a single token/event/Pratt path
   and generated typed CST views.
4. Narrow public facades, document them, and hide raw builders/cache layouts.
5. Add automatic or safely scoped host-resource release for the common async
   path.
6. Add a cached browser-worker session and an explicit WASM size target.

### Work that should remain deferred

The current product contract is right to defer a JIT, moving GC, coroutine hot
reload, runtime type mutation/monkey patching, and a custom IDE beyond the native
LSP server. In particular, retaining MIR and publishing JIT eligibility today
imposes real cost for a feature that remains out of scope. First make the
interpreter, artifacts, reload transaction, and embedding path small and
predictable.

## Suggested measurable gates

Before calling the relevant milestones complete, add gates for:

- **GC:** a live object created or linked after every incremental boundary
  survives; configured pause/work limits are measured, not merely stored.
- **Reload:** a stale update, cross-runtime update, and double application are
  rejected without partial host/program publication.
- **Reflection:** every stable ID round-trips losslessly and duplicate
  registration is atomic/fallible.
- **Embedding:** the documented minimal Rust application compiles with only the
  facade dependency, with dependency renaming and
  `#![forbid(unsafe_code)]`.
- **Calls:** warmed scalar/native/host calls reuse generation-wide metadata;
  benchmark allocations are bounded by argument/result shape, not registry size.
- **Retention:** 16 old generations retain only code/runtime schema/state layout,
  with a stated memory target for the standard harness.
- **Editor:** a one-file body edit reuses unchanged parse/HIR shards; stale work
  is cancelled/coalesced; file URI round trips cover spaces, Unicode, `%`,
  Windows drives, and UNC.
- **WASM:** compile/run uses a worker, has source/work caps, preserves 64-bit
  values, and stays within a tracked optimized/compressed size budget.

## Validation and limitations

Commands completed successfully during this review:

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

The audit did not rerun every long-duration benchmark. Performance conclusions
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
repair the GC/reload/reflection invariants, freeze one schema and one execution
image, stop retaining compiler state at runtime, and make one documented Rust
embedding path fast and hard to misuse. Doing that would preserve the project's
strongest engineering while making the implementation materially easier to
reason about, benchmark, and evolve.
