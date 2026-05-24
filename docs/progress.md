# Progress

## Current Milestone

M0-M11 runnable prototype, stable script metadata, broad executable language
surface, and host bridge foundations are complete enough to begin reflection
and permission expansion. Current milestone: M12 complete reflection and
permissions.

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
- Split `vela_host` into focused modules for paths, values, patches, errors,
  adapters, transactions, mock adapters, and tests while preserving the public
  crate API.
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
- Added `SetRecordField` bytecode and compiler lowering for direct script
  record field assignment and compound assignment without treating record
  fields as host paths.
- Added VM coverage proving existing record fields can be updated in inline and
  managed-heap execution while preserving heap-safe assigned values.
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
- Added return propagation for `return` statements inside block-expression
  `let` initializers.
- Extended `let` initializer return propagation to all-return `if` and
  `match` expression values.
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
- Added compiler lowering for `match` guards after pattern locals are bound,
  with false guards falling through to the next arm.
- Added VM coverage proving guards can read binding-pattern locals and
  record-pattern field bindings.
- Added compiler and VM execution for record-variant field subpatterns,
  including literal field checks and nested tuple-variant patterns in inline
  and managed-heap execution.
- Added executable grammar coverage for hexadecimal integer literals, binary
  integer literals, and decimal float literals with exponents.
- Added lexer, compiler, and VM coverage for leading shebang lines as source
  file layout before module items.
- Added lexer, compiler, and VM coverage for `\u{...}` Unicode string escapes.
- Added HIR resolution and compiler lowering for declared tuple-style enum
  constructor calls such as `Damage.Physical(7, 2)`.
- Added tuple variant pattern destructuring with positional enum fields and VM
  coverage for inline and managed-heap execution.
- Added parser and HIR metadata for function parameter default expressions and
  named call arguments.
- Added compiler and VM support for named script-call argument reordering plus
  callee-side default parameter prologues, including managed-heap string
  defaults.
- Added compiler lowering for lambda expressions into nested `CodeObject`
  closures with explicit capture registers.
- Added VM closure values, closure-call dispatch, and capture initialization
  before lambda parameters, preserving captures after the outer function returns.
- Added compiler and VM coverage proving captured closures and immediate lambda
  calls execute from source.
- Added bytecode and VM execution for postfix `?` propagation over dynamic
  `Option.Some`/`Option.None` and `Result.Ok`/`Result.Err` enum values.
- Added compiler and VM coverage proving `?` unwraps success payloads and
  returns failure variants early in both inline and managed-heap execution.
- Added lexer/parser support for exclusive `..` and inclusive `..=` range
  expressions.
- Added bytecode and VM execution for integer range values as lazy iterables,
  with compiler and VM coverage for range-based `for-in` loops in inline and
  managed-heap execution.
- Added script-value method lowering and VM dispatch for `len()` and
  `is_empty()` on strings, arrays, maps, records/enums, ranges, and heap-backed
  collection values.
- Added compiler and VM coverage proving value methods execute in inline and
  managed-heap execution while configured host methods still lower through
  `CallHostMethod`.
- Added compiler lowering for multi-segment local record paths and chained
  method receivers such as `reward.item_id.len()` without treating qualified
  module function calls as methods.
- Added read-only map script methods `has`, `get`, and `get_or` for inline and
  heap-backed maps.
- Added VM coverage proving map methods preserve dynamic map values, return
  `null` for missing `get` keys, and keep fallback values heap-safe in managed
  execution.
- Added deterministic read-only map script methods `keys`, `values`, and
  `entries`, with `entries` returning script-visible `MapEntry` records in
  inline and managed-heap execution.
- Added mutating collection script methods `array.push`, `array.pop`,
  `map.set`, and `map.remove`, with method dispatch writing mutated receiver
  registers back in inline and managed-heap execution.
- Added string predicate script methods `contains`, `starts_with`, and
  `ends_with` for inline and heap-backed strings.
- Added VM standard native registration for `math.max`, `math.min`,
  `math.clamp`, `math.floor`, `math.ceil`, and `math.abs`, with source-level
  inline and managed-heap execution coverage.
- Added array higher-order script methods `map`, `filter`, `find`, `any`,
  `all`, and `count` backed by script closures, preserving VM budgets, host
  context, and managed-heap roots during callback execution.
- Added map higher-order script methods `map_values` and `filter`, plus
  value-predicate `any`, `all`, and `count`, with receiver-category dispatch
  shared with array methods in inline and managed-heap execution.
- Added `array.sum` for direct numeric totals and callback-transformed numeric
  totals, preserving integer results until a float participates and returning
  `0` for empty arrays.
- Added `array.group_by` for lambda-derived string keys, returning deterministic
  maps of grouped arrays while preserving input order within each group in
  inline and managed-heap execution.
- Added `array.sort_by` for stable, non-mutating sorting by numeric or string
  lambda keys, with managed-heap execution support and explicit type errors for
  mixed key domains.
- Added first script-visible set APIs through `set.from_array`, `set.has`,
  `set.add`, `set.remove`, `set.values`, `len`, `is_empty`, and `for-in`
  iteration, backed by `Value::Set` and managed-heap `HeapValue::Set`.
- Added canonical Option/Result standard constructors `option.some`,
  `option.none`, `result.ok`, and `result.err`, with source-level `?`
  propagation coverage in inline and managed-heap execution.
- Added runnable context/event demo coverage for `ctx.now`, `ctx.tick`, and
  `ctx.emit(...)` through the existing HostRef/PatchTx bridge, with VM
  source-level managed-heap coverage and a structured CLI demo module.
- Added `examples/game_server_demo/scripts/monster_kill_reward.lang` as a
  runnable demo proving a monster kill can award exp, level up a player, record
  a reward host method call, and emit gameplay events through `PatchTx`.
- Added `examples/game_server_demo/scripts/quest_progress.lang` as a runnable
  demo proving quest counters and completion flags update through host field
  patches and emit a quest completion event at the safe point.
- Added `examples/game_server_demo/scripts/reflect_debug.lang` as a runnable
  demo proving script reflection can inspect allowed host fields, check trait
  metadata, perform controlled host writes, and call host methods through
  `PatchTx`.
- Added `examples/game_server_demo/scripts/hot_reload_function_swap_v1.lang`
  and `hot_reload_function_swap_v2.lang` plus a `vela_cli --hot-reload`
  command proving old program versions keep old code while new calls enter the
  updated version.
- Added CLI integration tests that execute the runnable game server demo
  scripts through the built `vela_cli` binary.
- Split the CLI demo runner into focused ID and host-state modules so the demo
  harness can keep growing without accumulating all logic in one file.
- Added a focused CLI demo TypeRegistry module for Player, Context, and Monster
  reflection metadata used by the runnable demo scripts.
- Added a focused CLI hot-reload demo module so function-swap workflows use the
  `vela_hot_reload` runtime instead of custom CLI-only code swapping.

### M10: Script Types, Shapes, Traits, And Dispatch

- Added HIR enum-shape metadata alongside existing struct-shape metadata so
  enum declarations expose variant names through the module graph.
- Exposed module graph declaration iteration for consumers that need stable
  metadata without re-reading syntax items.
- Added `TypeRegistry::register_script_types` in a focused reflection
  `script_types` module, registering HIR script structs and enums under
  module-qualified names.
- Added script `VariantDesc` IDs and deterministic `TypeId`/`FieldId`/
  `VariantId` generation from qualified type and member names.
- Added reflection tests proving script struct fields and enum variants enter
  the registry and member IDs survive source reordering.
- Added `TypeKind` and `SchemaHash` metadata to type descriptors, with
  order-independent schema hashes for script structs and enums.
- Added reflection tests proving schema hashes survive field/variant reordering
  and change when script type members or field hints change.
- Added `ShapeId` and a focused VM `ScriptFields` slot container for script
  record and enum payloads, replacing named-map storage in inline and heap
  script objects while preserving script-visible field behavior.
- Added VM coverage proving compiled record constructors produce stable
  slot-shape IDs across source field reordering.
- Added slot-index bytecode for record and enum field reads plus record field
  writes, with VM validation that the expected field still matches the slot.
- The bytecode compiler now lowers immediate record/enum literal field reads
  such as `Reward { count: 2 }.count` to slot-index bytecode.
- HIR now preserves script trait method signatures and whether a trait method
  has a default body.
- TypeRegistry now registers script trait declarations and attaches script
  impl-block trait metadata to script types, including stable trait and trait
  method IDs.
- Added syntax, HIR, and reflection coverage for trait default metadata and
  script `impl Trait for Type` registration.
- Bytecode programs now carry a script method dispatch table, and the compiler
  emits script impl methods as hidden code objects keyed by receiver type and
  method name.
- The VM now falls back from built-in script value methods to script impl method
  dispatch for record and enum receivers in inline, managed-heap, and
  module-qualified execution.
- Trait default method bodies now remain in the syntax AST, are bound in HIR,
  and compile into hidden method code objects when an impl omits the method.
- The VM now executes trait default methods through the same script method
  dispatch path, with explicit impl methods taking precedence in inline and
  managed-heap execution.
- Reflection now preserves script record and enum type names when converting VM
  values, allowing `reflect.type_of`, `reflect.fields`, `reflect.get`, and
  `reflect.implements` to query script type metadata at runtime.
- Script records and enums can now satisfy dynamic implements checks through
  `TypeRegistry::register_script_types`, including script-visible checks from
  module-qualified compiled programs.
- Script method dispatch table entries now carry stable `MethodId` metadata
  derived from the implemented trait method and can be looked up by receiver
  type plus `MethodId`, while dynamic name lookup remains available.
- Added `CallMethodId` bytecode plus VM dispatch through `receiver type +
  MethodId`, and the compiler emits it for immediate script record/enum
  receiver method calls when the script method metadata is known.
- Added focused local script type-flow facts for let-bound script record/enum
  values, allowing `player.bonus(...)` style calls to lower to `CallMethodId`
  after `let player = Player { ... }`.
- Extended compiler script type-flow facts to parameter and explicit `let`
  type hints, including unambiguous module-qualified script type names, so
  typed receiver calls can lower to `CallMethodId`.
- Hidden script impl/default method bodies now seed `self` as the impl target
  type, allowing `self.other_method()` calls to lower to `CallMethodId` when
  the target method metadata is known.
- Lambda compilation now carries captured local script type-flow facts into
  nested closure code objects, so captured script record/enum receivers can
  lower method calls to `CallMethodId`.
- Match binding patterns now preserve known script receiver type facts from
  the scrutinee for simple bindings such as
  `match player { bound => bound.bonus(5) }`.
- The compiler now derives declared script struct field slots from HIR shape
  metadata and lowers field reads and writes on typed struct receivers to
  `GetRecordSlot`/`SetRecordSlot`, extending slot bytecode beyond immediate
  record literals.
- Syntax, HIR, reflection, and compiler metadata now preserve declared enum
  tuple/record variant payload fields; typed locals initialized from declared
  enum constructors can lower variant field reads to `GetEnumSlot`.
- Destructured record and tuple variant pattern locals now preserve declared
  enum payload script type facts, allowing method calls on those locals to
  lower to `CallMethodId`.
- Host refs can now dispatch to script impl methods by registered host type
  name through VM-held `TypeRegistry` metadata, while script method bodies keep
  host interaction behind reflection/host APIs.
- `Vm::register_type_registry` and `Vm::with_type_registry` now install host
  type metadata explicitly, so host ref script impl dispatch no longer depends
  on also registering reflection natives.
- Added a focused `vela_engine` crate with an initial `Engine`/`EngineBuilder`
  API for explicit host type/schema registration, native function descriptors
  with stable IDs, duplicate validation, and installation into `Vm`.
- Engine registration now supports host-aware native functions that receive
  `HostExecution` and can record host mutations through `PatchTx`; duplicate
  native IDs and names are checked across pure and host-aware native entries.
- Added Engine-owned permission grants and native descriptor permission
  requirements; installed pure and host-aware natives now reject calls with a
  `PermissionDenied` VM error before invoking the Rust callback or recording
  patches.
- Engine now derives bytecode compiler host-method options from registered
  `TypeDesc::methods`, allowing host schemas to drive `CallHostMethod`
  lowering. The bytecode compiler now supports type-qualified host method
  mappings, so typed host receivers can disambiguate shared method names across
  different host schemas while preserving legacy name-only mappings.
- Added Engine native method descriptors and callables keyed by `HostMethodId`.
  Native method registration injects method metadata into the owner host type,
  exposes callable lookup/dispatch through `Engine::call_native_method`, and
  still takes a `HostPath` plus `HostExecution` instead of a Rust `&mut`.
- `CallHostMethod` bytecode now carries host field path segments, allowing
  configured calls such as `player.inventory.add(...)` to compile and record
  a `PatchTx` method call against `HostPath::new(player).field(inventory)`.
- Added field-only nested host path bytecode for reads, sets, and add-RMW
  operations. Configured paths such as `player.stats.level += 2` now compile
  to `AddHostPath`, record a nested `PatchTx` patch, and later reads observe
  the transaction overlay.
- Host path bytecode now carries ordered static field segments and dynamic
  bracket segments. Paths such as `player.inventory.items[item_id].count += 1`
  compile to indexed/keyed `AddHostPath`, and runtime string segment values
  become `HostPath::key` entries while integer values become `HostPath::index`
  entries.
- `CallHostMethod` now uses the same ordered host path segment bytecode as
  reads and writes, so calls such as
  `player.inventory.items[item_id].grant(20)` record method patches against
  indexed/keyed `HostPath` receivers.
- `PatchTx` now supports subtraction RMW patches with overlay reads, mock
  adapter validation/apply, and VM/compiler lowering for host `-=` through
  `SubHostField` and `SubHostPath`.
- `PatchTx` now supports push patches for array-valued host paths, including
  overlay updates, mock adapter apply, `HostValue::Array` conversion, and
  compiler/VM lowering for `host.path.push(value)` through `PushHostPath`.
- `PatchTx` now supports remove patches with transaction tombstones, mock
  adapter apply, and compiler/VM lowering for `host.path.remove()` through
  `RemoveHostPath`; reads after a remove fail from the overlay instead of
  falling back to the adapter snapshot.
- Host boundary value conversion now supports map-valued host paths in
  addition to arrays and scalars, including managed-heap script maps written
  through `PatchTx` and exact-path overlay reads returning script map values.
- Host boundary value conversion now also supports record-valued host paths
  with copied type names and fields. VM host conversions live in a focused
  `host_values` module, and managed-heap script records can be written through
  `PatchTx` and read back from exact-path overlays as script records.
- Host boundary value conversion now supports enum-valued host paths with
  copied enum names, variant names, and fields. Managed-heap script enums can
  be written through `PatchTx` and read back from exact-path overlays as
  script enum values.
- Host boundary value conversion now supports host-ref values as copied
  external handles through `PatchTx`. Host refs remain outside the script heap
  ownership model and are not traced as Rust-owned state.
- `PatchTx::apply` now routes through an adapter-level batch apply hook.
  `MockStateAdapter` validates the batch and restores its snapshot if a later
  patch fails during apply, proving failed mock applies leave adapter state
  unchanged.
- `MockStateAdapter` now supports explicit read, write, and call denial for
  host paths. Denied writes and calls fail during batch validation before any
  patch mutates adapter state or records a method call.
- Host errors now carry optional source spans, and `PatchTx`/adapter batch
  apply preserve patch instruction spans on transaction read failures,
  permission validation failures, and late apply failures. VM host-read errors
  now keep the bytecode instruction span when converting host errors.
- Read-modify-write and push patches now carry the base value observed before
  the transaction overlay mutated the path. `MockStateAdapter` reports a
  structured patch conflict if the host value changed before safe-point apply,
  preserving the patch source span and leaving adapter state unchanged.
- Host state adapters now expose a read-only method-return preview hook.
  `CallHostMethod` bytecode writes that copied preview value to the destination
  register while still recording a deferred method-call patch for safe-point
  apply, so scripts can observe host method returns without receiving mutable
  Rust references.

### M12: Complete Reflection And Permissions

- Added a focused reflection `modules` module with `ModuleDesc`,
  `FunctionDesc`, function parameter metadata, module exports, declaration
  origin metadata, and stable reflected function IDs.
- `TypeRegistry::register_script_modules` now registers script modules and
  function metadata from the HIR module graph, including visibility, type-hint
  display metadata, default-parameter markers, return hints, and module export
  entries.
- Added read-only script-visible `reflect.module`, `reflect.exports`, and
  `reflect.function` queries backed by registered module/function metadata.
  Results are copied records/arrays, and unknown module/function lookups include
  candidate hints without allowing runtime schema mutation.
- Added a focused reflection member-query module plus script-visible
  `reflect.methods`, `reflect.has_method`, `reflect.traits`,
  `reflect.variants`, `reflect.variant`, and `reflect.variant_is` natives.
  These return copied method, trait, and variant metadata records and preserve
  current enum variant inspection without exposing mutable schema handles.

## Next

- Continue M12 with field detail queries, reflection permissions, lookup
  budgets, and permission-bounded reflective calls.
