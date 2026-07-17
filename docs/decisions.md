# Decisions

This file is the active architecture decision index. Full pre-compaction
decision history lives in
[archive/decisions-full-2026-06-01.md](archive/decisions-full-2026-06-01.md).

## Standing Constraints

- Script-language generics are not supported.
- Function overloading by arity, type hint, or native signature is not
  supported.
- Scripts never receive real Rust `&mut T` references.
- Host mutation must go through `HostRef`, `HostPath`, `PathProxy`, and
  `HostAccess`.
- Reflection can query metadata and perform controlled reads, writes, and
  calls, but cannot mutate runtime type structure or implement monkey patching.
- The MVP does not include JIT, hot migration of suspended async frames, moving
  GC, or a custom full IDE product. Executor-neutral `async fn` and `.await`
  preserve sequential semantics and expose no task/coroutine handles or manual
  resume. A full native LSP capability track is allowed before the MVP when it
  stays analysis-only and does not change language or runtime semantics.
- Pre-release code should replace obsolete internal APIs instead of preserving
  compatibility shims. Product-level hot reload ABI and schema compatibility
  checks remain required.
- Ordinary active source files should stay under 1200 lines unless a clear
  exception is documented. Over-threshold implementation and test files should
  be reviewed and split by responsibility when no exception exists.
- `crates/vela_vm/src/linked_execution.rs` may exceed the ordinary 1200-line
  threshold when it remains opcode dispatch glue. New semantic work should
  still move into focused VM modules, and the dispatch loop should only decode
  operands, charge budgets, preserve source spans, update control flow, and
  call those boundaries.
- Standard library and builtin APIs must remain domain-neutral. Game-specific,
  commerce-specific, or other business-domain capabilities belong in Engine
  host registration, native functions, schemas, or examples, not in builtin
  language surface.
- Runtime call budget presets should stay domain-neutral. Hosts should choose
  per-script or per-call budgets explicitly with `CallOptions::new(...)`;
  `CallOptions` intentionally has no default preset.
- Runtime authorization uses coarse capability profiles, not arbitrary
  business permission strings. Native and standard-library execution checks
  compare effect bits against the engine `CapabilitySet`; business-domain
  isolation is primarily controlled by what host surface the embedding
  registers.

## Active Architecture Decisions

### Explicit Runtime State Ownership

The pre-release `global` declaration and embedding model is replaced outright
by `state` and `extern state`. `state` is a contextual module-item introducer:
the lexer emits it as an identifier, while the parser recognizes it only after
attributes and optional `pub`, or after reserved `extern`. It remains legal as
an ordinary identifier in every other binding position. The supported forms
are `state name: Type = expression;` and `pub extern state name: Type;`; VM
state requires both an explicit type and initializer, while extern state
forbids an initializer.

Each `state` owns one mutable VM cell per `Runtime`. Each `extern state` owns no
script value and resolves only to a type-checked host binding; Vela cannot
replace its root, and nested mutation continues through `HostRef`, `HostPath`,
`PathProxy`, and `HostAccess`. Visibility affects name resolution and export
ABI only. No declaration may select VM versus host storage dynamically, and no
legacy parser, bytecode, runtime, or embedding compatibility surface remains.

State identity is the stable `StateId` derived from package, module, and
declaration name. Dense `StateSlot` operands are local to one executable
generation. Initializers use the verified HIR/MIR/linked execution pipeline,
run once for every newly created Runtime and only for newly added VM state on
reload, and are bounded and restricted from state, extern, native, host,
provider, reflection, capability, IO, event, time, random, and async effects.
Initialization and reload publication are transactional from script-visible
state.

Rust-side VM-state replacement resolves an exact canonical qualified type
first. Qualified spellings never fall back to a leaf name; unqualified names
are accepted only when the linked generation has one permitted candidate. The
same linked-aware boundary recursively validates record fields and enum
variants/payloads, then materializes canonical `RecordIdentity` or
`EnumIdentity`. Validation and insertion are one operation, so accepted
`set_state` and `update_state` values retain ordinary field, guard, and pattern
semantics.

Hot reload preserves an existing value or extern binding only when the same
`StateId` has the same storage kind and exact normalized type contract. It does
not rerun a preserved initializer. Storage or type changes reject; rename is
remove plus add. Removed state remains addressable through the slot map owned
by a live old frame, closure, value, or suspended execution generation and is
reclaimed only after the final such owner is gone. This preserves old
generation execution without migrating suspended frames or introducing state
schema/value migration.

Added-state reload staging copies the persistent heap graph directly. One
transaction budget is charged before each allocation, aliases and cycles are
preserved across all staged roots, and failure publishes neither the candidate
image nor any state cell. Generation liveness excludes linked-artifact owners
reachable only from inactive state roots, then closes transitively over state
needed by genuinely external frames, suspensions, closures, iterators, active
state, and retained runtime values. Thus a closure-valued removed state cannot
self-root its generation, while an external old owner still pins it until the
next ordinary safe point after release.

Initializer change reporting compares only the permitted reachable executable
graph. Direct script calls and `MakeClosure` targets, including nested closure
and parameter-default executables, are traversed with paired visited nodes so
recursive graphs terminate. Unrelated functions are not included in the
fingerprint.

### Linked-Only VM Execution

The compiler and verifier retain unlinked bytecode as a construction and
validation format. Before execution, the linker consumes that representation
and publishes a generation-owned `LinkedArtifact`. All VM entry calls, frames,
closures, method callbacks, iterator callbacks, and runtime type guards execute
against that artifact and its `LinkedProgram`; the VM has no unlinked
interpreter or name-based execution fallback. Tests may construct unlinked
fixtures, but they must link them before execution.

### Cursor-Specific HIR Query Bodies

Production editor queries select the narrowest `HirBody` whose source origin
contains the cursor, including lambda and parameter-default bodies. Queries
that need an enclosing call search HIR call facts across the module graph by
the shared cursor/call spans; they do not replace the active body with the root
body or reconstruct call identity from syntax. Record completion consumes the
active HIR record before consulting the isolated malformed-edit CST recovery
path.

### Stable HIR Identity At Compiler And Editor Boundaries

Bytecode semantic helpers receive `HirExprId` directly; source spans are only
for diagnostics and debug metadata, never for reconstructing expression
identity. Editor source projection resolves local declaration ranges once to
`HirLocalId`, and definition, hover, references, rename, symbol targets, and
semantic tokens consume that shared identity. Record-field completion obtains
constructor identity and fields from HIR whenever a record expression lowered;
CST traversal is limited to the explicitly named incomplete-edit recovery path.

### Heavy HIR Path Facts

Body-owned HIR path facts are the semantic source for expression paths, call
callee paths, record constructor paths, and pattern paths. Language-service,
analysis, bytecode, and future MIR work should consume those HIR facts instead
of reconstructing path sites from parsed body syntax. Syntax may still provide
source origins during HIR lowering and editor cursor recovery, but feature
producers should not keep parallel parsed path-site helpers.

### Heavy HIR Body Edit Invalidation

Body-only source edits must refresh Heavy HIR body facts, including call,
member, path, scope, binding, and source-origin records, even when declaration
and import fingerprints are unchanged. Incremental language-service updates may
reuse declaration/import indexes for those edits, but must not keep stale HIR
graphs once editor features consume body-owned HIR semantics.

### Tuple, Unit, And Null Direction

Future breaking value-model cleanup should add Rust-like tuple syntax and
`()` as the unit type/value. Unit should replace the current void-like use of
`null`; expected absence should use `Option`, and recoverable failure should
use `Result`. Ordinary script APIs should not use `null` for no-value,
not-found, or failed results. `?` propagation should stay Rust-aligned:
`Option` propagates through `Option`-returning functions, `Result` propagates
through `Result`-returning functions, and cross-family conversion requires
explicit helpers such as `ok_or`. The selected hard-switch policy removes
source-level `null` from ordinary Vela rather than keeping a compatibility
literal or type hint. Raw external null, if needed later, must be explicit at
the serde/JSON boundary, for example through a dedicated external-data wrapper,
and must not be overloaded as the VM no-value result. The first tuple slice
defers one-element tuples, direct tuple field access, and tuple Map/Set keys;
host Rust tuple conversion starts with arities 2 through 4. The implementation
plan lives in
[tuple-unit-null-refactor-plan.md](archive/tuple-unit-null-refactor-plan.md).

Reflection metadata must not use unit as a missing-data sentinel. Optional
copied metadata such as docs, attributes, source spans, schema hashes, module
owners, type hints, and return hints is script-visible as `Option::Some(...)`
or `Option::None`. A real unit return contract remains the type string `"()"`;
absence of a return hint is `Option::None`.

### Source And Artifact Naming

Vela source files use `.vela`. Future precompiled bytecode-only artifacts use
`.vbc`. If a future deployment package contains bytecode plus ABI manifests,
schema metadata, source maps, or reload metadata, it should use a separate
package extension rather than overloading `.vbc`.

### External C ABI Boundary

External binary embedding uses a dedicated `vela_c_api` crate. It is separate
from `vela_hot_reload`: hot-reload ABI describes script/module/schema
compatibility, while `vela_c_api` owns opaque C handles, C-compatible value
layouts, and future host adapter vtables. The C ABI must not expose Rust
references or place Rust host state under script GC.

### Record Field Assignment Roots

Script record field assignment targets use the leftmost receiver expression as
the root and evaluate that root exactly once. This allows `self.field += value`
and expression receivers such as `get_or_put(key).field += value` without
special-casing `self` or requiring a local path root. Host field assignments
still resolve through HostAccess first; non-host record writes mutate script
heap records through record field or slot bytecode.

Every assignment evaluates and captures its target components once, from left
to right, before evaluating the right-hand side. For an indexed target this is
receiver, then index, then RHS. A compound assignment performs its explicit
read-modify-write against the current target state after RHS evaluation, so an
alias write performed by the RHS is observable by the RMW. Host compounds keep
`HostMutate` as that current-state boundary; they are not lowered through a
detached host read or an ordinary MIR place.

Tuple values are immutable. Assignment through a tuple projection therefore
uses the exact receiver `TypeFact::Tuple` arity plus the authoritative
`CompileMemberTarget::TupleIndex`, reads unchanged siblings, allocates a new
tuple, and propagates rebuilt values outward in reverse through mixed
tuple/record chains. The rebuilt root is written to the captured local or
index; allocation and safepoint behavior stays explicit in MIR. A tuple target
with unknown/dynamic arity, a non-tuple receiver fact, or an out-of-range or
HIR-disagreeing index is inconsistent compiler input and has no fallback.

When a tuple suffix follows a HostAccess path, MIR reads the longest exact
host-path prefix once, rebuilds only the script-value suffix, and writes the
rebuilt prefix through the same captured `HostRef` root and `HostPath`. The
prefix must authorize both the required read and write; ordinary fields use
their readable/writable access, variant fields retain their existing
variant-write policy, and indexes require readable and writable capabilities.
No host prefix becomes a MIR place or a dereferenced host reference. Current
semantic input classifies that prefix as a read because the composite tuple
suffix ends outside HostAccess; the MIR input boundary rechecks composite
write authorization, while moving the same diagnostic fully into analysis is
a focused follow-up rather than a reason to infer a path from names.

Resolved methods do not become implicit bound-method values. A method member
used outside call position remains an ordinary dynamic field read using the
exact HIR member spelling, preserving the existing runtime lookup or missing-
member failure. Stable method descriptors select call lowering only; they do
not add hidden closure allocation, method binding, or monkey patching.

### Module Imports And Exports

Vela has no source-level `module` declaration. `compile_file(path)` is a
single-script entry mode where the file name is not module identity and the
ordinary entrypoint is `main`. `compile_dir(root)` is the module-graph mode:
each `.vela` file under `root` gets a module path from its relative path, so
`game/reward.vela` is `game::reward`. Imports and qualified calls use `::`;
the final import segment is the declaration name and the preceding segments are
the owning module path.

Public APIs should be imported from the module that owns them. Crate roots
should expose focused `pub mod` entries and avoid broad `pub use` facades unless
the item is an intentional crate identity entrypoint.

`vela_engine::prelude` is the embedding convenience import surface. It may
re-export common Engine, Runtime, native descriptor, host-handle, reflection
permission, and schema descriptor types needed to write host setup code, while
the crate root remains a focused module index.

Single-source embedding APIs do not require callers to provide `SourceId`.
`Engine::compile_source`, text hot-reload compile, and text hot-reload staging
assign internal single-source identity. Explicit source identity remains an
internal compiler/reload concern and belongs to module-graph loading,
diagnostic sources, and crate-local tests that need deterministic source
identity.

Single-source HIR and emitted metadata use the real root module path; the
synthetic name `main` is not module identity. Existing script `MethodId`
compatibility is the narrow exception: single-source inherent and user-trait
method hashes retain `main` as their logical identity namespace. That mapping
is applied only while constructing stable method identity, never to code/debug
symbols, name resolution, reflection metadata, or the retained `ModuleGraph`.

Rust source may use one direct-parent `super::...` reference inside a local
module group. Multi-level `super::super` paths are prohibited; cross-subsystem
imports should use explicit `crate::...` paths.

Handwritten Rust source uses real `mod` boundaries with the narrowest required
`pub(super)` or `pub(crate)` surface. `include!` is reserved for generated code
whose build-time origin is explicit; it must not be used to concatenate
handwritten implementation or test fragments into one privacy scope.
Production modules use standard Rust module file resolution and explicit
imports. `#[path]` is reserved for justified test or cross-target code sharing,
and production wildcard imports are denied at crate roots while test modules
may use them for local fixtures.

### Source Pipeline

The syntax layer owns tokens, AST, parser recovery, and source spans. HIR owns
module graph resolution, declaration IDs, binding maps, type-hint metadata, and
top-level semantic diagnostics. The source front door rejects syntax and HIR
diagnostics before bytecode compilation; the bytecode compiler consumes the
validated HIR graph and metadata.

Source-set parsing and `ModuleGraph` construction are HIR front-end
responsibilities. The bytecode compiler accepts an ordered `HirSourceSet`; the
set records authoritative single-source or module-graph mode when it is built,
so downstream callers cannot infer or override mode from module count. The
compiler must not accept source text or depend on `vela_syntax`. `vela_engine`
owns the embedding-facing source/file/directory and hot-reload orchestration,
including structured projection of front-end and backend errors. This boundary
is implemented as a breaking internal hard switch without compatibility
wrappers; the execution checklist is
[bytecode-source-boundary-hard-switch-plan.md](archive/bytecode-source-boundary-hard-switch-plan.md).

The HIR boundaries are `source_ingestion::build_single_source` and
`source_ingestion::build_module_source_set`. They return an ordered
`HirSourceSet` or a `HirSourceBuildError` explicitly staged as syntax or
semantic. Function compilation consumes a private-field `HirSourceFunction`
resolved by its owning source set, never an independently supplied
`HirDeclId`. This is required because declaration IDs are generation-local and
may have equal numeric values in different graphs. Whole-program symbols,
methods, constants/defaults, executable roots, and retained metadata use the
same source set and its authoritative mode.

Engine is the only production source orchestrator. It owns source/file/directory
ingestion, compiler options and registry input, bytecode compilation,
registry-aware linking, and source-to-hot-reload error projection.
`vela_hot_reload` begins at an Engine-linked `LinkedArtifact`; it owns version,
ABI/policy comparison, and update generation, and exposes no production API
that accepts source text, `ModuleSource`, HIR graphs/source sets, or
`CompiledProgram`. Front-end diagnostics remain Engine errors rather than
`HotReloadError` variants and return immediately instead of being staged as
artifact ABI/policy rejection reports.

Production parsing is rowan-backed and lossless. `vela_syntax` owns
`SyntaxKind`, `VelaLanguage`, syntax node/token aliases, and `Parse<T>`
green-tree results; `vela_syntax::parse_source` returns the production syntax
record. Typed AST APIs are views over syntax nodes and tokens, while semantic
extraction belongs in explicit AST accessors, HIR lowering, analysis, and
compiler code. Downstream crates consume typed syntax wrappers, HIR facts, or
compiler-owned payload facts instead of the old owned-AST body parser. The
deleted owned parser, legacy body-parser feature, CST-to-owned fallback
payloads, and token-gap formatter are not compatibility surfaces and should
not be restored.

The current semantic pipeline is being split into two explicit internal
architecture tracks. Heavy HIR is the semantic truth layer: body/expression/
pattern IDs, source origins, scopes, bindings, captures, type/effect/call/member
facts, and control-flow facts belong in `vela_hir` plus analysis facts keyed by
stable HIR IDs. MIR is the execution-shape layer: CFG, temporaries, places,
typed operations, guards, liveness, debug/root metadata, and lowering decisions
belong in a future internal `vela_mir` crate. MIR must consume Heavy HIR and
analysis facts only; it must not parse source or repair missing semantic facts.
The execution plans are
[heavy-hir-hard-switch-plan.md](archive/heavy-hir-hard-switch-plan.md) and
[mir-lowering-jit-foundation-plan.md](archive/mir-lowering-jit-foundation-plan.md).

Production executable analysis is total within each selected stable function's
runtime body closure, including nested lambdas and parameter-default bodies.
Every owned expression, local, parameter, self binding, and pattern has an
explicit type fact; unresolved values use `TypeFact::Unknown`, and every owned
expression has an explicit effect fact. An absent executable fact means the HIR
identity is outside that function generation, not that analysis silently
failed. Unknown placeholders are added only after inference reaches its fixed
point so they cannot suppress callback, pattern, or local-flow refinement.

An explicit erased builtin local contract keeps its declared outer family while
analysis fills `Unknown` payload slots from initializer, assignment, pattern,
and callback flow. `Any` remains an intentional dynamic boundary at every
nesting depth, and divergent control-flow joins fall back to the declared
contract rather than discarding its proven outer shape. Contract compatibility
uses the same recursive erased-slot rule, including `Option`/`Result` variants,
so analysis facts and compile validation cannot disagree about a refined local.

Policy-controlled native availability is separate from compile metadata.
Reflection natives use the backend-neutral `vela_stdlib` manifest for stable
IDs, parameter names, effects, and call placement. Low-level compilation with
no explicit registry may use that policy-neutral manifest, while compilation
with an explicit engine registry treats only registered reflection natives as
available. MIR therefore never fabricates a runtime-checked reflection
signature, and an empty explicit registry cannot accidentally enable
reflection.

A member miss on a source-owned closed record or enum remains an explicit
dynamic member target for the current language contract. This preserves the
existing runtime lookup/failure behavior until a separately approved
unknown-member diagnostic is introduced. Compile-target generation must never
misclassify that valid semantic outcome as inconsistent MIR input.

MIR v1 is a generation-local non-SSA IR with mutable script/synthetic locals
and single-assignment temporaries. Branch joins use synthetic mutable locals;
MIR v1 does not add phi nodes, block parameters, or Rust-style move/borrow
semantics. Calls are effectful statements with destinations, safepoints,
effects, and implicit `may_trap` runtime exits; successful calls continue in the
same basic block. MIR v1 has no language-level exception or unwind CFG because
recoverable errors use explicit `Result`/`Option` values. Future explicit
`Await`, `Yield`, and `Suspend` operations may become terminators, but ordinary
calls do not reserve that shape in MIR v1. HostRef, HostAccess, dynamic
field/index access, allocation, guards, and reflection are explicit effectful
operations rather than ordinary places or pure rvalues. MIR may depend on HIR,
analysis, and required low-level stable target crates, but never on syntax,
bytecode, or VM crates. The physical MIR-to-bytecode backend belongs to
`vela_bytecode`, parameter defaults lower into their owning function prologue,
and const/schema evaluation remains a compile-time service outside runtime MIR
v1. MIR IDs are not runtime, hot-reload ABI, or serialized identities.

MIR operands contain only already-evaluated logical values and non-allocating
unit/bool/char/scalar immediates. Evaluated string, bytes, array, and map
constants stay in the compile-target snapshot and enter runtime MIR through an
explicit constant-materialization statement at each use, preserving heap
identity, memory-budget, GC, and safepoint behavior. Range values remain inline
runtime values: range construction may trap but does not allocate. Iterator
steps are call-capable safepoint terminators, while range steps distinguish a
proven `i64` mode from a dynamic-integer trapping mode.

When a mutable script-local operand must survive lowering a later source
operand, MIR snapshots that read into a single-assignment temporary before the
later expression. This makes left-to-right evaluation explicit for operators,
aggregate elements, receivers, callees, indexes, and arguments even when the
later expression reassigns the local. A backend may coalesce a snapshot only
when its allocation/liveness proof preserves the same observed value and
instruction-budget contract.

Pattern matching keeps checks and binding projection as separate ordered
phases: every structural/literal check completes before binding-only fields are
projected and written, then an arm guard runs with those bindings visible.
Pathless tuple binding uses an explicit trapping arity guard, while its match
test remains an ordinary predicate edge. `let` patterns preserve the current
binding-only contract rather than adding refutability: pathless tuples guard
arity, constructor patterns project declared bindings without a tag predicate,
and literal/path/wildcard nonbindings are no-ops.

Resolved method calls carry an already-evaluated receiver. Script calls retain
a complete ordered parameter-slot vector and may use the Missing sentinel only
for HIR-owned defaults evaluated by the callee prologue. External signatures
state whether positional arity is declared/defaulted, runtime-checked, or
proven variadic; this preserves existing native/host behavior instead of
silently adding static arity rejection. Resolved local/direct-lambda
callable-value calls remain positional-only, while genuinely dynamic callable
and method calls preserve ordered runtime names.
Compile-call input records the already-resolved HIR expression IDs in complete
script slots, validated positional order, or genuine runtime named order; the
MIR builder does not perform argument-name resolution.
Resolved local and direct-lambda callable-value calls are positional. A
genuinely dynamic callable retains ordered positional/named arguments in a
separate MIR call form, just as a dynamic method does; it must not erase names
or be repaired through a method/name fallback.

Contract guards are trap-only statements. Recoverable optimization guards are
terminators with explicit passed/slow CFG successors, and `Option`/`Result`
propagation uses ordinary variant tests, CFG edges, extraction, and return
rather than a hidden statement side exit.
Every contract guard retains a backend-neutral source context: logical
parameter index, return, local, state, or field plus the clean source/debug
name. A bytecode backend may encode that context, but must not reconstruct it
from a guard key, HIR, or a formatted diagnostic description.

Each MIR generation owns a canonical target table for function/method/type/
variant/field/state IDs, canonical link symbols, debug names, host runtime IDs,
logical layouts, and signatures. Per-call source/debug spellings remain on the
call operation. A backend must use that owned table rather than traverse HIR,
analysis, or a live registry. Every `MirFunction` also owns its code symbol,
ordered receiver/parameter/default ABI, ordered capture ABI, and return
contract; debug-local records are not an ABI reconstruction source.

The compile-target snapshot also owns source-to-stable identity indexes for
script function/type declarations, method nodes, compilation roots, and
canonical type names. Signature parameters retain an optional declaration
origin so source diagnostics can preserve secondary parameter labels; external
registry parameters without a source declaration use no synthetic origin.

Compile-target canonical type names are package-qualified semantic identities,
not source aliases or runtime display names. They use the same canonical
`DefPath` namespace as their stable `TypeId`, so `std::Result` and
`script::Result` may coexist in one closed generation. Source lookup spellings,
runtime record/enum names, debug names, and shape-hash seeds remain explicit
separate facts and must never be used to repair a missing stable identity.
Every compile type descriptor carries a nonempty producer-owned runtime name;
only its canonical name is indexed. Runtime names are neither lookup aliases
nor unique keys, and consumers must not recover them by removing a canonical
package prefix.

Standard-library record values that are not source declarations—currently
`MapEntry` and the `Reflect*` metadata records—are analysis-owned logical
records. Their type and field identities use the package-qualified
`std::value_records` definition namespace, while their `ShapeId` uses the
runtime record name plus canonical sorted fields to match `ScriptFields`.
Analysis carries exact recursive field facts, including each MapEntry key/value
specialization, and member facts carry stable field targets. A generation emits
one Standard descriptor per logical record on first use; MapEntry descriptor
field contracts remain erased because multiple exact specializations may share
that stable runtime layout in one generation. Physical record slots remain a
bytecode-backend concern.

Trait `MethodId` remains the shared dispatch identity across receiver
implementations. Executable method descriptors and MIR lookup maps are keyed
by `(TypeId, MethodId)`, while the owner-specific `FunctionId` remains the code
identity. Reflection MIR carries the resolved native `FunctionId` and only
evaluated operands; even literal member names materialize through an explicit
heap-constant operation first. Reflection, `set::from_array`, host remove, and
host push are explicit compile-target intrinsics, so lowering never selects
them by matching a debug or canonical name.

Schema defaults remain compile-time values keyed by their owning `HirBodyId`,
separate from declaration-keyed const values. Each expression constructor
target contains ordered resolved slots with stable `FieldId`, declared
parameter name/index, and either the explicit `HirExprId` or the selected
evaluated-default body. Pattern constructor targets are separate because they
do not deliver argument/default values. Set construction records its one
already-evaluated array source and one visible allocation boundary;
`set::from_array(values = source)` is canonicalized to that same single logical
operand, while missing or extra operands are source diagnostics before MIR.
MIR does not contain targetless index removal or speculative bitwise/shift
operations that have no Heavy-HIR/bytecode behavior. VM-state reads and writes
are explicit stable-identity operations.

Heavy HIR body ownership uses stable `HirBodyId` records with explicit owners:
declarations, trait default methods, impl methods, lambdas, and parameter
defaults. Nested executable regions such as lambdas and parameter defaults are
separate bodies with source origins and parent links, not syntax payloads hidden
inside downstream compiler or tooling callers. Lexical scope facts belong in
the owning body as `HirScopeId` records with parent/child links and owned local
IDs. Const and VM-state initializer expressions use the same body model.
Extern state has no initializer body.

### Native-First LSP Boundary

Vela's full native LSP capability track is allowed before the MVP and may
progress in parallel with M19/M20 optimization when it stays analysis-only. A
custom full IDE product remains outside the MVP. The primary desktop
integration uses native `vela_lsp_server` binaries so editor tooling can use
platform filesystem watchers, threads, cancellation, and large workspace
indexing. WASM may wrap the reusable language-service core for browser tooling,
but it must not constrain the native server architecture.

`vela_language_service` owns reusable editor analysis: virtual workspace
state, open-document overlays, module graph snapshots, diagnostics,
completion, hover, definitions, schema facts, and incremental invalidation. It
must not depend on LSP protocol types, read the filesystem directly, execute
scripts, inspect live host state, or mutate `TypeRegistry`.

`vela_lsp_server` owns protocol and platform integration: JSON-RPC transport,
document sync, workspace folders, file watching, request cancellation,
progress, and LSP position/range conversion. Editor plugins should stay thin
launchers around this binary. Host facts for editor tooling come from a static
schema artifact exported from `TypeRegistry`/`RegistryFacts`; the server must
not run the host application to discover schema metadata.

Thin editor launchers may pass initialization options that mirror `vela.toml`
using `workspace.roots` and `host.schema`. Those options are a fallback
configuration source for native server startup and later
`workspace/didChangeConfiguration` settings; a discovered `vela.toml` remains
the authoritative project configuration.

Native launch flags mirror the same fallback path: `--root` appends a
workspace root and `--schema` sets the host schema artifact before stdio
transport starts. Client-provided initialization options override those launch
defaults, while `vela.toml` discovery still wins once project configuration is
loaded.

The native LSP server's stdio transport uses `lsp-server` as the production
framing and typed message boundary. Normal stdio and optional TCP traffic enter
the same rust-analyzer-style main loop as typed `lsp_server::Message` values;
`GlobalState` is the sole mutable coordinator and owns lifecycle, capabilities,
watchers, request/cancellation state, reload/task scheduling, response emission,
and one `ProjectState`. That uniquely owned project state contains the live
workspace, language-service databases, disk sources, roots/open documents,
configuration, and diagnostic records. Project mutations refresh databases at
explicit commit points; diagnostic publication is read-only.

`GlobalStateSnapshot` is an immutable view bound to one authoritative project
generation. Worker queries cannot read later live state or write snapshots
back. The typed in-memory `TestServer` exercises the production lifecycle
gates, request queue, `handlers::dispatch`, task-result path, and response
emission. A second test dispatcher, local JSON parameter model, manual live
state synchronization, and compatibility wrapper are prohibited. Remaining
JSON handling is limited to protocol serialization/projection boundaries,
extension payloads, tracing/profiling byte counts, and tests that inspect final
protocol shapes.

Workspace symbol detail metadata is a Vela protocol extension carried in
`WorkspaceSymbol.data.detail`. Upstream `lsp_types::WorkspaceSymbol` has no
top-level `detail` field, so typed projection keeps module/type detail there
while preserving ordinary LSP symbol fields.

The optional native LSP TCP transport is a debug/remote-integration extension,
not the default editor transport. It must be selected explicitly with
`--listen <host:port>`, bind only loopback addresses unless a future unsafe
opt-in is designed separately, and feed the same typed message loop, request
queue, `GlobalState`, handler dispatch, cancellation, profiling, and protocol
projection path as stdio.

Native LSP trace diagnostics are opt-in and stdout-safe. `--log <jsonl-path>`
writes typed main-loop startup, message receipt, and response-send events to an
explicit JSONL file with request IDs, methods, document URIs, lane, transport,
and launch metadata. Stdio stdout remains reserved for LSP protocol framing;
human-readable transport notices use stderr only where needed.

Initial LSP formatting uses source-preserving text edits in
`vela_language_service`: full-document formatting is driven by
`vela_syntax::formatting`, while range formatting only trims trailing
spaces/tabs inside the requested range. Neither path requires a successful
parse. `vela_syntax::formatting` owns stable token/trivia extraction from
parser-token spans and skipped source gaps, and `vela_language_service`
projects that stream into an editor-neutral formatting IR that preserves raw
comments, shebang trivia, spans, and blank-line whitespace groups. The first
full-document formatter normalizes token spacing and brace indentation while
preserving comments. It also tracks declaration-member brace contexts for
initial struct field, enum variant, trait method, impl method, and adjacent
top-level declaration layout. The richer formatter still needs AST-aware range
and on-type formatting rules before it can claim complete semantic formatting
coverage.

When the configured host schema is missing or unavailable, editor tooling
reports a schema diagnostic and treats schema-owned host, record, trait, and
enum receivers as dynamic `Any` for unknown-member diagnostics. Builtin
receiver diagnostics, parser diagnostics, HIR diagnostics, and non-schema
analysis diagnostics should still be published from the available source
facts.

LSP code actions may apply structured quick fixes and source-owned refactors,
but semantic rewrites such as absence-check to Option/Result guard conversion
must wait for a structured diagnostic or syntax pattern that proves the edit is
local, source-owned, and semantics-preserving. The server must reject dynamic
receiver typo fixes and ambiguous imports rather than invent type facts or
choose arbitrary declarations.

Schema artifacts may omit `schemaVersion` and `schemaHash` while exporters are
still simple, but any provided metadata is validated at load time. `schemaHash`
is a 64-bit FNV-1a hash of the canonical `RegistryFacts` payload represented by
the artifact, formatted as decimal or `0x`-prefixed hexadecimal. A mismatch is
treated as an invalid or stale schema and host facts degrade to `Any`.

Editor callable facts may expose schema enum tuple variants as constructors
only when the schema fields for `Enum::Variant` are numeric reflected tuple
field names such as `0` and `1`. Named schema variant fields are treated as
record-style fields and must not be ordered into callable parameters until the
schema contract carries explicit constructor shape/order metadata.

The next native LSP cleanup rewrites language-service feature queries around a
shared editor-neutral query model: request context, cursor context, symbol
identity, display parts, edit plans, rich completion items, relevance metadata,
and protocol projection boundaries. This refactor may break and delete the
current coarse completion model, thin completion item shape, feature-local
cursor scanners, and LSP conversion assumptions rather than preserving
compatibility shims. It should borrow rust-analyzer's high-level separation of
context construction, feature producers, item models, and LSP projection while
avoiding Rust-specific macro, trait-solver, and full Salsa complexity unless a
Vela-specific need appears. The execution plan lives in
[lsp-clean-architecture-refactor-plan.md](archive/lsp-clean-architecture-refactor-plan.md).

LSP authoring UX should align with rust-analyzer where Vela syntax overlaps.
This is a user-facing behavior contract, not a semantic import from Rust:
formatter output keeps Rust-like type argument spacing such as
`Map<String, i64>`; typed receiver `.` completion uses known source, schema,
trait, and builtin method facts without unrelated workspace fallback; completion labels stay
short and put owner/module paths in detail fields; declaration bodies such as
`struct Player { }` use declaration-specific contexts; and statement
completion provides Rust-like snippets such as `for in` and `match`. Rust-only
features such as macros, borrow checking, Rust trait solving, or script
generics remain out of scope.

The LSP authoring correction is a model refactor, not a patch list.
Completion should follow a rust-analyzer-style two-phase shape: build a
structured service-owned `CompletionAnalysis` from syntax recovery plus
semantic facts, run feature producers over explicit contexts such as path,
type, dot access, declaration body, call argument, pattern, and statement, then
render editor-neutral completion items and project to LSP. Member completion
uses one combined source/schema/stdlib/builtin member surface, and completion
item label, insertion text, owner details, docs, identity, ranking, and resolve
payloads stay separate until protocol projection.

The formatter side of the same correction belongs in `vela_syntax` as
syntax-owned CST/AST layout policy. Rust-analyzer can delegate Rust formatting
to rustfmt; Vela cannot rely on token-only whitespace cleanup for Rust-like
type hints. Builtin container type arguments must share one compact layout
rule across local annotations, parameters, returns, struct fields, enum fields,
and nested `Option`/`Result` hints.

Semantic highlighting uses an editor-neutral Vela taxonomy in
`vela_language_service` with standard LSP names where they exist and explicit
fallback names for custom token types. Custom tokens such as `builtinType`,
`const`, `state`, `boolean`, operator families, punctuation families, and
unresolved references keep their Vela-specific names in the primary legend.
The pre-hard-switch `null` token classification is removed when source-level
`null` is deleted. `vela_lsp_server` owns client-specific fallback projection:
clients that declare limited semantic-token support receive standard fallback
token names and supported modifier fallbacks in the server legend without
changing service classification. Editor packages may contribute fallback scope
metadata, but must not compute semantic classifications.

### Function Identity

Vela does not support function overloading. A module has one function per
script-visible name, and a type or trait has one method per receiver/name pair.
Arity, type hints, default values, and native Rust signatures do not create
overload sets. Resolver, reflection, native registration, and hot-reload ABI
logic should model each function name as a single callable.

Script methods may be declared as inherent type methods with
`impl Type { ... }` or as protocol methods with `impl Trait for Type { ... }`.
Inherent script method IDs are derived from the fully qualified receiver type
and method name. Trait method IDs remain derived from the fully qualified trait
and method name. A receiver type may not have two script methods with the same
name, even if one comes from an inherent impl and another comes from a trait
impl.

Closed builtin comparison traits are VM-recognized protocol names, not open
operator overloading. `PartialEq::eq(self, other)` returns `bool`.
`PartialOrd::partial_cmp(self, other)` returns the standard `Option` enum:
`Option::None` means incomparable, while `Option::Some(i64)` uses negative,
zero, or positive values for less, equal, or greater. Source ordering operators
return `false` for incomparable results. This first-slice return shape avoids a
new standard `Ordering` enum while preserving an explicit incomparability
channel. `Ord::cmp(self, other)` returns `i64` using the same negative, zero,
or positive convention and drives total-order collection helpers such as
`Array.sort`, `Array.min`, `Array.max`, and non-leaf `Array.sort_by` keys.
Leaf scalar/string/bytes sorting remains a runtime fast path, while object
sorting requires `Ord`; floats remain rejected by total-order helpers until an
explicit total-float ordering API exists.

Compiler identity lookup uses the definition registry, not reflection metadata
or `CompilerOptions` identity maps. During the registry migration the engine
keeps a `DefinitionRegistry` compile sidecar derived from validated reflection
and native metadata; source and hot-reload compiler entry points pass a
`RegistryCompileView` so native calls resolve to `FunctionId` before bytecode
emission. Reflection metadata remains the user-visible query surface, while the
definition registry is the compiler/linker identity source.
`CompilerOptions` may carry only non-identity compile settings or capability
hints, such as host index capability metadata and native module roots. It must
not store native function IDs, value method IDs, host type IDs, host field IDs,
host method IDs, or method parameter metadata.

### Primitive Native Contracts

Native parameter type hints are contracts, not conversion requests. Known
primitive parameters are checked by the compiler or by linked runtime guards;
positional native calls may still pass optional or variadic arguments after the
known metadata prefix until the registry has first-class optional/variadic
metadata.

Macro-generated descriptors for Rust `Option<T>` parameters and returns use
`TypeHint::Any` for now. The tuple/unit/null hard switch changes the
script-visible value to the dynamic standard `Option` enum rather than `null`.
This is a macro bridge limitation, not a language type-hint limitation: source
and explicit metadata may express `Option<T>` as a builtin contract, but the
current generated native wrapper keeps Rust `Option<T>` payload metadata erased
until conversion semantics are tightened. Typed native conversion still decodes
the `Option<T>` value at the Rust boundary.

Embedding float conversions are exact: Rust `f32` maps to Vela `f32`, Rust
`f64` maps to Vela `f64`, and the embedding layer does not silently convert
between integer, `f32`, and `f64` values.

Wrapping arithmetic and bit manipulation are explicit stdlib helper functions
for the primitive refactor checkpoint. Bitwise syntax operators are deferred.
The current representative shift helpers use `u32` shift counts, return zero
when the count is greater than or equal to the left operand width, and rotate
helpers use native modulo-width rotate semantics.

### String Literals And Interpolation

Multiline strings use triple quotes, `"""..."""`, and preserve body text
without indentation trimming. Interpolated strings require an explicit `f`
prefix, as `f"..."` or `f"""..."""`; ordinary strings never interpolate.
Interpolation supports `{expr}` plus escaped literal braces `{{` and `}}`.

Interpolated strings lower to a dedicated format-string bytecode instruction.
They must not lower through numeric `+`, implicit string concatenation, or a
stdlib compatibility helper. Runtime formatting uses the same user-facing
`OwnedValue::display_text()` rule as standard output.

### Runtime And Heap

The VM is a register bytecode interpreter. Execution budgets cover
instructions, memory, call depth, and patches. Runtime budgets keep immutable
limits, mutable counters, and precomputed active flags separate so hot paths
test budget mode directly instead of repeatedly interpreting sentinel limit
values. Script heap values use stable, generation-checked non-moving handles;
host refs and path proxies remain external handles and are not traced as
Rust-owned state.

Execution budgets account for heap collection growth at the mutation boundary
when either memory bytes or explicit collection limits are enabled.
`ExecutionBudget::unbounded()` disables instruction, memory, call-depth, and
collection-growth bookkeeping, so hosts can choose the lower-overhead trusted
path. Array and set budget deltas are based on script-visible element count
rather than spare `Vec` capacity; map deltas are based on script-visible keys
and values. Hosts may add explicit collection length limits for arrays, maps,
and sets independently from the byte budget. Native allocator reserve failures
are runtime allocation errors when the growth-budget path is active.

Typed scalar fast paths are interpreter specializations, not alternate
language semantics. Proven `i64` hot paths may use typed frame slots and fused
typed branch bytecode such as immediate compare or remainder-compare jumps, but
they must preserve the same checked arithmetic, division-by-zero, source-span,
budget, and hot-reload behavior as the generic bytecode path. These
specializations replace pre-release bytecode shapes instead of preserving
compatibility aliases.

Type facts and type hints may select static linked bytecode, field slots, and
guarded inline caches, but they are not required for ordinary dynamic member
access. If a receiver type is unknown, dot field access remains name-based
dynamic bytecode and fails only at runtime when the actual value does not
support the requested member. Linked bytecode must preserve that dynamic path
instead of treating unresolved field slots as link errors.

Map string-literal index bytecode is a source-level lowering for ordinary
`map["key"]` reads and writes. The instruction stores a `ConstantId` pointing
at a string literal, and runtime dispatch borrows that constant directly,
avoiding per-iteration string-object lookup and key cloning. Dynamic string
indexes continue to use the generic index path, and benchmark-specific fused
condition or method-call shapes are not part of this lowering.

`OwnedValue` is the Rust boundary/materialized value name. `Value` is the VM
runtime slot and is `Copy`, containing only scalars or handles. `HeapValue`
stores script heap objects, and heap containers store runtime `Value` entries
directly. There is no separate heap-slot type. Re-export surfaces should stay narrow: embedding
convenience modules may expose `OwnedValue` when it is part of normal host
ergonomics, but internal runtime slot types should remain under their owning VM
modules.

Engine embedding APIs use explicit boundary types. `CallArgs`, `args!`,
prelude exports, registered native functions, typed native conversion traits,
and callable native methods use `OwnedValue` when values cross as detached Rust
data. `Runtime::call` returns a runtime-managed `VelaValue` so hosts can keep
script aggregates on the persistent VM heap by default. VM execution frames,
closures, iterators, heap containers, and internal method dispatch use runtime
`Value`; the engine installs explicit conversion bridges when registering
native functions into a VM. Public VM program entrypoints use `OwnedValue`;
low-level runtime-slot program entrypoints are explicitly named
`run_program_runtime*` and are reserved for VM internals, low-level tests, and
benchmark harnesses. Public program entrypoints convert `OwnedValue` through a
temporary script heap and materialize the return before dropping that heap, so
they do not depend on `Value` retaining owned aggregate variants as a boundary
representation.

Runtime embedding has one high-level return-value surface. `Runtime::call`
returns a runtime-managed `VelaValue` pinned as a persistent runtime heap root.
Hosts can pass that value back into later calls on the same `Runtime` without
materializing or copying the script aggregate, and can explicitly call
`value_to_owned` when Rust needs a detached representation. A `VelaValue`
belongs to the `Runtime` that created it; passing it to another runtime is a
runtime type error. `VelaValue` is still script VM state, not Rust host state,
and it does not expose real Rust references or place Rust objects under script
GC. With the `serde` feature enabled, `Runtime::from_value` deserializes a
`VelaValue` directly from runtime `Value` plus heap state, so Rust can decode
script-owned results into structs/enums/scalars without first constructing a
detached `OwnedValue`.

Semantic object equality and ordering are opt-in through closed builtin
operator traits. `PartialEq` drives user-object `==`/`!=`, `Eq` marks full
equivalence, `PartialOrd` drives ordering operators, and `Ord` drives total
ordering and sorting. User records/structs do not receive implicit structural
equality or ordering; they must implement the builtin trait explicitly or use
explicit derive such as `#[derive(PartialEq, Eq)]` or
`#[derive(PartialEq, Eq, PartialOrd, Ord)]` when every field satisfies the
required traits. Missing support is a compile-time diagnostic when statically
known and a source-spanned runtime error for dynamic values. `Hash` is not a
script-visible builtin trait. `f32` and `f64` implement partial comparison
semantics but do not satisfy `Eq` or `Ord`, so float sorting and float
`Eq`/`Ord` derivation are deferred until a later total-float-order or explicit
partial-sort design.
Reference identity comparison for script heap objects and host refs uses
`===` and `!==`. These operators are not overloadable, do not call user
`PartialEq`/`Eq`/`PartialOrd`/`Ord`, do not call `ValueKey`, and must not read
host state. Statically known non-reference operands are rejected; dynamic
non-reference operands fail with a source-spanned runtime error. `==` and `!=`
must not recursively materialize and deep-compare object graphs; deep equality
belongs in an explicit, budgeted helper if it is added later.

Map and Set key semantics are owned by a focused runtime `ValueKey` layer.
Map keys and Set elements are script runtime `Value`s, but lookup and uniqueness
do not use Rust `Value` equality or user comparison traits directly. Instead,
`ValueKey` follows stable key classes: immutable leaf keys compare by value,
script heap objects and host refs compare by identity, and transient values are
rejected. Mutable records and structs must not use structural or user-defined
business equality as Map/Set keys, because field mutation would make the
container index unstable. Transient mutation proxies such as `PathProxy` are
not keyable until they have an explicit host path identity policy.
Array membership and dedup helpers use the same container-equivalence boundary:
`contains`, `index_of`, and `distinct` compare by `ValueKey`, not by
`PartialEq` or `Eq`. Business equality remains explicit through `==`, `!=`, and
predicate helpers such as `find`, `any`, `filter`, and `count`.

High-frequency embedding can cache script entry lookup with `Runtime::entry`.
The common call API remains `Runtime::call`: a `&str` target performs ordinary
name resolution, while a `VelaFunction` target carries the runtime id, entry
name, active version id, and cached parameter metadata. Runtime execution
resolves to a `CodeObject` before entering the VM so the VM does not repeat the
entry-name lookup on the hot path. Hot reload does not freeze old entry
handles; if the runtime version has advanced, the handle re-resolves by name
against the active program and reports the normal missing-function or ABI
errors if the target is no longer valid.

Rust-side calls to methods on returned `VelaValue` handles use
`Runtime::bind_method`, then pass the receiver-bound target to `Runtime::call`
or `Runtime::call_async`. Methods remain type-level script methods keyed by the
receiver script type and stable `MethodId`; there is no per-value method
registration or monkey patching. `Runtime::method` optionally caches the owner
type, method name, method id, version id, and parameter metadata before binding.
Calls validate the receiver runtime and script type, then re-resolve by method
id when the active version changes.

With the `serde` feature enabled, Rust structs and enums that implement serde
traits can cross the ordinary script-owned value boundary explicitly through
`to_owned_value`, `from_owned_value`, `CallArgs::with_serde_value`, and
`Runtime::set_state`. This path serializes Rust data into Vela-owned
records, enums, arrays, maps, sets, and scalars. It is a
snapshot/data-transfer path for messages, configs, VM state, and return values,
not a host-state binding: script mutation of the value does not write back to
the original Rust object unless Rust deserializes a returned value and applies
it itself. Host state that must be mutated in place still uses `HostRef`,
`HostPath`, `PathProxy`, and `HostAccess`.

`Runtime` and `VelaValue` are `Send` so hosts can move a runtime and retained
script values into worker or actor threads. They are not a concurrent execution
model: script calls still require mutable runtime access, and one runtime must
not be called concurrently. Persistent extern-state objects stored inside a
runtime therefore require `Send`; call-scoped direct host references remain local to
that invocation.

The compiler may replace a multi-instruction source-level lowering with one
semantics-equivalent bytecode instruction, such as `Truthy` for dynamic
truthiness coercion. Execution budgets are charged against the emitted bytecode
instructions, and optimized opcodes must preserve the same host, reflection,
GC-root, hot-reload, and diagnostic boundaries as their expanded VM sequence.

Before inline caches or JIT work, hot dispatch operands should move from
script-visible strings to stable IDs, slots, reusable path keys, or resolved
call targets. Names remain available for diagnostics, reflection, and source
reports, but they should not be the primary runtime key for hot native,
stdlib, script function, method, record-field, or host-path dispatch.

Managed heap entrypoints materialize return values at API boundaries. Native
calls materialize heap-backed values as needed so existing host/native APIs do
not own script GC state.

Read-only runtime access should avoid materializing owned boundary values.
After the `Value` / `OwnedValue` split, stdlib helpers read compact runtime
`Value` entries from heap objects directly. Mutable accessors, callback calls,
host/native interfaces, GC tracing, and hot-reload ABI remain separate
boundaries.

### Host Boundary

Host state is mutated through call-scoped `HostAccess` operations. Direct host
field, host path, and host method bytecode routes through `HostExecution`,
`ScriptStateAdapter`, and `HostAccess`; the adapter is updated immediately and
`HostAccess` does not retain a journal or mutation counter. There is no patch
descriptor, overlay, journal, host-write count budget, or end-of-call apply step in
the default host boundary.

Embedding APIs may accept Rust `&T` and `&mut T` at a `CallArgs` invocation
boundary, but these references are immediately represented inside the VM as
call-scope `HostRef` handles. Field access still goes through a
`ScriptHostObject`/adapter surface and `HostAccess`; `&T` is read-only and
`&mut T` enables write-through mutation without exposing the real reference to
script code.

Host path map keys store the script string key, not an opaque VM symbol. This
lets directly injected Rust objects and generic adapters resolve
`player.inventory["gold"]` without reaching back into VM symbol interners.
Host object method dispatch receives the full receiver `HostPath`, so root
methods, child collection methods, and trait-object field methods share the
same registration and permission model.

`#[derive(ScriptHost)]` owns generated direct-object field/path access for all
script-visible host fields. Plain `get`/`set` field metadata also means the
field participates in generated direct host path access. `#[script_methods]`
owns generated direct-object method dispatch for `&self` and `&mut self`
receiver methods; method arguments cross the host boundary through scalar
`HostValue` conversions. Child receiver method calls are forwarded through
script-visible fields by default.

Host collection and trait-object surfaces use the same concrete host type
registration model as structs. Rust-side helpers may generate concrete specs
for `HashMap<K,V>`, `HashSet<T>`, `Vec<T>`, or trait-object fields, but scripts
do not see generics and the builder does not expose separate collection-specific
registration APIs. Optional index support is type metadata on the concrete host
schema. Host method parameters that refer to other host objects use typed path
wrappers such as `TypedHostRef<T>` and `TypedHostMut<T>`, which store
`HostPath` only and never expose Rust references to scripts.

High-level embedding calls construct `HostAccess` internally and return a
runtime-managed `VelaValue`. Host mutation counting is not part of the default
host boundary; hosts that need diagnostics should instrument their adapter or
domain operations directly.
The public execution surface is the `Runtime::call`/`Runtime::call_async` pair
over one sealed target contract and `CallArgs -> VelaValue` boundary. Fallback
adapters are carried by `CallArgs` into the execution-owned host; raw and
adapter-specific execution entrypoints remain internal or are removed.

Persistent cross-call state follows the Explicit Runtime State Ownership
decision above. VM state is initialized transactionally and stored as a
Runtime-owned GC root addressed by stable `StateId`; extern state is a
type-checked host binding addressed through `HostRef` and HostAccess. Dense
`StateSlot` values are generation-local execution operands. The public Rust
surface is state-specific (`state`, `state_as`, `set_state`, `update_state`,
builder binding, replacement, and reload staging), with no dual-store fallback.
State value preservation is independent of module export compatibility.
Private state may be removed or promoted to public during reload, but removing
an existing public state export or downgrading it to private is an ABI break.
Initializer change reporting compares the direct executable plus its transitive
static script-call graph; existing Runtime values remain preserved, while the
report identifies the changed initializer behavior used by new Runtimes.

There is no default end-of-call apply or automatic rollback. If a script writes
a host field and later traps, the earlier Rust-side mutation remains. PathProxy
wraps HostPath and uses HostAccess, but complex Rust objects remain handles
and paths; the high-frequency host field boundary accepts only scalar
HostValue conversion. Owned complex script values cross through explicit
serialization/owned-value paths.

`ScriptHost` derives may declare reflected host trait implementations with
static `implements` metadata. This records TypeRegistry trait metadata for
reflection and ABI/schema hashing; it does not create script monkey-patching or
runtime trait-structure mutation.

### Reflection

Reflection metadata is copied, permission-aware, and read-only with respect to
type structure. TypeRegistry descriptors are the source for reflected types,
fields, methods, traits, variants, modules, functions, source spans, docs,
attributes, effects, access, and reflection-tool permissions.

Function descriptors keep public export status separate from reflection
visibility and reflective callability. Private functions may be visible to
authorized reflection tooling without becoming public API or reflective call
targets, and hot-reload ABI checks compare those access bits explicitly.

Reflective reads, writes, and calls resolve descriptor metadata to stable IDs
and route host interaction through HostAccess. Private, effectful, host path, and
field-level operations require explicit reflection permissions.

### Capability Profiles

The engine runtime exposes a domain-neutral `CapabilitySet` and named
`ExecutionProfile` constructors. Capability bits include host read/write,
event emission, deterministic time, controlled random, and controlled
reflection effects. Native and context calls declare `EffectSet`; pure calls
take the fast path, while effectful calls require the corresponding capability
bit before execution.

Fine-grained business permission strings are not part of the runtime native
call hot path. Hosts that need strict isolation should register only the native,
context, schema, and reflection surface that a script may use, then choose a
coarse execution profile for the allowed effect classes. Reflection's own
`ReflectPermissionSet` remains a tooling/policy model for metadata visibility
and controlled reflection operations; it must not be used as host business
authorization for native execution.

### Macro Stable IDs

User-facing host and native macros do not accept manually chosen numeric stable
IDs. `ScriptHost` and `ScriptReflect` derive type and field IDs from the
script-facing stable type path and field name, while `#[script_methods]` and
native function macros derive method/function IDs from the owner path or public
`::` qualified function name. Optional `alias` values are the compatibility mechanism
for rename-safe schema evolution. Low-level descriptor constructors may still
take explicit IDs for engine internals and focused tests.

Script-owned struct and enum payload fields are reflected as writable by
default because script values can be copied and updated without touching host
state. Copy-returning `reflect::set` for script values still enforces
`reflect_writable` and field-level required permissions, while HostRef
`reflect::set` additionally requires host field writability before recording a
HostAccess write.

Global field reflection enumerates both type-level fields and enum variant
payload fields. Variant payload field metadata uses `Type::Variant` as the
owner, matching targeted variant reflection, and policy filtering applies to
each field before it appears in `reflect::fields()`.

### Static Path Syntax

Vela uses `::` for static namespace paths: imports, type paths, enum variant
paths, native module functions, macro schema paths, and reflection module or
function identities. `.` is reserved for runtime value access such as fields,
methods, host paths, and metadata record fields. Dotted text remains valid as
ordinary data, for example event names and permission keys.

### Hot Reload

Hot reload replaces function-level or module-level code objects at safe points.
Old ProgramVersion handles keep old code alive, rejected updates do not advance
versions, and reports carry copied diagnostics plus ABI details.

Compiled updates may be staged before a safe point. Staging never advances the
active ProgramVersion; hosts must call the runtime reload check at event, tick,
or explicit call-boundary safe points to consume the pending update and receive
the accepted or rejected report. Host mutations write through immediately via
`HostAccess` and `ScriptStateAdapter`, so reload checks do not commit, inspect, or
rewrite patch journals; `HostAccess` does not retain one by default.
Reload checks also reclaim dead generation sidecars and their old-only VM and
extern state after released retained values are collected, even when no update
is pending; reclamation never requires a second accepted update.

Function, method, module, trait, schema, effect, access, parameter, return, and
source-span metadata participate in ABI validation. Engine registries are the
source for host/native ABI manifests.

Accepted hot-reload reports distinguish actual bytecode-changed functions from
source-changed modules. Module impact is derived from deterministic source
hashes and reverse import dependencies so hosts can invalidate module-scoped
caches without treating every recompiled function as changed.

Changed-file hot reload events are watcher ergonomics, not partial compilation.
The engine validates the changed `.vela` path, then recompiles the full module
root so import resolution, dependency impact, and ABI checks always see the
complete module graph.

### Standard Library And Dynamic Types

Option and Result are dynamic enum-shaped values, not script generics. Stdlib
helpers and analysis TypeFacts may describe dynamic payloads, but the language
surface remains non-generic.

Script type hints are advisory metadata for analysis, reflection, dispatch
hints, and ABI. They do not enforce script-local runtime value types unless a
host, native, or schema boundary explicitly performs conversion or validation.
Function return annotations are optional and have the same metadata-first
semantics.

The tuple/unit/null hard switch replaces no-value and void-like results with
`()`, host nullable boundaries with typed `Option<T>` or an explicit future
external-data wrapper, and missing metadata with `Option`, omitted fields, or
structured absence. Expected absence should use `Option::None`, recoverable
business failure should use `Result::Err`, and unrecoverable script/runtime
failures should use VM diagnostics rather than `Result::Err`.

The core implementation names the no-value runtime concept `Unit` across
`Value`, `OwnedValue`, `HostValue`, bytecode `Constant`, `PrimitiveTag`,
reflection `TypeKind`, C API value kind, type facts, verifier names, hot-reload
schema ABI, and standard metadata. Public script and type-hint spelling is
`()`. Active protocol JSON `null` values remain allowed only as external
JSON-RPC/LSP encoding, not as a Vela language value.

Array, map, set, string, range, math, context, random, and other
domain-neutral helpers are deterministic unless an Engine-installed
capability-gated native explicitly provides controlled nondeterminism.

Host-provided deterministic time belongs to the `time` stdlib module
(`time::now`, `time::tick`, `time::elapsed_since`). `ctx` remains available for
host-registered context objects, fields, methods, events, and logging examples,
but it is not the builtin time module namespace.

### Reflection Permissions

The core reflection policy API owns base call authorization. Direct reflective
method calls and reflected function invocation must require
`reflect::call_methods` before checking callable metadata, required host
permissions, or effect-specific call permissions.

### Analysis And Tooling

TypeFacts, completions, hover, match exhaustiveness, effect diagnostics,
unit/tuple facts, Option/Result predicate narrowing, and pattern diagnostics
are analysis/tooling data. They should not change VM semantics unless a
separate compiler/runtime decision says so.

### Indexed For-In

`for index, value in iterable` is syntax-level sugar over the existing `for-in`
lowering, not an eager `enumerate()` collection method or a Rust-style iterator
adapter. The exposed index is the source iteration position. If the value
pattern skips an item, later matching iterations keep their original source
indexes instead of being renumbered by body execution count.

### Example Layout

Runnable examples live in a standalone `examples/Cargo.toml` package excluded
from the default workspace so `cargo test --workspace` stays focused on core
development tests. CI and release validation run the examples explicitly with
`cargo test --manifest-path examples/Cargo.toml`. Example bins live under
`examples/src/bin/<example>/`; each example keeps its `main.rs` and `.vela`
source files in the same directory so users can inspect and run one capability
without following a parameter-dispatched demo runner or a separate script tree.

### CLI Role

`vela_cli` is the final direct script execution binary, analogous to a language
runtime command. It must stay domain-neutral and must not embed example host
state such as Player, Monster, Context, or permission-denial fixtures. Host
world demos belong in `vela_examples`; `vela_cli <script.vela>` compiles the
file, runs `main()` with no host arguments, and prints the returned value.

### Package And Service Providers

Vela plugin discovery should use package manifests plus a trait-backed service
provider catalog, not script-side runtime `require`, `eval`, or directory
scanning. Package manifests own source roots, path dependencies, package
identity, and requested capabilities. Module identity is package-aware:
`PackageId + ModulePath`; `SourceId` remains internal.

Service providers are explicit trait implementations exported with
`#[provider(id = "...")]`. The service trait is inferred from
`impl ServiceTrait for ProviderType`; the attribute carries only the stable
provider identity and export intent. Provider identity is
`PackageId + ServiceTraitId + ProviderId`, so provider type renames do not
change host-visible SPI identity. First-slice package dependencies are path
dependencies only; foreign host-language modules, remote registries, version
solving, and script-side package loading are deferred.

`vela_package` is the dependency-light owner of the structured manifest and
package graph. `[host]` is accepted only in the root manifest; workspace member
and path-dependency manifests that contain it are rejected so imported code
cannot change host schema or grant policy. Engine and language-service IO front
doors consume the same graph builder and manifest diagnostics.

### Opt-In IO Stdlib

I/O is an Engine-side native stdlib extension, not a VM-default primitive.
Embedders must opt in with `with_stdio()` and/or `with_fs_io(root)` and grant
`io_read`/`io_write` capabilities. Filesystem helpers operate only on relative
paths inside the configured sandbox root. Ordinary filesystem failures return
script-visible `Result::Err(IoError)` values; capability failures and runtime
type errors remain VM diagnostics.

### Public Docs And Playground

Public documentation lives in `site/docs/{en,zh}` as bilingual Markdown, and
the GitHub Pages site is static HTML/CSS/JS without a frontend build system.
The browser playground uses a dedicated `vela_playground_wasm` crate compiled to
`wasm32-unknown-unknown`; Pages generates `site/pkg` with `wasm-bindgen` during
deployment rather than committing generated browser bindings.

The playground WASM boundary returns stable JSON strings for compile/run
results and diagnostics. It enables standard natives plus controlled time and
random capabilities, but does not expose host mutation, filesystem I/O, or host
state in the browser sandbox.

### Debugger Support

Debugger support is a post-MVP runtime and Debug Adapter Protocol capability,
not a script-language feature. Runtime debug hooks may expose source
breakpoints, stepping, stack frames, watches, safe HostRef display, HostAccess
preview, and hot-reload breakpoint rebinding, but they must respect reflection,
host access, HostAccess, and TypeRegistry boundaries.

Bytecode code objects carry read-only frame maps for debugger and diagnostic
inspection. These maps may name parameters, locals, pattern bindings, and
captures with their registers and source spans, but they must not affect VM
execution or allow runtime mutation of type or host structure. Runtime stack
frames should preserve caller bytecode offsets as observational metadata for
stepping, profiling, and future breakpoint rebinding. Runtime call frames
should also keep register-to-GC-root metadata separate from collection policy
so debuggers and future optimized backends can inspect roots without changing
which values the collector preserves.

### Cranelift JIT

Cranelift JIT is a mandatory post-MVP backend after interpreter optimization,
inline caches, debugger contracts, and conformance are stable. JIT must remain
disableable, must be semantically equivalent to VM execution, and must preserve
ExecutionBudget, GC roots, HostAccess, reflection policy, hot reload invalidation,
and debugger-visible frame/source metadata.

### Value Method Identity

Value method compilation resolves receiver value facts to stdlib `TypeId`
definitions and then resolves methods through the `DefinitionRegistry`.
`CallMethod` carries a typed `MethodId` for value methods when a registry view
is available. Named argument metadata and method identity come from method
definitions in the registry, not from `CompilerOptions`.

### Host Definition Runtime IDs

Host types, fields, and methods register into `DefinitionRegistry` with
semantic IDs derived from canonical `DefPath`. Adapter-facing runtime IDs such
as `HostTypeId`, host `FieldId`, and `HostMethodId` are stored as host runtime
metadata on those definitions and are used only when emitting current
`HostTargetPlan` and host call operands. This keeps registry identity globally
deterministic while preserving existing HostAccess adapter contracts.

### Unlinked Bytecode Naming

Compiler output bytecode is named `UnlinkedProgram`, `UnlinkedCodeObject`,
`UnlinkedInstruction`, and `UnlinkedInstructionKind`. These types may still be
consumed by current runtime image and VM paths until the linked-bytecode phase
lands, but new compiler-facing APIs should use the unlinked names and must not
reintroduce ambiguous `Program` or `CodeObject` output types.

### Linked Bytecode Shape

Executable bytecode is represented by `LinkedProgram`, `LinkedCodeObject`,
`Instruction`, and `InstructionKind`. Linked instructions carry dense runtime
handles or slots such as `NativeHandle`, `ScriptFunctionHandle`,
`MethodDispatchHandle`, `TypeHandle`, `VariantHandle`, and `FieldSlot`.
Human-readable names live in a `DebugNameTable` side table and linked
instructions reference them by `DebugNameId` only.

### Linked Bytecode Linker

`vela_bytecode::linker` converts `UnlinkedProgram` values into
`LinkedProgram` values. Native functions, methods, script functions, types,
and variants are stored in linked side tables owned by the linked program, and
instructions carry only dense handles, slots, host target plan IDs, or state
slots. Name-only method and record/enum field bytecode is rejected by
`LinkError` instead of being preserved as runtime fallback dispatch.

### Linked Bytecode Verification

Linked bytecode verification checks local register, constant, jump,
cache-site, and host target invariants plus linked-program side-table
references. Invalid debug names and out-of-bounds native, script function,
method dispatch, type, or variant handles are rejected before linked bytecode
can become executable.

### Linked Closure Ownership

Linked closures store `ScriptFunctionHandle` values and execute only with the
owning `LinkedProgram` that contains those handles. Higher-order stdlib
callbacks must carry linked-program context through `MethodRuntime`; they must
not reconstruct unlinked bytecode or rely on script-provided method names.

### Nested Linked Function Handles

The linker assigns nested `ScriptFunctionHandle` values in the same order that
linked nested functions are appended to the linked program side table. Recursive
linking must not reserve handles before recursively appending child functions,
because transitive closures would otherwise point at the wrong code object.

### Primitive Embedding Conversions

Rust embedding conversions preserve concrete scalar tags exactly. A Rust
`i32` argument becomes Vela `i32`, not `i64`; `HostValueInto`/`HostValueFrom`
use the same exact-tag rule for host fields and methods. Callers that intend an
`i64` contract must pass an explicit `i64` value, and HostAccess arithmetic
rejects mixed scalar tags instead of widening or narrowing.

Rust `Vec<u8>` and byte slices cross embedding and host boundaries as the
`bytes` primitive. Other `Vec<T>` values remain arrays; `Vec<u8>` decode
expects `OwnedValue::Bytes`/`HostValue::Bytes` instead of accepting an array of
`u8` scalars as an implicit conversion.

Rust `Option<T>` crosses embedding and serde owned/runtime value boundaries as
script `Option::Some(value)` or `Option::None`. Unit `()` is not accepted as an
`Option::None` sentinel, and raw payload values are not accepted as implicit
`Option::Some` values.

Serde owned-value conversion preserves primitive tags exactly. Rust `i8`,
`u32`, `u64`, `f32`, and the other scalar primitives become matching
`ScalarValue` variants, and deserialization expects the same concrete tag
rather than widening, narrowing, or integer-float conversion. `u64::MAX` is a
supported exact boundary value.

Serde byte buffers use the explicit Serde bytes hook (`serialize_bytes` /
`deserialize_byte_buf`) to cross as `OwnedValue::Bytes`. With `serde_json`,
that hook is represented as a JSON byte array, not base64 or hex. Large
unsigned integers use JSON integer text and must round-trip through Rust
`serde_json` as `u64` without precision loss; JavaScript-number-safe encodings
would require an explicit future config rather than a hidden conversion.

The C ABI value surface uses explicit primitive tags (`I8` through `U64`,
`F32`, `F64`) instead of old `Int`/`Float` tags. C arguments are copied into
Vela-owned `OwnedValue` values before execution; returned strings and bytes are
ABI-owned buffers that callers must release with `vela_value_free`, or with
the specific `vela_string_free` / `vela_bytes_free` helper when they own the
raw pointer directly.

Hot-reload function, method, trait, and schema compatibility checks normalize
primitive type hints through `PrimitiveTag` before comparing contracts.
Changing any primitive contract, such as `i32 -> i64`, `i64 -> u64`,
`f32 -> f64`, or `Bytes -> String`, is incompatible unless a future explicit
product compatibility rule is added. Report rendering may still use hint text
for diagnostics, but compatibility decisions must not depend on old `int` or
`float` names.

Host schema derive inference emits exact supported primitive hint names from
Rust field types (`i8` through `u64`, `f32`, and `f64`). Platform-sized or
unsupported wide Rust integer fields such as `usize`, `isize`, `i128`, and
`u128` do not receive an inferred primitive hint; callers must provide an
explicit supported contract instead of relying on an alias or hidden
conversion.

### Controlled Dynamic Method Dispatch

Unknown-receiver calls with a source-static method name are first-class linked
dynamic method calls, not legacy name-only fallback. Static known receiver
calls keep the `MethodId` / linked-dispatch fast path, and statically provable
missing methods may remain compile-time diagnostics. Runtime dynamic dispatch
resolves through controlled standard, script, or host metadata, preserves
source argument names until target lookup, reports source-spanned runtime
errors, and guards inline caches by receiver identity plus schema/hot-reload
epochs where applicable.

### Benchmark Comparison Modes

External language comparison rows must report their execution mode. Vela uses
`internal_hot_loop`, embedded Lua 5.4 and Rhai use `embedded_hot_loop`, and
Node.js/Python 3 use `process_hot_loop`. Mixed-mode benchmark rows are
directional references and must not be collapsed into one fairness ranking or
mixed with VM cache-delta rows.

### Typed Scalar Bytecode

The first non-JIT scalar specialization tier is verified `i64` bytecode emitted
from compiler-owned type facts. Dynamic or mixed numeric operands stay on
generic scalar bytecode, and typed operations preserve checked arithmetic,
source spans, hot-reload compatibility, and HostAccess boundaries. Direct
integer range loops may use `I64RangeNext`; broader numeric matrices and
superinstructions require separate measured justification.

I64 immediate comparisons use a single comparison opcode carrying the compare
operator. Arithmetic-with-immediate bytecode, such as remainder by a constant,
must stay separate from compare/jump bytecode unless a future profiling pass
justifies a broadly reusable superinstruction family. Do not add
benchmark-shaped fused opcodes such as remainder-by-immediate plus equality
plus jump.

Superinstructions must be lowered only when the compiler can prove the fused
condition shape directly or prove that removed temporary registers are not
observable. Do not add post-compile fused rewrites from adjacent opcodes alone.

### Runtime Scalar Value Layout

The VM runtime `Value` enum stores primitive scalar tags as direct variants
(`I8` through `U64`, `F32`, and `F64`) instead of wrapping a nested
`ScalarValue`. `ScalarValue` remains the boundary representation for
`OwnedValue`, `HostValue`, constants, reflection, serde, C/API-facing values,
and diagnostics. Runtime-to-boundary conversion must go through
`Value::from_scalar` and `Value::as_scalar` rather than reintroducing a nested
runtime scalar variant.

### First-Class Char Primitive

`char` is a first-class primitive with Rust `char` semantics: one Unicode
scalar value, not a byte and not a one-character string. Vela uses single-quote
char literals such as `'x'` and `'\u{5956}'`; double-quote literals remain
strings. String iteration yields `char` values. The pre-release implementation
does not preserve the old internal behavior where serde decoded
single-character strings as Rust `char`. Minimal char methods mirror Rust names
for common operations: `to_string`, `is_whitespace`, `is_ascii`, and
`is_ascii_digit`.

### Rust-Like String Indexing

Vela strings follow Rust `str` indexing semantics. `string.len()` returns byte
length, `string.find(needle)` returns an optional byte index, and
`string.slice(start, end)` uses a byte range that must land on UTF-8 character
boundaries. Character-level traversal uses `for ch in text`, yielding
first-class `char` values. Vela does not expose a `char_at` random-access API
because UTF-8 character indexing is O(n) and would misrepresent performance.

### String Parse Surface

String parsing methods use exact primitive names:
`parse_i8`, `parse_i16`, `parse_i32`, `parse_i64`, `parse_u8`, `parse_u16`,
`parse_u32`, `parse_u64`, `parse_f32`, `parse_f64`, `parse_bool`, and
`parse_char`. Each returns `Option<T>`. Integer parsers reject invalid or
out-of-range text, float parsers reject invalid, `NaN`, and infinite values,
`parse_bool` accepts only `true` and `false`, and `parse_char` accepts exactly
one Unicode scalar value.

### Iterator View Naming

Explicit one-shot iterator creation uses `values()` / `iter()` for arrays,
sets, and bytes, `iter()` for maps and ranges, and `chars()` / `bytes()` for
string traversal. Direct bytes `for-in`, `bytes.iter()`, and `bytes.values()`
yield `u8` values. Direct map `for-in` and `map.iter()` yield
`MapEntry { key, value }` records in key order, matching Rust's key/value map
iteration model without exposing references. `map.keys()` and `map.values()`
are explicit projection views, and `map.entries()` is equivalent to
`map.iter()`.

### Iterator Adapter Ownership

Lazy iterator adapters are one-shot cursors that take ownership of the source
iterator state and leave the original iterator exhausted. Adapter stepping,
`for-in`, and terminal methods use the callback-capable method runtime so
`map`, `filter`, `any`, `all`, `find`, and `collect_array` share callback
dispatch, heap-root protection, budget, and host-access behavior.
Iterator terminals that materialize collections are explicit:
`collect_array`, `collect_set`, and `collect_map`. `collect_map` consumes
`MapEntry { key, value }` records and duplicate keys follow map insertion
semantics, so later entries overwrite earlier entries.

### Iterator Source Bounds

Collection-backed iterators read source heap slots lazily instead of copying
the full collection at creation. Arrays and sets snapshot traversal length, and
maps snapshot traversal keys, so later writes to existing items are observed
while later growth does not extend the iterator.

### Public Type Hint Spelling

Public script type hints use lowercase only for scalar/literal primitive
contracts such as `bool`, `char`, `i64`, and `f64`. Unit uses `()` syntax.
Erased dynamic, text/binary, collection, callable, and Option/Result contracts
use capitalized names: `Any`, `String`, `Bytes`, `Array`, `Map`, `Set`,
`Range`, `Iterator`, `Function`, `Closure`, `Option`, and `Result`.

Only builtin type-hint contracts may be parameterized:
`Array<T>`, `Set<T>`, `Map<K, V>`, `Iterator<T>`, `Option<T>`, and
`Result<T, E>`. They exist to make contracts, diagnostics, static facts,
bytecode guard metadata, mutation checks, embedding metadata, reflection, and
hot-reload ABI precise without introducing a general script generic system.
`Map<K, V>` keys and `Set<T>` elements use the runtime `ValueKey` keyability
contract: immutable leaf values compare by value, script heap objects and host
refs compare by identity, and transient values such as `PathProxy` are
rejected. User/schema/host generics such as `Player<T>`, scalar
parameterization such as `String<T>`, and callable signature syntax such as
`Function<T>` remain rejected. Unparameterized `Array`, `Map`, `Set`,
`Iterator`, `Option`, and `Result` remain valid erased contracts.

### Try Propagation Families

Typed `?` propagation is family-preserving. An `Option`-returning function may
propagate `Option` values, and a `Result`-returning function may propagate
`Result` values. `TryPropagate` bytecode carries the expected family from the
enclosing typed return contract when known, and the VM rejects cross-family
`Option`/`Result` operands before either continuing with payloads or
short-circuiting with absence/error values. Explicit helper methods are the only
allowed bridge between `Option` and `Result`.

### Reflection Type Hint Descriptors

Reflection records expose raw type-hint strings for display plus optional copied
`ReflectTypeHint` descriptors for structured inspection. The descriptor fields
are `display`, `kind`, `name`, and `args`; tuple descriptors use
`kind == "tuple"` with element descriptors in `args` and `name == Option::None`,
while unit uses `kind == "unit"` and `name == Option::Some("()")`. Missing,
empty, or unparsable hint descriptors are represented as `Option::None`, not
unit or any sentinel value.
`reflect::type_of(value)` follows the same absence rule: values with registered
reflected type metadata return `Option::Some(ReflectType)`, and values without
registered metadata return `Option::None`.

### Missing Sentinel Scope

`Value::Missing` and `CallArgument::Missing` are VM-internal call/default
sentinels only. Public boundaries must not expose a Missing value or kind:
`OwnedValue`, `HostValue`, C ABI values, serde conversion, playground JSON,
reflection records, and user-visible no-result paths use `()`, `Option`,
`Result`, or typed structured data instead.

### HIR Default Expression Ownership

Schema field defaults are HIR expression bodies owned by
`HirBodyOwner::SchemaFieldDefault`, so path facts inside struct and enum field
defaults are resolved by HIR rather than compiler-local source scans.
Interpolated string placeholders are also lowered as child HIR expressions.
Tuple projection members are HIR field facts, but numeric tuple members do not
participate in composed value paths.

### LSP On-Type Formatting Scope

Native on-type formatting is conservative: it may respond to closing brace and
newline triggers, but edits must be limited to the current brace-delimited
construct or a current-line fallback. Broader AST-aware reflow remains a later
formatter capability and must not be reached through whole-document edits while
the user is typing.

### Scalar Constant Evaluation in MIR

Every source-derived unit, boolean, character, or numeric value has an explicit
single-assignment MIR temp definition at its evaluation point. The definition
records whether the value is a direct literal, a folded literal, an evaluated
const/schema value, or a pattern literal. This provenance is semantic input to
physical selection, not a bytecode opcode choice: the backend may select an
inline immediate only from a verified eligible definition and must otherwise
materialize the constant at that definition point. Compiler-synthesized CFG
constants remain inline operands where their enclosing MIR operation already
fixes the evaluation point.

### MIR Compatibility Canonical Forms

Record-pattern compatibility is decided before MIR construction. Unqualified
source, imported, and unresolved record patterns retain the legacy unspanned
`UnsupportedSyntax("match pattern")` error. Qualified source and registered
record patterns retain their legacy always-false behavior through an explicit
`NeverMatchesRecord` compile target and pure `NeverMatches` MIR predicate;
they are not represented as valid structural record matches. Dynamic variant
predicates check only owner and variant tags. A field that is actually used is
projected afterward through the enum-field family and may trap independently.

MIR field targets distinguish record-name and variant-name reads. Ordinary
dynamic member access uses the record family. Assignment preparation
normalizes stable or dynamic enum-family steps to record-name steps, preserving
the current `GetRecordField`/`SetRecordField` behavior and its runtime failure
for enum writes; MIR has no `SetEnum*` fiction.

Try propagation is a verifier-proven `TrySwitch` region with ordered
Option/Result continuation layouts, one payload read per continuation, a
shared return-original propagation edge, an owned type-mismatch edge, and an
explicit join. `TryTypeMismatch` is invalid outside such a region. Physical
backends consume the verified region atomically as the existing
`TryPropagate` instruction so instruction-budget behavior remains unchanged.

Recoverable specialization guards are context-free type assumptions with
distinct passed and slow CFG edges. MIR v1 bytecode lowering always chooses the
slow edge with an explicit jump; it does not invent a predicate, trap, or
deoptimization mechanism. Callable arity remains part of callable contracts,
and truthiness remains an ordinary rvalue plus branch rather than separate
guard-assumption kinds.

### MIR Liveness Semantics

MIR liveness is backward logical-value liveness before physical register
allocation. Statement destinations kill their prior local value or define a
single-assignment temp after operand uses. Iterator and range items are defined
only on the `next` edge; range cursor/exhaustion state is redefined on both
edges while its prior state remains a terminator use. Safepoint live sets are
the live-before set for the allocating/calling operation, so an operation's
result is never treated as live before it exists. Debug-local regions are the
blocks where their logical storage is live or defined, with parameter and
capture storage beginning at entry. The verifier recomputes and compares every
computed liveness, safepoint, and debug-region record.

### MIR Physical Backend Handoff

`verify_mir` proves structural and semantic MIR, including any explicitly
computed live metadata. Physical code generation accepts only the separate
borrowed `MirBackendHandoff`, which additionally requires computed liveness for
every defined function. This keeps test-configured uncomputed MIR useful for
isolated invariant tests without allowing it to reach register allocation or
instruction selection. The handoff owns no fallback queries: physical backends
consume its MIR target table, logical values, canonical CFG, effects, guards,
origins, debug records, and safepoint metadata directly.

### MIR Production Hard Switch

Verified MIR is the sole production runtime body-lowering input. Every compile
front door constructs Heavy HIR, one immutable analysis/compile-target
generation, MIR, `MirBackendHandoff`, and existing bytecode. There is no
backend selector, direct HIR body emitter, compatibility adapter, or fallback
query. Source parsing and compile-target preparation remain a front-door
service, while direct HIR traversal in `vela_bytecode` is limited to the
compile-time const/schema evaluator and never emits runtime code.

The bytecode MIR backend owns physical registers, constant/target interning,
instruction selection, CFG layout, cache sites, guards, frame projection, and
bytecode verification. MIR owns only logical liveness and safepoint/debug
metadata; the VM continues conservative register root tracing. Existing VM
instructions and instruction-budget observability are unchanged, MIR IDs stay
generation-local, and Cranelift remains M22 work.

Engine-owned reflection and native descriptors are authoritative compile-target
inputs. Registered reflected types preserve their `TypeKey` ID and split
qualified names into proper definition-path modules so exact host contracts
resolve by short or qualified name. Descriptor types without a runtime
`HostTypeId`, plus explicitly typed native boundary values, may be declared
opaque through compiler options; arbitrary unresolved registry hints remain a
compile-input error.

### Executable Generation Follow-On Contract

Post-hard-switch execution uses one explicit immutable generation boundary.
ProgramVersion owns same-generation verified MIR and one linker-produced linked
artifact. The linker is the only authority for flattened executable handles,
ProgramImage indexes, generation-global cache-site IDs, and immutable
cache/profile layouts. RuntimeState owns generation-keyed mutable sidecars for
cache entries, profile counters, hotness, and active tier selection, plus heap,
roots, and VM/extern state. Sidecars never index a different generation's
layout and are pruned at safe points according to external linked-artifact
ownership after excluding owners reachable only from inactive state roots.

Dense executable identities such as `ScriptFunctionHandle`, `CacheSiteId`, and
profile slots are valid only with their owner generation. Stable semantic IDs
support ABI comparison but never implicitly migrate an old frame or closure to
new code. Linked closures and active frames pin their immutable linked owner;
old closures execute old code, and new entry calls after safe-point reload use
the new ProgramVersion.

Verified MIR is an owned retainable backend contract. CFG value facts and guard
refinements are keyed by logical value and MIR program point, not physical
register or emission order. Typed operations require exact proven or
guard-refined facts. Safepoints are unique program points. Value liveness,
GC-root liveness, and lexical debugger availability are distinct analyses.

The detailed migration and acceptance gates are defined in
`docs/archive/mir-executable-generation-architecture-plan.md`. This contract
supersedes earlier future-facing statements that ProgramVersion owns mutable inline-cache
state, that raw bytecode instruction count must remain the permanent budget
unit, or that generation-local closure handles may be resolved against the
runtime's current program.

### Batch E Sealed Generation Representations

Whole-program compilation produces one non-cloneable `CompiledProgram` that
owns unlinked bytecode, verified MIR, and the executable identity sequence.
Only the linker can consume those parts, and it publishes a non-cloneable
`LinkedArtifact` already bound to the exact MIR functions. ProgramVersion,
restricted JIT input, RuntimeImage, linked calls, frames, and closures all carry
the same `Arc<LinkedArtifact>`; no parallel `Arc<LinkedProgram>` owner or
content-cloning publication path exists.

Every MIR budget point retains a backend-neutral site, class, and unit count in
linked instruction metadata. Backedges use successor-specific edge stubs, and
binding rejects missing, duplicate, reordered, moved, extra, or incorrectly
encoded charges before publication. Total units remain only a secondary
consistency check.

MIR functions own lexical scope origins derived during construction. Debug
availability is the intersection of definite initialization and active lexical
scope, independently of value liveness. Shape facts carry stable `FieldId` or
backend-neutral ordinals; each backend maps those identities to physical slots.
Cache-bearing opcodes select one compile-time exhaustive policy that defines
kind and sidecar/optional/required storage for compiler allocation, image
remapping, linking, and both verifier layers.

The Batch E file-size audit introduced no new over-threshold active file.
Pre-existing reviewed exceptions touched by the ownership migration remain
`vela_vm/src/runtime_type_guards.rs`, `vela_vm/src/tests/type_guards.rs`,
`vela_vm/src/script_method_calls.rs`, `vela_vm/src/linked_execution.rs`,
`vela_bytecode/src/verification/linked.rs`, and
`vela_bytecode/src/linked.rs`. They remain cohesive runtime contract,
execution, verifier, or instruction-schema surfaces; Batch E changes there are
localized ownership plumbing. Newly growing bytecode root and cache-fixture
files were kept below 1200 lines by extracting budget metadata and fixture
finalization.

### Batch F Fail-Closed Executable Representations

`LinkedArtifact` is the sole production linked-generation type and always owns
non-optional verified MIR plus the complete executable mapping. Low-level
linking stages through a private unbound value consumed by cohesive compiled
program linking; production Runtime, RuntimeImage, Engine, hot-reload, and JIT
input APIs cannot accept unlinked bytecode.

Each linked artifact owns an independently sealed executable budget layout.
Every row fixes the MIR site, exact instruction offset or edge, class, units,
and semantic boundary family. Publication validates that layout against both
verified MIR and the emitted operation, while instruction origin and charge
metadata remain construction-sealed. Function totals are only a secondary
consistency check.

One macro-generated exhaustive declaration defines cache kind, storage, read
access, and write access for linked and unlinked instructions; compiler
attachment, image remapping, linker projection, and both verifiers consume that
interface. MIR blocks, statements, and terminators likewise own active lexical
scope sets projected during lowering. Debug availability intersects those
program-point facts with definite initialization and never derives scope from
source-span containment.

### Backend-Neutral Execution Units

The long-term execution-budget unit is a deterministic MIR semantic work unit,
not emitted bytecode instruction count. Charges occur at verified program
points such as loop backedges, calls, allocation/dynamic work, and observable
host/reflection boundaries. Bytecode and future JIT consume the same schedule.
The migration is one explicit pre-release breaking change: public counters,
diagnostics, examples, and exact-edge tests move together, with no dual legacy
instruction-count mode. The concrete work-unit table is recorded when the
budget phase activates.

The execution-unit contract is now active and versioned by the bytecode/MIR
format generation. Each listed boundary costs one unit unless the operation
records an explicit positive unit count:

| MIR/runtime boundary | Unit rule |
|---|---|
| CFG backedge | one before taking the backedge |
| iterator/range step | one before requesting the next item |
| script, closure, native, stdlib, value-method, or dynamic call | one before dispatch |
| callback invocation | one before each callback entry |
| aggregate/closure/iterator/format allocation | one before allocation |
| dynamic operator, index, or runtime guard work | one before work; bounded container scans add one per inspected item |
| HostAccess read/write/mutate/remove/call | one before the host boundary |
| reflection read/write/call | one before the reflection boundary |

Pure register moves, constants that require no allocation, static scalar
arithmetic, branches, and returns cost no unit. Memory, collection-growth, and
call-depth limits remain separate. The bytecode backend attaches each explicit
MIR charge to the next semantic operation as instruction metadata; the VM also
charges runtime-sized scan/iterator/callback work. It never derives cost from
opcode dispatch count.

Trap ordering is part of the language contract. A charge preceding an effect
must succeed before that effect begins. Effects completed before a later charge
trap remain committed, including HostAccess writes. Backends may coalesce
charges only across pure, non-trapping, non-allocating regions without calls,
host/reflection effects, safepoints, or debugger boundaries.

### Retained M22 Input And Publication Boundary

`LinkedArtifact` stores an explicit mapping from each generation-local MIR
function identity to its linked executable handle. `ProgramVersion` exposes a
read-only restricted-JIT input view containing the same-generation verified MIR,
linked code, budget schedule, effects, liveness, safepoints, and eligibility
result. Eligibility reads no HIR, analysis service, source text, or current
registry state.

M22 may publish an immutable compiled artifact only into its owning
ProgramVersion generation. Runtime-local tier selection, hotness, counters,
and caches remain RuntimeState sidecars. Reload activates a new generation and
never rebases compiled handles. Compiled frames must report GC roots from the
verified root-live map, preserve debugger/source side exits, and exit through
the same HostAccess, reflection, budget, and diagnostic helpers. This track
does not create a compiled-artifact store or a JIT runtime option.

### Accepted Executable-Generation Interpreter Cost

The executable-generation correctness hard switch removed layout-dependent
bytecode peepholes and made verified MIR, CFG facts, liveness, safepoints,
debug availability, and budget points generation-retained. The Phase 9 scalar
comparison preserves the Phase 0 checksum but measures 18,688 ns per benchmark
iteration versus 8,312 ns at the pre-change checkpoint. This 124.8% regression
exceeds the normal threshold and is explicitly accepted for this architecture
boundary because restoring emission-order inference or unverified peepholes
would violate the correctness contract.

Execution-unit charges are fused into operation metadata, so the unbounded
interpreter does not execute separate charge opcodes. Paired quick scalar rows
measure 196,646 ns unbounded and 209,813 ns budgeted for the same eight-call
sample, a 6.7% bounded-budget premium with matching checksums.

Named follow-up: M20 interpreter close-out must recover scalar and call
throughput through verified-MIR instruction selection, superinstructions, or
other backend-local transformations that consume sealed facts. M22 may consume
the same retained MIR for machine code. Neither follow-up may reconstruct facts
from bytecode layout, restore source/HIR queries in the backend, or weaken the
execution-unit schedule.

### Package-Qualified Script Identity

Script module identity is `PackageId + ModulePath`, and every stable script
definition path includes its package. Linked and runtime function/type/method
identity uses resolved `FunctionId`, `TypeId`, and related semantic IDs. Source
names are display and debug metadata only: duplicate names from different
packages remain distinct, and ambiguous names are omitted from name-based entry
indexes instead of replacing or aliasing semantic entries. Convenience source,
file, directory, and scratch APIs use explicit reserved package IDs; linkers and
semantic consumers never synthesize or fall back to an implicit package.

### Sealed Package Compilation Requests

Engine package loading produces one immutable `PackageCompilationSnapshot`
that owns the package graph, deterministic source identities, and package-aware
HIR generation. A `PackageCompileRequest` is bound to that snapshot ID and
contains only canonical root `PackageId` values. Engine expands roots to their
transitive closure, enters the existing compiler and linker once, and seals a
generation-independent root fingerprint plus declared and statically observed
package capabilities into `LinkedArtifact`. Ordinary artifacts carry an empty
`InstalledProviderSet`; provider discovery is not an ordinary compilation
prerequisite. Reload rebuilds the same roots against a new snapshot and rejects
an incidental root-set change.

### Snapshot-Bound Provider Discovery

Provider discovery is a read-only projection over one sealed package
compilation snapshot. HIR retains attribute arguments structurally and accepts
providers only as `#[provider(id = "...")]` resolved trait impls whose target is
a public zero-field script record. Stable identity is `PackageId + TraitId +
ProviderId`; public catalog descriptors expose stable type/method IDs and file
locations, while generation-local HIR IDs remain internal. Engine derives
declared and statically observed capability metadata through analysis without
compiling or executing provider code. `ProviderSelection` retains the catalog's
snapshot ID, and cross-generation selection reuse is rejected before linking.

### Linked Provider Runtime

Provider compilation extends the ordinary snapshot-bound package request and
uses the same compiler/linker path. The linker alone converts selected stable
provider metadata into `TypeHandle` and `MethodDispatchHandle` entries owned by
the resulting `LinkedArtifact`; discovered but unselected providers never enter
runtime metadata. Engine Runtime resolves stable `ProviderKey`/`MethodId` pairs
against the current image, constructs a fresh zero-field receiver, and invokes
the linked script method through normal budget, GC-root, HostAccess, capability,
profiling, and safe-point machinery. A public `ProviderHandle` contains only its
owning runtime identity and stable key, so compatible reloads re-resolve it
against the newly active artifact rather than exposing generation-local handles.

### Package And Provider Reload ABI

Package reload reconstructs the prior canonical root and selected-provider
fingerprint against a newly loaded package snapshot. Hot reload derives package
and provider compatibility only from the previous and next linked artifacts:
roots and selected keys must remain stable, selected provider target and method
identities must match, and package capability requirements cannot expand without
explicit restaging. Unselected provider additions do not enter runtime ABI.
Accepted reports expose changed and impacted package IDs; provider ABI rejection
diagnostics retain the Vela provider span and canonical package manifest path.

### Package-Aware Tooling Is A Metadata Projection

ProjectState loads manifests and assembles sources through `vela_package`, with
open-document overlays taking precedence over disk snapshots and each refresh
committing exactly one database generation. Completion and navigation consume
the retained package graph and HIR provider metadata; they never rediscover
packages through an editor-specific parser or execute script, native,
reflection, or HostAccess code. Provider rename results carry hot-reload risk
metadata because the provider ID participates in stable `ProviderKey` identity.
The root manifest is durable project state: a member or dependency manifest
change triggers reconstruction from that root rather than promoting the changed
manifest to a new project. A failed reconstruction publishes diagnostics while
retaining the last valid graph and does not commit a database generation.

## Active Async Architecture Decisions

### Post-Review Closure Decision

The 2026-07-14 review reopened final async acceptance through Batch E of
[async-execution-model-plan.md](async-execution-model-plan.md). E1-E3 implement
the following durable contracts, and the final workspace, benchmark,
documentation, and zero-hit gates are recorded in
[the Batch E acceptance report](archive/async-execution-batch-e-acceptance-2026-07-14.md):

- Active async execution uses a VM-owned dynamic root-admission boundary. A
  reentry-returned heap value must join the current `HeapExecution` roots before
  its child frame roots are released; engine-owned Runtime retention alone is
  not an active-session GC contract.
- Lease request kind and acquired state must agree. Eligible `Sync`
  mutable-origin bindings may enter true `shared(n)` state; unsupported
  type-erased capabilities fail closed instead of being extracted through an
  exclusive lease labeled shared. Safe-Rust proof precedes any bound or
  capability correction, and no parallel CallArgs/Runtime mode is added.
- Script-visible callable reflection metadata is named `is_async`. The keyword
  field `async` was removed rather than retained as a compatibility alias.
- `linked_execution.rs` remains opcode dispatch/root glue. Execution-session,
  continuation, async-resume, and reentry policy live in focused VM modules;
  the file-size exception does not cover those responsibilities.
- Provider metadata/method/asyncness/shape/parameter resolution has one pure
  owner over the pinned `LinkedArtifact`. Outer and reentry paths may adapt
  receiver allocation and root admission only after that shared resolution.

The VM ownership split is active: `execution_session.rs` owns session, frame,
continuation, and start policy; `async_resume.rs` owns prepared async calls and
resume conversion; `execution_reentry.rs` owns push/abort policy. The remaining
over-threshold `linked_execution.rs` is reviewed only as the exhaustive opcode
driver/root glue. Provider outer and reentry entry construction both consume
the same pure resolution result before allocating their receiver roots.

The E1 representation is now fixed. `HeapExecution` lazily owns a dynamic root
registry; a reentry return receives a weak guard token before child protection
is truncated, and admission marks immediately when incremental collection is
already sweeping. The existing Runtime root registry remains the cross-call
owner and stores active tokens in a sparse root-ID sidecar, so ordinary
`VelaValue` and root-entry layouts are unchanged. Last-handle release removes
the token, and session teardown drops the VM registry without a custom
destructor or extended heap borrow.

Mutable-origin direct bindings now require `Send + Sync` and store their erased
borrow behind an owned read/write-guard slot. Read guards implement true
`shared(n)` state and write guards implement `exclusive`; both are scoped
`Send`, release by RAII, and support atomic rollback. This is a direct
pre-release capability correction: non-`Sync` mutable origins no longer enter
`with_host_mut`, and no exclusive lease is labeled shared.

Batch E performance keeps ordinary entry/provider and suspended-memory costs
within the accepted comparison, and creates no eager dynamic-root allocation.
The safe owned-guard lease representation raises the measured exclusive lease
row by 23.4%; `ASYNC-LEASE-PERF-1` is the M20 follow-up for profiling and
reducing that boundary cost without weakening exact lease state or RAII.

### Executor-Neutral Async Execution

The executor-neutral async contract is defined by
[async-execution-model-plan.md](async-execution-model-plan.md). Batch A makes
callable asyncness and explicit await/resume control flow authoritative from
source through linked execution. Known async calls require await, await is
illegal in sync functions, dynamic non-awaited async dispatch traps before
invocation, and sync-only callback APIs reject async callbacks.

All existing synchronous execution hard-switches to one `ExecutionSession`,
explicit frame stack, return-continuation model, and pending-operation state.
Script functions, closures, methods, providers, comparisons, guards, collection
callbacks, and iterators resume by frame push/pop. Production code does not
recursively invoke linked execution; the remaining `execute_linked_call` name
is only a non-recursive root driver shim.

The Runtime execution surface is exactly `call` and `call_async`.
Function names/handles, receiver-bound methods, and provider methods implement
one sealed call-target contract and resolve to one internal entry request;
method, provider, key/handle, adapter, raw, and event-safe-point combinations do
not create additional sync/async execution methods. Fallback adapters belong to
the execution-owned host input, while reload safe-point checks remain explicit
lifecycle operations after an outer call.

Batch B activates real Rust future suspension on the same session driver. Its
public Runtime future is scoped and `Send`, may borrow Runtime and host state
for the invocation lifetime, and need not be `'static`. Registered async
pure/context/host/HostPath-method factories are `Send + Sync + 'static`, but
their returned futures are scoped to the invocation and must be `Send` only for
that lifetime. Core crates do not own an executor, Engine/Runtime/CallArgs gain
no mode generic, and no parallel `!Send` registry/runtime is introduced.

Awaited sync targets complete in the current drive; async Rust targets return a
prepared owned call and suspend until the embedding executor polls again.
Dynamic HostPath methods and `reflect::call` resolve asyncness before dispatch,
so their non-awaited async forms fail without invoking the target while sync
reflection remains synchronous. Async macro support is active for free,
context, and host functions.

Batch C direct async methods acquire Rust-only scoped shared/exclusive leases
for `&self`/`&mut self` and typed `&T`/`&mut T` host parameters. Acquisition is
atomic in stable parameter order; direct bindings are restored by RAII across
success, error, cancellation, and panic unwind. Opaque adapters and Runtime
extern-state bindings fail closed unless they provide an explicit safe typed
lease contract.
Lease types never enter Vela values, reflection, or GC state.

NativeCallContext reentry uses the same call-target abstraction and pushes a
child marker/frame on the active session. It inherits generation, heap,
VM/extern state, host access, sidecars, budgets, capabilities, and cancellation state.
Nested binding scopes share one monotonic HostRef allocator; child refs expire
with the scope. An exclusive receiver may be reborrowed explicitly into child
CallArgs, while access through its raw parent HostRef remains busy. A caught
child error unwinds only that reentry segment and leaves the parked parent
native boundary resumable.

Batch D preserves explicit reload activation while permitting staging during a
suspended outer call. `HotReloadStagingHandle` shares only a synchronized
pending-update slot with the Runtime; it never owns or changes the current
`ProgramVersion`. Every active session continues on its pinned
`Arc<LinkedArtifact>`, and completion or cancellation must release the Runtime
borrow before `check_reload` can activate the staged generation. Callable
asyncness is reload ABI for script, native, reflected/event, trait-method, and
provider descriptors; sync/async changes require restart or explicit migration.

Reflection metadata records expose callable asyncness explicitly as `is_async`,
and language tooling reads callable asyncness from HIR or registry signature
facts rather than re-inferring it from token text. There is no keyword-field
`async` compatibility alias. Syntax recovery may still recognize `.await` as a
receiver boundary so completion can use the semantic awaited-result fact.

Direct CLI execution remains synchronous by default. `vela_cli --async`
explicitly opts into a small CLI-owned executor and calls `Runtime::call_async`;
core crates still own no executor. The synchronous C ABI does not grow a
poll/waker surface: an async entry returns the distinct
`VelaStatus::AsyncEntry` status and a descriptive error string.

Restricted JIT input keeps using the single verified-MIR/linked-artifact
contract. Declared async functions and any MIR function with an `AwaitCall`
terminator carry the explicit `MirJitIneligibility::Async` reason; no compiled
async path or second backend representation is introduced.

## Unified Rust/Vela Interop Decisions

### Binding Generation And Error Surface

The Engine/compiler emits one deterministic, language-neutral export schema;
the official Rust generator consumes that schema and may be invoked through a
CLI or build helper without reparsing Vela source. Generated artifacts record
the schema checksum and source origins. The Rust surface is a runtime-bound
package/module object with ordinary typed methods and an equivalent
`NativeCallContext`-borrowed carrier for same-session re-entry; neither uses an
ambient Runtime or requires a user-authored service trait.

`VmResult<T>` denotes call failure, while a boundary-safe `Result<T, E>` is an
ordinary Vela Result value and generates the corresponding Rust type. The
same return/error classification applies to replaceable entries; replacement
must not introduce a separate `VmResult<T>`-only authoring subset or attempt to
reconstruct a Rust `&T`/`&mut T` through `FromScriptArg`. The
initial trusted-native profile remains callable visibility, normalized effects
and derived coarse capabilities, exact type/lease safety, and budgets. A
special restricted profile is deferred until a concrete deployment requires
one and must use an explicit low-level `HostAccess` opt-in rather than changing
ordinary Rust signatures or callable ABI.

### Ordinary Signatures Are The Canonical Authoring Surface

The accepted direction is defined by
[rust-vela-interop-model-plan.md](rust-vela-interop-model-plan.md). Explicitly
exported Rust functions and methods use ordinary copied/owned values and
invocation-scoped `&T`/`&mut T`; Vela calls them with normal function or method
syntax. Rust calls exported Vela items through compiler-schema-backed generated
bindings. `HostRef`, `PathProxy`, lease guards, `CallArgs`, and erased runtime
values remain internal boundary mechanisms or explicit low-level escape hatches,
not normal business-function parameters.

### Rust Bindings Consume One Compiler-Owned Schema

Generated Rust-to-Vela bindings consume the deterministic schema attached to
`CompiledProgram` and `LinkedArtifact`; they never rescan or reparse Vela
source. The schema contains only public package/module callables and public
script record/enum definitions, structural signature and default shape,
sync/async form, transitive verified-MIR effect upper bounds, derived
capabilities, receiver-qualified method identities, callable/type
fingerprints, and source origins. Source positions and docs remain diagnostic
metadata, while Runtime grants, allowlists, reflection policy, budgets, and
other deployment policy do not participate in fingerprints. Trait method
identity is the pair of semantic receiver `TypeId` and protocol `MethodId`;
its executable `FunctionId` remains the direct call target.

`vela_bindgen` is the sole Rust code generator. It consumes the compiler-owned
schema directly and emits one runtime-bound package with deterministic module
accessors, sync/async typed methods, generated owned record/enum models, typed
script-method receivers, stable callable/type specifications, checksum, and
Vela source-origin documentation. CLI or build integration may wrap this
crate, but may not implement another source scanner, parser, or generator.

Generated host-reference parameters are classified from the verified MIR host
contract and emitted as ordinary `&T` or `&mut T`. A root binding installs a
normal call-scoped direct host argument. An active binding may install the
argument only when the concrete Rust reference address and host type match
provenance captured from a live parent lease; it then reuses the parent's
canonical `HostRef` in a child direct scope. This comparison never dereferences
a fabricated pointer and never constructs a Rust reference. The child scope
owns the Rust reborrow, shadows the parent identity only for the nested call,
and drops before parent use resumes. Missing provenance and shared-to-exclusive
upgrades fail before Vela execution.

### Borrowed Host Returns Freeze Their Parent Owner

A supported Rust `&T`/`&mut T` host return is exposed to Vela as a
call-tree-scoped HostRef backed by the retained, pinned parent owner/service
lease and provenance. It does not require a business ID, resolver, or
generation-based relookup. Shared-origin children permit later shared calls on
the owner but reject exclusive calls; an exclusive-origin child rejects every
later owner call. Conflicts fail immediately rather than block. These children
may propagate through local Vela values and nested Rust/Vela calls in the same
root, including scoped await suspension, but cannot escape through state,
globals, the root result, native caches, or unscoped tasks. Each distinct child
has one `BorrowLeaseId` shared by all its aliases. Conservative MIR last-use and
non-escaping lexical-scope analysis release proven-dead children automatically;
dynamic cases use the reserved `host::release(value)` intrinsic, never a bare
global `release`. Closing a child invalidates all of its aliases, while distinct
sibling children keep the parent frozen until each closes. Root cleanup remains
the unconditional fallback and releases parent freezes deterministically
without depending on GC timing. A durable cross-root HostRef remains a
separate, explicit model.

`host::release` is compiler-reserved and lowers through MIR to the dedicated
`ReleaseBorrowLease` instruction rather than an ordinary native call. Automatic
release is a sealed post-verification MIR analysis over direct scoped-return
facts, liveness, compiler-only temp aliases, and exact CFG edges. It never
classifies an ordinary HostRef, and observable local aliases, container/state/
closure escapes, root returns, or explicit release suppress the automatic path.

### Trusted Native Mutation Uses A Coarse Call Boundary

Direct Vela field and path mutation remains fine-grained through `HostAccess`.
For an exported trusted Rust callable, `HostAccess` instead gates callable
visibility/registration, effects, derived coarse capabilities, exact host
identity, and shared/exclusive leases before generated code creates an
invocation-scoped Rust reference. Once `&mut T` enters trusted Rust,
field-level Rust mutation is allowed. A future stronger sandbox may restrict
callable sets or opt selected functions into low-level HostAccess, but it must
not force proxies into the default authoring surface or create a second
execution model.

### Callable ABI Excludes Deployment Policy

An exported callable publishes one normalized effective `EffectSet`, and the
existing canonical mapping derives its domain-neutral `CapabilitySet`
requirement. The effective effect upper bound is callable ABI; the active
`ExecutionProfile`, granted capabilities, callable/host-type allowlists,
filesystem policy, and reflection member permissions are deployment policy and
do not enter interop callable or generated-binding fingerprints. Reflection
`required_permissions` must not be reused as native business authorization,
and ordinary native dispatch must not perform arbitrary permission-string
lookups. If Runtime policy later becomes mutable, one coarse Runtime policy
generation may invalidate prepared authorization caches without adding
per-field or per-object dimensions to ordinary call targets.

### Signature-Inferred Effects And Explicit Export Bundles

For Rust exports, the effective effect set is the parameter-classifier-derived
base union explicit additional effects. Shared host borrows infer `host_read`,
exclusive host borrows infer `host_write`, and value-only signatures infer
`pure`; `NativeCallContext` alone adds nothing. `effects(...)` may widen but
never remove this base. Capability-scoped context operations and generated
nested bindings reject any operation whose effects exceed the current Rust
callable's ceiling before it begins, even when the Runtime grants the wider
capability.

`NativeCallContext` carries the normalized coarse capability projection of the
currently executing Rust callable as an inherited effect ceiling. Nested lease
contexts preserve it, `require_capability` checks it in addition to Runtime
grants, and generated active bindings project the target Vela effect bits
through the same canonical mapping before pushing a child frame. `host_write`
continues to imply host-read authority while retaining the canonical
fingerprint projection. This is a fixed-bitset preflight, not a per-call
permission graph. Generated async host arguments use a prepared scoped
`CallArgs` future whose Runtime/context borrow may be shorter than the retained
host-reference lifetime; no reference lifetime is erased or extended.

Scattered functions use item-level `#[vela::export]`. Related functions use an
explicit `#[vela::export_module(path = "...")]` whose supported immediate
public functions form one approved export set and one generated
`vela_exports()` registration bundle; private helpers remain Rust-only.
`#[vela::methods]` is the equivalent explicit inherent-or-trait-impl boundary.
Engine registers bundles explicitly through `register_exports`; no ambient
inventory, process-global discovery, or module-wide default effect is
introduced. An unsupported public item inside either explicit group fails at
declaration time instead of being silently omitted.

### Rust Trait Exposure Is An Explicit Vela Protocol Mapping

Implementing a Rust trait does not automatically expose it to Vela. A Vela
protocol owns a stable public identity independent of the Rust trait path.
Annotatable trait impls use `#[vela::methods]` and the ordinary method adapter
path without inherent forwarding methods. Existing external impls that cannot
be annotated use a declaration-only adapter that lists the selected
boundary-safe signatures and generates type-checked UFCS thunks without a
duplicate Rust impl. Marker traits, unselected methods, generic methods,
associated-type surfaces, and other unsupported Rust-only signatures remain
unexposed unless explicitly mapped to a boundary-safe Vela protocol method.
Direct script-visible method names remain unique across inherent and trait
surfaces; unresolved collisions are registration errors rather than Rust-style
UFCS guessing.

### Hot-Replaceable Dispatch Is An Optional Interop Extension

Ordinary Rust/Vela calls require neither a service trait nor a dispatch slot.
Runtime replacement is explicit at declaration time: a host business macro
maps one selected handler, function, or method to one `ReplaceableSlotId`,
moves the authored body into a private Rust fallback, and keeps the original
public name and call syntax as a generated interception entry. Unannotated
calls and generated private fallbacks remain direct. No service trait,
complete Vela service implementation, caller-side proxy, or MVP grouping
identity is required.

The no-override path uses a dense build-local slot index into the pinned
immutable `DispatchGeneration`; an empty entry immediately calls the private
Rust fallback. It performs no runtime string/hash lookup, global lock,
allocation, serialization, or hot-replacement-only trait dispatch. The host
macro derives explicit dispatch authority from a receiver, context, or
parameter and never consults an ambient Runtime.

Vela binds one function to one statically resolved target with
`#[override(host::path::target)]`. The target callable contract supplies its
signature, parameter modes, return family, effects, and sync/async shape. The
generated adapter presents the original receiver, actor/context, message, and
business parameters while preserving HostRef and lease safety internally. A
package may override any subset of slots; staging applies that delta to a base
generation and atomically publishes one full immutable table for future host
roots. Adjacent methods remain Rust. Vela errors propagate without executing
the Rust fallback, and an explicit Vela base-call facility is deferred.

An override target retains stable callable and artifact identity, but it must
not own a mutable `Mutex<Runtime>` as a second execution authority. A host root
uses an explicit runtime-bound dispatch authority to enter one execution;
nested override calls use the active re-entry authority and push onto the same
`ExecutionSession`. They inherit the pinned artifact, heap, state view,
HostAccess, remaining budgets, effect ceiling, capabilities, tracing,
cancellation, and lease provenance. The integration must not use an ambient
Runtime, a reentrant global lock, or target-local default budgets. Independent
host roots must not be serialized solely because they select functions from
the same override package.

Applying a later partial delta preserves the old slot selection and stable
executable identity without reinterpreting an old `FunctionId` in a new
artifact. Every candidate and generation also carries an unforgeable
controller/layout identity; staging, activation, rollback, and target lookup
reject cross-controller values even when layouts have equal length. Override
adapters pass parameters positionally because Rust's synthetic receiver name
and Vela-local parameter names are not ABI, while the imported slot contract
supplies exact shared/exclusive modes, return/error mode, borrowed-return
provenance/freeze/access, sync/async shape, types, and normalized effect
ceiling. A strict effect subset is valid; exceeding the ceiling is not.
`ProviderKey` remains a separate provider declaration identity and never
doubles as a replaceable slot or dispatch-generation key.

Host integrations may pin a dispatch root before any Vela call, such as at the
start of an actor mailbox turn, so Rust handlers and nested Rust/Vela service
calls share one generation. Conversion, borrowed-return provenance/freeze,
lease, Runtime call target, execution session, policy, and diagnostics continue
to use the common interop paths.

The 2026-07-17 post-implementation review found that the first mechanism slice
did not satisfy the preceding decision. Controller-owned generation identity,
Engine-registered compile-time target linking, authoritative contract import,
and exact staging fingerprints now close the path-string and lossy-validation
findings. Target-owned Runtime locks and the `VmResult<T>`-only replaceable
macro remain implementation gaps, not accepted compatibility contracts, and
may be replaced outright during the post-review closure.

### Context Natives Use A Session-Aware VM Boundary

Runtime-driven linked sessions pause synchronous context-native calls before
the callback runs. `vela_engine` invokes the callback with an active re-entry
authority, resumes the saved destination, and continues the same linked
session. This preserves the pinned artifact, heap, state, host boundary,
budget, and call stack across Vela-to-Rust-to-Vela nesting without a nested
Runtime. Direct Engine/VM execution that has no re-entry session invokes the
same registered callback normally with re-entry unavailable; context-native
registration and capability checks remain shared rather than duplicated.

## Validation Rules

- Multi-level `super` scan must return no matches:

```bash
rg -n '(super::){2,}|super\s*::\s*super' crates examples tests --glob '*.rs'
```

- Remaining `pub use` entries should be deliberate API surface:

```bash
rg -n '^\s*pub use\b' crates --glob '*.rs'
```

## Update Rules

- Add or update entries here when a change creates a durable architecture rule,
  compatibility policy, naming convention, module boundary, or semantic
  constraint.
- Do not record routine implementation steps, small refactors, or test-only
  details here.
- Keep active entries concise. Move detailed historical rationale to
  `docs/archive/` when this file stops being quick to scan.
