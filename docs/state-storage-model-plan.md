# State Storage Model Architecture Plan

> **Track:** contextual `state` declarations, explicit VM/host ownership,
> restricted initialization, and hot-reload state compatibility
> **Document status:** Complete; Batches A-G accepted on 2026-07-17
> **Baseline:** second post-implementation review of `master` at `8a84bbec` on
> 2026-07-15
> **Execution style:** hard-switch the pre-release language and runtime in
> coherent batches. Intermediate edits inside a batch may be red, but every
> batch checkpoint and the final acceptance boundary must be green.

This plan replaces the overloaded `global` declaration with an explicit state
model:

```vela
state cache: Cache = Cache { hits: 0 };
pub state metrics: Metrics = Metrics::default();

extern state world: World;
pub extern state player_store: PlayerStore;
```

`pub` controls module visibility only. `state` declares mutable VM-owned state
that persists across calls in one `Runtime`. `extern state` declares a
host-provided persistent root backed by `HostRef`; all reads, writes, paths,
and calls still pass through the existing `HostPath`, `PathProxy`, and
`HostAccess` contracts.

The change is an ownership and hot-reload ABI redesign, not a keyword-only
rename.

The first implementation and Batch F passed their listed acceptance gates. The
second 2026-07-15 post-implementation review found five uncovered graph,
identity, and lifetime boundaries; Batch G closes all five with focused
regressions and the full acceptance matrix.

---

## 0. Codex Goal

Use this prompt to execute the plan:

```text
/goal Execute docs/state-storage-model-plan.md in full.

This is one persistent, multi-turn implementation goal. Continue across tasks,
turns, and commits until every batch checkpoint and every final completion
criterion in the document is satisfied. Completing the parser rename, one
runtime store, one initializer test, or one hot-reload case is progress only
and is not a valid stopping condition.

Implement the long-term state architecture directly. Hard-switch the
pre-release source language, semantic model, bytecode, runtime APIs, tooling,
examples, tests, and active documentation from the overloaded `global` model
to `state` and `extern state`. Do not retain parser aliases, duplicate runtime
APIs, legacy bytecode paths, or script-global/host-global fallback dispatch.

Preserve these decisions throughout execution:

1. `state` is a contextual keyword. The lexer emits it as IDENT. The parser
   recognizes it as a declaration introducer only at module-item head after
   attributes and optional `pub`, or after `extern`. It remains legal as a
   parameter, local, field, function, module member, and state declaration
   name.
2. `extern` is a reserved declaration modifier. `pub extern state` is the
   supported modifier order.
3. `state name: Type = expression;` requires an explicit type and initializer,
   owns a mutable per-Runtime VM state cell, and supports direct assignment and
   compound assignment.
4. `extern state name: Type;` requires an explicit type, forbids an
   initializer, and owns no script value. Its root binding is immutable from
   Vela; nested mutation remains write-through HostAccess.
5. `pub` affects name resolution and export ABI only. It never selects VM
   versus host storage and never determines whether a value is preserved.
6. Initializers run once for every new Runtime and only for newly added VM
   states during hot reload. A compatible existing state keeps its exact old
   value and does not rerun its initializer.
7. State identity uses stable StateId values. Dense slots are generation-local
   operands and must never be used as cross-generation identity.
8. The first state ABI accepts only exact normalized type contracts. Do not
   claim structural compatibility or migrate heap layouts until a separate
   explicit migration design exists.
9. Added-state initialization is budgeted, effect-restricted, and
   transactional from the script-visible perspective. Failure must leave the
   old image and old state map active.
10. Old frames, suspended async executions, closures, and retained values keep
    their creation generation. Removed state remains available to an old
    generation until no retained owner can address it.

Execute the batches in order:

- Batch A: contract, contextual syntax, AST, grammar, and stable identity.
- Batch B: HIR/MIR/bytecode ownership split and VM-state assignment.
- Batch C: persistent stores, restricted initialization, and embedding API
  hard switch.
- Batch D: state ABI diff, transactional reload, and generation lifetime.
- Batch E: tooling, reflection, examples, documentation, audits, and full
  acceptance.
- Batch F: post-implementation contract, budget, reload-ABI, and lifetime
  closure.
- Batch G: exact nominal embedding, graph-preserving staging, external-owner
  generation reclamation, and nested initializer-call closure.

At the start of every turn, follow AGENTS.md: read docs/goal.md,
docs/architecture.md, and docs/progress.md; inspect the current diff; and run
or inspect the most relevant failing test. Work on the smallest verifiable
piece inside the active batch. Keep changes modular, update durable decisions
when they become implemented truth, and commit coherent verified checkpoints
with Conventional Commits.

Never mark this goal complete while any of the following is true:

- `state` is globally reserved or normal identifiers named `state` fail;
- legacy `global` source is still accepted;
- one declaration can still resolve dynamically to either a VM value or a
  host global at runtime;
- VM state cannot store a scalar result back into its state cell;
- Vela can replace an `extern state` root directly;
- a state initializer can perform host, native, provider, reflection, IO,
  time, random, event, or async effects;
- existing compatible state reruns its initializer during reload;
- new-state initializer failure can partially publish a version or state map;
- ownership changes or incompatible state type changes are accepted;
- dense state slots are reused across generations without an owner-qualified
  mapping;
- an old frame or suspended execution can read the wrong state after a layout
  reorder, addition, or removal;
- host state enters the script GC or a real Rust reference reaches Vela;
- old GlobalId, GlobalSlot, LoadGlobal, RuntimeGlobalStore,
  ScriptGlobalValues, insert_global, set_global, global_as, or equivalent
  production compatibility surfaces remain;
- active architecture, grammar, decisions, examples, reflection, formatter,
  language service, or editor highlighting still describe `global` as the
  current language model;
- required focused tests, full workspace validation, examples, audits, or
  documentation updates have not passed;
- any Batch F or Batch G review item or its focused regression test remains
  open;
- the completed work is uncommitted or the final worktree is dirty.

Do not report blocked merely because the hard switch is broad. Report blocked
only when the same external decision prevents meaningful progress after the
repository-local alternatives have been exhausted.
```

---

## 1. Target Outcome

### 1.1 Source Surface

The complete declaration surface is:

```vela
state session: Session = Session {
    requests: 0,
};

pub state metrics: Metrics = Metrics::default();

extern state world: World;
pub extern state services: Services;
```

The following remains legal because `state` is contextual:

```vela
state state: Counter = Counter { value: 0 };

fn update(state: Counter) {
    let previous = state.value;
    state.value = previous + 1;
}
```

Legacy source is rejected with a focused repair diagnostic:

```vela
global cache: Cache;
// error: `global` declarations were removed; use `state` with an initializer
//        or `extern state` for a host-provided root
```

`global` becomes an ordinary identifier outside that targeted invalid
module-item shape. It is not kept as a reserved word or parser alias.

### 1.2 Ownership Summary

| Declaration | Owner | Root mutability in Vela | Initialization | GC treatment |
|---|---|---|---|---|
| `state x: T = expr;` | one `Runtime` | assignable | restricted Vela initializer | ordinary persistent VM root |
| `extern state x: T;` | Rust host | binding is not assignable; nested paths follow HostAccess permissions | host binding only | opaque `HostRef`, never traced as Rust-owned memory |

State is scoped to one Runtime instance. It is not process-global, shared
between runtimes created from the same image, durable on disk, or automatically
restored after process restart.

### 1.3 Visibility

`pub` is orthogonal to storage:

```text
private state        private VM-owned state
pub state            exported VM-owned state
private extern state private host binding
pub extern state     exported host binding
```

Visibility participates in module name resolution and exported-module ABI.
It does not participate in state preservation, initializer selection, or host
binding lookup.

---

## 2. Reviewed Baseline

The current implementation has useful pieces, but they do not yet form the
target contract:

1. The grammar reserves `global`, and `global` declarations have a required
   type but no initializer.
2. `GlobalMetadata` carries only a type hint. It does not record VM/host
   ownership or an initializer body.
3. `LoadGlobal` checks the script-global map first and falls back to the host
   adapter. Ownership is selected dynamically by which store contains a name.
4. `RuntimeGlobalStore` and `ScriptGlobalValues` both consume the same global
   name/slot layout.
5. Script globals are currently inserted from Rust. The VM has `LoadGlobal`
   but no production state-store instruction, so scalar state cannot be
   updated directly by Vela.
6. Layout rebinding preserves script values by qualified name. This proves the
   basic preservation mechanism, but there is no state ABI comparison,
   initializer staging, or explicit removal lifetime.
7. `ProgramVersion` exposes global names, not state descriptors. Hot-reload
   checks cover functions, module exports, packages, and schemas but not state
   ownership/type compatibility.
8. Rebinding one current slot vector is insufficient for old frames and
   suspended executions: an old dense slot can address a different declaration
   after the new layout is installed.
9. Current architecture and decisions intentionally forbid module-level
   mutable initialization and describe both ownership models through the same
   `global` surface. Those contracts must be replaced, not patched around.

Reviewed baseline proof:

```text
cargo test -p vela_syntax parser_parse_source_structures_use_const_and_global_items
```

passed at the plan baseline. It is a migration input and must be replaced by
contextual-state coverage during Batch A.

---

## 3. Contextual Keyword And Grammar Contract

### 3.1 Lexical Rule

`state` must not be added to `Keyword::from_text` or the grammar's globally
reserved keyword list. The lexer emits:

```text
"state" -> IDENT("state")
```

The parser recognizes that token contextually only when all of these are true:

- parsing a module item after attributes and optional `pub`; or
- parsing the required token after `extern` in the same item header; and
- the significant token text is exactly `state`.

The CST token may remain `Ident`. Keyword highlighting is derived from the
token's role inside `StateItem`, not from a globally lexed `StateKw` token.
This avoids teaching every identifier parser to special-case a reserved token.

`extern` is a true reserved keyword and receives an `ExternKw` syntax kind.
Only `extern state` is enabled by this plan; `extern fn`, extern types, and a
general FFI surface remain out of scope.

### 3.2 Grammar

The grammar target is conceptually:

```ebnf
contextual_state_kw = IDENT("state") ;

item_kind           = use_decl
                    | const_decl
                    | state_decl
                    | extern_state_decl
                    | function_decl
                    | struct_decl
                    | enum_decl
                    | trait_decl
                    | impl_decl ;

state_decl          = contextual_state_kw, ws, IDENT, ws,
                      type_annotation, ws, "=", ws, expr, item_term ;

extern_state_decl   = "extern", ws, contextual_state_kw, ws, IDENT, ws,
                      type_annotation, ws, item_term ;
```

The accepted modifier order is:

```text
attributes -> pub -> extern -> state -> name -> type -> initializer/terminator
```

`extern pub state`, `state extern`, and duplicate modifiers are syntax errors.

### 3.3 Required Diagnostics

The parser/HIR boundary must provide focused diagnostics for:

- missing state name;
- missing explicit type annotation;
- missing VM-state initializer;
- initializer attached to `extern state`;
- `extern` not followed by contextual `state`;
- legacy module-level `global name: Type;` with a two-choice repair hint;
- invalid modifier order;
- duplicate state declarations and visibility violations through the normal
  module resolver.

### 3.4 AST And Formatter

Replace `SyntaxGlobalItem` and `globals()` with `SyntaxStateItem` and
`states()`. The state AST exposes:

```text
attributes
visibility
extern token/storage kind
contextual state introducer token
name token
type hint
initializer expression when VM-owned
source spans for the declaration and initializer
```

Do not preserve old AST aliases. Formatter tests must prove both declaration
forms are deterministic and that ordinary identifiers named `state` format as
ordinary identifiers.

---

## 4. Language Semantics

### 4.1 VM-Owned State

`state` declares a mutable cell owned by one Runtime:

```vela
state counter: i64 = 0;

fn next() {
    counter += 1;
    return counter;
}
```

Direct assignment, compound assignment, and read-modify-write operate on the
cell. Values stored in the cell use the same runtime type guards as other typed
boundaries. Aggregates remain ordinary VM heap values, and their nested
mutation follows the existing script-value rules.

Two runtimes instantiated from one immutable image initialize and mutate
independent VM state cells.

### 4.2 Extern State

`extern state` declares a required host contract:

```vela
extern state world: World;

fn level_up() {
    world.player.level += 1;
}
```

Loading the declaration produces the bound opaque `HostRef`. Field/index/key
paths, reads, writes, mutations, removals, and methods use the existing
resolved HostTargetPlan and HostAccess path. The following is invalid:

```vela
world = replacement;
// error: an extern-state root is host-bound and cannot be reassigned by Vela
```

The host may explicitly replace a binding through the embedding API at a safe
boundary. Vela never receives a Rust reference and does not own, trace, or
drop the Rust object through script GC.

### 4.3 Name Resolution

State declarations remain ordinary module declarations with stable
package/module/name identity. Imports and `pub` work exactly like functions,
consts, and types. A local or parameter shadows an accessible state through the
normal binding rules; contextual-keyword recognition must not interfere with
that resolution.

---

## 5. State Identity And Executable Metadata

### 5.1 Stable Identity

Hard-switch `GlobalId` to `StateId` and derive it from the canonical
`PackageId + ModulePath + declaration name` definition path. Rename helpers
such as `script_global_id` and `script_global_path` to their state equivalents.
Do not keep type aliases.

A declaration rename is a remove plus add. Automatic name-based migration and
an explicit user-supplied stable-ID attribute are not part of this track.

### 5.2 Descriptor

Every linked artifact carries an ordered state descriptor table with at least:

```rust
pub struct StateDescriptor {
    pub id: StateId,
    pub qualified_name: String,
    pub visibility: Visibility,
    pub storage: StateStorage,
    pub type_contract: TypeContract,
    pub initializer: Option<ScriptFunctionHandle>,
    pub source_span: Option<Span>,
}

pub enum StateStorage {
    Vm,
    Extern,
}
```

The concrete ownership of strings/spans may follow existing artifact metadata
conventions, but equivalent information must be available to the linker,
runtime, hot-reload ABI checker, diagnostics, reflection, and tooling.

VM descriptors require an initializer handle. Extern descriptors forbid one.
The verifier rejects inconsistent descriptors even if malformed bytecode is
constructed without the source compiler.

### 5.3 Generation-Local Slots

Dense `StateSlot` values are executable-generation operands:

```text
old generation StateSlot -> old generation StateId
new generation StateSlot -> new generation StateId
StateId -> per-Runtime state cell or extern binding
```

The slot-to-ID table belongs to the immutable linked generation. Runtime state
is keyed by StateId. Inline-cache state is generation-owned and may cache a
validated slot/ID pairing, but it cannot reinterpret an old slot through the
new layout.

---

## 6. HIR, MIR, Bytecode, And Verification

### 6.1 HIR

Replace `GlobalMetadata` with state metadata that records:

```text
storage kind
explicit type hint and normalized contract
initializer body ID and span for VM state
visibility and declaration attributes
stable StateId
```

Initializer bodies enter the ordinary binding/effect/type-fact pipeline. They
are not reparsed by the bytecode compiler and do not bypass Heavy HIR.

### 6.2 MIR

Replace the generic global-read operation with explicit state operations:

```text
ReadVmState(StateId)
WriteVmState(StateId, value)
ReadExternState(StateId)
```

There is no `WriteExternState`. Compound assignment to VM state lowers through
an ordinary read/operation/write sequence. Nested extern mutation continues to
lower through HostAccess operations after `ReadExternState` obtains the root.

### 6.3 Linked Bytecode

Use statically owned operations, for example:

```text
LoadState
StoreState
LoadExternState
```

Exact names may follow the existing instruction naming style, but one opcode
must never search both stores. Remove `LoadGlobal` and rename global cache
kinds, operands, debug labels, verifier branches, profiler labels, and test
helpers to state terminology.

The verifier proves:

- the slot belongs to the instruction's executable generation;
- load/store storage kind matches the descriptor;
- `StoreState` targets only VM state;
- initializer handles target restricted initializer bodies;
- type-guard operands exist where dynamic values cross the declared contract.

---

## 7. Restricted Initializers

### 7.1 When They Run

For a newly constructed Runtime, every VM state is new and every initializer
runs exactly once. Shared Runtime images do not share initialized VM values.

During hot reload:

- compatible existing VM state keeps its current value;
- its initializer does not run, even if initializer source changed;
- only newly added VM state runs an initializer for that Runtime;
- a Runtime created after the reload runs the current initializer for every VM
  state because all states are new to that Runtime.

Initializer-only changes must be included in the reload report with wording
that makes the existing/new Runtime distinction clear.

### 7.2 Allowed Surface

An initializer may:

- use literals and module constants;
- construct script records, enums, tuples, arrays, maps, sets, strings, bytes,
  Option, and Result values;
- use deterministic operators and control-flow expressions;
- call synchronous script functions proven by analysis to have no disallowed
  effects;
- allocate in the persistent script heap under an initializer memory budget.

An initializer may not:

- read or write any VM state;
- read any extern state;
- call native, host, provider, or reflective targets;
- use HostAccess, IO, filesystem, event, logging, time, or random capabilities;
- declare or await async work;
- observe runtime identity, current generation, or initialization order.

Forbidding state reads avoids dependency ordering and initialization cycles in
the first implementation. Because permitted initializers are unobservable
except for their returned value, they execute in deterministic StateId order.

### 7.3 Budgets And Failure

Initialization has explicit execution-unit, allocation-byte, and call-depth
limits. It must not use `ExecutionBudget::unbounded()` as the production
default. Engine/runtime configuration supplies bounded defaults and may allow
the host to tighten them.

Runtime creation becomes fallible. No public constructor may silently panic or
leave a Runtime with partially initialized VM state. Public construction APIs
must return a structured error covering linking, initializer diagnostics,
budget exhaustion, allocation failure, and result contract mismatch. If a
builder is introduced for extern bindings, ordinary no-binding construction
must use the same initialization path rather than a second implementation.

During reload, initializer results are staged outside the published state map.
All new values must pass their declared type guard before commit. If any
initializer fails:

```text
active ProgramVersion unchanged
active state map unchanged
new state roots discarded
unreachable staging allocations collectible
structured reload rejection reported with state name and source span
```

No host-visible rollback journal is needed because restricted initializers
cannot produce host effects.

---

## 8. Runtime Stores And Embedding API

### 8.1 Store Split

Replace the current same-layout stores with explicit ownership:

```text
RuntimeVmStateStore
  StateId -> Value
  persistent ScriptHeap
  generation-aware roots

RuntimeExternStateBindings
  StateId -> HostRef/binding metadata
  host object ownership remains outside script GC
```

The VM state store traces active values plus values retained for live old
generations. The extern binding store never traces Rust objects as script-owned
heap state.

### 8.2 Rust API

Remove the overloaded global API and replace it with storage-specific names.
The target surface is conceptually:

```rust
runtime.state("main::counter")
runtime.state_as::<T>("main::counter")
runtime.set_state("main::counter", value)
runtime.update_state("main::counter", update)

builder.bind_extern_state("main::world", world)
runtime.replace_extern_state("main::world", world)
runtime.extern_state_ref("main::world")
```

`set_state` updates an already declared and initialized VM state after
validating storage kind and type contract. There is no ordinary `insert_state`
because declaration plus initializer creates the cell. A separate explicit
snapshot/restore design may later add bulk restoration; it is not implied by
this API.

Extern binding accepts only supported host-object/HostRef inputs and validates
the declared host type contract. Rebinding occurs only outside an active call
or through the runtime's existing safe ownership boundary.

Remove, without aliases:

```text
insert_global
set_global
global
global_as
update_global
insert_host_global
host_global_ref
IntoGlobalValue
RuntimeGlobalStore
RuntimeScriptGlobalStore
ScriptGlobalValues
```

Rename errors, serde helpers, C ABI entries, prelude exports, docs, and examples
to the new model. A missing extern binding and a missing VM state are distinct
structured errors.

---

## 9. Hot-Reload State ABI

### 9.1 Compatibility Key

Compare state declarations by stable StateId, not slot or source order. The
first implementation defines compatible type as exact equality of the
normalized `TypeContract`, including parameterized container contracts and
stable script/host type identity.

Existing schema rules that accept a defaulted field addition do not
automatically make an existing state value compatible. Until the runtime has a
separate proven state-value migration/default materialization mechanism, any
layout-affecting contract change is rejected for preserved state.

### 9.2 Compatibility Matrix

| Change | Result |
|---|---|
| same StateId, same storage, exact type contract | preserve existing value/binding |
| initializer changed only | preserve existing value; do not run initializer; report new-Runtime-only effect |
| private to public | preserve state; evaluate as an export ABI addition |
| public to private | preserve decision is independent; module export ABI may reject |
| declaration/source order changed | preserve by StateId; generation slots may change |
| new VM state | run restricted initializer during per-Runtime staging |
| new extern state | require a type-compatible host binding before activation |
| VM state changed to extern or extern changed to VM | reject |
| type contract changed | reject in the first implementation |
| declaration renamed | remove old plus add new |
| state removed | hide from new generation; retain for live old generations, then collect/unbind |

### 9.3 Activation Transaction

At a Runtime safe point:

1. Compare old and new state descriptors.
2. Reject incompatible storage or type changes.
3. Resolve and validate bindings for every added extern state.
4. Run and validate all added VM-state initializers into staging roots.
5. Build the new generation-local slot-to-StateId map and fresh sidecars.
6. Atomically publish the new image, generation map, and new state cells.
7. Keep old-generation-only cells/bindings while any old owner is live.
8. Prune dead generation metadata and unreachable state at later safe points.

Compilation and static ABI comparison may be shared, but initializer execution
and extern binding validation are per Runtime. One Runtime's state must never be
used to initialize or validate another Runtime created from the same image.

### 9.4 Old Frames, Closures, And Async Suspension

Old executable owners must resolve their own state slots. Tests must cover:

- an old synchronous frame continuing after a state layout reorder;
- a retained old closure reading a state removed from the new image;
- a suspended old async execution resuming after added/removed/reordered state;
- compatible state shared by old and new generations through the same StateId
  cell;
- removed state becoming collectible only after the last old owner exits.

The plan does not migrate suspended frames. It preserves their old executable
owner and makes that owner's state references remain valid.

---

## 10. Reflection, Tooling, And Documentation

### 10.1 Reflection

Reflection state metadata reports at least:

```text
qualified name
visibility
VM or extern storage
declared type contract
initializer presence for VM state
source origin/span when available
```

Reflection may read values only through existing permission and ownership
rules. It cannot create declarations, change storage kind, mutate type
structure, or expose a Rust reference.

### 10.2 Language Service And LSP

The language service must:

- classify the contextual introducer token as a keyword only inside a state
  declaration;
- classify normal identifiers named `state` according to their actual symbol;
- complete `state` and `extern state` at valid module-item positions;
- provide hover information for ownership, persistence scope, visibility, and
  initializer behavior;
- navigate, reference, and rename state declarations through StateId-backed
  symbol ownership;
- report that renaming a state is remove-plus-add for hot reload;
- reject unsafe rename conflicts and preserve the existing prepared-rename
  ownership rules;
- format both declaration forms idempotently;
- update semantic-token, symbol, completion, hover, reference, rename, and
  diagnostic fixtures.

Thin editor syntax definitions and the documentation-site highlighter must
also treat contextual `state` correctly rather than globally coloring every
identifier with that text as a keyword.

### 10.3 Active Documentation

Update implemented truth in:

```text
docs/grammar.ebnf
docs/architecture/language.md
docs/architecture/host-and-registration.md
docs/architecture/runtime.md
docs/architecture/hot-reload.md
docs/architecture/lsp.md
docs/decisions.md
docs/validation.md when commands/examples change
docs/progress.md only when focus, status, or remaining gaps change
```

Archived historical plans and progress records may continue to mention the old
model. Do not rewrite archives as if the old implementation never existed.

---

## 11. Implementation Ownership Map

| Area | Primary ownership |
|---|---|
| contextual token recognition, CST, AST, formatter | `vela_syntax` |
| declaration identity | `vela_def`, `vela_common` |
| state metadata, bodies, binding, visibility | `vela_hir` |
| effects, type contracts, diagnostics, editor facts | `vela_analysis` |
| state read/write operations and verification input | `vela_mir` |
| descriptor table, slots, instructions, linker, verifier | `vela_bytecode` |
| execution, heap roots, state operations, initializer session | `vela_vm` |
| HostRef binding and HostAccess routing | `vela_host` |
| per-Runtime stores, construction, embedding APIs, reload staging | `vela_engine` |
| state ABI comparison and reports | `vela_hot_reload` |
| metadata queries and permissioned reflection | `vela_reflect` |
| static editor behavior | `vela_language_service`, `vela_lsp_server` |
| Rust derives/registration helpers | `vela_macros`, `vela_registry` |
| C-facing names when exposed | `vela_c_api` |
| examples and end-to-end proof | `examples`, workspace fixtures |

Do not concentrate the feature in `lib.rs`, the linked opcode loop, or one
oversized runtime module. Split syntax state items, runtime state stores,
initializer execution, and reload ABI comparison by responsibility.

---

## 12. Batch Execution Plan

### 12.1 Batch A: Contract, Contextual Syntax, And Identity

Tasks:

- add the durable design decision before production semantics change;
- update grammar to contextual `state` plus reserved `extern`;
- implement item-head contextual recognition without reserving `state`;
- replace global CST/AST nodes and parser entrypoints with state forms;
- require initializer/type for VM state and forbid extern initializer;
- add focused legacy repair diagnostics;
- hard-switch GlobalId/GlobalSlot naming where required by front-end metadata;
- migrate parser/AST/formatter fixtures and `.vela` sources enough for the
  workspace front end to be coherent;
- add explicit tests for identifiers named `state` in every relevant binding
  position.

Checkpoint:

```text
cargo fmt --all -- --check
cargo test -p vela_syntax
cargo test -p vela_hir
```

Batch A is not complete while `state` is lexed globally as a keyword or legacy
`global` source still parses successfully.

### 12.2 Batch B: Semantic Ownership And Executable State Operations

Tasks:

- add HIR state storage kind, initializer body, contract, and StateId metadata;
- bind and analyze initializer bodies through normal HIR/analysis inputs;
- add VM-state read/write and extern-state read MIR operations;
- lower direct and compound assignment to VM state;
- reject extern-root assignment before bytecode generation;
- replace global program metadata with state descriptors and generation slots;
- replace LoadGlobal with storage-specific linked instructions;
- update linker, verifier, cache policy, profiling labels, disassembly, and
  malformed-bytecode tests;
- remove dynamic dual-store lookup from the VM path.

Checkpoint:

```text
cargo fmt --all -- --check
cargo test -p vela_analysis
cargo test -p vela_mir
cargo test -p vela_bytecode
cargo test -p vela_vm
```

Batch B is not complete while any instruction can search both VM and host
state, or a scalar VM state cannot be assigned by Vela.

### 12.3 Batch C: Stores, Initialization, And Embedding Hard Switch

Tasks:

- introduce StateId-keyed VM and extern stores;
- retain VM values as persistent GC roots per Runtime;
- route extern reads exclusively to validated HostRef bindings;
- implement the restricted initializer effect policy;
- compile initializers to verified linked executable bodies;
- add bounded per-Runtime initializer execution and transactional publication;
- make Runtime construction fallible without partial initialization or panic;
- replace global embedding APIs, traits, serde helpers, errors, prelude exports,
  examples, and C ABI names with state-specific surfaces;
- prove independent state for multiple runtimes sharing one image;
- prove host state is not traced or owned by script GC.

Checkpoint:

```text
cargo fmt --all -- --check
cargo test -p vela_vm
cargo test -p vela_engine
cargo test -p vela_c_api
cargo test --manifest-path examples/Cargo.toml
```

Batch C is not complete while Rust must manually insert ordinary VM state,
initialization can be unbounded/effectful, or the old global API remains.

### 12.4 Batch D: Hot-Reload ABI And Generation Lifetime

Tasks:

- add state descriptors to ProgramVersion/HotUpdate comparison inputs;
- implement the exact-contract compatibility matrix;
- report ownership, type, added, removed, and initializer-only changes;
- validate added extern bindings per Runtime before activation;
- run added VM-state initializers in the safe-point staging transaction;
- publish image, state additions, generation map, and sidecars atomically;
- retain removed state for old frames, closures, values, and suspended async
  execution owners;
- prune old-only state after generation ownership dies;
- add rejection/rollback source spans and repair hints;
- benchmark or at least profile state-load/store and reload staging overhead so
  the change does not silently regress M20 hot paths.

Checkpoint:

```text
cargo fmt --all -- --check
cargo test -p vela_hot_reload
cargo test -p vela_engine source_reload
cargo test -p vela_engine runtime_rebinds_state_after_reload_image_swap
```

Use the actual migrated test names. Batch D is not complete while compatible
state preservation is still name-map coincidence or old slots can observe a
new layout.

### 12.5 Batch E: Tooling, Documentation, Audits, And Acceptance

Tasks:

- update reflection metadata and permissioned state inspection;
- update language-service semantic tokens, hover, navigation, references,
  rename risk, completion, diagnostics, formatting, and inlay behavior;
- update LSP protocol fixtures and thin editor syntax/highlighting;
- migrate all active examples, fixtures, site snippets, and API docs;
- update architecture, decisions, validation, and concise progress truth;
- classify archived references as historical rather than rewriting archives;
- run zero-hit audits for production global terminology and dual-store paths;
- run focused, workspace, feature, example, documentation, fuzz-build, and
  site gates required by the touched surfaces;
- commit the final coherent checkpoint and leave a clean worktree.

Checkpoint:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --no-fail-fast
cargo doc --workspace --all-features --no-deps
cargo check --manifest-path fuzz/Cargo.toml --bins
```

Run the documentation-site and editor-extension checks listed in
`docs/validation.md` when those files change.

### 12.6 Batch F: Post-Implementation Review Closure

Status: implementation and listed regression gates completed on 2026-07-15.
Batches A-E stay landed. The second review found deeper boundary cases outside
the Batch F matrix; Batch G below owns final acceptance without rolling back
these fixes or adding language scope.

Tasks, in execution order:

- [x] `STATE-F1-SET-CONTRACT`: make `set_state` and `update_state` resolve the
  linked `StateDescriptor` and validate the complete normalized
  `MirTypeContract`. Valid parameterized containers, tuples, Option/Result, and
  qualified script values must pass; malformed nested values and metadata-free
  bypasses must fail before replacement.
- [x] `STATE-F2-EXTERN-CONTRACT`: require every `extern state` descriptor to
  carry a `MirTypeContract::Host`. Reject primitive, script-owned, container,
  callable, and `Any` contracts in both source compilation and bytecode
  verification; runtime binding must never interpret a non-host contract as
  "no expected type".
- [x] `STATE-F3-INIT-BUDGET`: enforce one execution/allocation budget across the
  complete Runtime-construction or added-state reload transaction. Do not
  recreate the full allowance per declaration or use an unbounded budget when
  materializing staged values into the published heap.
- [x] `STATE-F4-EXPORT-ABI`: keep state preservation separate from visibility,
  while rejecting removal or visibility downgrade of an existing public state
  export. Private additions/removals and private-to-public additions continue
  to follow the documented compatibility matrix.
- [x] `STATE-F5-GENERATION-RECLAIM`: prune dead generation sidecars, removed VM
  roots, and removed extern bindings after the last old-generation owner dies,
  at a normal Runtime safe point without requiring another accepted reload.
- [x] `STATE-F6-INIT-FINGERPRINT`: include the transitive permitted script-call
  graph in initializer change detection so a changed pure helper reports the
  affected state as new-Runtime-only behavior.

Required regression proof:

- recursive embedding-contract tests cover valid and invalid Array/Map/Set,
  tuple, Option/Result, record, enum, and qualified-type values;
- compiler and verifier tests reject `extern state value: i64;` and malformed
  non-host descriptors before Runtime construction;
- two individually valid initializers that exceed the shared total budget fail
  construction/reload transactionally, and live-heap staging is charged;
- hot reload rejects public-state removal and public-to-private change while
  retaining the old image/state map;
- dropping the final retained old closure/value allows removed VM and extern
  state to be reclaimed without a second reload;
- changing only a pure helper called by an initializer preserves the existing
  Runtime value and reports the initializer impact for new runtimes.

Checkpoint:

```text
cargo fmt --all -- --check
cargo test -p vela_bytecode state
cargo test -p vela_engine state
cargo test -p vela_hot_reload state
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
```

Do not add structural state migration, initializer state reads, a second
initializer evaluator, non-host extern values, compatibility aliases, or a
permanent generation root to make these tests pass. Keep commits small and
coherent, preferably separating contract, reload, and lifetime work.

### 12.7 Batch G: Graph, Identity, And Lifetime Closure

Status: complete. These tasks were executed in order against the landed Batch
F baseline. Each task is a correctness closure, not permission to add state
migration, new initializer effects, or compatibility paths.

Tasks:

- [x] `STATE-G1-EXACT-TYPE-RESOLUTION`: make embedding type-name resolution
  prefer an exact canonical qualified name. A qualified input must never fall
  back to leaf-name matching; an unqualified spelling may resolve only when
  the existing embedding contract intentionally permits it and the linked
  generation has exactly one candidate. Reject namespace-spoofed names and do
  not reject `a::Player` merely because `b::Player` is also linked.
- [x] `STATE-G2-NOMINAL-CANONICALIZATION`: replace boolean-only record/enum
  validation followed by generic owned-value insertion with one linked-aware
  canonicalization boundary. Recursively validate record field contracts and
  enum variant payload contracts, reject unknown variants and malformed
  payloads, and materialize `RecordIdentity`/`EnumIdentity` from linked
  `TypeId`, `ShapeId`, and `VariantId`. A valid `set_state` or no-op
  `update_state` must retain normal pattern matching, field use, and runtime
  type-guard behavior.
- [x] `STATE-G3-GRAPH-PRESERVING-STAGING`: move newly initialized reload state
  from the staging heap to the live heap with a graph-aware, budgeted copier
  that preserves aliases and cycles. Do not flatten persistent values through
  an unbudgeted `OwnedValue` tree. Charge the same transaction budget before
  allocation, terminate on every graph, and accept the same valid cyclic state
  result that clean Runtime construction accepts. Failure must leave the old
  image, state map, and reachable live heap unchanged.
- [x] `STATE-G4-EXTERNAL-OWNER-RECLAIM`: make old-generation retention depend
  on owners reachable outside that generation's old-only state roots. A
  removed state containing an old closure must not keep its own sidecar,
  linked artifact, removed VM cells, or removed extern bindings alive forever.
  A genuinely external old frame, suspended execution, closure, iterator, or
  retained runtime value must continue to pin the generation until it is
  released. Reclamation still occurs at an ordinary safe point without a
  second accepted reload.
- [x] `STATE-G5-NESTED-INIT-FINGERPRINT`: include script calls inside nested
  code objects, including closure and parameter-default bodies represented
  there, in initializer change detection. Traverse only the initializer's
  permitted transitive call graph, terminate on recursive graphs, and avoid
  reporting unrelated helper changes.

Required regression proof:

- two linked script types with the same leaf name accept their exact qualified
  values, while a wrong qualified prefix and ambiguous unqualified spelling
  fail;
- record values with correct field names but wrong payload types and enum
  values with unknown variants or malformed payloads fail before replacement;
  valid `set_state` and no-op `update_state` values still pass linked type
  guards and enum pattern matching when consumed by Vela;
- adding cyclic and alias-rich VM state through hot reload preserves aliases
  and cycle topology under the shared transaction budget; an insufficient
  budget returns a structured initializer failure without stack overflow,
  exponential detached expansion, or partial publication;
- a removed closure-valued state with no external old-generation owner is
  reclaimed together with old-only VM and extern state, while an externally
  retained old closure keeps the same data alive until its final drop;
- changing only a helper called from a nested initializer closure reports the
  affected state, an unrelated nested helper remains absent, and nested
  recursive call graphs terminate deterministically.

Checkpoint:

```text
cargo fmt --all -- --check
cargo test -p vela_vm owned_contract
cargo test -p vela_engine state
cargo test -p vela_hot_reload state
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --no-fail-fast
```

Do not close Batch G by reserving more type-name aliases, stamping identities
without validating payloads, disabling cyclic script values or initializer
collection mutation, retaining every old generation permanently, waiting for
another reload to reclaim state, or hashing every program function as if it
were initializer-reachable. Keep the graph copier, nominal canonicalizer,
generation liveness, and initializer fingerprinting in their owning modules.

---

## 13. Required Test Matrix

### 13.1 Syntax And Resolution

- private/public VM state parses;
- private/public extern state parses;
- `state` remains valid as parameter, local, field, method, function, import
  alias, and declaration name;
- `state state: T = ...;` parses with distinct introducer/name roles;
- missing VM initializer and extern initializer are rejected;
- invalid modifier order is rejected;
- legacy global declaration reports the two replacement choices;
- imports respect `pub`; storage kind does not affect visibility;
- duplicate declarations and shadowing follow existing resolver rules.

### 13.2 Compiler And Verifier

- VM read lowers to LoadState;
- VM direct/compound assignment lowers to StoreState;
- extern read lowers only to LoadExternState;
- extern root assignment is a semantic error;
- malformed storage/opcode combinations fail verification;
- generation slots resolve to the correct StateId after reordering;
- disassembly and diagnostics retain qualified names without hot-path string
  lookup.

### 13.3 Runtime And GC

- scalar state persists across calls;
- aggregate state and nested mutation persist across calls;
- two runtimes from one image are isolated;
- host `set_state` validates storage and type contracts;
- extern HostRef reads/writes/calls use HostAccess;
- missing and mismatched extern bindings are structured errors;
- extern roots are never script-owned GC objects;
- live VM state survives collection and replaced/removed unreachable VM state
  is eventually reclaimed.

### 13.4 Initializers

- every VM state initializes once per new Runtime;
- initializers may construct all allowed managed value categories;
- state/extern reads, native/host/provider/reflection calls, capabilities, and
  async are rejected;
- execution, allocation, and depth budgets trap with source spans;
- result contract mismatch rejects Runtime construction/reload;
- a later initializer failure publishes none of the earlier staged states;
- initializer source change does not affect an existing Runtime;
- a Runtime created from the updated image uses the changed initializer.

### 13.5 Hot Reload

- exact-compatible VM state preserves the old value;
- exact-compatible extern state preserves its binding;
- added VM state initializes once per Runtime;
- added extern state requires a binding;
- storage-kind and type-contract changes reject;
- visibility changes are separated from preservation decisions;
- reorder does not change identity;
- rename behaves as remove plus add;
- initializer failure rolls back image and state publication;
- old sync frames, closures, and suspended async executions use their
  generation's slot map;
- removed old-only state is retained and later collected correctly;
- shared images do not share mutable state or staging results.

### 13.6 Tooling And Public Surfaces

- semantic highlighting marks only contextual declaration introducers as
  keywords;
- hover explains VM/extern ownership and reload behavior;
- rename warns that declaration rename is remove plus add;
- formatter is idempotent and preserves comments/blank lines;
- reflection reports storage/visibility/type without exposing host ownership;
- Rust examples and C ABI use only state-specific APIs;
- documentation contains no active claim that the old global model remains
  supported.

---

## 14. Non-Goals And Forbidden Shortcuts

This track does not add:

- disk persistence, snapshots, replication, or cross-process restoration;
- state sharing between Runtime instances;
- arbitrary schema/value migration;
- suspended async frame migration to new code;
- initializer dependency graphs or state-to-state initializer reads;
- arbitrary module-level executable statements;
- extern functions, extern types, or a general FFI syntax;
- raw Rust references, script-owned host objects, or host state under GC;
- compatibility aliases for `global` syntax, types, bytecode, or Runtime APIs;
- a second interpreter or initializer evaluator that bypasses HIR/MIR/verified
  linked execution;
- unbounded initializer execution;
- silent reinitialization after an incompatible reload.

Do not infer VM versus host storage from initializer presence, runtime map
contents, Rust API choice, type name, or missing binding. Storage is explicit
in the declaration and executable descriptor.

---

## 15. Final Completion Criteria

The goal is complete only when all of these are true:

- [x] `state` is contextual and ordinary identifiers named `state` work.
- [x] `global` declarations are rejected and no parser compatibility alias
      exists.
- [x] `state` and `extern state` have statically distinct HIR, MIR, bytecode,
      verifier, and runtime paths.
- [x] VM state supports direct and compound root assignment.
- [x] extern roots are immutable in Vela and nested mutation uses HostAccess.
- [x] every VM state has a required explicit type and restricted initializer.
- [x] Runtime construction and added-state reload initialization are bounded,
      fallible, and transactionally published.
- [x] Rust-side state replacement resolves exact canonical names, validates
      complete linked record/enum payloads, and preserves nominal runtime
      identities through `set_state` and `update_state`.
- [x] state identity and preservation use StateId; dense slots are
      generation-local.
- [x] exact-compatible state preserves old values/bindings and does not rerun
      initializers.
- [x] incompatible type/storage changes reject with actionable diagnostics.
- [x] removed state remains valid for old generation owners and is later
      reclaimed.
- [x] initializer change reporting covers the complete permitted transitive
      call graph, including calls inside nested executable bodies.
- [x] multiple runtimes sharing an image keep independent VM state.
- [x] host state remains outside script GC and no Rust reference is exposed.
- [x] the old global embedding API and production terminology are removed.
- [x] reflection, language service, LSP, formatter, editor integrations,
      examples, C ABI, site snippets, and active docs use the new model.
- [x] focused Batch G regressions and full validation commands pass.
- [x] docs/decisions.md records the implemented durable decision.
- [x] Batch F landed exact embedding/extern contracts, state export ABI,
      transaction-wide initializer limits, generation reclamation, and
      transitive initializer-change reporting with its listed regressions.
- [x] Batch G closes exact qualified-name resolution, linked nominal
      canonicalization, graph-preserving budgeted staging, self-root-free
      generation reclamation, and nested initializer-call fingerprints.
- [x] docs/progress.md reflects the milestone truth without becoming a
      changelog.
- [x] implementation checkpoints are committed with Conventional Commits and
      the final worktree is clean.

The final report must summarize the activated language/runtime contract,
state ABI behavior, initializer restrictions, validation commands, remaining
explicit non-goals, commit list, and final worktree status.
