# MIR Lowering And JIT Foundation Plan

> **Track:** middle IR, bytecode lowering architecture, optimizer/JIT
> foundation after Heavy HIR
> **Document status:** Codex goal-mode execution plan
> **Execution status:** Phase 0 and the bytecode-independent Phase 1 model are
> complete; production compilation builds and validates one immutable
> executable-analysis/compile-target generation, and Phase 2 MIR builder work
> is active
> **Compatibility policy:** breaking pre-release MIR, bytecode-compiler, and
> internal test APIs are allowed. Preserve Vela language semantics, evaluation
> order, VM behavior, diagnostics, execution budgets, GC roots, HostAccess
> safety, reflection permissioning, hot-reload ABI/schema checks, cache/profile
> ownership, and current bytecode verifier guarantees.

This plan is designed for goal-mode execution. It is a deletion-first hard
switch, not a long-running compatibility migration. Intermediate compilation
errors are allowed inside a checkpoint. Every completed checkpoint and every
commit must be green for its declared validation scope.

Default commit granularity is a complete architecture checkpoint or a large
cohesive subsystem slice. Do not commit one MIR enum variant, one instruction,
one fallback, or one checklist item at a time. Do not add production feature
flags, fallback dispatch, aliases, temporary backend names, or dual-path tests
whose purpose is to keep old and new body lowering alive together.

The MIR model, builder, and verifier may exist before production bytecode uses
them because they are not a second production backend. Once MIR-to-bytecode
code generation is introduced, backend integration, compile-API switching,
and deletion of the direct Heavy-HIR-to-bytecode lowering path form one atomic
hard-switch checkpoint. That checkpoint must not be marked complete or
committed while both production backends remain callable.

MIR v1 lowers to the existing bytecode and VM. It must not introduce
Cranelift, machine code, speculative optimization, runtime deoptimization,
exact compiled stack maps, or user-visible language behavior changes.
Cranelift remains M22 work.

The goal is complete only after every Phase 7 acceptance item and every final
audit has passed. Creating `vela_mir`, lowering a representative subset, or
routing selected expressions through MIR is not completion.

---

## 0. Codex Goal

Use this prompt to execute the full refactor:

```text
/goal Execute docs/mir-lowering-jit-foundation-plan.md as a deletion-first
hard switch. Treat docs/goal.md as the product roadmap, docs/architecture.md
and docs/architecture/*.md as the architecture contract, docs/progress.md as
current milestone status, and docs/heavy-hir-hard-switch-plan.md as the
semantic input contract.

Start with the MIR-specific Phase 0 closure. Remove any remaining body-lowering
need to re-lex source, assign every compiler-local type/shape/target/diagnostic
fact to HIR, analysis, compile-target input, MIR, or the bytecode backend, and
freeze behavior fixtures before building the backend.

Add an internal vela_mir crate that consumes Heavy HIR, AnalysisFacts, and
backend-neutral compile-target facts. Keep its dependency graph one-way:
vela_mir must not depend on vela_syntax, vela_bytecode, or vela_vm. Model a
non-SSA CFG with mutable script locals, single-assignment temporaries, explicit
effectful operations, guards, source origins, liveness, and debug/safepoint
metadata. Do not model HostRef dereference or HostAccess as an ordinary place.

Build and verify complete MIR without adding a production MIR selector. Then
perform one atomic backend checkpoint: add vela_bytecode::mir_backend, validate
against frozen behavior fixtures, route every production compile API through
Heavy HIR -> MIR -> bytecode, and delete the old direct body lowering, its
helpers, migration tests, and temporary names before committing the checkpoint.
Never preserve a production fallback or dual backend.

Keep const/schema compile-time evaluation outside runtime MIR unless this plan
explicitly moves a proven shared pure subset. Lower parameter defaults into the
owning function prologue and lambdas into nested MIR functions. Preserve stable
FunctionId/MethodId/HirBodyId mappings while keeping MIR IDs generation-local.

Validate large subsystem slices with MIR builder/verifier tests, behavior and
diagnostic fixtures, bytecode verification, VM/host/reflection/hot-reload tests,
workspace checks, final architecture audits, and the examples workspace. Do
not mark the goal complete until all Phase 7 criteria and zero-hit searches
pass and no compatibility, fallback, temporary naming, or oversized active
source files remain.
```

---

## 1. Purpose And Current Baseline

The current compiler lowers Heavy HIR directly into unlinked bytecode while
also performing several distinct jobs:

- maintaining compiler-local runtime type, script type, and value-shape flow;
- resolving registry, stdlib, script, method, field, host-path, and guard
  targets;
- assigning physical registers and frame slots;
- selecting typed and generic instruction families;
- constructing constants, host-target tables, guards, cache sites, and nested
  code objects;
- patching bytecode jumps while source control flow is still being traversed;
- projecting source spans and frame debug metadata.

Heavy HIR and `AnalysisFacts` provide stable body/expression/pattern/local
identity plus type, call, member, constructor, operator, host-path, effect, and
control-flow facts. MIR-specific input closure is complete:

- ordered interpolated-string parts and contextual literal facts are
  HIR/analysis-owned, so runtime body lowering does not re-lex source;
- user-facing literal, call-placement, constructor, operator, loop-placement,
  and HostAccess diagnostics are produced before MIR construction;
- every production compile entry builds and validates the same immutable
  executable-analysis and compile-target generation that MIR consumes;
- the current direct backend still contains its execution-local
  `CompilerFacts`, `RuntimeTypeFact`, `ScriptTypeFlow`, `ValueTypeFlow`, and
  `ValueShapeFlow` migration oracle, but their final ownership is fixed and the
  atomic hard switch deletes them with direct body lowering;
- the VM conservatively traces all frame register values as roots; it does not
  consume an exact MIR root map.

MIR introduces one explicit execution-shape layer:

```text
source front door
  -> Heavy HIR + AnalysisFacts + compile-target snapshot
  -> verified MIR: CFG, typed ops, effects, guards, liveness, debug origins
  -> vela_bytecode::mir_backend
  -> existing unlinked bytecode, linker, verifier, VM, and ProgramVersion
  -> future restricted Cranelift backend in M22
```

MIR v1 succeeds when all executable bytecode body lowering consumes verified
MIR, the old direct body compiler is deleted, current behavior is preserved,
and the MIR boundary is precise enough for later debugger and JIT planning
without implementing either subsystem.

---

## 2. Scope And Non-Goals

MIR v1 includes:

- runtime-executable functions, trait default methods, impl methods, and
  lambdas;
- parameter-default evaluation as prologue control flow in the owning
  function;
- explicit CFG, local/temp data flow, typed and generic operations, effects,
  guards, calls, source origins, liveness, debug locals, and safepoints;
- one MIR-to-existing-bytecode backend;
- behavior, diagnostic, metadata, and invariant equivalence coverage.

MIR v1 does not include:

- Cranelift, machine code, JIT enablement, runtime side exits, or deoptimization;
- SSA construction, phi-node optimization, inlining, LICM, CSE, speculative
  specialization, or a general optimization pipeline;
- language-level `throw`, `catch`, `finally`, exception tables, or unwind CFG;
  recoverable errors remain explicit `Result`/`Option` values and unrecoverable
  runtime failures remain `may_trap` VM exits;
- MIR serialization, a public embedding API, a stable on-disk MIR format, or
  MIR identity in hot-reload ABI;
- new VM instructions or changed runtime semantics, unless a separately
  reviewed blocker proves an existing instruction cannot represent current
  behavior;
- replacing the conservative bytecode-VM root scan with exact root maps;
- treating const/schema initializer bodies as callable runtime functions;
- parsing source, syntax recovery, name binding, type inference, or semantic
  repair inside MIR.

Const and schema initializer evaluation remains a compile-time front-end
service. It may reuse a deliberately extracted pure literal/constant helper,
but it must not be represented as a fake runtime `MirFunction` merely because
HIR gives the initializer a `HirBodyId`.

---

## 3. Dependency And Ownership Contract

The dependency direction is mandatory:

```text
vela_common / vela_def / vela_host / vela_registry / vela_stdlib
                        ^
                        |
vela_hir -> vela_analysis -> vela_mir -> vela_bytecode -> vela_vm
```

`vela_mir` may depend directly on `vela_common`, `vela_def`, `vela_hir`, and
`vela_analysis`. It may depend on `vela_host`, `vela_registry`, or `vela_stdlib`
only for backend-neutral stable target descriptors that are actually required.
It must not depend on `vela_syntax`, `vela_bytecode`, `vela_vm`, engine/runtime
state, or language-service crates.

Layer ownership is fixed:

- `vela_hir`: executable structure, stable semantic IDs, source origins,
  bindings, scopes, captures, and semantic targets recorded during HIR
  lowering;
- `vela_analysis`: reusable type, call, member, constructor, operator,
  host-path, effect, and control-flow facts keyed by HIR IDs;
- `vela_mir`: execution order, CFG, logical locals/temps, typed/generic op
  selection, effectful operation shape, guards, safepoints, liveness, and
  source/debug projection;
- `vela_bytecode::mir_backend`: physical register assignment, constant and
  host-target interning, bytecode instruction selection/layout, jump offsets,
  cache-site allocation, frame-slot projection, unlinked guard conversion,
  nested `UnlinkedCodeObject` assembly, and bytecode verification;
- linker/VM/runtime: existing linked IDs, execution, conservative frame root
  tracing, budgets, caches, host boundaries, hot reload, and runtime errors.

`vela_mir` must not import or expose `Register`, `InstructionOffset`,
`UnlinkedInstructionKind`, `UnlinkedTypeGuardPlan`, `CacheSiteId`, or any other
bytecode representation type. The bytecode backend may consume MIR; MIR may
not know that bytecode is one of its consumers.

Define one explicit lowering input, conceptually:

```text
MirLoweringInput
  ModuleGraph / owning HirBodyId
  AnalysisFacts
  backend-neutral compile-target snapshot
  lowering configuration that changes representation, not language semantics
```

The compile-target snapshot must be derived once from the same registry,
stdlib, host, options, and script metadata used by production compilation. It
must carry stable IDs/descriptors where available and explicit `Dynamic`
targets where runtime dispatch is required. It must not contain syntax nodes,
bytecode registers/instruction kinds, or string fallback targets for entities
that already have stable IDs.

Before MIR implementation, inventory every field and query in current
`CompilerFacts`, `RuntimeTypeFact`, `ScriptTypeFlow`, `ValueTypeFlow`,
`ValueShapeFlow`, field-slot/schema-default facts, expected-type checking, and
call/host/record-shape resolution. Assign each item to exactly one owner:

```text
semantic truth            -> HIR or AnalysisFacts
compile environment       -> backend-neutral compile-target snapshot
execution/lowering choice -> MIR
physical encoding         -> vela_bytecode::mir_backend
compile-time constants    -> const/schema evaluator
```

Do not copy these systems into `vela_mir` under new names while leaving the old
compiler-owned versions alive.

### Diagnostic And Error Ownership

User-facing syntax, binding, target, type-contract, literal, named-argument,
constructor, and pattern diagnostics must be produced before or while building
the semantic input under an explicitly documented owner. MIR must not rediscover
them by parsing text or guessing missing facts.

Use separate error families:

- semantic/user diagnostics owned by syntax, HIR, analysis, or an explicit
  compile-target validation pass;
- `MirBuildError` for inconsistent/missing supposedly-valid semantic input;
- `MirVerifyError` for malformed MIR invariants;
- bytecode backend errors for physical limits such as register overflow and
  unrepresentable bytecode operands;
- existing bytecode verification errors after emission.

Missing semantic facts must fail the focused checkpoint with a source-spanned
internal/lowering error and be fixed at the owning layer. They must not select a
syntax, direct-bytecode, name-based, or dynamic fallback unless `Dynamic` is the
actual language/runtime target recorded by analysis.

---

## 4. Executable Units And Identity

`MirProgram` is a generation-local compilation batch. It is not a runtime
artifact, public API, serialization format, or hot-reload identity.

`MirFunction` corresponds only to a code-object-producing executable unit:

- top-level script function;
- trait default method;
- impl method;
- lambda/nested closure.

Parameter-default HIR bodies are lowered into entry/prologue CFG in their
owning `MirFunction`. They are not independently callable MIR functions and
must retain access to earlier parameters exactly as current execution does.

Const and schema initializer HIR bodies remain compile-time evaluation inputs.
They are outside runtime MIR v1 unless a later explicit decision creates a
verified const-MIR subset.

MIR IDs have the following identity rules:

- `MirFunctionId`, `MirBlockId`, `MirLocalId`, `MirTempId`, and operation IDs
  are deterministic within one MIR build but generation-local;
- every `MirFunction` records its owning `HirBodyId`, source origin, and stable
  script `FunctionId` or `MethodId`/method node identity where applicable;
- MIR locals that represent script bindings map back to `HirLocalId`;
- MIR temporaries do not acquire fake stable HIR identity;
- MIR IDs never enter hot-reload ABI, schema ABI, cache serialization, or public
  engine APIs.

Lambdas preserve HIR capture order and produce nested MIR functions. The
bytecode backend preserves existing nested-code-object and closure capture
semantics. Hot reload continues to use stable program/function ownership, not
MIR arena numbering.

---

## 5. MIR V1 Model

### 5.1 Data-Flow Form

MIR v1 is deliberately non-SSA:

- script locals are mutable logical storage and map to `HirLocalId` when they
  represent source bindings;
- synthetic mutable locals may represent branch joins and have no fake
  `HirLocalId`;
- compiler temporaries are single-assignment and generation-local;
- a temp must have exactly one definition that dominates every use;
- a local must be definitely initialized on every predecessor before use;
- Vela runtime values are copyable scalar/handle values, so MIR v1 has no
  Rust-style ownership move invalidation or borrow semantics;
- branch joins use synthetic mutable locals in v1. MIR v1 has no block
  parameters or phi nodes; do not add ad hoc join conventions in individual
  lowerers.

Core IDs and containers include `MirProgram`, `MirFunction`, `MirBlockId`,
`MirLocalId`, `MirTempId`, operation IDs, source origins, debug locals, and
stable HIR mappings.

### 5.2 Places, Operands, And Operations

`MirPlace` is restricted to logical local/temp destinations and any explicitly
proven internal direct slot form. It must not represent:

- dereferencing `HostRef` or exposing a Rust reference;
- host field/path access;
- dynamic field/index access with hidden evaluation;
- map/array/set mutation with hidden allocation or budget work;
- reflection mutation or calls.

`MirOperand` reads a constant or an already-evaluated logical value. It must not
perform calls, indexing, allocation, host access, or implicit name resolution.

`MirRvalue` contains effect-free scalar/tag operations whose operands have
already been evaluated. Aggregation, dynamic operations, guards, calls, and
operations that may allocate or trap use explicit MIR statements or
terminators.

Effectful operation families include explicit forms for:

- record/enum slot and dynamic field reads/writes;
- array/map/tuple/index reads and writes;
- aggregate and closure allocation;
- script, local/closure, native, stdlib, value-method, script-method, dynamic,
  and host calls;
- HostAccess read, write, mutate, remove, and call through resolved host target
  descriptors;
- reflection boundaries;
- runtime type/shape/arity guards;
- iterator/range steps and try propagation.

Calls are represented only as effectful statements with destination, target,
arguments, effect metadata, safepoint, and source origin. A successful call
continues with the next operation in the same basic block. An unrecoverable
runtime failure leaves the current VM execution path through the call's
`may_trap` behavior; MIR v1 does not add a language-level unwind successor.
Calls must not also appear as pure rvalues or terminators.

Future explicit async/coroutine work may add `Await`, `Yield`, and `Suspend`
terminators with resume IDs and live-frame metadata. MIR v1 reserves those
names and extension points but does not implement them, and ordinary calls do
not implicitly become suspension terminators. A future stackful model with
transitive implicit suspension requires a separate architecture decision.

### 5.3 CFG And Evaluation Semantics

Every basic block has exactly one terminator. MIR v1 terminators cover jump,
branch/switch, return, explicit fail/trap boundaries, and unreachable state.
Loop backedges and `Result`/`Option` try propagation lower through those forms.
`Await`, `Yield`, and `Suspend` remain reserved future terminators.

The builder must preserve:

- left-to-right operand and argument evaluation;
- short-circuit `and`/`or` behavior;
- condition and match-guard evaluation order;
- receiver, dynamic index, assignment target, and RHS evaluation exactly once;
- compound assignment read-modify-write order, including aliases;
- loop, range, iterator, break, continue, and return behavior;
- pattern binding visibility and partial-match behavior;
- `Option`/`Result` try propagation;
- parameter-default order and access to earlier parameters;
- call, allocation, host, reflection, and budget boundaries.

Do not encode an effectful expression by reconstructing it from source or by
re-evaluating its HIR children in the backend.

### 5.4 Effects, Guards, Safepoints, And Metadata

Every effectful MIR operation carries or derives a backend-neutral effect
classification. It must distinguish at least:

- pure computation;
- may trap;
- may allocate / GC safepoint;
- script/dynamic call;
- host read/write/call;
- reflection read/write/call;
- event, time, random, and IO effects represented by current registry facts.

Typed operations are selected only from proven analysis/compile-target facts.
Unknown facts lower to explicit generic dynamic operations. A failed
optimization guard is a normal slow-path transition, not a semantic error.

MIR v1 guard metadata describes checked assumptions and equivalent slow paths.
Do not call it runtime deoptimization and do not build optimized-frame recovery
state before M22.

Every operation maps to a `HirExprId`, `HirStmtId`, `HirPatternId`, or owning
`HirBodyId` plus source span as appropriate. Debug locals map logical storage to
names, kinds, HIR locals, scopes, and source origins.

Compute logical liveness for verifier checks, backend register reuse, debug
projection, and future safepoint planning. The existing bytecode VM continues
to conservatively trace all frame registers. MIR liveness must not narrow VM GC
roots in v1. Exact post-register-allocation compiled stack maps remain M22 work.

Execution-budget safety must remain explicit. The existing VM charges emitted
bytecode instructions, so changed bytecode layout can change observable
counters. The hard-switch checkpoint must preserve current termination and
charging boundaries and include representative limit-edge tests. Any intended
change to exact instruction-count observability requires a separate recorded
decision; it must not happen silently inside MIR migration.

---

## 6. Phase Status And Checkpoint Rules

Use this checklist as the durable execution tracker. Mark a task only after its
focused tests and declared validation commands pass.

```text
[ ] not started
[~] in progress
[x] complete
```

The expected large checkpoints are:

```text
A. MIR-specific HIR/analysis/input closure and frozen behavior baseline
B. vela_mir model + complete builder + verifier/liveness, not production-wired
C. atomic MIR-to-bytecode hard switch and deletion of direct body lowering
D. metadata cleanup, architecture audit, full acceptance
```

Subcommits inside A, B, or D must remain cohesive and green. Checkpoint C is
atomic with respect to production backend ownership: do not commit or mark it
complete with a selectable old backend, per-feature fallback, or production
dual path.

---

## 7. Phase 0: MIR-Specific Preconditions And Frozen Baseline

Purpose: prove MIR can consume semantic input without becoming a second parser,
resolver, type-flow engine, or diagnostic pipeline.

### 7.1 HIR Executable-Literal Closure

- [x] Replace interpolated-literal raw-text re-lexing in bytecode body lowering
  with ordered HIR-owned text/expression parts.
- [x] Delete the body-lowering dependency on `vela_syntax::lexer`,
  `TokenKind`, and `InterpolatedStringTokenPart`.
- [x] Inventory integer/float literal normalization, contextual conversion,
  invalid-literal diagnostics, map-key spelling, and format-string errors.
- [x] Put source-independent literal structure in HIR, contextual validity and
  user diagnostics in analysis or an explicit pre-MIR compile validation pass,
  and representation selection in MIR. Do not copy the current
  parser/compiler split into MIR.

### 7.2 Semantic And Compile-Target Input Closure

- [x] Inventory `CompilerFacts`, `RuntimeTypeFact`, `ScriptTypeFlow`,
  `ValueTypeFlow`, `ValueShapeFlow`, script field slots, schema defaults,
  expected-type checks, call argument metadata, host target resolution, and
  guard-plan construction.
- [x] Define `MirLoweringInput` and the backend-neutral compile-target snapshot.
- [x] Prove production registry/stdlib/host/script metadata can build the same
  AnalysisFacts and compile targets used by MIR.
- [x] Move semantic facts to HIR/analysis and keep physical encoding facts in
  the bytecode backend. Delete duplicate fact stores when their replacement is
  proven.
- [x] Inventory every user-facing bytecode compiler diagnostic by code, message,
  and span and assign its final owner before backend work begins.

### 7.3 Frozen Behavior Baseline

- [x] Build or organize fixtures that pin runtime results, host side effects,
  diagnostics, source spans, frame/debug locals, guard behavior, cache-site
  families, and hot-reload identity for the behavior matrix in Phase 7.
- [x] Pin selected structural bytecode snapshots where instruction family or
  metadata shape is a contract. Do not require all bytecode to remain
  byte-for-byte identical when equivalent CFG/register allocation is valid.
- [x] Keep the current direct compiler as the production baseline in Phase 0;
  do not introduce a MIR selector yet.
- [x] Record MIR as in progress in `docs/progress.md` only when implementation
  actually begins.

### 7.4 Phase 0 Ownership Record

This table is the required final ownership assignment for the compiler-local
stores that existed when Phase 0 began. A later implementation may refine a
type name, but it must not move semantic truth back into the bytecode backend
or turn physical bytecode encoding into MIR state.

| Current fact/store | Final owner | Required hard-switch result |
|---|---|---|
| function, type, global, body, parameter-default, lambda, binding, capture, path, signature, and source-origin facts | `vela_hir` | MIR starts from stable HIR IDs; no body source or span-to-ID reconstruction |
| expression/local/pattern runtime type, collection element/key/value type, script record/enum identity, member/call/operator/constructor/host-path target, expected-type compatibility, trait validity, effect, and control-flow facts | `vela_analysis::AnalysisFacts` | delete `RuntimeTypeFact`, `ScriptTypeFlow`, `ValueTypeFlow`, and the semantic portions of `ValueShapeFlow` after MIR consumes the analysis facts |
| stable registry/stdlib/host/script function, method, type, field, variant, global, host-runtime, signature/default, access, effect, and index-capability targets | backend-neutral `CompileTargetSnapshot` | MIR sees stable descriptors or explicit `Dynamic`; it never queries names as a fallback or retains a live registry view |
| typed-versus-generic operation selection, logical shape flow required by execution, guards, parameter-default prologue, branch joins, loop targets, and lambda nesting | `vela_mir` | replace the execution portions of the old compiler flows with verified MIR |
| const values and schema field defaults | compile-time const/schema evaluator | keep them outside runtime MIR; convert only the evaluated backend-neutral value at the bytecode boundary |
| record/enum physical slots, `GlobalSlot`, registers, constants, host-target table entries, cache sites, frame slots, bytecode guards, jump layout, and verification | `vela_bytecode::compiler::mir_backend` | delete direct HIR emission, register allocation, interning, and jump patching |
| register overflow and unrepresentable bytecode operands | bytecode backend diagnostics | never report these as semantic or MIR verification errors |
| missing supposedly-valid HIR/analysis/target facts and malformed MIR | `MirBuildError` / `MirVerifyError` | source-spanned internal lowering failure; never select a source, name, or old-backend fallback |

The concrete lowering input is fixed conceptually as:

```text
MirLoweringInput
  module graph and selected owning HirBodyId
  AnalysisFacts keyed by HIR IDs
  immutable CompileTargetSnapshot
  MirLoweringConfig containing representation policy only

CompileTargetSnapshot
  script functions: HirDeclId -> stable FunctionId + signature
  script methods: HirNodeId -> stable MethodId + owner + signature
  nested lambdas: (root FunctionId, HirBodyId) -> parent/expression/code symbol
                  + ordered HIR parameter identities/contracts
  globals: HirDeclId -> stable name/type target (no physical GlobalSlot)
  script schema: declarations/fields/variants -> stable IDs and logical layout facts
  external calls/members: HirExprId -> stable registry/stdlib/host target or Dynamic
  host targets: stable TypeId/FieldId/MethodId/runtime IDs, access, effects, and index capabilities
  guards: backend-neutral type/shape/variant/host descriptors plus source-level
          parameter/return/local/global/field context (no bytecode guard enum)
```

`MirLoweringInput` borrows or owns one immutable compilation generation. The
snapshot is derived once at the source front door from the same script graph,
definition registry, stdlib manifest, host schema, and compiler options used by
production compilation. `vela_mir` must not retain `RegistryCompileView`,
`CompilerOptions`, syntax values, or bytecode values.

The executable-literal closure is assigned as follows:

| Literal concern | Current seam | Final owner |
|---|---|---|
| decoded boolean, char, string, bytes, integer radix/digits/suffix, float spelling/suffix, and ordered interpolation text/expression parts | `vela_hir::body::HirLiteral` and syntax-to-HIR lowering | HIR; runtime lowering never re-lexes or asks for syntax tokens |
| intrinsic literal type and contextual compatibility with a declared primitive, including signed-min handling and out-of-range diagnostics | `vela_analysis::semantic_facts`, old `value_types`, and old `const_eval` | analysis or a pre-MIR compile-validation pass keyed by `HirExprId` |
| source spelling used as a string/numeric map-key name | HIR literal/path payload queried by old const/body lowering | HIR-owned spelling; MIR carries the logical key value |
| const and schema-default evaluation of the proven pure literal/unary/binary/aggregate subset | old `compiler::const_eval` | compile-time const/schema evaluator outside runtime MIR |
| typed scalar versus generic literal value and typed-literal binary operation | old `hir_lowering::{values,operators}` | MIR rvalue/operation selection |
| scalar/constant-pool encoding, inline-immediate selection, and operand limits | old bytecode compiler | `vela_bytecode::compiler::mir_backend` |
| malformed format-string braces and interpolation syntax | lexer/parser diagnostics; HIR receives ordered valid parts | syntax/HIR boundary, never MIR or bytecode |

The user-facing diagnostic inventory is also fixed before MIR construction:

The exhaustive code/message/span inventory is maintained in
[`mir-phase0-diagnostic-inventory.md`](mir-phase0-diagnostic-inventory.md).

| Diagnostic family | Final owner |
|---|---|
| invalid integer/float literal spelling, contextual literal contract, static type-contract mismatch | HIR/analysis compile validation |
| unresolved native/stdlib/host/script call or method target | analysis plus compile-target validation |
| unknown/duplicate/missing/named/positional call arguments | analysis call-argument placement |
| invalid identity comparison, missing comparison trait, missing `Ord` for array ordering | analysis operator/call validation |
| unknown variant, duplicate/unknown/missing constructor field | analysis constructor validation |
| read-only field and unsupported/read-only/write-only/mutate/remove/key-mismatch host index access | analysis plus compile-target validation |
| function selection by a compile API | source-front-door API error |
| register overflow, bytecode operand/layout limits, bytecode verification | bytecode backend |
| inconsistent HIR/analysis/target input | `MirBuildError` |
| malformed CFG/data-flow/debug/safepoint MIR | `MirVerifyError` |

The frozen behavior baseline is organized by durable subsystem fixtures:

| Contract | Fixture ownership |
|---|---|
| literals, interpolation, calls, defaults, named arguments, compiler diagnostics | `vela_hir` executable-fact tests and `vela_bytecode::compiler::tests` |
| evaluation order, control flow, patterns, closures, try propagation, budgets, GC, guards, cache/profile/linking behavior | `vela_vm` source/linked execution tests and conformance fixtures |
| HostAccess write-through, aliases, permissions, stale refs, host calls | `vela_host` and `vela_vm` host fixtures |
| reflection policy and source-spanned diagnostics | `vela_reflect`, `vela_vm`, and Engine reflection fixtures |
| stable function/method identity, accepted/rejected reloads, cache invalidation | `vela_hot_reload` and Engine source-reload fixtures |
| end-to-end embedding behavior | `examples/tests/runnable_examples.rs` |

The Phase 0 audit ties those ownership rows to durable fixtures: compiler
`diagnostic_contracts` and `closures_and_bindings` pin diagnostics, spans, and
frame/debug locals; VM `runtime_semantics`, `type_guards`, control-flow,
collection, GC, host, and reflection suites pin results, ordering, guards,
budgets, roots, and side effects; production-source compiler/VM assertions pin
every currently emitted cache-site family; hot-reload runtime and Engine reload
tests pin stable function/method identity, old-code lifetime, new-call routing,
and cache invalidation; runnable examples pin the embedding boundary. The
reserved `GlobalWrite` cache kind has no production compiler emitter and is not
treated as an emitted-family fixture.

The Phase 0 test additions are limited to uncovered rows: ordered HIR
interpolation parts, a complete compiler diagnostic contract fixture, explicit
single-evaluation/effect-order cases, representative instruction-budget edges,
emitted cache-site/instruction pairings, and stable identity across an accepted
reload. The MIR backend will be compared against these production-source
fixtures; a selectable dual-backend test harness is forbidden.

Validation:

```bash
cargo test -p vela_hir
cargo test -p vela_analysis
cargo test -p vela_bytecode
rg -n "vela_syntax::lexer|InterpolatedStringTokenPart|TokenKind" crates/vela_bytecode/src/compiler/hir_lowering crates/vela_bytecode/src/compiler/hir_lowering.rs
rg -n "CompilerFacts|RuntimeTypeFact|ScriptTypeFlow|ValueTypeFlow|ValueShapeFlow|ScriptFieldSlots|ScriptSchemaDefaults" crates/vela_bytecode/src/compiler.rs crates/vela_bytecode/src/compiler crates/vela_analysis/src crates/vela_hir/src
```

The first search must have zero hits after 7.1. The second search is an
ownership inventory: every remaining hit must match the documented final owner
before Phase 1 starts.

---

## 8. Phase 1: Add `vela_mir` And Core Types

Purpose: establish a bytecode-independent MIR ownership boundary.

- [x] Add `crates/vela_mir` to the workspace and workspace dependencies.
- [x] Add only the minimal allowed crate dependencies from Section 3.
- [x] Add deterministic generation-local IDs, `MirProgram`, `MirFunction`,
  block/local/temp arenas, source origins, HIR identity mappings, and debug
  local records.
- [x] Add operands, restricted places, pure rvalues, effectful statements,
  terminators, call targets, guard descriptors, effects, and safepoint records.
- [x] Encode non-SSA mutable-local/single-assignment-temp rules directly in the
  model APIs rather than relying on naming conventions.
- [x] Add stable human-readable MIR dumps for tests. They are test/debug output,
  not a stable serialization format.
- [x] Keep MIR internal to the workspace; do not re-export it from engine, VM,
  C API, or public runtime APIs.

Validation:

```bash
cargo test -p vela_mir mir_model
cargo check -p vela_bytecode
rg -n "^vela_(syntax|bytecode|vm)\s*=" crates/vela_mir/Cargo.toml
rg -n "vela_(syntax|bytecode|vm)::" crates/vela_mir/src
rg -n "Register|InstructionOffset|UnlinkedInstruction|UnlinkedTypeGuard|CacheSiteId" crates/vela_mir/src
```

All three searches must have zero hits.

---

## 9. Phase 2: Complete Heavy-HIR-To-MIR Builder

Purpose: build complete execution-shape MIR without creating a second
production bytecode backend.

- [x] Lower literals, locals, declarations, assignment, blocks, returns,
  if/else, short-circuit operators, loops, ranges, iterators, break/continue,
  match, guards, try propagation, and unreachable paths.
- [x] Lower records, enums, arrays, maps, sets where language construction
  exists, tuples, fields, indexes, tuple projections, constructors, and pattern
  destructuring.
- [x] Lower script, local/closure, native, stdlib, value-method, script-method,
  dynamic, and host calls with resolved targets and explicit effects.
- [x] Lower HostAccess read/write/mutate/remove/call without HostRef dereference
  places or hidden permission bypasses.
- [x] Lower lambdas as nested MIR functions with HIR capture order and parameter
  identity.
- [x] Lower parameter defaults into the owning function prologue in declaration
  order.
- [x] Preserve exact evaluation order and evaluate receiver/index/target/RHS
  subexpressions once.
- [x] Preserve source origins and HIR IDs on all executable operations.
- [x] Represent proven typed operations and explicit generic dynamic operations;
  never use `Unknown`, missing facts, or unsupported MIR as a reason to invoke
  old bytecode lowering.
- [x] Keep production compile APIs on the old backend throughout Phase 2. There
  must be no `use_mir` flag, runtime option, or fallback router.

Organize builder code by execution responsibility, for example:

```text
builder/core.rs
builder/control_flow.rs
builder/assignments.rs
builder/aggregates.rs
builder/patterns.rs
builder/calls.rs
builder/closures.rs
builder/host.rs
builder/guards.rs
```

Do not let a single builder file grow past the ordinary 1200-line limit.

Validation:

```bash
cargo test -p vela_mir mir_builder
cargo test -p vela_hir
cargo test -p vela_analysis
cargo test -p vela_bytecode
```

Builder tests must include complete-function MIR snapshots, not only isolated
enum-construction tests.

---

## 10. Phase 3: MIR Verifier, Data Flow, And Liveness

Purpose: make every assumption required by bytecode and future JIT backends
explicit before code generation.

- [x] Verify every referenced function/block/local/temp/target/origin ID exists.
- [x] Verify every materialized block is reachable from entry and has exactly
  one terminator with valid successors.
- [x] Verify mutable locals are definitely initialized on every path before use.
- [x] Verify every temp has exactly one definition that dominates all uses.
- [x] Verify there is no Rust-style move invalidation or implicit hidden read in
  an operand/place.
- [x] Verify branch/switch destinations, return values, loop targets,
  break/continue scopes, and try propagation families.
- [x] Verify each effectful call statement has a valid target, arguments,
  destination, effect, safepoint, and source origin without a hidden
  continuation block or unwind edge.
- [x] Verify call target kind, arity, named/default argument placement, capture
  placement, and result destination.
- [x] Verify type/guard consistency only from proven semantic facts; dynamic
  operations must remain explicitly dynamic.
- [x] Verify every may-allocate/call/host/reflection operation has required
  effect, source-origin, and safepoint metadata.
- [x] Verify source/debug coverage for every operation that can produce a user
  runtime error or debugger-visible step.
- [x] Compute block/local/temp liveness and test loops, joins, early returns,
  nested calls, closures, and try edges.
- [x] Add one negative test per verifier invariant; malformed MIR must not reach
  bytecode code generation.

Validation:

```bash
cargo test -p vela_mir mir_verifier
cargo test -p vela_mir mir_liveness
```

---

## 11. Phase 4: Typed Ops, Guards, Debug Data, And Backend Contract

Purpose: finish the verified backend-neutral MIR contract before production
bytecode integration.

- [x] Represent proven scalar arithmetic/comparison, boolean branch, tuple
  projection, and i64 range-loop operations without embedding bytecode enums.
- [x] Represent generic dynamic operation families and their equivalent slow
  paths explicitly.
- [x] Represent guard assumptions and slow-path targets without runtime deopt or
  optimized-frame recovery state.
- [x] Produce logical debug-local/capture/parameter records with HIR IDs, names,
  kinds, source spans, scopes, and live regions.
- [x] Produce safepoint/live-value input for future compiled-frame work while
  preserving conservative VM root tracing.
- [x] Define the backend handoff for logical values, constants, calls, targets,
  source spans, frame/debug data, guards, effects, and CFG layout.
- [x] Do not implement generic optimization passes. Any canonicalization needed
  by all backends must be deterministic, semantics-preserving, and verified.

Validation:

```bash
cargo test -p vela_mir mir_typed_ops
cargo test -p vela_mir mir_guards
cargo test -p vela_mir mir_debug_metadata
```

---

## 12. Phase 5: Atomic MIR-To-Bytecode Hard Switch

Purpose: make verified MIR the only runtime body-lowering input and delete the
direct Heavy-HIR-to-bytecode compiler in one checkpoint.

**Atomic checkpoint rule:** every checklist item in this phase belongs to one
hard-switch checkpoint. The old compiler may be used as an uncommitted
migration oracle while this checkpoint is red. Do not mark, commit, or leave
this phase with two callable production backends.

### 12.1 Bytecode Backend

- [ ] Add the focused directory module
  `vela_bytecode::compiler::mir_backend`; it consumes verified MIR and does not
  traverse HIR expression/statement/pattern kinds or AnalysisFacts.
- [ ] Assign physical registers and preserve register-overflow diagnostics.
- [ ] Emit existing unlinked instruction families, constants, host targets,
  nested code objects, frame slots, parameter/return guards, source spans, and
  cache sites.
- [ ] Linearize CFG and resolve jump targets without source-HIR traversal or
  migration jump-patching helpers.
- [ ] Preserve linker/verifier contracts, stable function/method identity,
  ProgramVersion ownership, cache/profile invalidation, and hot-reload ABI.

### 12.2 Behavior Comparison And Production Switch

- [ ] Run frozen Phase 0 fixtures through the MIR backend and compare runtime
  values, side effects, diagnostics, spans, frame/debug metadata, guard
  behavior, cache-site families, and hot-reload identity.
- [ ] Compare selected bytecode structure where an instruction or metadata
  family is contractually important. Do not require incidental register or
  block numbering to match.
- [ ] Cover instruction-budget limit edges for loops, calls, guards, host
  boundaries, and try propagation; do not silently change budget observability.
- [ ] Route every source/program/module/function/method/lambda production compile
  API through Heavy HIR + analysis/target snapshot -> MIR -> bytecode.
- [ ] Verify no production option, environment variable, test flag, or feature
  can select the old backend.

### 12.3 Deletion

- [ ] Delete `compiler/hir_lowering.rs` and its direct-lowering submodules.
- [ ] Delete direct `compile_hir_expression`, statement/pattern/root/value-body
  dispatch and old `Compiler<'...>` body-emission state.
- [ ] Delete old source-control-flow jump patching, direct register allocation,
  and emission helpers once the MIR backend owns their replacements.
- [ ] Delete compiler-local semantic/type/shape/target flows whose facts moved
  to HIR, analysis, or MIR; retain only explicitly owned front-end/const/backend
  data.
- [ ] Delete test-only dual-backend entry points, comparison flags, migration
  adapters, aliases, temporary names, and tests that prove both backends remain
  callable.
- [ ] Keep source parsing/HIR construction only at the compiler front door and
  const/schema HIR traversal only in the documented compile-time evaluator.

Focused validation before committing the checkpoint:

```bash
cargo test -p vela_mir
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_engine
cargo test -p vela_hot_reload
rg -n "mod hir_lowering|compile_hir_(expression|statement|pattern|root_body|value_body)|struct Compiler<'" crates/vela_bytecode/src/compiler.rs crates/vela_bytecode/src/compiler
rg -n "use_mir|enable_mir|mir.*fallback|fallback.*mir|old_backend|legacy.*lower|direct.*lower|new_mir|mir_v2" crates/vela_bytecode crates/vela_mir
rg -n "vela_(hir|analysis|syntax)::|HirExprKind|HirStmtKind|HirPatternKind|HirBodyRoot" crates/vela_bytecode/src/compiler/mir_backend
rg -n "HirExprKind|HirStmtKind|HirPatternKind|HirBodyRoot" crates/vela_bytecode/src/compiler -g "!const_eval.rs"
```

All four searches must have zero hits. Direct HIR expression traversal may
remain only in `compiler/const_eval.rs` for the documented compile-time
const/schema evaluator; it must not emit runtime bytecode bodies.

---

## 13. Phase 6: Architecture Cleanup And JIT Foundation Boundary

Purpose: remove migration residue and leave a clean internal API for debugger
and M22 planning without implementing those milestones.

- [ ] Ensure `vela_mir` contains no syntax, bytecode, VM, engine, LSP, or runtime
  execution dependency.
- [ ] Ensure the bytecode MIR backend consumes only verified MIR plus physical
  backend context.
- [ ] Ensure all body-semantic HIR traversal outside `vela_mir` is gone from
  runtime bytecode lowering. Document the narrow source-front-door and
  const/schema evaluator allowlist.
- [ ] Ensure exact GC roots are not inferred from pre-register-allocation MIR
  liveness in the bytecode VM; conservative tracing remains intact.
- [ ] Ensure guard metadata describes assumptions/slow paths, not implemented
  runtime deoptimization.
- [ ] Ensure no MIR storage, IDs, or snapshots leak into public engine/runtime
  APIs, serialized bytecode ABI, or hot-reload compatibility identity.
- [ ] Split active MIR and touched bytecode compiler files below 1200 lines by
  model, builder, verifier, analysis, backend, and tests.
- [ ] Remove migration comments, temporary tests, compatibility helpers, and
  transitional names such as `new_mir`, `mir_v2`, `legacy`, `compat`, or
  `temporary` where they describe the completed migration rather than domain
  behavior.
- [ ] Update `docs/decisions.md` if implementation resolves a new MIR model,
  budget, debug, root-map, or backend decision not already recorded.
- [ ] Update `docs/progress.md` only after focused and workspace validation pass.

Architecture audits:

```bash
rg -n "^vela_(syntax|bytecode|vm|engine|language_service|lsp_server)\s*=" crates/vela_mir/Cargo.toml
rg -n "vela_(syntax|bytecode|vm|engine|language_service|lsp_server)::" crates/vela_mir/src
rg -n "Register|InstructionOffset|UnlinkedInstruction|UnlinkedTypeGuard|CacheSiteId" crates/vela_mir/src
rg -n "mod hir_lowering|compile_hir_(expression|statement|pattern|root_body|value_body)|struct Compiler<'" crates/vela_bytecode/src/compiler.rs crates/vela_bytecode/src/compiler
rg -n "use_mir|enable_mir|mir.*fallback|fallback.*mir|old_backend|legacy.*lower|direct.*lower|temporary_mir|new_mir|mir_v2" crates/vela_bytecode crates/vela_mir
rg -n "vela_(hir|analysis|syntax)::|HirExprKind|HirStmtKind|HirPatternKind|HirBodyRoot" crates/vela_bytecode/src/compiler/mir_backend
rg -n "HirExprKind|HirStmtKind|HirPatternKind|HirBodyRoot" crates/vela_bytecode/src/compiler -g "!const_eval.rs"
cargo tree -p vela_mir
```

The first seven searches must have zero hits. Review `cargo tree -p vela_mir`
and confirm the dependency direction in Section 3; transitive syntax code used
inside HIR/analysis does not permit direct MIR syntax access.

Source-front-door parsing may remain outside `mir_backend`. Compile-time
const/schema evaluation may retain documented direct HIR traversal because it
does not emit runtime bytecode bodies. Every such hit outside the zero-hit
backend search must be reviewed explicitly; it is not a blanket exception for
direct body lowering.

---

## 14. Phase 7: Behavior Matrix And Final Acceptance

Purpose: prove semantic equivalence and prevent goal mode from completing on a
partial MIR implementation.

### 14.1 Required Behavior Matrix

- [ ] Literals: all scalar widths/suffixes, invalid numeric diagnostics,
  strings/bytes/chars, interpolation ordering, unit/null/tuple distinctions.
- [ ] Evaluation order: nested calls, short-circuit operators, side-effecting
  receivers/indexes/RHS, compound assignment, aliasing, and single evaluation.
- [ ] Control flow: if/else values, match/guards, loops, ranges, iterators,
  break/continue/return, unreachable paths, and try propagation.
- [ ] Bindings: parameters, defaults, locals, destructuring, pattern bindings,
  captures, nested/transitive lambdas, and frame debug names/spans.
- [ ] Calls: script, local closure, lambda, native, stdlib, value method, script
  method, dynamic, named/default arguments, and return guards.
- [ ] Values: arrays, maps, sets, records, enums, tuples, field/index access,
  constructors, projections, and mutations.
- [ ] Host/reflection: HostAccess read/write/mutate/remove/call, dynamic indexes,
  permissions, read-only fields, stale refs, reflection policy, and error spans.
- [ ] Runtime contracts: instruction/memory/call-depth budgets, GC under
  allocation/calls/closures, conservative roots, bytecode verification, cache
  sites, profile ownership, linking, hot reload identity/invalidation, and
  runnable examples.
- [ ] Diagnostics: existing error code, message, primary span, labels, and
  candidate/repair behavior where currently asserted.

### 14.2 Completion Criteria

- [ ] `vela_mir` model, builder, verifier, liveness, debug metadata, and tests
  cover every emitted MIR form.
- [ ] MIR-to-bytecode covers every currently executable language behavior.
- [ ] Verified MIR is the single runtime body-lowering input.
- [ ] The old direct Heavy-HIR-to-bytecode lowering path and migration oracle are
  deleted.
- [ ] No compatibility backend, fallback, feature flag, alias, temporary helper,
  transitional test, or migration naming remains.
- [ ] MIR IDs remain internal/generation-local and stable runtime identities are
  preserved.
- [ ] VM instructions and semantics remain unchanged; any separately approved
  metadata-only change is documented.
- [ ] JIT implementation remains deferred to M22.
- [ ] All active affected source/test files satisfy the 1200-line guideline.
- [ ] `docs/progress.md` describes MIR as complete only after all final commands
  and audits pass.

Final validation:

```bash
cargo test -p vela_hir
cargo test -p vela_analysis
cargo test -p vela_mir
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo test -p vela_host
cargo test -p vela_reflect
cargo test -p vela_hot_reload
cargo test -p vela_engine
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
```

Final zero-hit audits:

```bash
rg -n "^vela_(syntax|bytecode|vm|engine|language_service|lsp_server)\s*=" crates/vela_mir/Cargo.toml
rg -n "vela_(syntax|bytecode|vm|engine|language_service|lsp_server)::" crates/vela_mir/src
rg -n "Register|InstructionOffset|UnlinkedInstruction|UnlinkedTypeGuard|CacheSiteId" crates/vela_mir/src
rg -n "mod hir_lowering|compile_hir_(expression|statement|pattern|root_body|value_body)|struct Compiler<'" crates/vela_bytecode/src/compiler.rs crates/vela_bytecode/src/compiler
rg -n "use_mir|enable_mir|mir.*fallback|fallback.*mir|old_backend|legacy.*lower|direct.*lower|temporary_mir|new_mir|mir_v2" crates/vela_bytecode crates/vela_mir
rg -n "vela_(hir|analysis|syntax)::|HirExprKind|HirStmtKind|HirPatternKind|HirBodyRoot" crates/vela_bytecode/src/compiler/mir_backend
rg -n "HirExprKind|HirStmtKind|HirPatternKind|HirBodyRoot" crates/vela_bytecode/src/compiler -g "!const_eval.rs"
```

All searches must have zero hits. Also run a recursive line-count audit over
`crates/vela_mir/src` and touched active files under
`crates/vela_bytecode/src/compiler`; split every ordinary active source or test
file over 1200 lines unless a concrete exception is documented.

The plan is complete only when the final production pipeline is:

```text
source front door -> Heavy HIR + AnalysisFacts/compile targets
                  -> verified MIR
                  -> existing bytecode backend/linker/verifier
                  -> existing VM/runtime
```

No direct executable HIR-to-bytecode body lowering may remain beside it.
