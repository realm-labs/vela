# Decisions

## 2026-05-24: Start With A Dedicated `vela_common` Crate

Common IDs, spans, diagnostics, and symbol interning live in `vela_common`
instead of the root package. This keeps later parser, bytecode, VM, host, and
reflection crates sharing one stable foundation without circular ownership.

Stable IDs are transparent newtypes over integer primitives so they remain
cheap to copy while preventing accidental mixing between fields, methods, host
objects, source files, and related schema items.

## 2026-05-24: Parse Declaration Items Before Full Function Bodies

The first `vela_syntax` parser recognizes module-level declarations and keeps
function bodies as balanced token ranges rather than full statement/expression
trees. This gives later milestones a tested item surface for functions, host
events, records, enums, traits, and attributes while keeping M1 incremental.

Statement and expression parsing will be added behind the same lexer and
diagnostic model, preserving source spans and recovery behavior.

## 2026-05-24: Split Syntax Into Focused Modules

Status: Accepted

Context:
The syntax crate grew past the point where lexer, token, AST, and parser
responsibilities were easy to review in one file. M1 also needs richer
function-body parsing before bytecode lowering can begin.

Decision:
Keep `lib.rs` as the crate facade and split implementation into `token`,
`lexer`, `ast`, and `parser` modules. Function bodies now parse into an AST
instead of balanced token ranges.

Consequences:
- Later bytecode and HIR work can consume a structured function body.
- Parser tests can assert concrete statement and expression shapes.
- Control-flow headers parse expressions without treating the following `{` as
  a record literal, so `if`, `for`, and `match` bodies remain unambiguous.

## 2026-05-24: Store Script Functions In A Named Bytecode Program

Status: Accepted

Context:
M2 needs script functions to call other script functions before hot reload and
ABI indirection exist. The VM also needs a simple entrypoint API that can pass
arguments into parameter registers.

Decision:
Introduce a `Program` that maps function names to `CodeObject` values. A
`CodeObject` stores parameter names, and the VM initializes the first registers
from entrypoint or call arguments. Calls to known script functions compile to
`CallFunction`; other path calls remain `CallNative`.

Consequences:
- The current VM can execute multi-function source programs.
- Function-level hot reload can later replace entries behind this named program
  boundary with stable function identifiers and ABI checks.
- Native calls stay explicit and separate from script calls.

## 2026-05-24: Start Host Patching With A Host-Scoped Value Type

Status: Accepted

Context:
M3 needs `PatchTx` and overlay semantics before the VM/host bridge is wired.
The existing VM `Value` currently lives in `vela_vm`, and making `vela_host`
depend on the VM would create the wrong crate direction for later bytecode
operations.

Decision:
Use a small `HostValue` enum inside `vela_host` for the first PatchTx slice.
It covers the primitive values needed for `Set` and `Add` overlay tests while
keeping host patching independent from VM execution internals.

Consequences:
- The host crate can evolve without a VM dependency cycle.
- A later bridge can convert between VM values and host patch values at the VM
  host-boundary instruction layer.
- PatchTx semantics can be tested before full script-to-host execution exists.

## 2026-05-24: VM Host Mutation Requires An Explicit Host Context

Status: Accepted

Context:
M3 needs bytecode-level host field reads and writes while preserving the rule
that scripts never receive real Rust `&mut` references. The normal VM execution
path should continue to run pure script bytecode without requiring host state.

Decision:
Add explicit host field bytecode operations and execute them only through a
`HostExecution` context containing a `ScriptStateAdapter` and `PatchTx`.
`GetHostField`, `SetHostField`, and `AddHostField` build `HostPath` values from
script-visible `HostRef` values and route all reads/writes through the
transaction overlay and adapter.

Consequences:
- Host mutation remains opt-in at the VM boundary.
- Script bytecode can read overlay writes in the same transaction.
- Adapter state is mutated only when the host applies the collected patches at
  a safe point.

## 2026-05-24: Reflection Metadata Is Read-Only Runtime Data

Status: Accepted

Context:
M4 needs controlled reflection without becoming a monkey-patching system.
Reflection must be able to query host type metadata and perform approved reads
and writes, while type structure changes remain outside runtime script control.

Decision:
Introduce `vela_reflect` with a `TypeRegistry` of immutable descriptors.
Reflective field reads and writes resolve descriptor metadata to stable
`FieldId` values, then use `PatchTx` and `ScriptStateAdapter` for host access.
Record-like values can be read reflectively, but host schema structure is not
mutated by reflection APIs.

Consequences:
- `reflect.set` can create host patches without exposing Rust `&mut`.
- Read-only fields and unknown fields are reported at the reflection boundary.
- Future hot reload ABI checks can reuse the same stable descriptor surface.

## 2026-05-24: Reflection Natives Use Host-Aware VM Calls

Status: Accepted

Context:
Script-visible reflection needs access to `TypeRegistry`, `PatchTx`, and host
adapter reads. The existing native function path only accepted script values,
which is sufficient for pure functions but not for controlled reflective host
reads, writes, and calls.

Decision:
Add a separate host-native registration path to `vela_vm`. Host natives receive
the current `HostExecution` and may be registered under normal native names
such as `reflect.get` and `reflect.set`. The reflection natives convert VM
values into reflection values, resolve metadata through `TypeRegistry`, and
route host mutation through `PatchTx`.

Consequences:
- Pure native functions remain available without host access.
- Reflection is script-visible without exposing real Rust `&mut` references.
- Reflective writes and calls continue to be deferred until host safe-point
  patch application.

## 2026-05-24: MVP Records Use Named Fields Before Shape Slots

Status: Accepted

Context:
M5 needs a runnable record constructor and field-read loop before the runtime has
full shape interning, slot specialization, or GC-managed object layouts.

Decision:
Represent first-phase script records in the VM as a type name plus a deterministic
map of named fields. Compile record literals to `MakeRecord` and record field
reads to `GetRecordField` when no host field binding is configured. Shape and
slot optimization remain a later implementation detail.

Consequences:
- Script records become first-class values now, without blocking on object
  layout optimization.
- Host field reads still use `FieldId` specialization when the compiler is given
  a host-field binding.
- Later shape-slot work can replace the internal representation without changing
  the source-level record constructor behavior.

## 2026-05-24: MVP Enum Constructors Reuse Variant Record Syntax

Status: Accepted

Context:
The parser already represents `Enum.Variant { field: value }` as a record
literal with a multi-part path. M5 needs enum constructors and match-tag
execution before a full semantic type resolver exists.

Decision:
Treat record literals with a multi-part path as enum variant constructors in the
bytecode compiler. The final path segment becomes the variant name, and the
preceding path becomes the enum name. Match lowering supports enum path and
record-variant patterns by comparing tags and binding simple named fields.

Consequences:
- The runnable enum constructor and match loop works without a full resolver.
- Single-name record literals remain script records.
- A later resolver can replace this syntactic heuristic with explicit type
  metadata while preserving source behavior.

## 2026-05-24: Hot Reload Versions Own CodeObject Indirection

Status: Accepted

Context:
M6 needs function-level code replacement where old executions can continue using
old code while new calls enter updated code. The VM currently runs immutable
`Program` values, so the first hot-reload layer needs to preserve old code
without forcing the VM to own global mutable state.

Decision:
Introduce `vela_hot_reload::ProgramVersion`, which maps stable
`FunctionSymbolId` names to `Arc<CodeObject>` values. `HotReloadRuntime` swaps
the current `Arc<ProgramVersion>` at update safe points, while callers that
already hold an old version can still run that old code. ABI validation rejects
updates that delete existing function parameters.

Consequences:
- Function-level updates are explicit and versioned.
- Old version lifetime is represented by normal `Arc` ownership.
- The VM can continue executing immutable `Program` snapshots while hot-reload
  policy evolves around it.

## 2026-05-24: Budgeted VM Entrypoints Preserve Existing Convenience Runs

Status: Accepted

Context:
M7 needs bounded execution without forcing all existing prototype tests and demo
callers to construct runtime policy objects immediately.

Decision:
Add `ExecutionBudget` to `vela_vm` and expose explicit budgeted run entrypoints
next to the existing unbudgeted convenience entrypoints. The budget tracks
instruction count and current call depth internally, while configured limits
remain public runtime policy. Direct VM host patch operations reserve patch
capacity before recording a patch; opaque host-native calls are checked after
they return.

Consequences:
- Existing M0-M6 callers keep working while embedders can opt into limits.
- Recursive script calls share one budget through the existing recursive VM
  execution path.
- Reflection and other host natives need a later `NativeCallContext` to reserve
  patch budget before mutation instead of only being checked after return.

## 2026-05-24: Script Heap Uses Generation-Checked Non-Moving Handles

Status: Accepted

Context:
M7 requires script heap values to be reclaimed without moving references and
without placing Rust host state under the script GC. The current VM still stores
first-class values inline, so the heap model needs to land before value storage
is migrated.

Decision:
Add a `vela_vm::heap` module with `GcRef { index, generation }` handles into a
non-moving arena. Collection is full mark-sweep from explicit roots. Heap values
trace only script references; `HostRef` may appear as an external slot value but
does not trace or own Rust host state. Allocation charges the VM memory budget
before mutating the heap, and sweeping can release charged bytes.

Consequences:
- Stale handles cannot access new objects after an arena slot is reused.
- Cyclic script objects can be reclaimed without a moving collector.
- VM register and value migration can build on the heap API without changing
  the host mutation boundary.
- The first memory accounting is shallow; precise recursive object sizing can
  be refined after VM values are fully heap-backed.

## 2026-05-24: VM Values Trace HeapRefs Before Full Value Migration

Status: Accepted

Context:
M7 needs call frames to provide GC roots, but the current runnable prototype
still stores strings, arrays, maps, records, and enums inline in `Value`.
Switching every executable value to heap-backed storage will touch compiler,
VM, reflection, native calls, and existing tests, so the root contract should
land first.

Decision:
Add a temporary `Value::HeapRef(GcRef)` bridge and a `Value::trace_heap_refs`
helper that recursively visits current inline aggregate values. `CallFrame`
can derive explicit root lists from active registers. Normal script execution
does not produce `HeapRef` yet; later value migration will replace inline
owned values with heap-backed values while reusing the same tracing path.

Consequences:
- GC root discovery can be tested against VM call-frame registers now.
- Existing source behavior and public script values remain unchanged for the
  runnable prototype.
- The next GC step is to produce heap refs from normal bytecode execution and
  call collection only at safe points with active frame roots.

## 2026-05-24: Step GC Uses Resumable Sweep Slots First

Status: Accepted

Context:
M7 needs `step_gc` pacing at event/tick safe points. The current heap has a
full mark-sweep collector, but no runtime event loop or heap-backed bytecode
values yet. Tests still need deterministic GC pacing behavior.

Decision:
Add `GcConfig`, `GcBudget`, and `GcStepStats` to `vela_vm::heap`. A step starts
by marking from explicit roots, then resumes sweeping from the previous slot on
subsequent calls. `GcBudget::sweep_slots` provides deterministic pacing for
tests; `GcBudget::micros` carries the public pause-budget shape from the
architecture and currently maps to an unbounded sweep slot budget until runtime
wall-clock pacing exists. Completed collections update the next collection
threshold from the configured heap growth factor.

Consequences:
- Event/tick integrations can call `step_gc` without moving heap objects.
- Deterministic tests can prove pause/resume behavior without relying on wall
  clock timing.
- Full collection explicitly aborts and restarts any in-progress step so marks
  do not leak across collection modes.

## 2026-05-24: Heap-Backed Execution Is Explicit During Migration

Status: Accepted

Context:
M7 needs VM-owned strings, arrays, maps, records, and enums to move onto the
script heap, but existing M0-M6 tests and demo callers still assert inline
`Value` return shapes. Replacing all value behavior at once would couple heap
allocation, native calls, reflection, host conversion, and demo behavior in one
large change.

Decision:
Add explicit heap-backed VM entrypoints that take a `HeapExecution` context.
When heap execution is selected, string constants and aggregate bytecode
constructors allocate `HeapValue` objects in `ScriptHeap`, return
`Value::HeapRef`, and charge memory through `ExecutionBudget`. Record/enum
field reads and enum tag checks resolve both inline values and heap refs.
Existing non-heap entrypoints keep their current inline behavior during the
migration.

Consequences:
- Heap-backed bytecode execution can be tested without breaking the runnable
  prototype demo.
- Memory-budget enforcement now covers VM-created script heap objects.
- Native calls, reflection, and host conversion still need heap-aware value
  resolution before heap execution can become the default path.

## 2026-05-24: Heap Mode Materializes Values At Native Boundaries

Status: Accepted

Context:
Heap-backed bytecode now produces `Value::HeapRef` for script strings and
aggregates. Existing native functions and host patch conversion code expect
ordinary `Value` or `HostValue` inputs, and should not need to know about
temporary migration handles.

Decision:
When heap-backed execution calls a native or host-native function, materialize
heap refs into inline `Value` shapes for the call. If the native returns a
string or aggregate while heap execution is active, store that result back in
`ScriptHeap` and return a `HeapRef`. Host `HostValue` conversion resolves
heap-backed strings for host field writes and method-call patch arguments.

Consequences:
- Existing native functions can run under heap-backed bytecode without direct
  heap access.
- Host patches can record copied string values from heap-backed scripts without
  exposing Rust references.
- Reflection calls can reuse this materialized boundary for host refs, strings,
  and aggregate metadata while heap execution remains explicit.

## 2026-05-24: Heap Equality Materializes Values During Migration

Status: Accepted

Context:
Heap-backed execution stores strings and aggregate values as `Value::HeapRef`,
but existing language equality expects to compare script-visible values rather
than temporary heap handles. Reflection exposes common comparisons such as
`reflect.type_of(player) == "Player"`, where the native returns a heap-backed
string under heap execution.

Decision:
Resolve both operands through the same materialization helper used by native
boundaries before executing `Equal` and `NotEqual`. This keeps equality
semantic over script values during the heap migration, while leaving ordering
operators to their existing primitive paths until those operations gain broader
heap-backed coverage.

Consequences:
- Reflection type-name checks work under heap-backed execution.
- Equality remains independent of `GcRef` allocation identity for migrated
  strings and aggregates.
- Future default heap execution can preserve the same comparison behavior while
  replacing materialization with direct heap-aware equality where worthwhile.

## 2026-05-24: Heap Safe Points Protect Caller Frame Roots

Status: Accepted

Context:
Heap-backed execution now allocates VM values into `ScriptHeap`, and the heap
has stepped mark-sweep collection. The VM executes script calls recursively, so
a GC step inside a callee cannot discover heap refs stored in caller registers
unless those refs are explicitly exposed.

Decision:
Make `HeapExecution` the owner of a protected-root stack and safe-point GC
policy. Before a script function call, the VM pushes the caller frame's heap
roots into that stack, executes the callee, then truncates the stack even when
the call returns an error. After normal instructions, heap-backed execution
runs a stepped GC safe point when the heap threshold is reached or an
incremental collection is already in progress. Roots are the protected caller
roots plus the current frame's register roots.

Consequences:
- A callee allocation can trigger GC without collecting heap values still live
  in suspended caller frames.
- Safe-point GC releases swept memory back to `ExecutionBudget`, keeping memory
  accounting aligned with the heap.
- Heap-backed execution now has a constructor-controlled runtime context,
  leaving inline compatibility entrypoints unchanged during migration.

## 2026-05-24: Managed Heap Entrypoints Materialize Results

Status: Accepted

Context:
Explicit heap-backed entrypoints are useful for tests and embedders that want
to own `ScriptHeap`, but the default embedding path should not require callers
to manage temporary heap lifetime just to execute a script. Returning raw
`HeapRef` values from a VM-owned temporary heap would be invalid after the heap
is dropped.

Decision:
Add managed heap entrypoints that create a temporary `ScriptHeap`, run the
heap-backed VM path with `ExecutionBudget`, materialize the returned value into
ordinary `Value` shapes, then collect the temporary heap with no roots to
release live memory accounting. The cleanup runs for both successful returns
and VM errors. The CLI demo now uses this managed heap path with explicit
runtime budgets.

Consequences:
- Embedding-facing execution can use heap-backed bytecode without exposing
  invalid temporary `GcRef` handles to callers.
- `ExecutionBudget::memory_bytes_allocated` returns to zero after managed
  execution completes or fails.
- Explicit heap entrypoints remain available for long-lived hosts that own a
  script heap across ticks.

## 2026-05-24: HIR Starts With A Declaration Module Graph

Status: Accepted

Context:
M8 needs a semantic layer shared by the compiler, diagnostics, hot reload, and
future tooling. The existing bytecode compiler reads syntax AST directly, which
is workable for the prototype but does not provide stable declaration IDs,
cross-module resolution, or duplicate-name diagnostics.

Decision:
Introduce a dedicated `vela_hir` crate with stable HIR IDs, a `ModuleGraph`,
per-module declaration indexes, and first-phase lowering from parsed syntax
items. This slice indexes functions, structs, enums, and traits, preserves
source spans and visibility, resolves `use` imports across modules, and reports
duplicate or unresolved names with related spans and candidate hints.

Consequences:
- Later bytecode compiler work can consume HIR without depending on syntactic
  item scans for every semantic question.
- Hot reload and future reflection/schema work gain stable declaration handles
  before full type metadata lands.
- Expression lowering, binding maps, top-level side-effect checks, and type
  hint metadata remain explicit follow-up slices instead of being hidden inside
  the syntax parser.

## 2026-05-24: HIR Binding Maps Resolve Safe Value Names First

Status: Accepted

Context:
M8 needs binding maps and expression IDs before the compiler can stop scanning
syntax directly. The current parser still represents some dotted expressions
and namespace-style calls syntactically, so reporting every unresolved path
would create false positives for native namespaces such as `reflect.*` and
future module paths.

Decision:
Add function-level `BindingMap` values that allocate stable `HirExprId` and
`HirLocalId` handles, record parameter/`let`/`for`/lambda/pattern bindings,
and resolve expression paths to local bindings, module declarations, or import
leaf names. Unresolved-name diagnostics are emitted for value-position and
assignment-target paths only; callee and field-base paths are recorded when
they resolve but otherwise left for later semantic/type-aware passes.

Consequences:
- Local name diagnostics can produce candidate hints without misclassifying
  native namespaces as missing variables.
- Later HIR expression lowering has stable IDs and binding facts to attach
  type facts, effects, and compiler lowering decisions.
- Full module-qualified path semantics remain a dedicated resolver follow-up
  instead of being inferred from syntax alone.

## 2026-05-24: Bytecode Compilation Runs Through HIR Diagnostics

Status: Accepted

Context:
M8 requires the bytecode compiler to consume HIR instead of relying only on raw
syntax scans. Fully replacing the compiler's AST lowering in one step would
mix semantic validation, local register allocation, and bytecode emission in a
large change.

Decision:
Route source compilation through a single-module `ModuleGraph` before bytecode
generation. Syntax diagnostics still return as syntax errors. If HIR produces
semantic diagnostics, compilation stops before bytecode generation. Valid
sources continue through the existing bytecode lowering path for now.

Consequences:
- Duplicate declarations and unresolved value names are reported by HIR before
  code generation.
- Existing valid compiler and demo behavior remains stable while the compiler
  is migrated incrementally.
- The next compiler slice can replace local/script-function scans with HIR
  declaration and binding facts without changing public entrypoints again.

## 2026-05-24: Type Hints Are Parsed As Non-Generic Metadata

Status: Accepted

Context:
M8 needs type hints available to HIR, diagnostics, hot reload, and future
tooling, while the language remains dynamically typed and explicitly excludes
script-language generics.

Decision:
Represent syntax type hints as source-spanned path metadata on function
parameters, returns, `let` bindings, lambda parameters, and struct fields.
HIR copies those hints into function signatures, struct field metadata, and
local bindings. Generic type argument syntax such as `Array<int>` is rejected
by the parser before bytecode generation.

Consequences:
- Type hints are available for later schema, ABI, and analysis work without
  changing runtime value semantics.
- Public hint syntax stays small and does not create script generics or
  monomorphization pressure.
- The bytecode compiler can continue executing valid hinted programs while
  deeper HIR consumption is implemented incrementally.

## 2026-05-24: Const Initializers Are HIR-Checked For Top-Level Effects

Status: Accepted

Context:
M8 needs module-level declarations from the grammar to lower into semantic
metadata, and hot reload should not rely on arbitrary top-level script
execution for registration or mutation.

Decision:
Parse `const` as a module item with optional type-hint metadata and an
expression initializer. HIR indexes const declarations and validates their
initializers for top-level side effects. Calls, assignments, and loops in const
initializers produce `hir::top_level_side_effect` diagnostics before bytecode
generation.

Consequences:
- Pure configuration constants can appear at module scope without becoming
  executable top-level script code.
- Event registration and host mutation remain routed through attributes,
  reflection scanning, and function execution rather than module-load effects.
- More precise const value evaluation and binding analysis can be added later
  without weakening the no-arbitrary-top-level-effects rule.

## 2026-05-24: Impl Blocks Lower To Metadata Before Dispatch

Status: Accepted

Context:
M8 needs the grammar's declaration surface to lower into stable semantic
metadata, while M10 will handle actual trait/protocol dispatch and runtime type
registration.

Decision:
Parse `impl Trait for Type { fn ... }` blocks as module items and lower them to
HIR impl declarations. HIR stores the implemented trait path, target type path,
method signatures, method spans, and per-method binding maps keyed by stable
HIR nodes. Impl methods are not exported as top-level bytecode functions in
this slice.

Consequences:
- Later trait dispatch, schema hashing, and hot reload ABI checks have stable
  impl metadata to consume.
- Method bodies receive local binding diagnostics and metadata without changing
  the current function-call surface.
- Full runtime method dispatch remains an explicit M10/M11 follow-up rather
  than being inferred from syntax heuristics.

## 2026-05-24: Bytecode Source Compilation Uses HIR Function Metadata

Status: Accepted

Context:
M8 requires the bytecode compiler to consume HIR instead of treating the syntax
AST as the only semantic source. The compiler still lowers function bodies from
syntax, but declaration and signature facts now exist in HIR.

Decision:
Keep the HIR `ModuleGraph` produced during semantic validation and use it to
discover script functions and retrieve function signatures. `CodeObject`
parameter names now come from HIR signature metadata, and impl methods remain
metadata-only until method dispatch is implemented.

Consequences:
- Function discovery now follows HIR declaration kind instead of ad hoc syntax
  scans.
- Future ABI checks and function lowering can reuse the same signature source
  as diagnostics and tooling.
- Body bytecode generation can migrate to HIR binding maps incrementally
  without changing the public source compilation entrypoints again.

## 2026-05-24: Bytecode Local Lookup Prefers HIR Binding IDs

Status: Accepted

Context:
The initial compiler used a flat string-to-register map for locals, which was
enough for early examples but could not represent nested shadowing correctly.
M8 binding maps now provide stable local IDs and expression-to-binding
resolutions.

Decision:
Keep the existing AST body lowering, but allocate and look up local registers by
HIR local ID when binding facts are available. Expression path lowering consults
HIR span-based binding resolutions before using the legacy name map fallback.

Consequences:
- Nested shadowed locals compile against the semantic binding chosen by HIR.
- Existing bytecode lowering remains incremental and compatible with current
  examples.
- Later HIR expression lowering can replace the span bridge without changing
  the register allocation model again.

## 2026-05-24: Script Calls Require HIR Declaration Resolution

Status: Accepted

Context:
Name matching made a local that shadows a function compile as a `CallFunction`
because the compiler only checked whether the callee text matched any script
function name.

Decision:
Bytecode call lowering emits `CallFunction` only when the callee expression
resolves to a HIR declaration ID whose declaration kind is function. Other
calls keep using the existing native/dynamic fallback until richer callable
value semantics are implemented.

Consequences:
- Shadowing now follows semantic resolver facts instead of source spelling.
- Future closure and local callable support can be added without reserving
  top-level function names in local scopes.
- Module-qualified and imported function call lowering remain explicit M8
  follow-ups rather than being inferred from names alone.

## 2026-05-24: Record Shorthand Fields Use HIR Bindings

Status: Accepted

Context:
Record shorthand syntax such as `Reward { count }` reads a local by name, but
the compiler previously resolved it through the legacy string-to-register map.
That allowed nested block shadowing to affect shorthand fields even when HIR
resolved ordinary path reads correctly.

Decision:
Record fields now preserve the source span for the field name. HIR records a
binding resolution for shorthand fields as though the field name were a value
read, while explicit `field: expr` syntax continues to bind the expression.
Bytecode lowering uses that span-based HIR resolution when compiling shorthand
fields.

Consequences:
- Record shorthand now follows the same semantic local binding rules as path
  expressions.
- Unresolved shorthand fields are diagnosed during semantic validation.
- The syntax AST carries one more span needed by HIR and future tooling.

## 2026-05-24: Resolved Imports Become Declaration Bindings

Status: Accepted

Context:
HIR binding maps represented imported names as strings even after module import
resolution succeeded. That preserved enough information for diagnostics, but
downstream compiler, hot reload, and tooling work needs imported references to
carry the same stable declaration IDs as same-module references.

Decision:
Function and impl-method binding maps may keep unresolved imports as import
placeholders during initial lowering, but resolved imports are converted to
`BindingResolution::Declaration` values. `ModuleGraph::resolve_imports()` also
refreshes existing binding maps so forward imports gain declaration facts after
the target module is added.

Consequences:
- Imported value reads now expose stable declaration IDs to later compiler and
  ABI stages.
- Forward imports remain possible without producing duplicate unresolved-name
  diagnostics from body binding.
- Unresolved imports are still reported by module import diagnostics rather
  than by ad hoc function-body name lookup.

## 2026-05-24: Match Pattern Registers Track HIR Locals

Status: Accepted

Context:
HIR already models match pattern bindings as locals, but bytecode lowering only
inserted record-pattern field bindings into the legacy string register map.
Nested arm-body shadowing could therefore make a later read of the pattern
binding fall back to the wrong same-name register.

Decision:
When lowering a match arm, record pattern field registers in both the legacy
name map and the HIR local map using the pattern binding facts from the arm
body span. Snapshot and restore both maps around each arm.

Consequences:
- Match pattern bindings follow the same HIR local-resolution path as
  parameters, `let` bindings, and record shorthand fields.
- Nested shadowing inside match arm bodies no longer changes the selected
  pattern binding register.
- More pattern forms can reuse this HIR-local register path as M9 expands
  executable match support.

## 2026-05-24: Literal Const Reads Compile From HIR Declarations

Status: Accepted

Context:
HIR indexes const declarations and rejects side-effecting const initializers,
but bytecode local path lowering still treated a reference such as `BONUS` as a
missing local even when HIR resolved it to a top-level const declaration.

Decision:
Carry literal const initializer values from the semantic source into the
bytecode compiler. When a path expression resolves to a const declaration with
a literal initializer, emit a normal `LoadConst` instruction instead of falling
back to legacy local lookup.

Consequences:
- Simple configuration constants are executable through the same HIR
  declaration facts used by functions and imports.
- Top-level consts remain side-effect free; no module-load execution path is
  introduced.
- Non-literal const evaluation remains a separate compiler feature rather than
  being inferred during general expression lowering.

## 2026-05-24: Const Expression Evaluation Is Source-Order And Scalar

Status: Accepted

Context:
After literal const reads became executable, pure const expressions such as
`const BONUS = BASE + 5 * 2` still could not be read despite HIR accepting them
as side-effect free.

Decision:
Evaluate top-level const initializers during semantic source preparation only
for scalar literals, unary scalar operations, numeric/comparison binary
operations, and references to earlier const declarations in the same module.
Unsupported expressions remain metadata-only for now.

Consequences:
- Common configuration constants can be composed without creating module-load
  bytecode or host side effects.
- Evaluation is finite and deterministic because it only uses earlier
  source-order values, avoiding recursive const dependency walks.
- Aggregate consts, forward references, and richer expression forms remain
  explicit follow-ups for later language-surface work.

## 2026-05-24: Import Aliases Define Binding Names

Status: Accepted

Context:
The grammar allows `use path as Alias`, but syntax and HIR only preserved the
import path. Function-body binding therefore resolved imported names by the
source declaration name and could not represent aliases as semantic facts.

Decision:
Preserve import aliases in the syntax AST and HIR import metadata. Binding maps
use the alias as the imported binding name when present, while import
resolution still targets the original declaration path.

Consequences:
- `use game.reward.grant as give_reward` lets function bodies refer to
  `give_reward` and records the target declaration ID.
- Candidate suggestions and future tooling can surface the local imported name
  instead of only the source declaration name.
- Alias support stays declarative; it does not introduce re-export or wildcard
  import behavior.

## 2026-05-24: Multi-Module Bytecode Uses Declaration Symbols

Status: Accepted

Context:
HIR could resolve imports across modules, but bytecode compilation still
accepted only a single source file. Call lowering also emitted the callee's
source spelling, which meant aliased imports could not target the actual script
function symbol.

Decision:
Add a multi-module bytecode compilation entrypoint over HIR `ModuleSource`
inputs. Carry a declaration-to-function-symbol map into body lowering and emit
`CallFunction` with the resolved declaration's function symbol instead of the
callee spelling.

Consequences:
- Imported script functions, including aliased imports, can compile and execute
  across modules.
- The current VM `Program` remains string-keyed, so this slice keeps simple
  function names and leaves duplicate cross-module symbol policy to a later ABI
  and program-version step.
- Native/dynamic fallback remains available for calls that do not resolve to a
  known script function declaration.

## 2026-05-24: Multi-Module Function Symbols Are Qualified

Status: Accepted

Context:
The initial multi-module bytecode path kept function symbols as plain function
names. That allowed modules with the same function name to overwrite each other
in the string-keyed `Program` map and made imported call targets ambiguous.

Decision:
For multi-module compilation, use `module.path.function` as the bytecode
function symbol and emit imported script calls to that qualified symbol.
Single-source compilation keeps plain function names for compatibility with the
existing hot reload and demo paths.

Consequences:
- Multi-module programs can contain same-named functions from different
  modules without collision.
- VM entrypoints for multi-module programs use qualified names such as
  `game.main.main`.
- Future program-version and ABI work has stable module-qualified symbols to
  build on while the single-file prototype path remains unchanged.

## 2026-05-24: Imported Const Evaluation Uses Resolved Imports

Status: Accepted

Context:
Scalar const evaluation could compose earlier consts in the same module, but
multi-module compilation did not let const initializers use imported const
aliases. That left configuration split across modules dependent on runtime
fallback behavior instead of HIR declaration facts.

Decision:
Evaluate multi-module scalar consts against resolved HIR import declarations.
Imported const aliases are available once their target declarations have
scalar values, while same-module references still only see earlier
source-order const values.

Consequences:
- Modules can define consts from imported configuration values without
  generating module-load bytecode or host side effects.
- Source input order across modules does not determine whether an imported
  const value is available.
- Same-module forward references, recursive const cycles, aggregate consts,
  and non-scalar const expressions remain unsupported follow-ups.

## 2026-05-24: Known Constructor Types Use Declaration Symbols

Status: Accepted

Context:
Record and enum constructor lowering still used source spelling for type
names. That meant imported aliases such as `Prize { ... }` or
`Hit.Physical { ... }` produced alias-shaped runtime metadata instead of the
declared type identity.

Decision:
Record HIR declaration resolutions for constructor paths when the constructor
root names a known struct or enum declaration. Multi-module bytecode uses the
resolved declaration's module-qualified type symbol for record and enum
construction; single-source bytecode keeps plain type names for compatibility.

Consequences:
- Imported constructor aliases now execute with stable declared type metadata
  such as `game.reward.Reward`.
- Undeclared prototype record literals remain supported and keep their
  source-spelled type names until script type validation is tightened in M10.
- Variant validation and slot-based script type layouts remain later M10 work.

## 2026-05-24: Match Pattern Tags Use Resolved Type Symbols

Status: Accepted

Context:
Imported enum constructors now emit declaration-qualified type metadata, but
match pattern lowering still used the source-spelled enum root. An alias such
as `Hit.Physical` could therefore construct `game.damage.Damage` and then
compare the tag against `Hit`.

Decision:
Record HIR declaration resolutions for enum-like match pattern paths in the
function binding map. Until pattern HIR has dedicated node IDs and spans, these
resolutions are keyed by pattern path. Bytecode match tag checks use the
resolved declaration's type symbol when available.

Consequences:
- Imported enum aliases now construct and match against the same declared type
  metadata across modules.
- Pattern path resolution remains focused on known declarations; wildcard and
  binding patterns are unchanged.
- Richer pattern metadata, variant validation, and pattern-specific node IDs
  remain future HIR/M10 work.

## 2026-05-24: Qualified Paths Refresh Through The Module Graph

Status: Accepted

Context:
Imports and aliases resolved across modules, but direct module-qualified value
paths such as `game.reward.grant()` or `game.config.BONUS` still relied on
source spelling and bytecode fallbacks. This kept common module-style calls
from using declaration IDs and broke qualified const reads before codegen.

Decision:
HIR binding maps record unresolved module-qualified paths as refreshable
semantic placeholders. After `resolve_imports()` has a complete module graph,
the graph also resolves those placeholders to declaration IDs. Bytecode then
uses the same declaration facts as imports for script function calls and scalar
const reads.

Consequences:
- Direct module-qualified function and const references work across files,
  including forward module order.
- Native namespace calls that do not resolve to script declarations still fall
  back to native dispatch.
- Qualified constructor and variant validation remain intentionally limited
  until script type metadata and pattern HIR become richer in later milestones.

## 2026-05-24: Cross-Module Resolution Respects Visibility

Status: Accepted

Context:
HIR preserved declaration visibility, but import and qualified path resolution
treated private declarations as cross-module targets. That made `pub`
metadata observable in syntax but not in semantic access control.

Decision:
Only expose declarations across module boundaries when their visibility is
`pub`. Same-module binding still sees private declarations. Private imports
produce `hir::private_import` diagnostics before bytecode generation, and
qualified-path refresh maps are filtered to visible declarations.

Consequences:
- Module APIs now have an enforceable public/private boundary in HIR.
- Private imports fail during semantic validation instead of compiling to
  declaration-backed bytecode.
- Unresolved or non-script namespace calls can still fall back to native
  dispatch when they do not name a visible script declaration.

## 2026-05-24: Unary Not Uses Existing Truthiness

Status: Accepted

Context:
M9 starts expanding the executable language surface from parsed syntax into
bytecode and VM behavior. Unary `!` and unary `-` are already part of the
parsed AST, but bytecode generation previously rejected them.

Decision:
Compile `!expr` to a dedicated `Not` instruction that inverts the VM's
existing truthiness rules. Compile `-expr` to a dedicated `Negate` instruction
that accepts integers and floats, reports a VM type mismatch for non-numeric
values, and treats integer minimum overflow as a VM error.

Consequences:
- Conditional truthiness and explicit logical-not now share one semantic
  definition.
- Numeric negation is executable without widening the language's implicit
  conversion rules.
- Logical binary operators and richer expression forms remain separate M9
  slices.

## 2026-05-24: Logical Operators Short-Circuit To Booleans

Status: Accepted

Context:
M9 needs `&&` and `||` to execute with short-circuit behavior. The VM already
has truthiness for conditions and conditional jumps, but the language has not
defined operand-returning logical values.

Decision:
Compile `&&` and `||` into branch-based bytecode that skips the right-hand side
when the left-hand side decides the result. Logical expressions normalize their
result to `Bool` using the same truthiness rules as `if` and unary `!`.

Consequences:
- Side effects and unknown calls on a skipped RHS are not executed.
- Logical expressions have predictable boolean results independent of operand
  value categories.
- If operand-returning logic is ever desired, it will be an explicit language
  change instead of accidental VM behavior.

## 2026-05-24: Local Assignment Writes Stable Registers

Status: Accepted

Context:
M9 needs local assignment and compound assignment before loop and closure
execution can be completed. The current bytecode uses registers and HIR local
IDs rather than mutable stack slots, while host-field assignment already routes
through PatchTx-specific instructions.

Decision:
Compile assignment to a single-name local by computing the assigned value and
writing it back into the local's stable register with `Move`. Compound
assignment reads the stable local register, emits the matching numeric
operation, moves the result back into the stable local register, and evaluates
to the computed result. Host-field assignment remains on the existing host
patch bytecode path.

Consequences:
- Local reassignment works without introducing mutable script references or
  changing the VM register model.
- HIR-resolved shadowing remains authoritative for which local is written.
- Repeated bytecode, such as loop bodies, observes the latest local value on
  each iteration.
- Future closure/upvalue work must promote captured locals from stable
  function registers into explicit upvalue cells.

## 2026-05-24: Index Reads Start With Arrays And Maps

Status: Accepted

Context:
M9 requires index reads and writes. The VM already has array and map values in
both inline and heap-backed execution modes, while host-path indexing and
write/RMW behavior belongs to the later PathProxy/PatchTx expansion.

Decision:
Add `GetIndex` bytecode for array integer indexes and map string keys. Index
lookups work for inline `Value::Array`/`Value::Map` and heap-backed
`HeapValue::Array`/`HeapValue::Map`. Invalid base types, invalid key types,
missing keys, and out-of-bounds array indexes are VM errors.

Consequences:
- Script collection reads are executable without changing host mutation
  boundaries.
- Heap-backed index reads return heap slots as VM values, preserving existing
  managed-heap materialization at return/native boundaries.
- Index writes and nested host path indexing remain explicit follow-up slices.

## 2026-05-24: Index Writes Mutate Script Collections Only

Status: Accepted

Context:
M9 includes index writes, but host-path indexing and nested host mutation must
remain under the HostPath/PathProxy/PatchTx model planned for M11. The VM now
has array/map index reads for inline and heap-backed script values.

Decision:
Add `SetIndex` bytecode for script arrays and maps. Inline arrays/maps are
updated by writing the mutated collection value back to the base register.
Heap-backed arrays/maps mutate the script heap object after converting the
assigned value into a heap slot with the existing memory-budget path. Compound
index assignment lowers to `GetIndex`, the numeric operation, and `SetIndex`.

Consequences:
- Script collection mutation becomes executable without exposing host mutable
  references or bypassing PatchTx.
- Heap-backed writes preserve managed-heap result materialization and memory
  accounting for newly stored heap values.
- Host collection/index writes still require the later PathProxy and nested
  PatchTx work.

## 2026-05-24: For-In Starts With Snapshot Collection Iterators

Status: Accepted

Context:
M9 needs executable `for value in iterable` loops before break/continue and
host-provided iterables can be completed. Arrays and maps already exist in
inline and heap-backed execution modes.

Decision:
Compile `for` loops to `IterInit` plus `IterNext` bytecode. The VM creates a
snapshot iterator over script arrays or map values, preserving map iteration in
key order through `BTreeMap`. The loop binding writes into a stable local
register, and normal local assignment inside the loop mutates stable registers
at runtime.

Consequences:
- Array and map `for-in` loops execute in both inline and managed-heap modes.
- Mutating the iterated collection during iteration does not change the current
  iterator snapshot.
- Break/continue and host-provided iterables remain explicit follow-up slices.

## 2026-05-24: Break And Continue Use Loop-Scoped Jump Patching

Status: Accepted

Context:
M9 requires `break` and `continue` to work through nested control-flow blocks.
The bytecode VM already supports unconditional jumps, and `for-in` loops have a
stable iteration head at the `IterNext` instruction.

Decision:
The bytecode compiler keeps a stack of loop contexts while compiling loop
bodies. `break` emits a placeholder `Jump` recorded on the innermost loop and
patches to the loop end. `continue` emits a placeholder `Jump` recorded on the
innermost loop and patches to the loop iteration head. Using these statements
outside a loop remains an explicit unsupported-syntax diagnostic.

Consequences:
- Nested `if` and `match` blocks can produce loop exits without special loop
  lowering cases.
- Inner loop exits remain scoped to the nearest enclosing loop.
- Future loop forms can reuse the same compiler context stack with their own
  continue target.

## 2026-05-24: Root Host Method Calls Use Configured Bindings

Status: Accepted

Context:
M9 needs method-call syntax to become executable, and the host bridge already
has `CallHostMethod` bytecode plus PatchTx recording. M11 will add nested
PathProxy lowering for calls such as `player.inventory.add(...)`, but root host
reference method calls can use the current host-safe mutation boundary now.

Decision:
`CompilerOptions` can register host method names to stable `HostMethodId`
values. When the bytecode compiler sees a configured method call on a root
value, such as `player.grant_exp(20)`, it emits `CallHostMethod` with the
receiver register and compiled argument registers. Unconfigured method syntax
remains explicitly unsupported instead of falling through to unsafe dynamic
mutation.

Consequences:
- Source-level host method calls record PatchTx method-call patches and are
  applied only at the host safe point.
- Rust hosts control which source method names lower to host effects.
- Nested host-path method calls and script/stdlib method dispatch remain
  separate follow-up slices.

## 2026-05-24: Block And If Values Reuse Branch Register Merging

Status: Accepted

Context:
M9 requires block, `if`, and `match` expression values. The VM already has
register moves, constants, and jumps, so the first executable slice can be
implemented in bytecode lowering without adding new VM instructions.

Decision:
The compiler treats a block expression's final expression statement as the
block value and uses `null` for empty or statement-only blocks. `if` expressions
allocate one destination register and compile each non-returning branch to move
its value into that destination before jumping to the common end. Statement
`if` syntax can still omit `else`, but expression-valued `if` requires `else`
so every non-returning path produces a value.

Consequences:
- Source such as `let x = { let base = 2; base + 3; };` and
  `let y = if cond { x; } else { 0; };` now compiles and runs.
- The VM register model remains unchanged.
- The syntax parser still does not preserve expression-statement terminators,
  so this first slice treats the final expression statement as a value even if
  the source used a semicolon. A later syntax/HIR refinement can preserve
  terminator intent without changing the VM lowering model.

## 2026-05-24: Match Values Reuse Existing Pattern Lowering

Status: Accepted

Context:
M9 requires `match` expression values, while the runnable prototype already
supports statement-style matching for enum tag and record-variant patterns plus
wildcards. Match guards, literal patterns, binding patterns, and tuple variants
are still separate M9 work items.

Decision:
The compiler now lowers expression-valued `match` by compiling each supported
arm with the existing pattern checks and moving the selected arm value into one
destination register. Block arm bodies reuse block-value lowering, and
expression arm bodies compile directly then move into the destination. If no
arm matches and there is no wildcard, the expression value falls back to
`null` for now.

Consequences:
- Existing executable enum-pattern matches can now participate in larger
  expressions such as `let reward = match value { ... };`.
- Pattern field bindings keep the same HIR-local restoration behavior as
  statement matches.
- Match guards and richer pattern forms remain explicitly unsupported until
  their dedicated lowering slices.

## 2026-05-24: Literal Match Patterns Use Dynamic Equality

Status: Accepted

Context:
M9 requires literal patterns. The VM already has dynamic `Equal` bytecode,
including heap-aware equality for strings and aggregate values where supported.
Literal patterns do not need new runtime value categories or host access.

Decision:
The compiler lowers a literal pattern by compiling the pattern literal into a
constant register, emitting `Equal` against the match scrutinee, and branching
to the next arm when the result is false. This is shared by statement and
expression-valued `match` lowering.

Consequences:
- Integer, float, string, bool, and null literal patterns use the same equality
  semantics as ordinary expressions.
- Heap-backed string literal patterns work through existing heap-aware equality.
- Binding patterns, tuple variants, and guards remain separate M9 slices.

## 2026-05-24: Binding Match Patterns Copy The Scrutinee

Status: Accepted

Context:
M9 requires binding patterns. The HIR binding map already declares pattern
locals scoped to match arm bodies, but binding a pattern name directly to the
scrutinee register would make assignment to that name mutate the original
scrutinee local when the scrutinee is a local variable.

Decision:
Binding patterns are catch-all patterns. The compiler emits a `Move` from the
scrutinee into a fresh pattern-local register, then binds that register through
the HIR local map for the arm body.

Consequences:
- `match value { bound => bound + 1 }` executes from source.
- Assigning to `bound` inside the arm updates only the pattern local.
- Tuple variant destructuring remains a separate M9 slice.

## 2026-05-24: Match Guards Run After Pattern Binding

Status: Accepted

Context:
M9 requires match guards. Guards should be able to refer to names introduced by
the arm pattern, including whole-scrutinee binding patterns and record-variant
field bindings.

Decision:
The compiler lowers an arm by checking the pattern first, binding pattern locals
to registers, then compiling the guard expression. A false pattern check or
false guard both jump to the next arm. An arm with no pattern fallthrough and no
guard remains a catch-all arm.

Consequences:
- Guards can read destructured pattern locals.
- Guarded binding or wildcard arms no longer stop later arms from being
  considered when the guard is false.
- Guard expressions reuse ordinary truthiness and budgeted bytecode execution.

## 2026-05-24: Tuple Enum Variants Use Positional Field Slots

Status: Accepted

Context:
M9 requires tuple variants. The current runtime enum representation stores
variant payloads as named fields, and record-style enum variants already use
field names. Tuple constructors and patterns need a stable MVP mapping without
introducing script generics or new enum storage yet.

Decision:
The compiler lowers declared enum constructor calls such as
`Damage.Physical(7, 2)` into `MakeEnum` with positional field names `"0"`,
`"1"`, and so on. Tuple variant patterns check the enum tag first, then read
positional fields for subpattern checks and pattern-local bindings.

Consequences:
- Tuple variant source now executes through the existing enum value path.
- HIR resolves constructor-call callee paths through enum constructor rules, so
  imported enum aliases work consistently with record-style constructors.
- M10 can replace the string positional field names with stable `VariantId` and
  field-slot metadata without changing source syntax.

## 2026-05-24: Default Parameters Use Callee Prologues

Status: Accepted

Context:
M9 requires function parameter defaults and named call arguments. Defaults can
be ordinary expressions and should run in the callee environment so they can
refer to parameters already initialized for that call.

Decision:
The parser and HIR preserve default parameter expressions and named argument
syntax. The compiler reorders named script-call arguments against the resolved
function signature and emits an omitted-argument marker for parameters that
have defaults. Each compiled function emits a prologue that replaces omitted
parameter registers by evaluating that parameter's default expression.

Consequences:
- Defaults execute with the same budgeted bytecode path as ordinary script
  expressions.
- Entrypoints and script-to-script calls can omit defaulted parameters.
- Named arguments are supported for resolved script functions; host/native
  named argument support remains a later signature-aware bridge/stdlib task.

## 2026-05-24: Closures Capture Snapshot Values

Status: Accepted

Context:
M9 requires lambda expressions and closures while preserving the host boundary
rules: scripts cannot hold real Rust references, host mutation must enter
`PatchTx`, and the first interpreter is not a moving-GC or JIT runtime.

Decision:
The compiler emits `MakeClosure` with a nested `CodeObject` and explicit capture
registers discovered from HIR local binding facts. The VM stores closures as
inline values containing an `Arc<CodeObject>` plus captured `Value` snapshots.
Closure calls initialize capture registers first, then lambda parameters, and
execute through the same budgeted VM call path as script functions.

Consequences:
- Captured script values remain available after the outer frame returns.
- Closures do not expose Rust references or bypass `PatchTx` for host mutation.
- Captures are snapshot values in this MVP; shared mutable upvalue cells, if
  needed, are a later runtime feature rather than implicit reference capture.
- Closure objects can later move to heap-backed storage without changing source
  lambda syntax.

## 2026-05-24: Try Propagation Uses Dynamic Option And Result Enums

Status: Accepted

Context:
The grammar includes postfix `?`, and the product roadmap calls for
Option/Result-style propagation without script generics. Vela already represents
script enums dynamically, including tuple variants with positional payload
fields.

Decision:
Lower `expr?` to a dedicated `TryPropagate` bytecode instruction. The VM accepts
dynamic enum values whose final type segment is `Option` or `Result`.
`Option.Some(value)` and `Result.Ok(value)` unwrap tuple payload field `"0"`;
`Option.None {}` and `Result.Err(value)` return the original enum value from the
current function immediately.

Consequences:
- Propagation works before a full generic stdlib type system exists.
- Failure values keep their original enum identity and payload.
- Non-Option/Result values, unknown variants, and malformed success variants are
  VM type errors rather than silent fallthrough.
- Later `vela_std` work can register canonical Option/Result schemas while
  preserving this dynamic enum execution behavior.

## 2026-05-24: Range Expressions Produce Lazy Integer Iterables

Status: Accepted

Context:
The grammar includes `range = additive, [ (".." | "..="), additive ]`, and M9
requires executable grammar coverage. Eagerly expanding `1..large_number` into
an array would create a large allocation from a tiny source expression and work
poorly with budgeted execution.

Decision:
Lex and parse `..` and `..=` as range operators. The compiler lowers them to
`MakeRange`, and the VM stores an inline `RangeValue` with integer start/end
bounds plus an inclusive flag. `for-in` iteration uses a range cursor that yields
integers lazily instead of allocating an array.

Consequences:
- Range-based loops execute in both inline and managed-heap VM modes without
  charging script heap memory.
- Descending ranges are empty in the MVP; step values and reverse ranges remain
  later stdlib/language conveniences.
- Non-integer range bounds are VM type errors.
- Range values stay outside host conversion and reflection until the standard
  library defines a public range API.

## 2026-05-24: Script Value Methods Use Explicit VM Dispatch

Status: Accepted

Context:
M9 requires method calls on script values and stdlib-style values, while host
mutation must remain routed through `CallHostMethod`/`PatchTx`. Existing
lowering treated unconfigured receiver calls as record-field reads or native
namespace calls, so ordinary collection syntax such as `values.len()` did not
execute.

Decision:
Add `CallMethod` bytecode for script value methods. Configured host methods keep
priority and still lower to `CallHostMethod`; otherwise receiver method calls on
locals or field expressions lower to `CallMethod`. The VM dispatches first-phase
script methods through a focused `script_methods` module. This slice implements
side-effect-free `len()` and `is_empty()` for strings, arrays, maps,
records/enums, ranges, and heap-backed collection values.

Consequences:
- Common collection and string method syntax now executes without native
  namespace glue.
- Script value methods cannot mutate host state or bypass `PatchTx`.
- Method coverage can grow in `script_methods` while M10/M13 add stable metadata
  and richer stdlib APIs.
- Unknown script methods report a VM method error instead of being miscompiled as
  record field calls.

## 2026-05-24: Map Methods Stay Read-Only Until Receiver Mutation Lands

Status: Accepted

Context:
M9 needs script-value method dispatch coverage, and M13 calls for richer map
APIs. Mutating collection methods need explicit receiver write-back/value
category semantics so local mutation, heap-backed mutation, and future host-path
mutation do not blur together.

Decision:
Implement the first map method slice as side-effect-free `has`, `get`, and
`get_or` dispatch in the VM `script_methods` module. Missing `get` returns
`null`; `get_or` returns the supplied fallback. Heap-backed maps are read
through stable heap slots, and method returns pass through the VM heap storage
boundary before being written to registers.

Consequences:
- Map lookup syntax is available without introducing host mutation bypasses.
- `get_or` can return dynamic fallback values in managed-heap execution.
- Mutating methods such as `set`, `remove`, and collection transforms remain a
  later stdlib/runtime slice that can define receiver mutation semantics
  explicitly.

## 2026-05-24: Record Variant Patterns Match Field Subpatterns

Status: Accepted

Context:
The grammar allows record-variant match fields to either bind by shorthand or
specify an explicit nested pattern with `field: pattern`. The first executable
match implementation only checked the variant tag and treated record fields as
simple bindings, which meant field literals and nested variant patterns were
accepted by syntax but not semantically matched.

Decision:
Record-variant match lowering now emits enum-field reads for explicit field
subpatterns and recursively applies the existing pattern compiler to those
field values. Shorthand fields continue to bind the field name, and explicit
binding or wildcard subpatterns do not add extra equality checks.

Consequences:
- `Reward.Grant { kind: "xp", amount }` now rejects non-matching `kind`
  values instead of matching by tag alone.
- Nested patterns such as `Reward.Grant { payload: Payload.Xp(amount) }`
  execute through the same bytecode path as top-level tuple variant patterns.
- Missing or invalid field accesses still surface as VM enum-field errors,
  matching the current dynamic enum behavior.

## 2026-05-24: Math Standard Library Registers As VM Natives

Status: Accepted

Context:
M13 requires deterministic math helpers, and existing bytecode lowering already
routes qualified calls such as `math.max(...)` through native dispatch when no
script function or receiver method applies. The VM needed a structured way to
install these helpers without moving stdlib logic into the VM facade.

Decision:
Add a focused `stdlib` VM module with `register_standard_natives()`. The first
slice registers `math.max`, `math.min`, `math.clamp`, `math.floor`,
`math.ceil`, and `math.abs` as pure native functions. Integer-only operations
preserve integer results; mixed numeric operations use floats; invalid numeric
domains report VM type errors.

Consequences:
- Hosts can opt into the deterministic math stdlib through one VM API call.
- Qualified math calls execute through the existing native-call bytecode path.
- Additional stdlib namespaces can grow in the `stdlib` module without adding
  implementation logic to `lib.rs`.

## 2026-05-24: Array Higher-Order Methods Reuse VM Closure Calls

Status: Accepted

Context:
M13 requires collection methods that work with lambdas. These callbacks must
not bypass execution budgets, call-depth accounting, host context, or managed
heap root protection.

Decision:
Implement `array.map`, `array.filter`, `array.find`, `array.any`, `array.all`,
and `array.count` through the existing VM closure-call path. Method dispatch
passes the VM, current program, host execution context, heap execution context,
budget, and caller frame roots into a focused `array_methods` module. Callback
results are returned through the normal method result storage path so managed
heap execution materializes arrays safely.

Consequences:
- Collection callbacks share the same budget and call-depth behavior as normal
  closure calls.
- Heap-backed arrays can be transformed or filtered without collecting caller
  roots or accumulated callback results during nested calls.
- The VM facade only exposes a small closure-call helper; array method behavior
  stays isolated from `lib.rs`.

## 2026-05-24: Map Higher-Order Methods Dispatch By Receiver Category

Status: Accepted

Context:
M13 requires map methods with lambdas, including `map_values` and `filter`.
Some method names such as `filter`, `any`, `all`, and `count` are shared with
arrays, so dispatch must select behavior from the receiver category without
falling back to unsafe host mutation or namespace-native glue.

Decision:
Add a focused `map_methods` VM module. `map.map_values(|v| ...)` transforms map
values while preserving keys, `map.filter(|k, v| ...)` keeps entries by
predicate, and `map.any/all/count(|v| ...)` evaluate predicates over values.
`script_methods` routes shared higher-order names to map or array
implementations by inspecting inline or heap-backed receiver shape.

Consequences:
- Maps now support lambda-based transformations in both inline and managed-heap
  execution.
- Shared method names stay explicit and type-directed instead of relying on
  native namespaces or host state.
- Map callback logic remains separate from the general method-dispatch module.

## 2026-05-24: Array Sum Preserves Integer Totals Until Float Input

Status: Accepted

Context:
M13 collection coverage includes `array.sum`. The method needs to work in both
direct and lambda-transformed forms while reusing the existing callback safety
path for budgets, host context, and managed heap roots.

Decision:
Implement `array.sum()` and `array.sum(|value| ...)` in the focused
`array_methods` VM module. Direct and transformed values must be numeric. Empty
arrays return integer `0`; integer-only sums return an integer with checked
addition; the result becomes a float once any float participates.

Consequences:
- Scripts get deterministic numeric aggregate behavior without adding script
  generics or host-specific collection hooks.
- Callback-transformed sums share the same execution-budget and heap-root
  behavior as other array higher-order methods.
- Numeric aggregation semantics remain isolated from general VM dispatch.

## 2026-05-24: Array Grouping Uses String Map Keys

Status: Accepted

Context:
M13 requires `array.group_by(|x| ...)`, while the current dynamic map runtime
uses deterministic string keys. Grouping must preserve dynamic array elements
and managed-heap roots during callback execution.

Decision:
Implement `array.group_by` in the focused `array_methods` VM module. The
callback result must be a string key; each key maps to an array of original
values in input order. The returned groups use the existing map representation
and the same callback execution path as other array higher-order methods.

Consequences:
- Grouped results are deterministic because map keys remain ordered strings.
- Scripts can group dynamic values without script generics or host-specific
  adapters.
- Non-string grouping keys fail with a VM type error instead of silently
  stringifying values.

## 2026-05-24: Array Sort By Is Stable And Non-Mutating

Status: Accepted

Context:
M13 requires `array.sort_by(|x| ...)` for gameplay collection workflows.
Callback execution must remain budgeted and heap-safe, and the method should
not mutate receiver arrays unexpectedly while collection APIs are still dynamic
value operations rather than shape-specialized methods.

Decision:
Implement `array.sort_by` in the focused `array_methods` VM module as a
decorate/sort/undecorate operation. The callback runs once per element and must
return a numeric or string key. Numeric keys can mix ints and floats; string
keys sort lexicographically; mixed numeric/string domains fail with a VM type
error. Equal keys preserve original input order, and the receiver array is not
mutated.

Consequences:
- Sorting is deterministic and stable for gameplay scripts.
- Callback execution reuses the same budget, host context, and heap-root path
  as other array higher-order methods.
- More advanced comparator-style sorting remains a later extension instead of
  running arbitrary callbacks inside a sort comparator.

## 2026-05-24: Sets Start From Array Conversion And Scalar Elements

Status: Accepted

Context:
M13 requires set APIs, and the managed heap already has `HeapValue::Set`, but
scripts previously had no way to create or manipulate set values. Set literal
syntax and shape-specialized element facts are still later work.

Decision:
Expose sets through `set.from_array(array)` and a focused `set_methods` VM
module. Runtime sets preserve insertion order for iteration and `values()`,
deduplicate scalar elements, and support `has`, `add`, `remove`, `len`,
`is_empty`, and `for-in`. The first element domain is `null`, bool, int,
finite float, and string; nested collections, closures, host refs, and
non-finite floats fail with VM type errors.

Consequences:
- Scripts can use set workflows before dedicated set literal syntax exists.
- Managed-heap set values use the existing non-moving `HeapValue::Set` storage
  and budgeted heap conversion path.
- Element semantics stay deterministic and conservative until TypeFacts and
  script type metadata can describe richer set values.

## 2026-05-24: Option And Result Constructors Live In Stdlib Natives

Status: Accepted

Context:
The VM already implements `?` over dynamic `Option` and `Result` enum values,
but scripts had to declare those enum shapes manually to create canonical
values. M13 requires Option/Result-style conveniences without script generics.

Decision:
Register `option.some`, `option.none`, `result.ok`, and `result.err` as
standard natives. These constructors create ordinary dynamic enum values named
`Option` and `Result`, with tuple payloads stored in field `"0"` where needed.
The existing `TryPropagate` bytecode remains the only propagation mechanism.

Consequences:
- Scripts can use Option/Result propagation without local enum boilerplate.
- Constructor values work in both inline and managed-heap execution because
  they use the normal enum value and heap conversion paths.
- The design stays non-generic and leaves future TypeRegistry stdlib schemas as
  metadata rather than a separate runtime representation.

## 2026-05-24: Context Time And Emit Use The Host Bridge

Status: Accepted

Context:
M13 calls for context/time helpers and event emit workflows. The architecture
requires host mutation and side effects to stay behind HostRef, HostPath, and
PatchTx rather than direct VM access to server state or wall-clock time.

Decision:
Model `ctx.now` and `ctx.tick` as configured host field reads, and model
`ctx.emit(...)` as a configured host method call that records a
`CallHostMethod` patch. The CLI demo runner provides `ctx` and `player` host
refs by matching `main` parameter names, while the VM/compiler continue to use
the existing host field and host method bytecode path.

Consequences:
- Context time remains supplied by the embedding host instead of direct system
  time inside scripts.
- Event emission is deferred until PatchTx safe-point apply, matching other
  host effects.
- The demo runner can prove context and player workflows without adding a
  separate event bus abstraction before the Engine API stabilizes.

## 2026-05-24: Demo Workflows Use Named HostRef Parameters

Status: Accepted

Context:
The final demo suite needs level-up, context/event, and monster-kill workflows
before the full Engine API and derive macros exist. Scripts still must not
receive Rust references or mutate host state outside `PatchTx`.

Decision:
Keep the CLI demo runner as a thin embedding harness that binds known `main`
parameter names (`ctx`, `player`, `monster`) to `HostRef` values. Demo fields
and methods are registered through `CompilerOptions`, and side effects are
observed only after applying the collected `PatchTx`.

Consequences:
- Demo scripts can grow toward the final game-server acceptance suite without
  adding custom native glue for each script.
- The harness remains temporary and can be replaced by the Engine API later
  without changing script-side host mutation semantics.
- Unknown demo parameters fail explicitly instead of silently receiving dynamic
  placeholder values.

## 2026-05-24: Quest Demo Uses Scalar Host Fields Before Nested Paths

Status: Accepted

Context:
The final demo needs quest progress, but M11 nested `HostPath` lowering and
PathProxy values are still future work. The current runnable host bridge safely
supports root host refs, configured fields, and method-call patches.

Decision:
Represent the first quest-progress demo with scalar player fields
`quest_count`, `quest_goal`, and `quest_done`. The script updates those fields
through ordinary host field bytecode and emits completion through `ctx.emit`.
Nested quest objects remain a later M11 host-path expansion.

Consequences:
- The quest workflow is runnable and covered by the same CLI binary tests as
  the other game-server demos.
- Host mutation still flows only through `PatchTx`; scripts do not receive
  direct Rust state.
- The script surface can later move from scalar fields to nested host paths
  without changing the safe-point patching contract.

## 2026-05-24: CLI Reflection Demo Registers Static Metadata

Status: Accepted

Context:
The final demo suite needs a reflection workflow before the full Engine API and
derive macros are available. Reflection must remain controlled: scripts may
inspect metadata and perform permitted reads, writes, and calls, but they must
not mutate type structure.

Decision:
Register a static demo `TypeRegistry` in a focused CLI module for Player,
Context, and Monster host refs. The `reflect_debug` script uses VM reflection
natives to query type names, fields, and traits, then performs a controlled
`reflect.set` and `reflect.call` that record patches in the active `PatchTx`.

Consequences:
- Reflection demos run through the same parser, compiler, managed heap VM, and
  safe-point patch application path as other game-server scripts.
- The demo does not introduce runtime schema mutation or monkey patching.
- The static registry can later be replaced by Engine/derive-macro registration
  without changing script reflection semantics.

## 2026-05-24: Hot Reload Demo Uses ProgramVersion Handles

Status: Accepted

Context:
The final game-server demo suite needs a hot-reload workflow that proves old
frames or handles continue on old code while new calls enter the updated code.
The current hot reload crate already models function-level replacement through
`ProgramVersion` handles.

Decision:
Add a CLI `--hot-reload <initial> <updated>` demo command that compiles an
initial script through `compile_initial`, keeps the old `ProgramVersion`
handle, applies `compile_update`, and runs `main` from both the old and new
versions. The demo scripts return different kill-exp values so the output shows
old-before, old-after, and new-after behavior.

Consequences:
- The runnable demo exercises the real `vela_hot_reload` runtime rather than a
  CLI-specific replacement mechanism.
- The demo remains function-level and does not claim schema/effect ABI checks
  that belong to later milestones.
- Future Engine/Runtime APIs can reuse the same version-handle semantics at
  event or tick safe points.

## 2026-05-24: Script Type Metadata Uses Qualified Stable IDs

Status: Accepted

Context:
M10 needs script-defined structs and enums to appear in `TypeRegistry` before
runtime record slots, enum field layouts, schema hashes, and trait dispatch can
replace the earlier named-map prototype representation. The metadata must be
stable across source reordering and must not introduce script generics,
monkey-patching, or direct host state ownership.

Decision:
HIR lowers enum variants into enum-shape metadata, matching the existing
struct-shape path. Reflection registers script structs and enums from the
module graph through a focused `script_types` module, using module-qualified
type names and deterministic IDs derived from type/member names for
`TypeId`, `FieldId`, and `VariantId`. Script type descriptors also carry a
`TypeKind` and an order-independent `SchemaHash` derived from the qualified
type name, script type kind, member IDs, member names, and available field
type-hint metadata.

Consequences:
- Script type, field, and variant metadata can be queried through the existing
  registry surface without re-parsing syntax.
- Field and variant IDs, and the script schema hash, survive declaration
  member reordering.
- This does not yet implement slot-based object layout, trait method dispatch,
  or runtime type-structure mutation.

## 2026-05-25: VM Script Objects Store Ordered Field Slots

Status: Accepted

Context:
M10 requires script records and enums to move away from named-map object
storage toward stable shape and slot metadata. The existing bytecode still
names fields in instructions, and full compiler slot-index lowering belongs to
a later M10 slice, but the runtime storage can stop depending on map layout now.

Decision:
Add `ShapeId` and a focused VM `ScriptFields<T>` container that stores fields
as ordered slots with a deterministic shape ID derived from the object owner
and sorted field names. Inline `Value::Record`/`Value::Enum` and heap
`HeapValue::Record`/`HeapValue::Enum` now use `ScriptFields` instead of
`BTreeMap` payloads. Field reads and writes still resolve by field name at the
bytecode boundary, then operate on slots internally.

Consequences:
- Runtime script object payloads are slot-oriented and have stable shape IDs
  across source field reordering.
- Existing script behavior, reflection materialization, GC tracing, and demo
  outputs remain compatible.
- Bytecode field instructions still need a later slot-index specialization pass
  before field access is fully metadata-driven.

## 2026-05-25: Slot Bytecode Keeps Field Validation

Status: Accepted

Context:
M10 field access needs to move toward slot-index execution, but Vela is still
dynamically typed and not every expression has stable type facts yet. A stale
or overly optimistic slot index must not silently read or write the wrong
field if a value with a different shape reaches the instruction.

Decision:
Add `GetRecordSlot`, `SetRecordSlot`, and `GetEnumSlot` bytecode forms that
carry both the expected field name and the slot index. The VM reads or writes
by slot only when the slot exists and its field name matches; otherwise it
reports the same unknown-field error family used by name-based access. The
compiler emits slot bytecode for immediate record/enum literal field reads,
where the shape is known locally, and leaves broader dynamic field access on
the existing name-based instructions until type-flow metadata is available.

Consequences:
- Slot-index execution can land incrementally without losing dynamic safety.
- Immediate literal field reads now exercise the slot path end to end.
- Later HIR type facts can reuse the same bytecode forms for locals,
  parameters, pattern bindings, and declared script types.

## 2026-05-25: Script Trait Registration Is Metadata First

Status: Accepted

Context:
M10 needs trait declarations, default methods, impl blocks, and dynamic
implements checks. The runtime does not yet execute impl methods through
method dispatch, but reflection and reload checks need stable trait metadata
before dispatch can be wired safely.

Decision:
Preserve trait method signatures and default-body presence in HIR. Register
script trait declarations in `TypeRegistry` with stable `TraitId` and
`MethodId` values, and attach script `impl Trait for Type` metadata to the
target script `TypeDesc`. Trait paths are module-qualified in the same style
as script struct and enum type names.

Consequences:
- Reflection and future ABI checks can observe script trait methods without
  re-parsing syntax.
- Script type descriptors can report implemented script traits before method
  dispatch is executable.
- Actual trait method dispatch and default method execution remain later M10
  work.

## 2026-05-25: Script Impl Methods Compile As Hidden Functions

Status: Accepted

Context:
M10 needs executable script impl methods while preserving function-level hot
reload semantics and avoiding a separate method-body interpreter. The compiler
already emits ordinary `CodeObject` values for functions, and the VM already
has budgeted nested script calls.

Decision:
Compile each script `impl Trait for Type` method into a hidden `CodeObject`
whose first parameter remains `self`. Store a `Program` script method dispatch
table keyed by runtime receiver type name and method name, with the table
pointing at the hidden function symbol. Runtime `CallMethod` keeps built-in
value methods first, then falls back to the script method table for record and
enum receivers.

Consequences:
- Impl method execution reuses existing call depth, budget, heap root, and hot
  reload code-object behavior.
- Top-level script functions are not polluted by source-level impl method
  names such as `bonus`.
- Trait default methods, host type impl dispatch, and MethodId-based dispatch
  caching remain later M10 work.

## 2026-05-25: Trait Defaults Reuse Script Method Dispatch

Status: Accepted

Context:
Trait default methods need to execute without adding a second method-body
runtime path. The previous trait metadata kept only a `has_default` flag, so
the compiler could not emit executable code for omitted impl methods.

Decision:
Preserve trait default bodies in the syntax AST and bind them in HIR with
stable method body nodes. During bytecode compilation, when a script impl omits
a trait method with a default body, emit that default body as the dispatch
target for the impl receiver type. Explicit impl methods keep precedence over
defaults.

Consequences:
- Trait defaults share the same hidden `CodeObject`, VM budget, heap root, and
  dispatch-table behavior as explicit script impl methods.
- Default bodies can read `self` and other script values without exposing Rust
  references or changing host mutation boundaries.
- Dynamic implements checks and MethodId-based dispatch caching remain later
  M10 work.

## 2026-05-25: Script Reflection Preserves Runtime Type Names

Status: Accepted

Context:
`reflect.implements` originally worked only for host references. Script record
values were converted into anonymous reflection records, which made useful
field reads possible but discarded the runtime type name needed to query
`TypeRegistry` trait metadata.

Decision:
Add typed script record and script enum reflection values that preserve their
runtime type names while still exposing controlled field reads. Keep anonymous
`ReflectValue::Record` for map-like reflection data. `reflect.type_of`,
`reflect.fields`, and `reflect.implements` now consult registered script type
metadata for typed script records and enums.

Consequences:
- Runtime implements checks work for script-defined types without monkey
  patching or mutating type descriptors.
- Script-visible reflection can inspect script values and host references
  through the same `TypeRegistry` surface.
- Host type impl dispatch remains separate from script record/enum reflection
  and is still later M10 work.

## 2026-05-25: Script Method Tables Carry Stable MethodId Values

Status: Accepted

Context:
M10 dispatch needs to move from method-name-only lookup toward stable method
identifiers that can support inline caches and reload ABI checks. Script impl
methods already compile into hidden functions, but the dispatch table only
stored receiver type plus method name.

Decision:
Store a stable `MethodId` with each script method table entry. The ID is
derived from the implemented trait method, matching the stable trait-method ID
scheme used by script reflection metadata. Lookup by dynamic method name
remains supported, and lookup by `receiver type + MethodId` is available for
future typed call sites and caches.

Consequences:
- Multiple receiver types can implement the same trait method ID without
  colliding.
- Existing dynamic method calls keep working while compiler and VM dispatch can
  incrementally adopt MethodId-specialized call paths.
- Host type impl dispatch and call-site MethodId threading remain later M10
  work.

## 2026-05-25: CallMethodId Specializes Proven Script Receivers

Status: Accepted

Context:
After script method table entries gained stable `MethodId` metadata, call
sites still used string-only `CallMethod` bytecode. M10 needs MethodId-based
dispatch to land incrementally without requiring whole-program type inference.

Decision:
Add `CallMethodId` bytecode carrying both the source method name and stable
`MethodId`. The compiler emits it only when the receiver type is known locally,
starting with immediate script record and enum literals whose type symbol is
already resolved. The VM dispatches through `receiver type + MethodId`; normal
`CallMethod` remains the dynamic fallback for less certain receiver values.

Consequences:
- MethodId dispatch now executes end to end for a concrete script call-site
  category.
- Dynamic method calls keep their existing behavior while type-flow facts can
  opt into the specialized instruction later.
- The method name is still carried for diagnostics and unknown-method errors.

## 2026-05-25: Local Script Type Facts Stay In Compiler Submodules

Status: Accepted

Context:
M10 needs MethodId call-site lowering to move beyond immediate literals, but
the compiler is already large. Adding receiver type tracking directly into
`compiler.rs` would make later slot and dispatch work harder to review.

Decision:
Keep local script receiver type facts in `compiler/script_types.rs`. The main
compiler records facts at let and local assignment boundaries and asks the
module to recover a script type for simple local-path receivers. This supports
`let player = Player { ... }; player.bonus(...)` lowering to `CallMethodId`
without introducing whole-program type inference.

Consequences:
- MethodId dispatch now covers a common local receiver pattern while preserving
  dynamic fallback dispatch.
- The type-flow surface remains deliberately narrow and can grow toward
  parameter hints, pattern bindings, and slot lowering without piling the logic
  into one file.
- These facts are compile-time hints only; scripts still use dynamic runtime
  values and do not gain generics or Rust references.

## 2026-05-25: Type Hints Seed MethodId Receiver Facts

Status: Accepted

Context:
M10 MethodId dispatch should benefit from explicit script type metadata without
turning Vela into a statically typed language. HIR already preserves function,
lambda, and local `let` type hints as metadata, and scripts still execute with
dynamic values.

Decision:
Seed compiler-local script type-flow facts from parameter hints, lambda
parameter hints, and explicit `let` type hints. The compiler resolves a hint to
a known script type only when the type name is exact or an unambiguous suffix
of registered script type metadata. Calls such as `fn main(player: Player) {
player.bonus(5) }` can then lower to `CallMethodId`; dynamic fallback remains
for unknown or ambiguous facts.

Consequences:
- MethodId dispatch now covers typed function parameters and typed locals,
  including imported module-qualified script types.
- Type hints remain advisory compile-time metadata and do not introduce script
  generics, monomorphization, or Rust reference semantics.
- Broader type-flow for `self`, pattern bindings, captures, and slot lowering
  remains separate M10 work.

## 2026-05-25: Impl Method Bodies Seed Self Receiver Type

Status: Accepted

Context:
Hidden script impl and trait-default methods are compiled as ordinary code
objects, but the compiler already knows the concrete impl target type while
building those code objects. Without threading that fact into method bodies,
`self.other_method()` falls back to dynamic name dispatch even when the target
method has stable `MethodId` metadata.

Decision:
Compile hidden script method bodies through a dedicated constructor that seeds
`self` in the compiler-local script type facts with the impl target type. This
lets calls from one script method to another lower to `CallMethodId` when the
receiver method is known, while preserving normal dynamic fallback for other
receivers.

Consequences:
- Trait default methods and explicit impl methods can use MethodId dispatch
  when they call other trait methods on `self`.
- The fact is compiler-local and does not expose Rust references, add script
  generics, or change runtime value layout.
- Captured `self` in closures and pattern-bound receiver facts remain later
  M10 type-flow work.

## 2026-05-25: Captured Locals Preserve Script Receiver Facts

Status: Accepted

Context:
Lambda bodies reuse HIR local IDs for captured outer locals, and the VM already
passes captured values through closure registers. However, compiler-local
script type-flow facts for captured receiver values were not copied into the
nested lambda compiler, so captured script records fell back to dynamic method
name lookup.

Decision:
When compiling a lambda, copy any known script receiver type facts for captured
locals into the lambda compiler before lowering the nested body. This allows a
captured local such as `player` in `|_| player.bonus(5)` to lower to
`CallMethodId` when the outer compiler already proved `player` is a script
record or enum type.

Consequences:
- Captured script receivers can participate in MethodId dispatch without
  changing closure value layout or runtime capture semantics.
- The copied facts remain advisory compile-time metadata; scripts still execute
  dynamically and hold no Rust references.
- Pattern-derived receiver facts and broader slot/type-flow propagation remain
  later M10 work.

## 2026-05-25: Match Bindings Preserve Scrutinee Receiver Facts

Status: Accepted

Context:
Simple binding patterns create fresh local registers that represent the matched
scrutinee value. When the scrutinee is already known to be a script record or
enum type, dropping that fact forces method calls on the binding through
dynamic name dispatch.

Decision:
When compiling a `match`, compute the script receiver type fact for the
scrutinee once and apply it to simple binding patterns in each arm. This covers
patterns like `match player { bound => bound.bonus(5) }` without attempting to
infer destructured variant-field types yet.

Consequences:
- Binding-pattern locals can participate in `CallMethodId` dispatch when the
  scrutinee receiver type was already known.
- Arm-local facts are restored after each arm, preserving existing match scope
  isolation and assignment behavior.
- Destructured enum/record field type facts remain later M10 work because they
  need schema-aware field metadata.

## 2026-05-25: Typed Struct Fields Use HIR Slot Facts

Status: Accepted

Context:
Record and enum runtime values already store fields in stable sorted slots, and
immediate record literals can lower direct field reads to slot bytecode. Field
access through typed locals still used dynamic name lookup even when HIR struct
shape metadata made the slot known.

Decision:
Derive script struct field slot facts from HIR `StructShape` metadata and carry
them in bytecode compiler facts. When type-flow proves a receiver is a script
struct type, lower field reads to `GetRecordSlot` and field writes to
`SetRecordSlot`; otherwise keep the existing host-field and dynamic
record-field fallback paths.

Consequences:
- Declared script struct field access can use stable slot bytecode beyond
  immediate literals.
- The slot facts come from declared script metadata and do not alter dynamic
  runtime values or host mutation boundaries.
- Slot lowering for enum variant payloads remains separate M10 work.

## 2026-05-25: Enum Variant Payloads Are HIR Shape Metadata

Status: Accepted

Context:
The grammar allows enum variants to declare tuple or record payload fields, but
the syntax and HIR layers previously kept only variant names. The compiler could
specialize immediate enum literals by inspecting constructor syntax, but
typed locals initialized from enum variants still fell back to dynamic enum
field lookup.

Decision:
Represent enum variant payload fields in the syntax AST, HIR `EnumShape`, and
TypeRegistry variant metadata. Derive enum variant field slot facts from HIR
shape metadata, preserve compiler-local variant facts for enum constructor
values, and lower field reads on values known to be a specific declared variant
to `GetEnumSlot`.

Consequences:
- Declared record and tuple variant payloads now participate in stable script
  enum metadata and schema hashes.
- Reflection exposes declared payload fields on `VariantDesc`.
- Typed locals initialized from enum constructors can use slot bytecode for
  declared variant field reads without changing runtime enum layout or host
  mutation boundaries.
- Destructured match pattern field facts and host type impl dispatch remain
  later M10 work.

## 2026-05-25: Destructured Variant Fields Preserve Declared Type Facts

Status: Accepted

Context:
Record and tuple variant patterns bind payload fields into fresh locals. Even
after enum variant payload metadata became available in HIR, those locals were
still bound without script receiver type facts, so method calls on destructured
script record values fell back to dynamic method-name dispatch.

Decision:
Carry script type facts for declared enum payload fields in the compiler's HIR
field metadata table. When binding record or tuple variant pattern locals, look
up the matched enum variant and payload field, then seed the bound local with
the declared script type fact.

Consequences:
- Destructured variant payload locals can lower script trait method calls to
  `CallMethodId`.
- The facts are still compiler-local metadata; runtime enum layout and host
  mutation boundaries are unchanged.
- Host type impl dispatch remains separate M10 work.

## 2026-05-25: HostRef Script Impl Dispatch Uses Registered Type Names

Status: Accepted

Context:
Script impl methods can be compiled for target names that are not script
structs or enums, but VM script method lookup previously only knew how to get
receiver type names from script records and enums. A host ref therefore could
implement a script trait in metadata but still fail dynamic method dispatch.

Decision:
Keep the reflection `TypeRegistry` attached to the VM when reflection natives
are registered. Script method lookup now resolves `HostRef` receiver type names
through that registry and dispatches to the program's hidden script impl method
table by registered host type name. Script method bodies still interact with
host state through reflection/host APIs such as `reflect.get`, preserving the
PatchTx boundary for mutation.

Consequences:
- Host refs can enter script impl/default method dispatch when their host type
  is registered in `TypeRegistry`.
- Scripts still do not receive Rust references or direct mutable host access.
- A fuller Engine API for explicit type registry installation remains future
  work.

## 2026-05-25: VM TypeRegistry Registration Is Explicit

Status: Accepted

Context:
The VM needs host schema metadata for host-ref script impl dispatch even when a
host does not expose reflection natives to scripts. Previously the VM retained
the `TypeRegistry` only as a side effect of `register_reflection_natives`, which
coupled script method dispatch metadata to reflection function installation.

Decision:
Add `Vm::register_type_registry` and `Vm::with_type_registry` as explicit
registration APIs for immutable host type metadata. Keep
`register_reflection_natives` as a convenience that delegates to the explicit
registration path before installing reflection native functions.

Consequences:
- Host refs can dispatch to script impl methods using registered host type
  metadata without exposing reflection natives.
- Reflection metadata remains read-only from scripts and does not allow runtime
  type structure changes or monkey patching.
- A fuller Engine builder for schema and native function registration remains
  future M10 work.

## 2026-05-25: Engine API Starts In A Focused Crate

Status: Accepted

Context:
The embedding roadmap requires a stable `Engine`/`EngineBuilder` surface for
host schemas and native functions. Continuing to add host-facing registration
logic directly to the VM would couple embedding policy to bytecode execution
and keep growing an already broad runtime module.

Decision:
Introduce a focused `vela_engine` crate with separate builder, engine, error,
and native descriptor modules. The first API slice registers explicit host type
descriptors, registers native functions with stable IDs and metadata, rejects
duplicate type/native IDs and names at build time, and installs the immutable
`TypeRegistry` plus native function table into `Vm`.

Consequences:
- Hosts gain an initial stable registration boundary without exposing Rust
  references or bypassing `PatchTx`.
- VM remains the execution engine while `vela_engine` owns embedding-time
  validation and installation.
- Native methods, host-call context, permission enforcement, descriptor
  serialization, and derive macro output remain future Engine work.

## 2026-05-25: Host-Aware Natives Enter Through HostExecution

Status: Accepted

Context:
The engine API must support host services that can read host context or record
host mutations, but scripts must still never receive Rust references or mutate
host state outside `PatchTx`. Pure native functions that only receive script
values are not enough for gameplay helpers such as context emitters or
controlled host writes.

Decision:
Add `EngineBuilder::register_host_native_fn` for native descriptors whose
callable receives `HostExecution`. Engine installation registers these through
`Vm::register_host_native`, while pure natives continue to use
`Vm::register_native`. Engine build validation treats pure and host-aware
natives as one ABI namespace for stable IDs and names.

Consequences:
- Host-aware native functions can record mutations through `PatchTx` without
  exposing `&mut` host objects to scripts.
- Duplicate native ABI IDs and names are rejected across both native kinds.
- Permission enforcement and native method registration remain separate Engine
  work.

## 2026-05-25: Engine Enforces Native Permission Requirements

Status: Accepted

Context:
Native descriptors already carry required permission metadata, but the first
Engine API slice only stored that metadata. The architecture requires
permission checks before native dispatch, especially for host-aware natives
that can record `PatchTx` mutations.

Decision:
Add an Engine-owned `PermissionSet` and builder APIs for granting permissions.
During VM installation, wrap pure and host-aware native callables with a
permission check against the descriptor's `FunctionAccess`. Missing
permissions return `VmErrorKind::PermissionDenied` before the Rust callback is
invoked.

Consequences:
- Host applications can configure which native capabilities an Engine grants.
- Denied host-aware natives cannot record patches because rejection happens
  before callback dispatch.
- This is Engine-level native permission enforcement; field/method reflection
  permissions and native method registration remain future work.

## 2026-05-25: Engine Derives Host Method Compiler Options

Status: Accepted

Context:
The bytecode compiler can lower configured host method names to
`CallHostMethod`, but hosts still had to duplicate method registrations between
`TypeDesc::methods` and `CompilerOptions::with_host_method`. The current
compiler option surface is name-based, so ambiguous method names across host
types would compile nondeterministically if accepted by Engine registration.

Decision:
Expose `TypeRegistry::types` for read-only metadata iteration and add
`Engine::compiler_options`, which derives host method lowering options from
registered `TypeDesc::methods`. Engine build validation rejects duplicate host
method IDs and duplicate host method names within the same registered host
schema.

Consequences:
- Hosts can register method metadata once in Engine schemas and compile source
  with matching host method lowering.
- Method calls still enter the existing `CallHostMethod`/`PatchTx` path and do
  not expose mutable Rust references to scripts.
- Type-aware host method disambiguation and callable native method dispatch
  are handled by later Engine checkpoints.

## 2026-05-25: Native Method Callables Use HostPath Receivers

Status: Accepted

Context:
Engine schemas can now drive host method lowering, but there was still no
Engine-owned callable table keyed by `HostMethodId`. The architecture requires
native methods to be registered separately from reflectable descriptors while
preserving the rule that scripts never receive real mutable Rust references.

Decision:
Add `NativeMethodDesc` and `NativeMethodEntry` in a focused Engine method
module. `EngineBuilder::register_native_method_fn` accepts an owner `TypeKey`,
injects `MethodDesc` metadata into that host type before building the immutable
registry, and stores a callable keyed by `HostMethodId`. `Engine::call_native_method`
invokes the callable with a `HostPath`, script `Value` args, and
`HostExecution`, enforcing descriptor permissions before dispatch.

Consequences:
- Host method callables are registered through Engine with stable method IDs
  and can be invoked by host-side integration code without exposing `&mut`
  Rust objects.
- Script-compiled method syntax still lowers to `CallHostMethod` and records
  `PatchTx` operations; VM direct native-method dispatch remains future work.
- Native method owners must be registered host schemas, and duplicate method
  IDs/names remain rejected within an owner type.

## 2026-05-25: Host Method Lowering Can Use Receiver Type Facts

Status: Accepted

Context:
Host method lowering was originally name-based through
`CompilerOptions::with_host_method`, so two host schemas could not safely share
the same method name. Engine schemas now know the owner type for each method,
and the compiler already tracks lightweight receiver type facts from type
hints.

Decision:
Extend `CompilerOptions` with known host type names and type-qualified host
method mappings. During call lowering, the compiler first tries
`receiver type + method name` when the receiver has a type fact, then falls back
to the legacy name-only mapping. `Engine::compiler_options` now emits
type-qualified mappings for registered host methods.

Consequences:
- Different host types can share a method name when scripts provide receiver
  type hints such as `player: Player` and `monster: Monster`.
- Legacy name-only host method mappings still work for existing compiler
  callers.
- The VM path is unchanged: typed host method calls still lower to
  `CallHostMethod` and record patches rather than exposing Rust references.

## 2026-05-25: Host Method Bytecode Carries Field Paths

Status: Accepted

Context:
Host method syntax could record `PatchTx::CallHostMethod` for a root
`HostRef`, but calls on host-owned subobjects such as
`player.inventory.add(...)` first read the field into a register and then hit
the root-only VM method-call path. The architecture requires these calls to
target a `HostPath` without exposing mutable Rust references.

Decision:
Extend `CallHostMethod` with ordered host field path segments. The compiler's
host-method helper resolves configured host field names in the receiver path,
emits the root receiver register plus the resolved field IDs, and the VM builds
the corresponding `HostPath` before recording the method call patch.

Consequences:
- Host method calls can target field paths such as
  `HostPath::new(player).field(inventory)` while preserving `PatchTx` as the
  only mutation channel.
- Existing root host method calls use an empty field segment list.
- This slice covers field-only receiver paths; index, key, and variant-field
  path segments remain future path/proxy lowering work.

## 2026-05-25: Nested Host Field Access Uses Path Bytecode

Status: Accepted

Context:
Direct host field reads and writes had dedicated bytecode, but nested host
field syntax such as `player.stats.level` lowered as a read of
`player.stats` followed by a second root-field read. That only worked if the
intermediate value was itself a `HostRef`, and it did not model the intended
single `HostPath` transaction target.

Decision:
Add `GetHostPath`, `SetHostPath`, and `AddHostPath` bytecode for ordered host
field paths. The compiler resolves configured host field segments through a
focused `compiler::host_paths` helper, keeps existing direct host-field
instructions for one-segment paths, and emits path bytecode for field-only
nested paths. The VM builds a `HostPath` from the root `HostRef` and field
segments before reading or recording `PatchTx` operations.

Consequences:
- `player.stats.level += 2` records one nested add patch against
  `HostPath::new(player).field(stats).field(level)`.
- Reads after nested writes use the existing `PatchTx` overlay semantics.
- This does not yet add path proxy values, index/key/variant host path
  segments, or non-add RMW operations.

## 2026-05-25: Host Path Bytecode Supports Dynamic Bracket Segments

Status: Accepted

Context:
M11 requires paths like `player.inventory.items[item_id].count += 1`, which mix
static host fields with dynamic bracket segments. The previous path bytecode
only carried `FieldId` segments, so bracket syntax compiled through normal
script indexing and could not target a single `HostPath`.

Decision:
Generalize `GetHostPath`, `SetHostPath`, and `AddHostPath` to carry ordered
bytecode host path segments. Static dot segments remain `FieldId` values, and
bracket segment expressions compile into registers. At runtime, integer
segment values become `HostPath::index` entries and string segment values are
interned into VM-local host path keys.

Consequences:
- Scripts can record nested patches for paths such as
  `player.inventory.items[item_id].count` without exposing mutable host
  references.
- Existing direct field bytecode remains the one-segment fast path.
- VM-local key interning is sufficient for path identity within one VM
  execution; a host-owned key registry may be needed later for cross-runtime
  persistence or human-readable diagnostics.
- Variant-field path segments, `PathProxy` values, and non-add RMW operations
  remain future M11 work.

## 2026-05-25: Host Method Calls Share Host Path Segments

Status: Accepted

Context:
`GetHostPath`, `SetHostPath`, and `AddHostPath` can now represent dynamic
field/index/key receiver paths, but `CallHostMethod` still carried only static
field IDs. That left method calls on indexed host paths unable to target the
same receiver shape as host reads and writes.

Decision:
Change `CallHostMethod` to carry the same ordered bytecode host path segments
used by the path read/write instructions. Host method receiver lowering now
uses the focused compiler host path helper, and the VM constructs the method
receiver `HostPath` through the same runtime segment conversion path.

Consequences:
- `player.inventory.items[item_id].grant(20)` records a host method patch
  against the indexed/keyed receiver path.
- Direct root method calls use an empty segment list, preserving existing
  behavior.
- The method call still records a patch instead of dispatching to Rust with a
  mutable reference; script-visible host method return values remain future
  work.

## 2026-05-25: Host Subtraction Uses RMW Patch Transactions

Status: Accepted

Context:
`PatchOp::Sub` was part of the host patch model, but there was no transaction
overlay behavior, mock adapter support, bytecode instruction, or compiler/VM
lowering for host `-=`. Scripts could record addition RMW patches but could
not naturally decrement host state through the same boundary.

Decision:
Add `PatchTx::sub_path`, numeric `HostValue` subtraction, and mock adapter
validation/apply support for `PatchOp::Sub`. Add `SubHostField` and
`SubHostPath` bytecode and lower host `-=` assignments to those instructions.
The VM reads the current transaction overlay or adapter snapshot before
recording the subtraction patch, matching the existing add-RMW behavior.

Consequences:
- Scripts can decrement direct and nested host paths without exposing mutable
  Rust references.
- Reads after host `-=` observe the transaction overlay.
- Multiplicative/divisive/rem RMW operations remain future M11 transaction
  work.

## 2026-05-25: Host Path Push Records Array Patch Transactions

Status: Accepted

Context:
`PatchOp::Push` existed in the host patch model, and script arrays already
have a `push` method, but host paths could not use that syntax without either
reading a copied array value or registering an explicit host method. M11 calls
for push-style patch transaction effects while preserving the rule that
scripts do not receive mutable host references.

Decision:
Add array-valued `HostValue` support and `PatchTx::push_path`, which records a
`PatchOp::Push` and updates the transaction overlay by appending to the
current host array snapshot. The compiler lowers `host.path.push(value)` to
`PushHostPath` when the receiver resolves as a configured host path and no
configured host method handles the call. The VM converts script array/scalar
values through `HostValue` and records the push patch after reading the base
snapshot or overlay.

Consequences:
- Scripts can append to array-valued host paths through PatchTx with natural
  method syntax.
- Reads after `host.path.push(value)` observe the overlay array.
- Host methods named `push` still take precedence when explicitly configured.
- Non-array push targets and richer host value conversion for maps, records,
  and enums remain future M11 work.

## 2026-05-25: Host Path Remove Uses Overlay Tombstones

Status: Accepted

Context:
`PatchOp::Remove` existed in the host patch model, but the transaction overlay
previously represented only concrete values. Treating a removed path as an
absent overlay entry would make reads after `remove()` fall through to the
adapter snapshot, which is incorrect inside a transaction.

Decision:
Store PatchTx overlay entries as either a host value or a removed tombstone.
`PatchTx::remove_path` records `PatchOp::Remove` and inserts a tombstone, while
later reads of that path return `MissingPath` until the same transaction writes
a replacement value. The compiler lowers `host.path.remove()` to
`RemoveHostPath` when the receiver resolves as a configured host path, and the
VM records the remove patch without exposing a mutable host reference.

Consequences:
- Scripts can delete nested host paths through PatchTx with natural method
  syntax.
- Reads after removal observe the transaction tombstone rather than stale host
  adapter state.
- A later `set` in the same transaction replaces the tombstone with a value.
- Rollback-safe multi-patch apply and conflict reporting remain future M11
  work.

## 2026-05-25: Host Values Support String-Keyed Maps

Status: Accepted

Context:
M11 requires host value conversion for common script containers. Arrays and
scalars could already cross the host boundary, but script maps could not be
written to host paths or returned from exact-path host reads without failing
conversion.

Decision:
Add `HostValue::Map(BTreeMap<String, HostValue>)` and convert script
`Value::Map` plus heap-backed `HeapValue::Map` into that host representation.
Host map values read back from an exact host path become script `Value::Map`
values, preserving the existing string-keyed script map model.

Consequences:
- Managed-heap script maps can be recorded in `PatchTx::Set` patches.
- Reads of exact map-valued host paths can use existing script map methods such
  as `len`.
- Nested key reads after setting an entire map still require a future
  overlay-descendant lookup model; this change does not reinterpret a
  `HostPath::key` as indexing into an overlaid parent map.
- Richer nullable conversions remain future M11 work.

## 2026-05-25: Host Values Support Script Records

Status: Accepted

Context:
M11 requires host value conversion for script records as copied data. The VM
already represents records with stable `ScriptFields`, but host boundary
conversion still rejected `Value::Record` and heap-backed `HeapValue::Record`.
Keeping this conversion logic in `vela_vm/src/lib.rs` would also keep growing a
large file with host bridge details.

Decision:
Add `HostValue::Record { type_name, fields }` using a copied type name and
string-keyed `BTreeMap<String, HostValue>` fields. Move VM host value
conversion into a focused `host_values` module, and convert both immediate
script records and heap-backed records through that module. Exact host path
reads convert the copied host record back into a script `Value::Record`.

Consequences:
- Managed-heap script records can be recorded in `PatchTx::Set` patches.
- Exact overlay reads preserve the record type name and script field values.
- The host still receives copied data only; scripts do not receive Rust
  references or mutate host-owned record structure.
- Nullable and descendant overlay conversions remain future M11 work.

## 2026-05-25: Host Values Support Script Enums

Status: Accepted

Context:
M11 requires host value conversion for script enums as copied data. Records
could already cross the host boundary, but `Value::Enum` and heap-backed
`HeapValue::Enum` still failed conversion. Script enums carry both the enum
name and variant name, and variant field shapes are owned by the
`enum.variant` pair.

Decision:
Add `HostValue::Enum { enum_name, variant, fields }` with copied names and
string-keyed `BTreeMap<String, HostValue>` fields. Convert both immediate and
heap-backed script enum values in the VM `host_values` module, and convert
exact host path reads back into script `Value::Enum` using the same
`enum.variant` field owner convention as the runtime.

Consequences:
- Managed-heap script enums can be recorded in `PatchTx::Set` patches.
- Exact overlay reads preserve enum name, variant name, and field values.
- The host receives copied enum data only; scripts do not mutate host-owned
  enum structure.
- Nullable and descendant overlay conversions remain future M11 work.

## 2026-05-25: Host Values Support HostRef Handles

Status: Accepted

Context:
M11 requires host value conversion for host refs, but scripts must never own
Rust host state or receive real mutable references. The VM already treats
`Value::HostRef` and heap `HeapSlot::HostRef` as external handles that are not
traced as Rust-owned state by the script GC.

Decision:
Add `HostValue::HostRef(HostRef)` and convert script `Value::HostRef` to that
copied handle when recording host patches or method arguments. Exact host path
reads convert the handle back into `Value::HostRef`. This conversion copies
only the stable host handle; it does not move host state under script heap
ownership and does not expose Rust references.

Consequences:
- Scripts can pass host-ref values through `PatchTx::Set` and exact overlay
  reads.
- Host refs remain external to the script heap and keep the existing GC
  behavior.
- Nullable and descendant overlay conversions remain future M11 work.

## 2026-05-25: Patch Apply Uses Adapter Batch Hook

Status: Accepted

Context:
M11 requires failed patch apply to leave adapter state unchanged. `PatchTx`
previously validated all patches and then applied each one individually through
`ScriptStateAdapter::apply_patch`. A failure during a later patch could leave
earlier patches committed in adapters that did not provide their own rollback
mechanism.

Decision:
Add `ScriptStateAdapter::apply_patches` as the batch apply entry point used by
`PatchTx::apply`. The default implementation preserves the old validate-then-
apply behavior for adapters that have not yet implemented rollback. The mock
adapter overrides the hook by cloning its state after validation and restoring
that snapshot if any patch fails during the apply phase.

Consequences:
- `PatchTx` now has a clear adapter-level safe-point commit hook.
- Mock adapter tests prove late apply failures leave mock state unchanged.
- Production adapters can implement their own transaction, snapshot, or
  rollback mechanism behind the same hook.
- Conflict reporting and mandatory rollback semantics for every external
  adapter remain future M11 work.

## 2026-05-25: Mock Adapter Enforces Host Access Policies

Status: Accepted

Context:
M11 requires read-only and permission-denied host paths to fail before apply.
The mock adapter previously validated object freshness and method existence,
but it had no explicit read/write/call policy surface for host bridge tests.

Decision:
Add path-level read, write, and call denial sets to `MockStateAdapter` and
report `HostErrorKind::PermissionDenied { path, action }` when a denied access
is attempted. Reads check read policy after freshness validation. Patch
validation checks write policy for mutating patch operations and call policy
for host method patches before the batch apply phase can mutate state or
record method calls.

Consequences:
- Host bridge tests can exercise permission-denied read, write, and call
  paths without exposing mutable Rust references.
- Denied patch writes and calls fail during batch validation and leave adapter
  state unchanged.
- Engine-level policy wiring and richer permission scopes remain future M11
  work.

## 2026-05-25: Host Errors Carry Source Spans

Status: Accepted

Context:
M11 requires source-span propagation into patches and host errors. Patch
records already carried optional spans, but host failures returned from
transaction reads, patch validation, and safe-point apply only exposed the
`HostErrorKind`, making it harder to tie host boundary failures back to the
script operation that produced them.

Decision:
Add `source_span: Option<Span>` to `HostError`. `PatchTx` attaches spans to
transaction read and read-modify-write failures, `ScriptStateAdapter` batch
apply preserves patch spans by default, and `MockStateAdapter` preserves patch
spans during validation and rollback-safe apply. VM host-read errors propagate
the instruction span when converting `HostError` into `VmError`.

Consequences:
- Host bridge diagnostics can point at the script operation that attempted the
  denied or invalid host access.
- Existing host error kind comparisons remain stable, with source location
  carried separately.
- Broader diagnostic rendering remains future work; this change only preserves
  the structured span data across the host boundary.

## 2026-05-25: RMW Patches Carry Expected Base Values

Status: Accepted

Context:
M11 requires conflict reporting for host patch transactions. RMW operations
already read a host path before recording `Add`, `Sub`, or `Push` patches, but
the recorded patch only stored the delta. If host state changed before
safe-point apply, the mock adapter would apply the delta to the newer value
without reporting that the transaction was based on stale data.

Decision:
Store `expected_base: Option<HostValue>` on patches. `PatchTx` records the
expected base only for the first RMW/push patch that reads the adapter value
for a path; later mutations that read the transaction overlay do not add a
second adapter-base expectation. `MockStateAdapter` compares expected and
actual host values during patch validation and reports
`HostErrorKind::PatchConflict { path, expected, actual }` before apply mutates
state, with copied conflict values boxed so ordinary host results remain small.

Consequences:
- Mock host transactions now report external host-state changes before
  committing RMW and push patches.
- Sequential mutations within the same transaction continue to compose through
  the overlay without false conflicts.
- Production adapters can use the same patch metadata for optimistic conflict
  checks or map it onto their storage transaction semantics.

## 2026-05-25: Host Method Returns Use Copied Previews

Status: Accepted

Context:
M11 requires host method calls to return script-visible copied values without
exposing Rust `&mut` references. `CallHostMethod` previously recorded a
deferred method-call patch and wrote `null` to the destination register. That
preserved the safe-point mutation boundary, but scripts could not use copied
host method return values during the same execution.

Decision:
Add `ScriptStateAdapter::preview_method_return` as a read-only return-value
hook. `CallHostMethod` asks the adapter for a copied `HostValue`, writes the
converted script value to the destination register, and still records the
method call as a `PatchTx` patch for safe-point apply. The mock adapter returns
configured method-return values without recording a method call during preview.

Consequences:
- Scripts can consume host method return values while host mutation remains
  deferred through `PatchTx`.
- The host boundary still passes copied `HostValue` data rather than Rust
  references.
- Production adapters can compute or validate read-only return previews
  separately from applying the method effect at the safe point.

## 2026-05-25: Reflection Registers Script Modules And Functions

Status: Accepted

Context:
M12 requires `TypeRegistry` and reflection to cover modules and functions in
addition to types, fields, methods, traits, variants, attributes, and
permissions. The registry already consumes HIR for script type metadata, but
module and function declarations were still only visible through the compiler
and HIR graph.

Decision:
Add a focused `vela_reflect::modules` module with `ModuleDesc`,
`FunctionDesc`, parameter metadata, module exports, declaration origin, and
stable reflected function IDs. `TypeRegistry::register_script_modules` walks
the HIR module graph and registers script modules plus function descriptors
with visibility, type-hint display strings, default-parameter markers, return
hints, and export entries.

Consequences:
- Reflection has a stable metadata surface for script modules and functions
  without adding more responsibilities to `lib.rs`.
- Runtime schema mutation remains disallowed; this is registration-time
  metadata derived from HIR.
- Script-visible `reflect.module`, `reflect.exports`, and function permission
  checks remain follow-up M12 work.

## 2026-05-25: Reflection Module Queries Return Copied Metadata

Status: Accepted

Context:
M12 requires scripts to inspect modules and functions through controlled
reflection. The registry now stores script module/function descriptors, but the
script-visible API still needs to preserve the no-monkey-patching rule.

Decision:
Expose `reflect.module`, `reflect.exports`, and `reflect.function` as read-only
host natives that return copied metadata records and arrays from `TypeRegistry`.
Unknown module/function errors include name candidates. These queries do not
return mutable descriptor handles and do not alter type, module, or function
structure at runtime.

Consequences:
- Admin and debug scripts can inspect registered module exports and function
  signatures.
- Reflection remains a controlled query surface instead of a schema mutation
  surface.
- Permission-bounded reflective calls and method/variant query coverage remain
  follow-up M12 work.

## 2026-05-25: Reflection Member Queries Stay In A Focused Module

Status: Accepted

Context:
M12 needs method, trait, and variant reflection in addition to type, field,
module, and function queries. Adding all of that directly to the reflection
crate facade would make `lib.rs` harder to review and blur descriptor ownership.

Decision:
Put read-only member query helpers in a dedicated `members` module. The VM
registers script-visible `reflect.methods`, `reflect.has_method`,
`reflect.traits`, `reflect.variants`, `reflect.variant`, and
`reflect.variant_is` as thin native bindings over those helpers. Returned
metadata is copied into records/arrays, and current enum variant inspection does
not expose mutable registry descriptors.

Consequences:
- Reflection member behavior is tested independently of VM native dispatch.
- `lib.rs` remains the crate facade instead of becoming the home for every
  reflection query shape.
- Runtime schema mutation remains unavailable; permission checks and field
  detail queries remain follow-up M12 work.

## 2026-05-25: Field Reflection Reuses Copied Member Records

Status: Accepted

Context:
M12 still needs `reflect.name`, `reflect.kind`, `reflect.field`, and
`reflect.has_field`. Field descriptors already exist in `TypeDesc`, and the new
member query module already owns copied metadata record construction for
methods, traits, and variants.

Decision:
Add field, name, and kind queries to the same focused `members` module. The VM
registers thin script-visible natives over those helpers. `reflect.field`
returns a copied `ReflectField` record with stable ID, name, and writable flag;
unknown field lookups reuse ranked candidate hints.

Consequences:
- Type and field query APIs now cover the first-version reflection surface
  without returning mutable descriptor handles.
- The facade still only re-exports helpers and does not become a large query
  implementation file.
- Attribute/doc metadata and permission-gated reflection remain follow-up M12
  work.

## 2026-05-25: Reflection Permissions Are Enforced At Native Entry

Status: Accepted

Context:
M12 requires reflection permission checks while preserving the existing host
boundary rule that mutations only enter `PatchTx`. Reflection helpers are also
usable below the VM, so policy enforcement needs a clear embedding boundary.

Decision:
Add a focused `permissions` module in `vela_reflect` with
`ReflectPermission` and `ReflectPermissionSet`. Keep the existing permissive
`Vm::register_reflection_natives` for tests and demos, and add
`Vm::register_reflection_natives_with_permissions` for bounded installs. The
Engine API exposes `EngineBuilder::reflection_permissions` as an opt-in hook
that installs permissioned reflection natives with the registered type
registry.

Consequences:
- Missing reflective write/call permissions fail before any host patch is
  recorded.
- Hosts can enable read-only, admin, or custom reflection policies through the
  stable Engine API without bypassing `PatchTx`.
- Lookup budgets and deeper `EffectSet`/access metadata checks remain follow-up
  M12 work.

## 2026-05-25: Reflection Metadata Exposes Copied Attrs And Docs

Status: Accepted

Context:
M12 requires reflection to cover attributes and docs/origin metadata. Descriptor
types already had `AttrMap` placeholders, and native/function descriptors had
docs, but scripts could not inspect those values through the reflection API.

Decision:
Add builder/query APIs to `AttrMap` and docs/attribute builder methods to
reflected descriptors. A focused metadata helper converts attrs/docs into copied
host values. `reflect.attrs` and `reflect.docs` query type metadata for a
target value, while reflected field, method, trait, trait-method, variant,
module, and function records include copied `attrs` and `docs` fields where
the descriptor supports them.

Consequences:
- Admin/debug scripts can inspect schema annotations without receiving mutable
  descriptor handles.
- Engine-registered native method docs are copied into reflected method
  metadata.
- Parser/HIR extraction of script attributes and deeper access/effect metadata
  remain follow-up M12 work.

## 2026-05-25: Reflection Lookup Budgets Are Per VM Install

Status: Accepted

Context:
M12 requires reflection to be bounded as well as permissioned. Engine policy
configuration may be reused to create multiple VMs, but consumed lookup counts
must not leak from one VM install to another.

Decision:
Represent reusable reflection configuration with `ReflectPolicy`, containing a
permission set plus an optional lookup limit. When a VM installs reflection
natives, it creates a fresh shared `ReflectLookupBudget` counter for that native
set. Each script-visible reflection native checks permissions first, then
consumes one lookup before performing metadata queries, reads, writes, or calls.

Consequences:
- A bounded reflection script fails with `LookupBudgetExceeded` before any host
  patch is recorded after the limit is exhausted.
- `EngineBuilder::reflection_lookup_budget` can install bounded reflection
  without sharing consumed counters across `Engine::into_vm` calls.
- Finer per-call-frame or per-event reflection budgets can be layered later
  without changing the schema-safe reflection helper APIs.

## 2026-05-25: Script Attributes Are Copied Into Reflection Metadata

Status: Accepted

Context:
M12 requires reflection to expose attributes and docs for script-defined
metadata, not only host-registered descriptors. The parser already recognized
attribute syntax, but payloads and member attributes were discarded before HIR
and reflection registration.

Decision:
Preserve simple string or identifier attribute payloads in syntax and HIR.
HIR stores declaration and member attributes as copied metadata. Reflection
registration converts `#[doc("...")]` into descriptor docs and copies all other
attributes into `AttrMap` using `"true"` for marker attributes without payloads.

Consequences:
- Script functions, structs, fields, enum variants, traits, and trait methods
  can now expose copied docs/attrs through the existing reflection query
  records.
- Attribute reflection remains schema-safe because scripts receive copied
  metadata values, not mutable descriptor handles.
- Richer attribute arguments can be added later without changing the reflection
  descriptor boundary.

## 2026-05-25: Reflective Host Calls Respect Method Metadata

Status: Accepted

Context:
M12 requires reflective calls to respect method access and effect metadata.
Engine native methods already carried `EffectSet` and `FunctionAccess`, but
reflected `MethodDesc` only exposed identity/docs/attrs, and VM `reflect.call`
could record a host method patch after only checking the broad
`ReflectPermission::CallMethods` bit.

Decision:
Add copied `MethodEffectSet` and `MethodAccess` metadata to `MethodDesc`.
Engine native method registration converts native descriptor effects/access
into reflected method metadata. VM-installed reflection natives call
`reflect.call_with_policy`, which rejects non-reflect-callable methods and
methods requiring ungranted permissions before entering `PatchTx`.

Consequences:
- Gameplay/admin reflection policies can allow broad reflection while still
  restricting which host methods may be called dynamically.
- Reflected method query records now include copied effects and access records
  for debug/admin tooling.
- The host boundary remains patch-only; denied reflective calls do not record
  host patches or invoke native method callbacks.

## 2026-05-25: Native Function Metadata Is Reflected Through TypeRegistry

Status: Accepted

Context:
M12 requires reflection over modules and functions, including effects and
permissions. Engine native functions already had parameter hints, return hints,
effect bits, access policy, and docs, but that metadata only lived in the
embedding layer and was not visible through `reflect.function`.

Decision:
Move reflection access/effect descriptor types into a focused `access` module
and add function-specific access/effect metadata to `FunctionDesc`.
`EngineBuilder` copies registered native and host-native function descriptors
into `TypeRegistry`, creating module export metadata for dotted native names.

Consequences:
- Admin/debug scripts can inspect host-native function signatures, docs,
  effects, reflect visibility, and required permissions through copied
  reflection records.
- Reflection still cannot mutate module or function structure at runtime.
- Function metadata now has enough shape for later hot-reload effect and access
  ABI compatibility checks.

## 2026-05-25: Hot Reload ABI Checks Use Copied Manifests

Status: Accepted

Context:
Function-level hot reload already preserved old code and rejected deleted
parameters, but M12 requires schema and effect/access compatibility checks at
safe points. The reflection registry owns the schema hashes and copied access
metadata needed for those checks, while hot reload should not hold mutable
schema descriptors or expose runtime monkey patching.

Decision:
Represent hot-reload compatibility with a copied `HotReloadAbi` manifest built
from `TypeRegistry` or explicit descriptor entries. `compile_update_with_abi`
compiles the new source, validates existing parameter compatibility, and then
rejects removed/changed schema hashes or changed function/method effect and
reflective access metadata before producing a `HotUpdate`.

Consequences:
- Hot reload can enforce schema and permission/effect ABI stability without
  giving scripts mutable access to type structure.
- Existing code objects are still swapped only through `HotReloadRuntime` at
  the update boundary.
- The CLI and Engine hot-reload paths can later pass registry-derived manifests
  without changing the core versioning API.

## 2026-05-25: Engine Registries Are The Hot Reload ABI Source

Status: Accepted

Context:
The hot-reload ABI manifest needs to match the schema and permission metadata
that scripts and hosts actually use. Duplicating that metadata in a CLI demo or
separate host configuration would make the checked path easy to drift from the
reflection registry.

Decision:
Expose `Engine::hot_reload_abi()` as a registry-derived manifest and use the
game-server demo `TypeRegistry` to drive the CLI hot-reload command through
`compile_initial_with_abi` and `compile_update_with_abi`.

Consequences:
- Host applications can use one reflected registry as the source for both
  runtime reflection and hot-reload compatibility checks.
- The runnable CLI function-swap demo now proves the ABI-checked update path,
  not only raw function replacement.
- Future Engine hot-reload policy work can compose around the existing manifest
  instead of adding another schema description surface.

## 2026-05-25: Schema Hint Diagnostics Are Candidate Driven

Status: Accepted

Context:
Scripts can refer to script-defined schemas and traits, but hosts may also
provide schema names that HIR cannot know without Engine context. M12 still
needs useful related-span diagnostics when a script misspells a known schema or
trait name.

Decision:
HIR validates type hints and `impl Trait for Type` paths only when an
unresolved name has a close visible schema/trait candidate. The diagnostic uses
the unknown reference as the primary span and adds ranked related labels on the
candidate declarations. Names without any close local candidate remain metadata
so host-provided schemas can be resolved by later Engine/compiler context.

Consequences:
- Misspelled script schemas fail before bytecode generation with actionable
  candidate spans.
- External host schema names are not rejected just because HIR lacks a host
  registry.
- Later Engine-integrated semantic validation can tighten host-schema checks
  without changing the HIR diagnostic shape.

## 2026-05-25: Engine Hot Reload Uses Engine Compiler Metadata

Status: Accepted

Context:
The Engine already owns host schemas, native methods, reflection metadata, and
hot-reload ABI manifests. Requiring embedders to call lower-level hot-reload
compile functions directly would make them pass compiler options and ABI
metadata separately, which is easy to get out of sync.

Decision:
Add option-aware hot-reload compile helpers and Engine methods that compile
initial versions and updates with `Engine::compiler_options()` plus
`Engine::hot_reload_abi()`.

Consequences:
- Host method lowering and ABI validation now share the same Engine registry
  source in the embedding API.
- Hot-reload demos and hosts can move through Engine-level helpers instead of
  manually assembling manifests and compiler options.
- Lower-level hot-reload APIs remain available for tests and specialized
  runtimes that build their own manifests.

## 2026-05-25: Hot Reload Preserves Existing Parameter ABI

Status: Accepted

Context:
Hot reload swaps function code objects at safe points while existing call frames
continue on old code. New calls may still target old callers or host event
bindings that know the previous parameter names and order, so accepting
renamed or reordered existing parameters can silently reinterpret arguments.

Decision:
Function-signature compatibility lives in a focused hot-reload module. Updates
must preserve every existing parameter name at the same position, reject
deleted parameters, and may append new parameters after the preserved prefix.

Consequences:
- Event handlers and host-facing functions keep a stable positional ABI across
  function-level updates.
- Appended parameters remain possible for later default-aware compatibility
  policy, while existing call sites continue to see the same prefix contract.
- The compile driver delegates signature policy instead of accumulating more
  compatibility logic directly in `compile.rs`.

## 2026-05-25: Engine Owns Hot Reload Policy Selection

Status: Accepted

Context:
The default hot-reload behavior accepts new helper functions and defaulted
parameter additions, but production hosts may want narrower policies for live
game shards or privileged admin workflows. Requiring hosts to bypass
`Engine::compile_hot_reload_update` to enforce those choices would split policy
from the registry-derived compiler and ABI metadata.

Decision:
Represent reload choices with `HotReloadPolicy`, expose policy-aware compile
helpers in `vela_hot_reload`, and store the selected policy on `Engine`.
`EngineBuilder::hot_reload_policy` configures the policy used by
`Engine::compile_hot_reload_update`.

Consequences:
- Embedders can opt into locked-down reload behavior without giving up Engine
  compiler options or ABI checks.
- The default policy preserves existing runnable helper-update workflows.
- Additional reload policy controls can grow in `vela_hot_reload::policy`
  without adding more one-off booleans to `Engine`.

## 2026-05-25: Hot Reload Reports Summarize Safe-Point Updates

Status: Accepted

Context:
The architecture expects hot reload to return a report with accepted/rejected
status, errors, and repair hints. The runtime only returned the new
`ProgramVersion`, which proved code swapping but gave hosts no structured
summary to log or surface in admin/debug tooling.

Decision:
Add a focused hot-reload report module with `HotReloadReport` and
`HotReloadDiagnostic`. `HotReloadRuntime::apply_hot_update_report` returns the
accepted safe-point update summary, while the existing `apply_hot_update`
convenience API remains available. Rejected diagnostics can be built from
`HotReloadError` and include a stable reason plus optional repair hint.

Consequences:
- Hosts can inspect changed function names and version transitions after an
  accepted safe-point swap.
- Rejected reload paths have a common diagnostic shape before richer source
  span and related-location reporting is added.
- The CLI hot-reload demo now exercises the report API without changing the
  underlying function-level swap semantics.

## 2026-05-25: Rejected Hot Reload Results Do Not Advance Versions

Status: Accepted

Context:
Compile, ABI, and policy checks happen before a hot update reaches the
safe-point swap. Hosts still need the same report shape for these rejected
updates as they get for accepted swaps, and rejected updates must not change the
runtime's current `ProgramVersion`.

Decision:
Add `HotReloadRuntime::apply_hot_update_result_report`, which accepts a
`HotReloadResult<HotUpdate>`. Successful updates delegate to
`apply_hot_update_report`; rejected results produce `HotReloadReport::rejected`
using the current version ID and leave the runtime unchanged.

Consequences:
- Embedders can route compile/update results through one reporting boundary.
- Rejected reload reports now prove which version remained active.
- CLI hot-reload workflows can surface structured diagnostics for failed update
  compilation or ABI checks without custom branching.

## 2026-05-25: Hot Reload Diagnostics Have Codes And Targets

Status: Accepted

Context:
Human-readable reload reasons and repair hints are useful for logs, but hosts
and admin tooling also need stable fields for routing, aggregation, and UI
actions. Parsing reason strings would make those tools brittle.

Decision:
`HotReloadError` now exposes a stable diagnostic code and optional affected
target. `HotReloadDiagnostic` copies these values alongside the reason, repair
hint, and original error. Function, schema, and method ABI failures all provide
targets; compile failures keep the target absent until source spans and related
diagnostics are lifted into reload reports.

Consequences:
- Host tools can branch on codes such as `reload.function.new_denied` instead
  of matching human text.
- Rejected reports can identify affected functions, schemas, or methods in a
  consistent field.
- Future source-span and related-location work can extend diagnostics without
  changing the current report shape.

## 2026-05-25: Host Ref Metadata Requires InspectHostPath

Status: Accepted

Context:
M12 separates normal type metadata reads from host path inspection. `ReadTypeInfo`
is useful for script values and general schemas, but host refs identify live host
objects and their configured path surface, so exposing that metadata should use
the dedicated `InspectHostPath` permission.

Decision:
Keep `ReadTypeInfo` as the base permission for metadata natives, then require
`InspectHostPath` when the reflected target is a `HostRef`. This applies to
host-ref type/name/kind/field/method/trait/variant metadata and
`reflect.implements`; module and function registry queries are unchanged because
they do not inspect a host object path.

Consequences:
- Read-only gameplay/config policies can still inspect script-value metadata
  without gaining host-ref metadata access.
- GM/admin policies continue to use `ReflectPermissionSet::all()` for host-ref
  inspection workflows.
- Dynamic host value reads, writes, and calls remain controlled by their
  existing field and method permissions and still route mutations through
  `PatchTx`.

## 2026-05-25: Private Reflective Methods Require AccessPrivate

Status: Accepted

Context:
Reflection method metadata already records `MethodAccess::public`, but the
reflective call boundary only enforced `reflect_callable` and method-specific
permissions. That allowed a host to mark a method non-public while still making
it callable whenever the method-specific permission was present.

Decision:
Add `ReflectPermission::AccessPrivate` and enforce it in
`ReflectPolicy::require_method_access` whenever a method is not public. Private
method calls still require `reflect_callable`, the global `CallMethods`
permission at the VM native boundary, and all method-specific permissions.

Consequences:
- Gameplay policies can call approved public reflective methods without gaining
  access to private/admin methods.
- Admin/debug policies can opt into private reflective calls explicitly.
- The call still records only a `PatchTx` host-method patch; no real mutable
  Rust reference is exposed to scripts.

## 2026-05-25: Reflect Function Metadata Respects FunctionAccess

Status: Accepted

Context:
`FunctionDesc` already carries copied `FunctionAccess` metadata for public
visibility, reflection visibility, and function-specific permissions. The
script-visible `reflect.function` native only checked `ReadTypeInfo`, so it
could expose metadata for hidden, private, or permissioned functions.

Decision:
Add `ReflectPolicy::require_function_access` and route `reflect.function`
through a policy-aware metadata helper. Non-reflect-visible functions are
rejected, private functions require `AccessPrivate`, and required function
permissions must be present on the policy before metadata is returned.

Consequences:
- Hosts can register admin/debug-only function metadata without exposing it to
  normal gameplay reflection policies.
- `FunctionAccess` and `MethodAccess` now have matching policy enforcement at
  their reflection boundaries.
- The raw registry helper remains available for trusted host-side inspection
  and tests that do not model script permissions.

## 2026-05-25: Reflect Host Field Access Uses FieldAccess

Status: Accepted

Context:
Field descriptors only carried a `writable` boolean, while the architecture
requires field access metadata that separates host readability/writability from
reflective readability/writability. That made it impossible for hosts to expose
a field to normal host operations while hiding it from script reflection.

Decision:
Add `FieldAccess` to `FieldDesc` while preserving the existing `writable`
facade for compatibility. `reflect.field` and `reflect.fields` return copied
`ReflectFieldAccess` metadata. `reflect.get` rejects host fields that are not
`reflect_readable`, and `reflect.set` requires both host writability and
`reflect_writable` before recording a `PatchTx` write.

Consequences:
- Host schemas can hide sensitive fields from reflective reads without removing
  the field from the registered type.
- Reflective writes can be disabled independently from host writability.
- Existing `.writable(true)` schema builders continue to opt fields into both
  host writability and reflective writability.

## 2026-05-25: Hot Reload Compile Reports Carry Source Labels

Status: Accepted

Context:
Rejected hot-reload reports carried stable codes, targets, reasons, hints, and
the original `HotReloadError`, but compile failures still exposed no direct
source location. Host tooling had to unpack the embedded compiler error to point
at parser or semantic diagnostics.

Decision:
Add `source_span` and copied compiler `labels` to `HotReloadDiagnostic`.
Compile errors lift the first available primary span plus all compiler labels
from syntax or semantic diagnostics into the report. ABI and policy errors keep
these fields empty because their current targets are schema/function/method
identifiers rather than source locations.

Consequences:
- Admin/debug tooling can render rejected compile updates without parsing the
  embedded compiler error first.
- Existing machine-readable reload codes and targets remain stable.
- Future richer report details can add more structured fields without changing
  the safe-point update semantics.

## 2026-05-25: Hot Reload Reports Copy Source Diagnostics

Status: Accepted

Context:
Compile rejection reports exposed a primary span and flattened labels, but host
tooling still needed to inspect the embedded compiler error to access diagnostic
messages, diagnostic codes, and per-diagnostic spans. That made the report less
self-contained for admin/debug UIs.

Decision:
Add `source_diagnostics` to `HotReloadDiagnostic` and copy syntax/semantic
compiler diagnostics into that field for compile failures. Keep `source_span`
and `labels` as convenience fields for the first primary span and flattened
related labels.

Consequences:
- Hosts can render compile rejection messages directly from the reload report.
- Existing report consumers can continue using `code`, `target`, `reason`, and
  `repair_hint` without unpacking compiler internals.
- ABI and policy failures keep `source_diagnostics` empty until they gain
  source-location evidence.

## 2026-05-25: Hot Reload Reports Carry ABI Detail Records

Status: Accepted

Context:
Rejected hot-reload reports had stable diagnostic codes and targets, but ABI
failures still required hosts to parse human-readable reasons or match the
embedded `HotReloadErrorKind` to render old/new function parameters, schema
hashes, effect metadata, or access metadata.

Decision:
Add a focused `HotReloadDiagnosticDetail` report type and expose it through
`HotReloadDiagnostic::detail`. The detail records copy the specific ABI data
needed for report rendering: function parameter lists, added parameters, schema
hashes, and old/new function or method effect/access metadata.

Consequences:
- Admin/debug UIs can render ABI rejection details from the report boundary
  without parsing strings.
- Compile failures continue to use source diagnostics instead of ABI details.
- The safe-point code swap semantics and embedded `HotReloadError` remain
  unchanged for hosts that need full internal inspection.

## 2026-05-25: Variant Checks Diagnose Registered Unknown Variants

Status: Accepted

Context:
M12 requires candidate hints for unknown variant names. `reflect.variants`
returned copied metadata, but `reflect.variant_is(value, name)` treated every
misspelled registered variant name as a plain `false`, which made admin/debug
scripts silently hide typos.

Decision:
When the target enum type is registered, validate the queried name against the
registered variant descriptors before comparing it to the value's active
variant. Unknown names now return `ReflectErrorKind::UnknownVariant` with
ranked candidates. If no enum schema is registered, preserve the old dynamic
comparison behavior.

Consequences:
- Reflection tooling gets the same typo-help behavior for variants that fields,
  methods, modules, and functions already expose.
- Existing unregistered dynamic enum values can still be compared by name.
- The change remains schema-safe: reflection only reads registered metadata and
  does not mutate type structure.

## 2026-05-25: Module Exports Respect Function Reflection Policy

Status: Accepted

Context:
`reflect.function` enforced function visibility, private access, and
function-specific permissions, but `reflect.module` and `reflect.exports`
returned every registered export name. That allowed script-visible module
metadata to reveal hidden, private, or unapproved function names even though
direct function metadata access would be denied.

Decision:
Keep raw `module` and `exports` helpers for trusted host-side registry
inspection, and add policy-aware module/export helpers for the VM reflection
natives. Script-visible `reflect.module` and `reflect.exports` now include only
function exports allowed by `ReflectPolicy::require_function_access`.

Consequences:
- Gameplay reflection policies no longer leak inaccessible function names
  through module export metadata.
- Admin/debug policies can still reveal private or permissioned exports by
  granting `AccessPrivate` and the relevant function permissions.
- Registry metadata remains immutable and schema-safe; the policy only filters
  copied records and arrays returned to scripts.

## 2026-05-25: Method Metadata Respects Method Reflection Policy

Status: Accepted

Context:
`reflect.call` enforced `MethodAccess`, private access, and method-specific
permissions, but `reflect.methods` and `reflect.has_method` still exposed raw
method names and metadata. That allowed gameplay policies to discover hidden,
private, or unapproved method names even though calls would be rejected.

Decision:
Keep raw member helpers for trusted host-side inspection, and add policy-aware
method metadata helpers for VM reflection natives. Script-visible
`reflect.methods` and `reflect.has_method` now include only methods accepted by
`ReflectPolicy::require_method_access`.

Consequences:
- Gameplay reflection policies can enumerate only callable, public, approved
  methods.
- Admin/debug policies can reveal private or permissioned methods by granting
  `AccessPrivate` and the relevant method permissions.
- Reflection remains schema-safe because the policy filters copied metadata and
  never mutates registered type structure.

## 2026-05-25: Field Metadata Respects FieldAccess Readability

Status: Accepted

Context:
`reflect.get` rejected host fields marked `reflect_readable = false`, but
`reflect.fields`, `reflect.field`, and `reflect.has_field` still exposed those
field names and metadata. That allowed gameplay reflection policies to discover
fields that controlled reads would deny.

Decision:
Keep raw field metadata helpers for trusted host-side inspection, and add
policy-aware field helpers for VM reflection natives. Script-visible
`reflect.fields` and `reflect.has_field` include only `reflect_readable`
fields, and `reflect.field` returns `FieldNotReflectReadable` for hidden field
metadata requests.

Consequences:
- Gameplay reflection policies no longer leak hidden host field names through
  metadata enumeration.
- Admin/debug tooling can still inspect raw registry field metadata from host
  code.
- Reflection remains schema-safe because the policy filters copied metadata and
  never mutates registered field descriptors.

## 2026-05-25: Hot Reload Reports Expose Render Lines

Status: Accepted

Context:
Hot-reload reports carried structured diagnostics, ABI detail records, source
diagnostics, labels, and hints, but hosts still had to assemble those fields
into display rows themselves. The CLI demo also fell back to debug-formatting
raw errors on rejection, which was not a useful admin/debug rendering boundary.

Decision:
Add a focused `report_render` module with `HotReloadReportLine` and
`HotReloadReportLineKind`. `HotReloadReport::render_lines` returns categorized
summary, changed-function, diagnostic, ABI-detail, repair-hint,
source-diagnostic, and source-label rows with optional diagnostic indexes and
source spans. The CLI hot-reload demo now prints these lines and uses them for
rejection messages.

Consequences:
- Embedders can render reload reports without parsing reasons or matching
  internal error variants.
- UIs can group lines by kind and diagnostic index while retaining source spans
  for compile labels.
- The core report and diagnostic data remains unchanged, and rendering stays in
  a separate module instead of expanding the crate root or runtime code.

## 2026-05-25: Reflect Implements Diagnoses Unknown Traits

Status: Accepted

Context:
M12 reflection diagnostics already reported candidates for unknown fields,
methods, variants, modules, and functions. `reflect.implements(target, name)`
still returned `false` for misspelled trait names, making typos
indistinguishable from valid traits that a target simply does not implement.

Decision:
Add `ReflectErrorKind::UnknownTrait` and validate the queried trait name
against trait metadata known to the `TypeRegistry`, including explicitly
registered traits and trait descriptors embedded on registered types. Known
traits that are not implemented by the target continue to return `false`.

Consequences:
- Admin/debug scripts get typo diagnostics for trait checks instead of silent
  negative results.
- Negative capability checks remain possible when the trait is known to the
  registry.
- Reflection still only reads registered metadata and does not mutate type or
  trait structure at runtime.

## 2026-05-25: Variant Metadata Respects FieldAccess Readability

Status: Accepted

Context:
`reflect.fields`, `reflect.field`, and `reflect.has_field` already hid fields
whose `FieldAccess::reflect_readable` flag is false. `reflect.variants` still
returned raw payload field metadata for each enum variant, which let scripts
discover hidden variant fields through a different metadata path.

Decision:
Keep raw `variants` metadata for trusted host-side inspection and add a
policy-aware variant metadata helper for VM reflection natives. Script-visible
`reflect.variants` now filters each variant's `fields` array to include only
fields marked reflect-readable.

Consequences:
- Gameplay policies no longer leak hidden enum payload field names through
  variant metadata.
- Admin/debug host code can still use the raw helper when it needs full schema
  inspection.
- Reflection remains schema-safe because the policy filters copied metadata and
  does not mutate registered variant descriptors.

## 2026-05-25: Trait Definitions Are Queryable By Name

Status: Accepted

Context:
`TypeRegistry` could register trait descriptors and `reflect.traits(value)`
could report traits implemented by a target, but scripts had no direct way to
inspect a registered trait definition by name. A native named `reflect.trait`
would be natural but is not script-callable because `trait` is a reserved
keyword path segment.

Decision:
Add a Rust reflection helper exported as `trait_metadata_by_name` and a
script-visible native named `reflect.trait_info(name)`. The lookup returns a
copied `ReflectTrait` record and reuses `UnknownTrait` ranked candidate
diagnostics for misspelled names. It can find explicitly registered trait
definitions and trait descriptors embedded on registered types.

Consequences:
- Admin/debug scripts can inspect trait methods, docs, and attributes without a
  target value.
- `reflect.traits(value)` remains the target-capability query, while
  `reflect.trait_info(name)` is the descriptor lookup.
- The API remains schema-safe because it only copies registered metadata and
  does not permit runtime trait mutation.

## 2026-05-25: Type Descriptors Are Queryable By Name

Status: Accepted

Context:
Reflection could identify a value's type with `reflect.type_of(value)` and
query field or method metadata through a target value, but admin/debug scripts
could not inspect a registered type descriptor by name. M12 expects
TypeRegistry coverage for types as well as modules, functions, members, traits,
variants, attributes, and permissions.

Decision:
Add a focused `vela_reflect::types` module with copied type descriptor records.
Rust callers can use `type_metadata_by_name` and `type_metadata_names`, while
scripts can use `reflect.type_info(name)` and `reflect.types()`. Type records
include stable ID, name, kind, optional schema hash, docs, attrs, and member
counts. Unknown names report `UnknownTypeName` with ranked candidates.

Consequences:
- Admin/debug scripts can inspect registered schemas without needing a live
  host object or script value instance.
- The descriptor is intentionally copied summary data; detailed fields,
  methods, traits, and variants remain behind their existing policy-aware
  reflection calls.
- Runtime schema mutation remains unavailable.

## 2026-05-25: Reflective Calls Require Effect Permissions

Status: Accepted

Context:
Reflected host methods already carried `MethodEffectSet` metadata, and
`reflect.call` enforced `MethodAccess` plus method-specific permissions before
recording a `PatchTx` method-call patch. The effect bits were still only
informational, so a policy that allowed method calls could invoke host-reading,
host-writing, or event-emitting methods without explicitly approving those
declared side effects.

Decision:
Add effect-specific reflection permissions for host-read, host-write, and
event-emitting methods. `ReflectPolicy::require_method_access` now checks the
method's `MethodEffectSet` after the existing callable/private/specific
permission checks, and rejects missing effect grants with a structured
`MethodEffectPermissionDenied` error before any patch is recorded.

Consequences:
- Gameplay policies can allow pure reflective calls while separately gating
  host reads, host writes, and event emission.
- `ReflectPermissionSet::all()` remains an admin/test policy and includes the
  new effect permissions.
- Host mutation still enters only through `PatchTx`; effect enforcement happens
  before patch creation.

## 2026-05-25: Reflection Registry Metadata Lives Outside The Crate Root

Status: Accepted

Context:
M12 reflection work has expanded the `vela_reflect` crate root with type
descriptors, registry storage, reflection errors, value access helpers, and
tests. Keeping all of that logic in one file makes future permission and
metadata work harder to review and conflicts with the repository's modularity
constraint.

Decision:
Move reflection error definitions into `error.rs` and registry/descriptor
metadata into `registry.rs`. Keep the root `lib.rs` as the public re-export
surface plus the value access and reflective get/set/call API for now.

Consequences:
- Existing callers keep using `vela_reflect::{TypeRegistry, TypeDesc,
  ReflectError, ...}` through stable root re-exports.
- Future registry metadata changes have a focused module boundary.
- Runtime schema mutation remains unavailable; this is a structural refactor,
  not a behavior change.

## 2026-05-25: Reflection Permissions Are Queryable Metadata

Status: Accepted

Context:
M12 requires reflection to cover permissions as well as types, modules,
functions, fields, methods, traits, variants, and attributes. Before this
decision, `ReflectPolicy` enforced reflection permissions and member-specific
permissions, but scripts could not inspect the active reflection permission set
for admin/debug tooling.

Decision:
Expose read-only permission metadata through `permission_names` and
`has_permission` helpers plus script-visible `reflect.permissions()` and
`reflect.has_permission(name)`. These queries require `ReadTypeInfo`, consume
the reflection lookup budget, and validate unknown permission names with ranked
candidates.

Consequences:
- Scripts can branch on the active reflection policy without gaining new write
  or call capability.
- Permission metadata is copied string data; policies and registered schema
  structure remain immutable from script code.
- Unknown permission names are diagnosed consistently with other reflection
  lookup failures.

## 2026-05-25: Reflection Descriptors Carry Source Spans

Status: Accepted

Context:
Runtime reflection errors already return ranked unknown-name candidates, and
HIR diagnostics preserve declaration spans for compile-time schema errors. M12
also expects reflection diagnostics/tooling to include related schema
locations. Reflection descriptors did not carry declaration spans, so scripts
and host tooling could inspect metadata but could not map reflected script
schemas back to source.

Decision:
Add optional `source_span` fields to reflected top-level schema descriptors:
types, traits, functions, and modules. Script registration populates these
fields from HIR declaration spans, and copied reflection records expose them as
`ReflectSourceSpan { source, start, end }` records or `null` for host-provided
metadata without a source location.

Consequences:
- Admin/debug tooling can navigate reflected script schemas back to source
  declarations.
- Host-registered schemas remain supported by leaving `source_span` unset or
  setting it explicitly through builder methods.
- This remains read-only copied metadata and does not allow runtime schema
  mutation.

## 2026-05-25: Reflection Unknown Lookups Carry Related Candidates

Status: Accepted

Context:
M12 requires unknown-name diagnostics to include ranked candidates and related
schema spans. Reflection errors already carried ranked candidate names, and
top-level reflected descriptors now carry optional source spans, but the error
payload did not connect those two pieces for host tooling.

Decision:
Add a focused reflection candidate helper module and expose
`ReflectCandidate { name, source_span }` as copied diagnostic metadata. Unknown
type, trait, module, and function reflection errors keep their existing
`candidates: Vec<String>` compatibility field and add `related:
Vec<ReflectCandidate>` with matching ranking and optional descriptor source
spans.

Consequences:
- Existing candidate-name consumers can continue reading `candidates`.
- Admin/debug tooling can navigate top-level unknown lookups to nearby schema
  declarations when spans are known.
- Member-level candidate spans need a separate syntax/HIR span propagation
  decision before they can be reported accurately.

## 2026-05-25: Reflection Member Metadata Carries Source Spans

Status: Accepted

Context:
Top-level reflected descriptors carry source spans, but M12 diagnostics also
expect unknown field, method, and variant lookups to report related schema
locations. Struct fields, enum variants, tuple/record variant fields, and trait
methods did not preserve their parsed spans through HIR, so reflection could
not report accurate member locations.

Decision:
Store source spans on syntax and HIR member metadata, then copy those spans into
`FieldDesc`, `MethodDesc`, `TraitMethodDesc`, and `VariantDesc`. Reflected
member records expose `source_span`, and unknown reflected field, method, and
variant errors add related candidate records with optional spans while keeping
the existing candidate-name list.

Consequences:
- Script-defined member metadata can be used by admin/debug tools for
  navigation and typo repair.
- Host descriptors remain valid without source spans and can opt in through
  builder methods.
- Reflection still exposes copied metadata only; schema structure remains
  immutable at runtime.

## 2026-05-25: Field Reflection Exposes Copied Type Hints

Status: Accepted

Context:
M12 reflection includes `TypeHint` metadata. Function and module reflection
already copied function parameter and return hint strings, and HIR already
preserved field type hints for schema hashes. Reflected field records still
dropped that hint, so tools could inspect a field name and access policy but
not its declared value shape.

Decision:
Add an optional copied `type_hint` string to `FieldDesc`. Script struct fields
and enum payload fields populate it from HIR hints, host descriptors can opt in
through a builder, and copied `ReflectField` records expose the value as
`type`, using `null` when no hint is known.

Consequences:
- Admin/debug tooling can display field value hints consistently with function
  parameter metadata.
- This remains documentation/tooling metadata; it does not add script generics
  or static enforcement.
- Unhinted dynamic fields and host schemas without hints remain valid.

## 2026-05-25: Method Reflection Exposes Copied Signatures

Status: Accepted

Context:
M12 reflection includes `TypeHint` metadata for functions and methods. Function
reflection already exposed copied parameter and return hints, but reflected
host methods and script trait methods only exposed IDs, names, effects, access,
docs, attrs, and source spans. That left admin/debug tooling unable to present
method call shapes without out-of-band host knowledge.

Decision:
Add copied method parameter descriptors and optional return hints to
`MethodDesc` and `TraitMethodDesc`. Host native method registration populates
them from `NativeMethodDesc`, and script trait registration populates them from
HIR signatures. Copied reflection records expose `params`, `return`, and
`returns`; `returns` is a script-accessible alias because `return` is a
keyword.

Consequences:
- Tooling can display method signatures consistently with function signatures.
- Signature metadata remains copied read-only data and does not change runtime
  dispatch or type enforcement.
- The no-generics rule remains intact because hints are copied display strings.
