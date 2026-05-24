# Progress

## Current Milestone

M0-M8 runnable prototype and semantic lowering complete enough to expand the
executable language surface. Current milestone: M9 complete executable
language surface. Loop-specific and closure/upvalue GC acceptance remains tied
to the later language constructs that introduce loops and closures.

## Completed

### M0: Workspace And Infrastructure

- Created the Cargo workspace layout with `crates/vela_common`.
- Added stable ID newtypes for common host, type, field, method, function,
  variant, object, and source identifiers.
- Added `SymbolInterner`, `SourceId`, `Span`, and a basic `Diagnostic` model.
- Added focused unit tests for symbol interning, span behavior, and diagnostic
  construction.
- Documented the default validation commands in `docs/validation.md`.

### M1: Syntax Frontend

- Added the `vela_syntax` crate.
- Implemented a lexer that produces tokens with `SourceId`/`Span` metadata and
  diagnostics for unterminated strings, unterminated block comments, and
  unexpected characters.
- Implemented a recoverable declaration parser for `use`, `fn`, `pub fn`,
  `struct`, `enum`, `trait`, and attributes.
- Added tests covering core module items, token spans, host-style compound
  assignment tokens, and recovery after invalid input.
- Split syntax code into focused AST, token, lexer, and parser modules.
- Added function-body AST parsing for `let`, `return`, `if/else`, `for-in`,
  `match`, blocks, field access, method calls, indexing, array/map literals,
  record literals, lambdas, assignments, and binary/unary expressions.
- Added parser tests for body statements, host-style assignment expressions,
  match arms, record/map literals, lambdas, and literal returns.
- Added compact parser snapshot coverage for core M1 syntax and recovery tests
  that assert malformed function-body diagnostics keep source spans.

### M2: Minimal Bytecode VM Loop

- Added `vela_bytecode` with `CodeObject`, constants, register IDs,
  instruction offsets, and an initial register instruction set.
- Added `vela_vm` with dynamic `Value`, call-frame registers, arithmetic,
  comparison, branching, returns, and registered native function calls.
- Added focused bytecode and VM tests for code-object construction, arithmetic,
  branches, and a mock `log` native call.
- Added a minimal AST-to-bytecode compiler for function bodies with literal
  constants, local `let` bindings, arithmetic/comparison expressions, returns,
  and native calls.
- Added VM integration tests that execute compiled source strings through the
  parser, compiler, bytecode, and VM loop.
- Added `Program` function storage, `CallFunction` bytecode, parameter register
  initialization, entrypoint argument passing, and VM dispatch for script
  function calls.
- Added compiled-source tests for calling one script function from another and
  passing arguments into a program entrypoint.
- Added `MakeArray` and `MakeMap` bytecode operations, compiler lowering for
  array/map literals, and VM tests that return array/map values from compiled
  source.
- Added compiler lowering for `if/else` statement branches using
  `JumpIfFalse`/`Jump` bytecode patching, with compiled-source tests for both
  then and else return paths.
- Added bytecode, compiler lowering, and VM execution for remainder and the
  remaining comparison operators used by M2 (`!=`, `<=`, `>`, `>=`), with a
  compiled-source operator test.

### M3: HostRef And PatchTx

- Added the `vela_host` crate with `HostRef`, `HostPath`, `PathSegment`,
  `Patch`, `PatchOp`, `HostValue`, `HostObjectSnapshot`, and `PatchTx`.
- Implemented transaction overlay updates for `Set` and read-modify-write
  `Add` patches without exposing Rust `&mut` references.
- Added host tests for set patch recording, add patch overlay behavior,
  read-after-write overlay semantics, and stale generation errors.
- Added `ScriptStateAdapter` and `MockStateAdapter` for host snapshot reads,
  patch validation, and safe-point patch application.
- Added tests that transaction reads prefer overlay values, adapter snapshots
  remain unchanged before apply, `Set`/`Add` patches commit at apply time, and
  stale generations are rejected on read/apply.
- Added VM host-field bytecode for `GetHostField`, `SetHostField`, and
  `AddHostField`, plus `Value::HostRef` and a host execution context carrying
  `ScriptStateAdapter` and `PatchTx`.
- Added VM tests that host reads go through `PatchTx`, host writes record
  patches without mutating adapter state until apply, `+=` records `Add`, and
  stale generations fail at the VM host boundary.
- Added compiler host-field bindings that lower parsed source such as
  `player.level = 10`, `player.level += 1`, and `return player.level` into
  host field bytecode.
- Added an end-to-end source test for script host mutation through
  parser -> bytecode -> VM -> `PatchTx` -> safe-point apply.
- Added `CallHostMethod` bytecode plus `PatchTx::call_method` recording and
  mock adapter safe-point application for controlled host method calls.
- Added host/VM tests showing host method calls are recorded as patches and
  applied later through the adapter.

### M4: Reflection System

- Added the `vela_reflect` crate with `TypeRegistry`, `TypeKey`, `TypeDesc`,
  `FieldDesc`, `MethodDesc`, `TraitDesc`, `VariantDesc`, and `AttrMap`.
- Added controlled `type_of`, `fields`, `reflect.get`, and `reflect.set`
  helpers for host refs and record-like reflection values.
- Routed `reflect.get(host_ref, "field")` through `PatchTx` overlay reads and
  `ScriptStateAdapter`, and routed `reflect.set(host_ref, "field", value)` to
  `PatchTx::set_path`.
- Added tests for host-ref patch creation, overlay reads, record field reads,
  read-only field errors, unknown-field candidate hints, type field queries,
  and propagation of host generation errors.
- Added controlled `reflect.call` and `reflect.implements` helpers that resolve
  host method and trait metadata through `TypeRegistry`.
- Routed `reflect.call(host_ref, "method", args)` to `PatchTx::call_method`
  so reflective host calls are applied only at the host safe point.
- Added tests for reflective host method patch recording, deferred apply,
  invalid reflective call arguments, unknown-method candidate hints, and trait
  implementation checks.
- Added VM host-native registration and script-visible `reflect.type_of`,
  `reflect.fields`, `reflect.get`, `reflect.set`, `reflect.call`, and
  `reflect.implements` native functions backed by `TypeRegistry`.
- Added compiled-source tests proving script reflection reads overlay host
  values, writes through `PatchTx`, returns field metadata, checks trait
  metadata, and records reflective host method calls for deferred apply.

### M5: Struct, Enum, And Match

- Added first-class VM record values with a type name and named fields.
- Added `MakeRecord` and `GetRecordField` bytecode operations.
- Lowered parsed record literals such as `Reward { item_id: "gold", count: 2 }`
  into record bytecode, including shorthand fields resolved from locals.
- Lowered two-part field reads to host-field access when a host binding exists,
  otherwise to record-field access.
- Added compiled-source tests for returning record values and reading record
  fields in arithmetic.
- Added first-class VM enum values with enum name, variant name, and named
  variant fields.
- Added `MakeEnum`, `GetEnumField`, and `EnumTagEqual` bytecode operations.
- Lowered multi-part record literals such as `Damage.Physical { amount: 7 }`
  into enum constructors.
- Added minimal match-tag lowering for enum path and record-variant patterns,
  including simple variant field bindings.
- Added compiled-source tests for returning enum values and matching enum tags
  with field destructuring.

### M6: Hot Reload First

- Added the `vela_hot_reload` crate with `ProgramVersion`,
  `ProgramVersionId`, `FunctionSymbolId`, `HotReloadRuntime`, `compile_update`,
  and `apply_hot_update`.
- Stored function code objects behind per-version `Arc<CodeObject>` entries so
  old `ProgramVersion` handles keep old code alive while the runtime points new
  calls at the updated version.
- Added ABI validation that rejects updates deleting existing function
  parameters.
- Added tests proving new calls enter new code, old version handles keep old
  code runnable, deleted parameters are rejected, and newly added helper
  functions are accepted.
- Added the `vela_cli` crate and
  `examples/game_server_demo/scripts/level_up.lang` as an executable demo path.
- Verified the demo script runs through parser, bytecode compiler, VM host
  execution, `PatchTx`, and safe-point host apply.

### M7: Runtime Safety, Budgets, And GC

- Added `ExecutionBudget` and `ExecutionBudgetKind` to the VM with limits for
  instructions, memory bytes, call depth, and patch count.
- Added budgeted VM entrypoints for plain code, programs, host execution, and
  host program execution while preserving the existing unbudgeted convenience
  entrypoints.
- Charged the instruction budget in the VM dispatch loop and preserved the
  executed-instruction counter on budget errors.
- Enforced maximum call depth across recursive script function calls.
- Enforced patch-count limits before direct VM host writes/method calls and
  after opaque host-native calls that may record patches.
- Added VM tests for instruction exhaustion, recursive call-depth exhaustion,
  and patch-count exhaustion without mutating host adapter state.
- Added a `vela_vm::heap` module with stable generation-checked `GcRef`
  handles, shallow memory accounting, explicit roots, and non-moving full
  mark-sweep collection.
- Added heap values for strings, arrays, maps, sets, records, and enums, with
  `HostRef` stored only as an external slot value that is not traced as
  Rust-owned state.
- Connected heap allocation and collection to the VM memory budget counters.
- Added heap tests proving live rooted objects survive collection, cyclic
  unrooted objects are reclaimed, stale references cannot access reused slots,
  host refs are not traced, and memory-budget failures do not mutate the heap.
- Added `Value::HeapRef` plus VM value tracing so active call-frame registers
  and nested inline aggregate values can be converted into explicit GC roots.
- Added VM tests proving register-held and nested heap refs keep heap objects
  alive during full collection while unrooted objects are swept.
- Added `GcConfig`, `GcBudget`, `GcStepStats`, threshold tracking, and
  resumable `step_gc` sweeping for event/tick safe-point pacing.
- Added heap tests proving stepped GC can pause and resume sweeping, preserves
  roots across incremental steps, releases execution memory budget for swept
  objects, restarts cleanly when a full collection interrupts a step, and
  updates collection thresholds from heap growth config.
- Added explicit heap-backed VM execution entrypoints with `HeapExecution`.
- In heap execution mode, string constants plus array, map, record, and enum
  bytecode constructors allocate into `ScriptHeap` and charge the memory budget.
- Added heap-backed record field reads, enum field reads, and enum tag checks
  while preserving the existing inline execution APIs and return shapes.
- Added VM tests proving heap execution allocates compiled array/string values,
  reads heap-backed record fields, matches heap-backed enum variants, and
  rejects bytecode allocations that exceed the memory budget.
- Added heap-aware native call argument materialization so native functions see
  ordinary `Value` shapes when called from heap-backed bytecode.
- Added heap-aware native return storage for string and aggregate results.
- Added heap-aware host `HostValue` conversion for heap-backed strings used in
  host field writes and host method call arguments.
- Added VM tests proving heap-backed native args/results and host string patch
  conversions work under memory budgeting.
- Added heap-aware equality by materializing compared heap refs, allowing
  comparisons such as `reflect.type_of(player) == "Player"` to work in
  heap-backed execution.
- Added VM tests proving heap-backed reflection natives can query traits,
  read/write host state through `PatchTx`, and return field metadata arrays
  stored in the script heap.
- Added safe-point stepped GC execution to heap-backed VM dispatch, using
  current call-frame roots plus protected caller roots during nested script
  calls.
- Added a VM test proving safe-point GC can sweep unreachable objects created
  by a nested call without collecting heap refs still held by the caller frame.
- Added managed heap VM entrypoints that own a temporary `ScriptHeap`, execute
  the heap-backed path, materialize returned heap refs, and release temporary
  heap memory from `ExecutionBudget` after success or failure.
- Moved the `vela_cli` demo runner onto managed heap execution with explicit
  instruction, memory, call-depth, and patch budgets.

### M8: Resolver, HIR, And Module Graph

- Added the `vela_hir` crate and workspace wiring.
- Added `ModuleId`, `HirNodeId`, `HirExprId`, and `HirDeclId` stable HIR IDs.
- Added `ModuleGraph`, `ModulePath`, `ModuleSource`, `DeclarationIndex`, and
  first-phase declaration metadata for functions, structs, enums, and traits.
- Lowered parsed module items into HIR declaration indexes while preserving
  source spans and visibility.
- Added cross-module `use` resolution for imported declarations.
- Added diagnostics for duplicate modules, duplicate declarations with both
  related spans, unresolved modules, and unresolved imports with candidate
  hints.
- Added HIR tests for declaration indexing, cross-module import resolution,
  duplicate declaration spans, and unresolved import suggestions.
- Added first-phase function binding maps with stable `HirExprId` and
  `HirLocalId` allocation.
- Binding maps now track parameter, `let`, `for`, lambda parameter, and match
  pattern bindings, plus expression-to-binding resolutions for locals,
  module-level declarations, and imported names.
- Added unresolved value-name diagnostics with candidate hints while avoiding
  false positives for namespace-style native/module calls.
- Added HIR tests for local binding resolution, nested `for`/lambda scopes,
  imported names in function bodies, and unresolved-name suggestions.
- Wired `vela_bytecode` source compilation through the HIR module graph before
  bytecode generation.
- Added compiler tests proving HIR diagnostics reject duplicate declarations
  and unresolved names before bytecode generation, valid program bytecode still
  compiles, and top-level mutation remains rejected before code generation.
- Added syntax AST nodes for lightweight type hints on function parameters,
  function returns, `let` bindings, lambda parameters, and struct fields.
- The parser now preserves type-hint metadata, rejects script generic type
  syntax such as `Array<int>`, and keeps bytecode execution semantics unchanged.
- HIR now exposes function signature metadata, struct field metadata, and
  optional local binding type hints for parameters, `let` bindings, and lambda
  parameters.
- Added syntax, HIR, and compiler tests proving type hints are preserved as
  metadata and generic type hints are rejected before bytecode generation.
- Added parser and HIR support for module-level `const` declarations with
  optional type hints and expression initializers.
- HIR now indexes const declarations, preserves const initializer spans and
  type-hint metadata, and rejects side-effecting const initializers such as
  calls and assignments with `hir::top_level_side_effect`.
- Added compiler tests proving pure const declarations can coexist with
  functions while side-effecting const initializers stop before bytecode
  generation.
- Added parser support for `impl Trait for Type { fn ... }` blocks and method
  parameter parsing for `self`.
- HIR now indexes impl declarations, preserves trait/target paths, method
  signatures, method body spans, and per-method binding maps keyed by stable
  HIR nodes.
- Added HIR and compiler tests proving impl metadata participates in semantic
  validation while impl methods remain out of top-level bytecode program
  exports.
- Bytecode source compilation now carries the HIR module graph forward after
  semantic validation and uses HIR function declarations/signatures for script
  function discovery and emitted `CodeObject` parameter names.
- Added compiler tests proving HIR signatures drive code object params and impl
  methods are not exported as top-level script functions.
- Exposed focused HIR binding-map lookups for local bindings and expression
  span resolutions.
- Bytecode local register allocation now records HIR local IDs and resolves
  local/path reads through HIR binding facts before falling back to legacy name
  lookup.
- Added compiler regression coverage proving nested shadowed locals return the
  HIR-resolved outer binding instead of the most recent same-name register.
- Bytecode call lowering now carries HIR function declaration IDs and emits
  `CallFunction` only when the callee expression resolves to a HIR function
  declaration.
- Added compiler regression coverage proving a local that shadows a function
  name no longer compiles as a script function call.
- Record shorthand fields now carry source spans, bind through HIR like value
  reads, and compile from HIR local resolutions instead of legacy name lookup.
- Added HIR and compiler regression coverage proving record shorthand fields
  resolve the semantic binding even after nested block shadowing.
- HIR binding maps now resolve imported names to stable declaration IDs when
  imports are available, and refresh existing binding maps after
  `resolve_imports()` handles forward module imports.
- Added HIR coverage proving imported value reads resolve to declaration facts
  instead of string-only import placeholders.
- Bytecode match lowering now records record-pattern field bindings by HIR
  local ID and restores HIR local maps after each match arm.
- Added compiler regression coverage proving match pattern field bindings still
  resolve correctly after nested arm-body shadowing.
- Bytecode local path lowering now recognizes HIR const declaration
  resolutions and compiles literal const initializers into loadable bytecode
  constants.
- Added compiler regression coverage proving literal top-level const reads no
  longer fall through to legacy unknown-local lookup.
- Bytecode const lowering now evaluates source-order pure scalar const
  expressions, including references to earlier const declarations, without
  introducing top-level execution.
- Added compiler and VM coverage proving const expression reads compile and run
  as scalar values.
- Syntax and HIR now preserve `use path as alias` metadata and bind imported
  declarations under the alias name in function bodies.
- Added syntax and HIR coverage proving import aliases resolve to the target
  declaration while exposing the alias as the local binding name.
- Added multi-module bytecode compilation from HIR `ModuleSource` inputs and
  declaration-to-function symbol mapping for script calls.
- Added compiler and VM coverage proving an aliased imported function call
  compiles as `CallFunction` and executes across modules.
- Multi-module bytecode now uses module-qualified function symbols such as
  `game.reward.grant`, preventing same-named functions in different modules
  from overwriting each other in `Program`.
- Added compiler and VM coverage proving same-named functions in separate
  modules compile and dispatch through their qualified symbols.
- Multi-module scalar const evaluation now follows resolved import
  declarations, allowing const initializers and function bodies to read
  imported const aliases without top-level execution.
- Added compiler and VM coverage proving imported const expressions compile
  and execute across modules independently of source input order.
- HIR binding maps now record declaration resolutions for record and enum
  constructor paths when the type name or alias is known.
- Multi-module bytecode now emits declaration-qualified type names for imported
  struct and enum constructors, while undeclared prototype record literals keep
  their source-spelled names.
- Added HIR, compiler, and VM coverage proving imported constructor aliases
  compile and execute with qualified type metadata.
- HIR binding maps now record enum match pattern root resolutions separately
  from expression resolutions, keyed by the pattern path until pattern HIR has
  dedicated node IDs.
- Bytecode match tag checks now use HIR-resolved enum type symbols for
  imported aliases, keeping constructor and match metadata consistent.
- Added HIR, compiler, and VM coverage proving imported enum aliases match
  qualified constructed values across modules.
- HIR binding maps now preserve unresolved module-qualified paths as
  refreshable semantic placeholders and resolve them to declarations after the
  full module graph is available.
- Bytecode path lowering now recognizes HIR-resolved qualified const paths, and
  call lowering uses HIR-resolved qualified function paths for `CallFunction`.
- Added HIR, compiler, and VM coverage proving direct paths such as
  `game.reward.grant()` and `game.config.BONUS` compile and execute across
  modules even when the target module is parsed later.
- HIR import and qualified-path resolution now respect declaration visibility:
  cross-module resolution only exposes `pub` declarations while same-module
  references can still see private declarations.
- Added HIR and compiler coverage proving private imports are rejected before
  bytecode generation and private qualified paths do not resolve to
  cross-module declaration IDs.

### M9: Complete Executable Language Surface

- Added bytecode instructions for unary logical-not and numeric negation.
- Lowered parsed unary `!` and unary `-` expressions through the bytecode
  compiler instead of rejecting them as unsupported syntax.
- Added VM execution for truthiness-based `!` and numeric-only unary `-`,
  including overflow/type errors through the VM error path.
- Added compiler and VM coverage proving unary operators compile and execute
  from source.
- Added compiler lowering for short-circuiting `&&` and `||` using existing
  truthiness, branch, constant, and unary-not bytecode.
- Added VM coverage proving short-circuited logical RHS calls are not executed
  and logical expressions produce boolean results.
- Added local assignment lowering for `=`, `+=`, `-=`, `*=`, `/=`, and `%=`
  by writing computed values back into stable HIR local registers.
- Added compiler and VM coverage proving local assignment statements and
  assignment expressions compile and execute from source.
- Added `GetIndex` bytecode, compiler lowering for index read expressions, and
  a focused VM indexing module for array/map lookup.
- Added compiler and VM coverage proving array and map index reads execute in
  both inline and managed-heap execution modes.
- Added `SetIndex` bytecode and compiler lowering for array/map index
  assignment expressions, including compound numeric assignment.
- Added VM coverage proving index writes execute for inline and managed-heap
  arrays/maps while keeping host-path indexing out of this M9 slice.
- Added `IterInit` and `IterNext` bytecode plus compiler lowering for `for-in`
  loops over script arrays and maps.
- Added VM coverage proving `for-in` loops execute in inline and managed-heap
  modes, with map loops iterating values in key order.
- Added compiler lowering for `break` and `continue` inside `for-in` loops via
  loop-scoped jump patching, with explicit diagnostics outside loops.
- Added compiler and VM coverage proving `break`/`continue` work through nested
  control-flow blocks.
- Added source lowering for configured root host method calls such as
  `player.grant_exp(20)` into `CallHostMethod` bytecode.
- Added VM coverage proving source-level host method calls record `PatchTx`
  method-call patches and apply them only at the host safe point.
- Added compiler lowering for block expression values using the final
  expression statement as the block value, falling back to `null` for empty or
  statement-only blocks.
- Added compiler lowering for `if` expression values by merging branch results
  into a stable destination register, with explicit diagnostics when an
  expression-valued `if` omits `else`.
- Added compiler and VM coverage proving block and `if` expression values
  compile and execute from source.
- Added compiler lowering for `match` expression values using the existing
  executable enum tag, record-variant, and wildcard patterns.
- Added VM coverage proving `match` expression arms merge values from both
  expression and block arm bodies.
- Added compiler lowering for literal match patterns by comparing the scrutinee
  with compiled literal constants.
- Added VM coverage proving integer and heap-backed string literal patterns
  execute from source.
- Added compiler lowering for binding match patterns by moving the scrutinee
  into a fresh pattern-local register.
- Added VM coverage proving binding patterns execute from source and assignment
  to the binding does not mutate the original scrutinee.

## Next

- Continue M9 language-surface execution by lowering and running the remaining
  planned expression and statement forms from `docs/grammar.ebnf`.
