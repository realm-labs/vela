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
