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
