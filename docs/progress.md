# Progress

## Current Focus

M0-M11 runnable prototype, stable script metadata, broad executable language
surface, and host bridge foundations are complete enough to continue
reflection, permissions, standard-library expansion, focused Engine embedding
slices, and hot-reload ABI/source workflows. Current milestone: M12/M13
reflection and standard-library completion, with targeted M14/M15 Engine API
work as it unblocks embedding and safe-point reload workflows.

Performance work is now documented as an explicit post-MVP track. The target is
an optimized non-JIT interpreter that can be compared against Lua 5.x on
representative gameplay workloads; LuaJIT, Node.js, and Cranelift JIT remain
post-MVP reference or optional tracks, not MVP requirements.

## Milestone Status

| Milestone | Status | Notes |
|---|---|---|
| M0-M6 | Complete | Runnable source -> bytecode -> VM -> host patch -> hot reload prototype loop. |
| M7 | Complete | Execution budgets, managed heap path, GC roots, and managed heap entrypoints are in place. |
| M8 | Complete enough | HIR, module graph, imports, declarations, binding maps, and compiler integration are active. |
| M9 | Complete enough | Broad executable language surface works; conformance continues to catch edge cases. |
| M10 | Complete enough | Stable script metadata, shapes, slots, traits, and dispatch foundations exist, with optimization still pending. |
| M11 | Complete enough | HostRef, HostPath, PathProxy, PatchTx, overlays, and rollback-safe host boundaries are established. |
| M12 | In progress | Reflection metadata surface and permission-aware queries are still being polished. |
| M13 | In progress | Standard library helpers are broad but still need final gameplay, string, math, context, and propagation polish. |
| M14 | Partial | Engine APIs, native descriptors, context helpers, and macros exist in slices; embedding still needs completion. |
| M15 | Partial | Function, descriptor, module, trait, schema, and source reload ABI checks exist; full production safe-point workflow remains. |
| M16 | Partial | Runtime diagnostics, common rendering, and CLI compile-error rendering have started; broader tooling fixtures remain. |
| M17 | Partial | First conformance fixture and integration harnesses exist; game-server demo still needs expansion. |
| M18 | Not started | Official performance baselines are not in tracked benchmark form yet. |
| M19-M22 | Not started | Non-JIT optimization, inline caches, optional JIT evaluation, and release performance hardening are post-MVP work. |

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
- Kept `PatchTx` orchestration separate from patch overlay state by moving
  overlay read/write policy into a focused host module.
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
  reflection helper results and heap-backed strings to compare correctly in
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
  into a stable destination register and treating missing `else` branches as
  `null`.
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
  dynamic `Option.Some(value)`/`Option.None` for `get`, and keep fallback
  values heap-safe in managed execution.
- Added deterministic read-only map script methods `keys`, `values`, and
  `entries`, with `entries` returning script-visible `MapEntry` records in
  inline and managed-heap execution.
- Added mutating collection script methods `array.push`, `array.pop`,
  `map.set`, and `map.remove`, with method dispatch writing mutated receiver
  registers back in inline and managed-heap execution.
- Updated `map.remove` to return dynamic `Option.Some(value)` or
  `Option.None`, matching `map.get` and the analysis-only stdlib facts.
- Added string predicate script methods `contains`, `starts_with`, and
  `ends_with` for inline and heap-backed strings.
- Split string script methods into a focused VM module and added gameplay
  string utilities `trim`, `to_lower`, `to_upper`, and `split` with inline and
  managed-heap execution coverage.
- Added `string.parse_int()` for deterministic text-to-int parsing. It returns
  dynamic `Option.Some(int)` on valid `i64` input and `Option.None` for invalid
  or out-of-range strings in inline and managed-heap execution.
- Added `string.parse_float()` for deterministic finite text-to-float parsing.
  It returns dynamic `Option.Some(float)` for finite `f64` input and
  `Option.None` for invalid or non-finite strings in inline and managed-heap
  execution.
- Added `string.parse_bool()` for deterministic text-to-bool parsing. It
  accepts exact `true` and `false` literals, returns dynamic
  `Option.Some(bool)` or `Option.None`, and works in inline and managed-heap
  execution.
- Added VM standard native registration for `math.max`, `math.min`,
  `math.clamp`, `math.floor`, `math.ceil`, and `math.abs`, with source-level
  inline and managed-heap execution coverage.
- Added Engine-installed controlled random through permission-gated
  `math.random(min, max)`. The seeded native is deterministic per Engine,
  carries stable reflection metadata, and fails before execution unless the
  host grants `std.random`.
- Added Engine source-loading APIs `compile_file` and `compile_dir`.
  `compile_file` uses Engine-derived compiler options for a single source
  file, while `compile_dir` recursively loads `.lang` files, derives module
  paths from relative file paths, assigns deterministic source IDs, and
  compiles the resulting module graph.
- Added an Engine `Runtime` wrapper with `CallOptions`. Runtime calls install
  Engine schemas/natives into a VM, execute with configured instruction,
  memory, call-depth, and patch budgets, use managed heap execution by default,
  and leave host mutations in the caller-provided `PatchTx` for safe-point
  application.
- Added array higher-order script methods `map`, `filter`, `find`, `any`,
  `all`, and `count` backed by script closures, preserving VM budgets, host
  context, and managed-heap roots during callback execution.
- Updated `array.find` to return dynamic `Option.Some(value)` or
  `Option.None` instead of `null`, aligning runtime behavior with
  Option-style propagation and the analysis-only stdlib facts.
- Added map higher-order script methods `map_values` and `filter`, plus
  value-predicate `any`, `all`, and `count`, with receiver-category dispatch
  shared with array methods in inline and managed-heap execution.
- Added `map.find` with lambda predicates, returning
  `Option.Some(MapEntry { key, value })` or `Option.None` in inline and
  managed-heap execution.
- Split shared higher-order method callback execution into a focused VM
  `method_runtime` module, so array and map stdlib modules no longer own each
  other's heap-root and budget plumbing.
- Moved the remaining map script-value method bodies (`has`, `get`, `get_or`,
  `set`, `remove`, `keys`, `values`, and `entries`) into the focused
  `map_methods` VM module, leaving `script_methods` as receiver dispatch glue.
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
- Extended the monster-kill demo workflow to advance quest progress, complete
  the seeded quest, and emit the quest completion event in the same `PatchTx`
  safe-point path that applies exp, level, inventory, and reward changes.
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
- Extended the same member-query module with read-only `reflect.name`,
  `reflect.kind`, `reflect.field`, and `reflect.has_field` natives. Field
  detail records include stable IDs, names, and writable flags, and unknown
  fields keep ranked candidate hints.
- Added `ReflectPermission` and `ReflectPermissionSet` in a focused reflection
  permissions module, plus `Vm::register_reflection_natives_with_permissions`
  and `EngineBuilder::reflection_permissions`. Permissioned reflection natives
  reject missing read, write, and call permissions before recording host
  patches.
- Added descriptor docs and `AttrMap` builder/query APIs for reflected types,
  fields, methods, traits, trait methods, variants, modules, and functions.
  Script-visible `reflect.attrs` and `reflect.docs` expose copied type metadata,
  and reflected field/method/trait/variant/module/function records now include
  copied `attrs`/`docs` fields where applicable.
- Added `ReflectPolicy` and per-VM-install `ReflectLookupBudget` support for
  reflection natives. `Vm::register_reflection_natives_with_policy` and
  `EngineBuilder::reflection_lookup_budget` now bound script-visible reflection
  lookups while preserving permission checks and preventing exhausted lookups
  from recording host patches.
- Added parser, HIR, and reflection propagation for script attributes on
  declarations and supported members. String-valued attributes such as
  `#[event("monster.kill")]` are preserved in HIR; `#[doc("...")]` populates
  descriptor docs, and other attributes are copied into reflected `AttrMap`
  metadata for script functions, structs, enum variants, fields, traits, and
  trait methods.
- Added reflected host method `MethodEffectSet` and `MethodAccess` metadata.
  Engine native method registration now injects effect bits, reflect-callable
  status, and required permissions into `MethodDesc`; VM `reflect.call` checks
  that metadata through `ReflectPolicy` before recording a `PatchTx` method
  call, so unapproved or unpermissioned reflective calls fail without host
  patches.
- Added focused reflection access/effect metadata types outside the crate root
  and extended `FunctionDesc` with copied function effect and access records.
  Engine-registered native and host-native functions now enter
  `TypeRegistry` as reflected host functions with parameter hints, return
  hints, module exports, docs, effect bits, reflect visibility, and required
  permissions for `reflect.function` tooling.
- Split `vela_hot_reload` into focused ABI, compile, runtime, symbol, version,
  and error modules. Added `HotReloadAbi` manifests that can be built from
  `TypeRegistry` and validated during `compile_update_with_abi`, rejecting
  removed or changed schema hashes plus function/method effect and reflective
  access changes before a hot update reaches the runtime safe-point swap.
- Added `Engine::hot_reload_abi()` so hosts can derive hot-reload compatibility
  manifests from the Engine registry. The CLI hot-reload demo now builds its
  manifest from the game-server demo `TypeRegistry`, whose host schemas and
  methods carry stable schema hashes and effect/access metadata, so the
  runnable function-swap workflow exercises the ABI-checked update path.
- Added HIR schema-reference diagnostics for close-but-unresolved type hints
  and `impl Trait for Type` paths. Unknown schema/trait names now report a
  primary span plus ranked related candidate declaration spans, and the
  bytecode compiler rejects these semantic diagnostics before code generation
  while still allowing external host schema names when no local candidate is
  known.
- Added option-aware hot-reload compile helpers and focused Engine hot-reload
  methods. `Engine::compile_hot_reload_initial` and
  `Engine::compile_hot_reload_update` now compile scripts with Engine-derived
  host schema/method compiler options and validate against the registry-derived
  ABI manifest, so embedders do not need to manually keep compiler metadata and
  reload policy metadata in sync.
- Added a focused hot-reload function-signature compatibility module. Updates
  now reject reordered or renamed existing function parameters, continue to
  reject deleted parameters, and still allow appending new defaulted parameters,
  tightening the function ABI checks required before a safe-point code swap.
- Added `HotReloadPolicy` and Engine-level policy wiring. Hosts can now keep
  the default helper/defaulted-parameter behavior or install a locked-down
  policy through `EngineBuilder::hot_reload_policy`, and
  `Engine::compile_hot_reload_update` applies that policy alongside
  Engine-derived compiler options and ABI manifests.
- Added structured hot-reload reports in a focused report module.
  `HotReloadRuntime::apply_hot_update_report` now returns accepted status,
  version transition IDs, changed function names, and structured diagnostics
  with reasons and repair hints for rejected errors. The CLI hot-reload demo
  now exercises the report path and prints the accepted update summary.
- Extended the same runtime report boundary with
  `HotReloadRuntime::apply_hot_update_result_report`, so compile, ABI, and
  policy failures can produce rejected reports without advancing the current
  program version. The CLI hot-reload demo now routes compile/update results
  through this report boundary.
- Extended `HotReloadDiagnostic` with stable machine-readable diagnostic codes
  and affected targets for function, schema, and method ABI failures, so hosts
  can route rejected reload reports without parsing human-readable reasons.
- Enforced `ReflectPermission::InspectHostPath` for host-ref reflection
  metadata queries such as `reflect.type_of`, `reflect.fields`, method/trait
  metadata, variants, and `reflect.implements`. Script-value metadata remains
  available with `ReadTypeInfo` alone, preserving read-only tooling for
  non-host values while keeping host path inspection behind the admin/debug
  permission bit.
- Added `ReflectPermission::AccessPrivate` and enforced
  `MethodAccess::public` during reflective host method calls. Private methods
  can still be exposed to admin/debug policies, but they now require both the
  private-access bit and any method-specific permission before a `PatchTx`
  method-call patch is recorded.
- Added policy-aware `reflect.function` metadata lookups. Function descriptors
  marked non-reflect-visible are denied, private functions require
  `AccessPrivate`, and function-specific permissions must be granted on the
  `ReflectPolicy` before metadata is returned to scripts.
- Added `FieldAccess` metadata to reflected fields and exposed copied
  `ReflectFieldAccess` records through `reflect.field`/`reflect.fields`.
  Reflective host field reads now require `reflect_readable`, and reflective
  host field writes require both host writability and `reflect_writable` before
  any `PatchTx` patch is recorded.
- Extended `HotReloadDiagnostic` for compile rejections with a primary source
  span and copied compiler labels. Rejected reload reports now surface parser or
  semantic diagnostic locations directly while preserving the original
  `HotReloadError` for full host-side inspection.
- Added copied compiler diagnostic records to rejected compile reload reports,
  so host tooling can inspect source diagnostic messages, codes, spans, and
  labels from `HotReloadDiagnostic::source_diagnostics` without unpacking the
  embedded compiler error.
- Added focused hot-reload diagnostic detail records for function parameter
  ABI changes, schema hash changes, and function/method effect or access ABI
  changes. Rejected reports now expose this rendering data through
  `HotReloadDiagnostic::detail` without requiring host tooling to parse
  human-readable reasons or unpack internal error variants.
- Added `ReflectErrorKind::UnknownVariant` candidate diagnostics for
  `reflect.variant_is` when the target enum schema is registered. Misspelled
  variant checks now report ranked variant candidates through both the
  reflection API and compiled VM native path instead of silently returning
  `false`.
- Added policy-aware module export reflection. Script-visible
  `reflect.module` and `reflect.exports` now filter function exports through
  the same reflective function access policy as `reflect.function`, preventing
  hidden, private, or unapproved function names from leaking through module
  metadata while preserving raw registry queries for trusted host inspection.
- Added policy-aware method metadata reflection. Script-visible
  `reflect.methods` and `reflect.has_method` now filter host methods through
  `MethodAccess` and method-specific permissions, so gameplay policies see only
  callable, public, approved methods while raw registry member queries remain
  available for trusted host inspection.
- Added policy-aware field metadata reflection. Script-visible
  `reflect.fields`, `reflect.field`, and `reflect.has_field` now respect
  `FieldAccess::reflect_readable`, so hidden host fields are not enumerated or
  reported as present to gameplay policies while raw registry field queries
  remain available for trusted host inspection.
- Added structured hot-reload report render lines in a focused renderer module.
  `HotReloadReport::render_lines` now returns categorized summary,
  changed-function, diagnostic, ABI-detail, repair-hint, source-diagnostic, and
  source-label records with optional diagnostic indexes and spans, and the CLI
  hot-reload demo prints those lines instead of formatting raw errors.
- Added `ReflectErrorKind::UnknownTrait` candidate diagnostics for
  `reflect.implements`. Known-but-unimplemented traits still return `false`,
  while misspelled or unregistered trait names now report ranked candidates
  through both the reflection API and compiled VM native path.
- Added policy-aware variant metadata reflection. Script-visible
  `reflect.variants` now filters each variant's field metadata through
  `FieldAccess::reflect_readable`, so hidden enum payload fields are not
  exposed to gameplay policies while raw registry variant queries remain
  available for trusted host inspection.
- Added registered trait metadata lookup. Rust callers can query
  `trait_metadata_by_name`, and scripts can call `reflect.trait_info(name)` to
  inspect copied `ReflectTrait` records by name with ranked unknown-trait
  candidates, while `reflect.traits(value)` continues to report traits
  implemented by a target value.
- Added registered type metadata lookup in a focused reflection types module.
  Rust callers can query `type_metadata_by_name`/`type_metadata_list`, and
  scripts can call `reflect.type_info(name)` plus `reflect.types()` to inspect
  copied `ReflectType` records with kind, schema hash, docs, attrs, and member
  counts, including ranked unknown-type candidates.
- Added effect-specific reflective call permissions for host-read,
  host-write, and event-emitting methods. `ReflectPolicy` now checks
  `MethodEffectSet` before `PatchTx` method-call patches are recorded, and the
  VM `reflect.call` native reports a structured effect-permission denial when a
  policy approves method calls but not the method's declared side effects.
- Split reflection error definitions and registry/descriptor metadata out of
  the `vela_reflect` crate root into focused modules, keeping the public
  re-export surface stable while making future M12 reflection work less
  monolithic.
- Added read-only reflection permission metadata. Rust callers can enumerate
  active `ReflectPolicy` permission names and validate permission names with
  ranked unknown-permission candidates, and scripts can use
  `reflect.permissions()` plus `reflect.has_permission(name)` behind the same
  `ReadTypeInfo` permission gate.
- Added optional source-span metadata to reflected top-level schema
  descriptors for script types, traits, functions, and modules. Copied
  reflection records now include `source_span` data when available, giving
  admin/debug tooling declaration locations without exposing mutable schema
  handles.
- Added structured related-candidate metadata for unknown reflected type,
  trait, module, and function lookups. Reflection errors now keep the existing
  ranked candidate names and also include optional source spans for candidates
  whose descriptors have declaration locations.
- Added syntax, HIR, and reflection source-span propagation for script member
  descriptors, including struct fields, enum variants and payload fields, and
  trait methods. Copied member records now expose `source_span`, and unknown
  field, method, and variant reflection errors include related candidate
  records with source spans when available.
- Added reflected field type-hint metadata. `FieldDesc` can carry an optional
  copied type-hint string, script struct fields and enum payload fields
  populate it from HIR, and `reflect.field`/`reflect.fields` expose it as a
  `type` field without introducing script generics or runtime schema mutation.
- Added reflected method signature metadata. Host native methods and script
  trait methods now carry copied parameter and return hints, and
  `reflect.methods`/`reflect.trait_info` expose `params`, `return`, and the
  script-friendly `returns` alias.
- Added source-span propagation for hot-reload ABI rejections. ABI manifests
  now copy optional declaration spans from reflected schemas, functions, and
  methods, and rejected schema/function/method ABI diagnostics plus rendered
  report lines carry those spans when available.
- Added script-value support for `reflect.set`. Host refs still record
  `PatchTx` writes and return `null`, while script records and enum payload
  records now return updated copied values, reject unknown fields, and never
  mutate type structure or expose mutable references.
- Added schema-backed unknown-field diagnostics for dynamic `reflect.get` and
  `reflect.set` calls on script records and enum payloads. When registered
  script metadata is available, errors now report the actual script type or
  variant and related field source spans instead of falling back to anonymous
  record candidates.
- Added per-field reflection permission metadata. `FieldAccess` can require
  named permissions, policy-aware field and variant metadata filters enforce
  them, copied field access records expose the required names, and VM
  `reflect.get`/`reflect.set` deny host field access before reading or
  recording patches when the active policy lacks a required field permission.
- Extended dynamic script-value reflection to honor registered field
  permissions. Policy-aware `reflect.get` and copy-returning `reflect.set` on
  script records and enum payloads now consult script schema field metadata
  when available, while unregistered dynamic records keep the existing
  schema-free behavior.
- Split the controlled reflection value API out of the `vela_reflect` crate
  root into a focused value module. Public re-exports remain stable while
  host-ref reads, script-value writes, reflective host calls, and dynamic
  implements checks no longer add more logic to `lib.rs`.
- Split VM-side reflection value conversion into a focused
  `reflection_values` module. Script `Value` to copied `ReflectValue`
  translation now stays near reflection native registration instead of growing
  the VM crate root, while preserving the controlled host-ref and script-value
  reflection boundary.
- Split VM-side reflection native registration into focused modules for
  policy metadata, type metadata, fields, modules/functions, methods, traits,
  variants, and controlled value operations. The public `Vm` registration APIs
  are unchanged, but future M12 reflection work no longer has to grow a single
  monolithic registration file.

### M14: Engine, Native Functions, And Rust Host Macros

- Added the `vela_macros` proc-macro crate with first-pass
  `#[derive(ScriptHost)]` and `#[derive(ScriptReflect)]` support for named Rust
  structs. Annotated host structs now generate copied `TypeDesc`/`FieldDesc`
  schema metadata with stable type/host/field IDs, field access flags,
  permissions, docs, module attrs, inferred or explicit type hints, and a
  deterministic schema hash.
- The macro slice rejects missing type IDs and duplicate exposed field IDs
  during expansion, and generated metadata is tested against equivalent
  hand-written reflection descriptors without exposing Rust references or
  applying host mutations.
- Added `NativeCallContext` and
  `EngineBuilder::register_context_host_native_fn` for host-aware native
  functions that need Engine metadata, active permissions, `ScriptStateAdapter`,
  `PatchTx`, and execution-budget access. Context natives install into the VM
  with the same descriptor reflection metadata and duplicate-ID validation as
  existing native functions, can charge instruction budgets explicitly, and
  still mutate host state only by recording patches.
- Added M14 embedding convenience macros `args!` and `host!` plus
  `IntoScriptArg` conversions for copied Rust scalar, string, array, map, VM
  value, and `HostRef` arguments. Runtime-call tests now show hosts can build
  script argument lists and host-ref values without exposing Rust state or
  bypassing `PatchTx`.
- Added initial Rust signature conversion rules through `FromScriptArg` and
  `ScriptArgsExt`. Host-native callbacks can now extract copied Rust bool,
  integer, float, string, array, map, VM value, and `HostRef` arguments with
  structured VM arity/type errors, while keeping host object access behind
  external handles and `PatchTx`.
- Added first-pass `#[script_methods]` and `#[script_method]` host method
  metadata macros in a focused macro module. Annotated inherent impl blocks now
  generate `NativeMethodDesc` lists with stable method IDs, effect/access
  metadata, docs, receiver/context skipping, and conservative type hints while
  rejecting duplicate method IDs and Rust `self` receivers.
- Added focused Engine schema traits plus
  `EngineBuilder::register_host_schema::<T>()`. `ScriptHost`,
  `ScriptReflect`, and `script_methods` macro output now implements stable
  Engine traits for host schema and method metadata, letting embedders register
  macro-generated host schemas without copying descriptors by hand.
- Added `EngineBuilder::register_host_method_desc` and
  `register_host_method_metadata::<T>()` for deferred host methods. Macro
  generated method metadata can now populate the Engine registry/compiler
  options without a dummy native callback, and a macro integration test proves
  `player.grant_exp(5)` compiles to a `PatchTx` host-method patch.
- Added a focused Engine typed-native adapter module plus
  `EngineBuilder::register_typed_native_fn`. Pure native functions can now be
  registered from Rust closures with typed copied arguments and typed returns
  for arities 0-3, reusing `FromScriptArg`/`IntoScriptArg` and reporting
  structured VM arity/type errors.
- Extended the same typed adapter boundary to context host natives with
  `EngineBuilder::register_typed_context_host_native_fn`. Host-aware callbacks
  can now receive `NativeCallContext` plus typed copied arguments for arities
  0-3, charge budgets, and record `PatchTx` writes while conversion failures
  still happen before any patch is recorded.
- Added `EngineBuilder::register_typed_host_native_fn` for the existing
  `HostExecution` native path. Host-native callbacks can now use typed copied
  arguments for arities 0-3 while still writing only through `PatchTx`, and
  conversion errors are covered before any transaction patch is recorded.
- Added `Option<T>` support to Engine script argument conversion. Rust typed
  native callbacks can now accept `Option<T>` from dynamic `null`/value inputs
  and return optional copied values back as `null` or the inner script value,
  without adding script-language generics.
- Added `EngineBuilder::register_typed_native_method_fn` for callable native
  host methods. Method callbacks now receive the safe `HostPath` receiver plus
  `HostExecution` and typed copied arguments, preserving the `PatchTx` mutation
  boundary while reusing the Engine signature conversion rules.
- Added Rust `Result<T, E>` support to Engine script argument conversion.
  Typed native functions can now return or extract copied Rust results through
  the existing dynamic `Result.Ok`/`Result.Err` enum value shape, preserving the
  no-script-generics language boundary.
- Added a focused `#[script_function]` macro for pure native Rust functions.
  Annotated functions now generate stable `NativeFunctionDesc` metadata plus an
  EngineBuilder registration helper that uses the typed native function API,
  with tests proving macro-generated natives compile and execute from scripts.
- Added `#[script_context_function]` for host-aware native Rust functions that
  receive `NativeCallContext`. Macro-generated registration uses
  `EngineBuilder::register_typed_context_host_native_fn`, skips the context
  parameter in reflected script metadata, and tests prove callbacks can charge
  budget and record `PatchTx` writes without exposing Rust references.
- Added `#[script_host_function]` for host-native Rust functions that receive
  `HostExecution`. Macro-generated registration uses
  `EngineBuilder::register_typed_host_native_fn`, skips the host boundary
  parameter in reflected script metadata, and tests prove callbacks record
  `PatchTx` writes through the typed Engine API.
- Extended `#[script_methods]` with a generated
  `vela_register_native_method_fns` helper for callable native methods whose
  Rust signature uses the safe `HostPath` plus `HostExecution` boundary. The
  helper registers through `EngineBuilder::register_typed_native_method_fn`,
  while metadata-only methods keep their existing descriptor path.
- Added the first context logging gameplay helper. `ctx.log(...)` is modeled
  as a configured host method call, records a `PatchTx` patch just like
  `ctx.emit(...)`, appears in the game-server demo context schema, and the
  context-event demo now proves emit plus log patches at the host safe point.
- Added Option/Result helper natives `option.is_some`, `option.is_none`,
  `option.unwrap_or`, `result.is_ok`, `result.is_err`, and
  `result.unwrap_or`. These operate on the existing dynamic enum value shapes,
  reject mismatched shapes with VM type errors, and run in both inline and
  managed-heap execution without adding script-language generics.
- Added Option/Result conversion helpers `option.ok_or(option, err)` and
  `result.to_option(result)`. They use the existing dynamic enum shapes,
  compose with `?` propagation, work in managed-heap execution, and expose
  analysis TypeFacts plus completion metadata.
- Added the first `vela_analysis` crate slice for analysis-only `TypeFact`
  metadata and stdlib method facts. Array, map, set, and string methods now
  have focused internal facts for lambda parameter hints and return facts
  without exposing generic syntax to scripts.
- Extended map stdlib analysis metadata for `map.any`, `map.all`, and
  `map.count`, matching the VM's value-predicate map helpers with value
  parameter facts and boolean/count return facts.
- Added analysis-only stdlib function facts for dynamic Option/Result helpers,
  math helpers, permissioned random, and `set.from_array`, so future
  diagnostics/completion can infer return facts for namespace-style native
  calls without exposing script generics.
- Added HIR-backed analysis fact collection. `vela_analysis` now resolves
  public type hints into internal `TypeFact` values, records function
  signature and local binding facts from `ModuleGraph`, qualifies script
  record/enum/trait schema facts, and degrades ambiguous or unresolved hints to
  `Unknown` without adding script-language generics.
- Added TypeRegistry-backed analysis facts. `vela_analysis` can now copy
  host/script type facts, reflected fields, enum variants, methods, registered
  functions, traits, and trait methods out of `TypeRegistry` metadata for
  diagnostics/completion, resolving descriptor type hints while degrading
  missing precision to `Unknown`.
- Added first-pass expression TypeFact inference. The analysis crate now
  infers cheap deterministic facts for literals, arrays, maps, record
  literals, path references, branches, matches, lambdas, stdlib function calls,
  and stdlib collection methods with lambda parameter facts, while HIR-backed
  `AnalysisFacts` records expression facts for resolved local and declaration
  references.
- Added completion data helpers over analysis facts. `vela_analysis` can now
  produce copied completion items for TypeRegistry-backed fields, methods,
  enum variants, functions, types, and traits from `TypeFact` receivers without
  querying or mutating runtime reflection state.
- Extended those completion helpers to standard-library APIs. Collection and
  string receivers now expose copied method signatures, including callback
  function facts for lambda-taking methods, and global completions include
  Option/Result, math, random, and set helper functions without adding
  script-visible generics.
- Added HIR-backed local binding completion helpers. `vela_analysis` can now
  combine `ModuleGraph` binding names with copied `AnalysisFacts` to produce
  current-function parameter, `let`, loop, lambda, and pattern binding
  completions, falling back to `Unknown` for untyped dynamic locals.
- Added HIR-backed declaration completion helpers. Script consts, functions,
  structs, enums, and traits now produce qualified completion items from
  `ModuleGraph` declaration names plus copied `AnalysisFacts`, while impl
  blocks remain metadata and are skipped as completion labels.
- Added HIR-backed module completion helpers. `vela_analysis` now exposes
  module and parent namespace completion items as copied `TypeFact::Module`
  values derived from module paths in the `ModuleGraph`, keeping module
  completion independent from runtime reflection queries.
- Added TypeRegistry-backed hover helpers. `vela_analysis` now returns copied
  hover records for types, fields, methods, functions, traits, trait methods,
  variants, and modules, including docs, attrs, source spans, origin, effects,
  access, permissions, and `TypeFact` metadata without exposing mutable
  reflection state.
- Added a focused source-aware diagnostic renderer in `vela_common`. Existing
  `Diagnostic` values can now render stable line/column output with primary
  spans, related labels, and fallback source offsets, with snapshot-style tests
  covering the formatting boundary for future tooling.
- Added an analysis diagnostics module for unknown member accesses. Given
  copied expression facts plus `TypeRegistry` facts, `vela_analysis` can now
  report unknown fields and methods for precise receivers with candidate
  labels while degrading cleanly for dynamic `Unknown` receivers.
- Added analysis diagnostics for unknown match pattern variants. Known enum,
  dynamic Option, and dynamic Result scrutinees now report misspelled
  `match` variants with ranked candidate labels while unknown or dynamic
  scrutinees still degrade without blocking execution.
- Added analysis effect diagnostics. `RegistryFacts` now carries copied
  function and method effect summaries, and callers can ask `vela_analysis` to
  report host-read, host-write, or event-emission calls that exceed a provided
  allowed-effect set while unknown calls still degrade without blocking
  execution.
- Added first-pass analysis flow narrowing for null checks. `TypeFact` can now
  remove or select `null` from union facts, and `ExprFactScope` applies that
  narrowing to `if value == null` / `if value != null` branches so expression
  facts and member diagnostics use the branch-local receiver fact.
- Added analysis-only match exhaustiveness diagnostics for known enum facts.
  `vela_analysis` now compares unguarded match patterns against copied
  `TypeRegistry` enum variants, treats wildcard/binding arms as exhaustive,
  and reports missing variants without changing runtime match semantics.
- Added analysis-only match-pattern flow narrowing. `ExprFactScope` can now
  narrow a matched enum scrutinee to the active variant and bind record/tuple
  variant payload names from copied `RegistryFacts`, letting expression facts
  and member diagnostics understand match arm bodies without changing runtime
  match behavior.
- Extended match-pattern flow narrowing to dynamic Option/Result values.
  `Option.Some(value)`, `Option.None`, `Result.Ok(value)`, and
  `Result.Err(error)` patterns now bind payload facts from
  `TypeFact::Option`/`TypeFact::Result` even without registered generic
  schemas, while preserving the no-script-generics boundary.
- Added predicate-style Option/Result flow narrowing. `if
  option.is_some(value)`, `option.is_none`, `result.is_ok`, and
  `result.is_err` branches now narrow copied analysis facts to exact
  `Option.Some`/`Option.None` and `Result.Ok`/`Result.Err` shapes, including
  negated predicates, so stdlib facts such as `unwrap_or` can preserve branch
  payload precision without runtime schema mutation.
- Added first-pass VM runtime call stack metadata. `VmError` now carries
  copied script stack frames with function names and call-site spans, and nested
  script/closure call failures preserve that stack while retaining a source
  span fallback for runtime diagnostics.
- Added `VmError::to_diagnostic()` so runtime failures can be converted into
  shared `Diagnostic` values with stable VM error codes, source spans, and
  script call-stack labels that render through the existing diagnostic
  renderer.
- Aligned map lookup/removal runtime behavior with those facts:
  `map.get(key)` and `map.remove(key)` now return dynamic Option values in
  both inline and managed-heap execution.
- Aligned `array.pop()` with the same Option-style collection boundary. It now
  returns dynamic `Option.Some(value)` or `Option.None` in inline and
  managed-heap execution, and analysis stdlib facts expose the Option return
  shape without adding script-visible generics.
- Added Engine-installed permissioned context clock helpers `ctx.now()` and
  `ctx.tick()`. `EngineBuilder::with_context_clock(now, tick)` registers
  deterministic no-arg natives that require `ctx.time`, expose reflection
  metadata, and have analysis TypeFacts as integer-returning context helpers.
- Split VM standard-library implementation so Option/Result helpers and math
  natives live in focused modules, and collection methods reuse the same
  dynamic Option constructor for Option-style returns.
- Added script-visible `reflect.functions()` over copied TypeRegistry function
  metadata. The query returns policy-filtered function records, matching
  `reflect.function` and module export visibility rules.
- Added script-visible `reflect.modules()` over copied TypeRegistry module
  metadata. Listed module records include policy-filtered exports, matching
  `reflect.module` and `reflect.exports`.
- Added zero-argument `reflect.traits()` for copied TypeRegistry trait
  metadata enumeration while preserving `reflect.traits(value)` for
  value-implemented trait metadata.
- Added zero-argument `reflect.methods()` for copied, owner-qualified
  TypeRegistry method metadata enumeration using the same policy filtering as
  `reflect.methods(value)`.
- Added zero-argument `reflect.fields()` for copied, owner-qualified
  TypeRegistry field metadata enumeration using the same policy filtering as
  `reflect.fields(value)`.
- Added zero-argument `reflect.variants()` for copied, owner-qualified
  TypeRegistry variant metadata enumeration using the same payload-field
  policy filtering as `reflect.variants(value)`.
- Added analysis stdlib facts and completion metadata for the script-visible
  `reflect.*` native surface, including type/module/function/member queries,
  field reads/writes/calls, permissions, and variable-arity `reflect.call`,
  without adding script-visible generic types.
- Added `string.replace(old, new)` for inline and managed-heap string values,
  with analysis stdlib facts and completion metadata for its two-string
  replacement signature.
- Propagated Engine-granted permissions into reflection field and function
  metadata filters, alongside existing method permission propagation, so
  script-visible reflection lists match the Engine permission set without
  exposing ungranted schema members.
- Added Option-returning `array.first()` and `array.last()` endpoint helpers
  for inline and managed-heap arrays, with no-script-generics analysis facts
  and completion metadata.
- Added gameplay-oriented `math.lerp(start, end, t)` as a numeric standard
  native with inline and managed-heap coverage plus analysis and completion
  facts.
- Split EngineBuilder reflection metadata injection and duplicate validation
  into focused private modules, keeping the Engine registration path structured
  instead of growing builder orchestration into a single implementation file.
- Completed hot-reload ABI removal checks for registered function and method
  metadata. Reload updates now reject missing ABI entries with explicit
  diagnostics, source spans, and repair hints instead of silently accepting
  removed reflective callable surfaces.
- Strengthened M14 macro signature safety. Native function and method macros
  now reject script-visible Rust reference parameters at expansion time while
  preserving the allowed NativeCallContext, HostExecution, and HostPath
  boundary parameters; shared signature helpers keep macro parsing structured.
- Extended M14 typed native signature conversion to four script arguments for
  pure, host, context-host, and native method callbacks, with Engine tests
  proving copied conversions and PatchTx host effects.
- Split Engine typed-native adapter implementations into focused pure,
  context-host, host, method, return, and trait modules so signature conversion
  logic can grow without piling unrelated adapter code into one file.
- Added M14 Rust set signature conversion for `BTreeSet<T>` and `HashSet<T>`.
  Typed Engine natives and macro-generated native functions can now accept and
  return copied script set values through the existing `Value::Set` boundary.
- Added public-boundary M14 coverage for Rust `HashSet<T>` signatures through
  typed Engine natives and macro-generated native registration, complementing
  the existing ordered `BTreeSet<T>` tests.
- Added `EngineBuilder::with_standard_natives()` so embedded runtimes can opt
  into deterministic VM standard natives through the stable Engine API.
  Runtime coverage now proves Engine-installed math, set, and Option helpers
  execute together under `Runtime::call`.
- Added M14 Rust `HashMap<String, T>` signature conversion. Engine typed
  natives, `args!`, and macro-generated native functions can now accept and
  return copied script map values through the existing `Value::Map` boundary,
  matching the macro metadata that already reports HashMap parameters as maps.
- Added M14 coverage for Rust `BTreeMap<String, T>` signatures through the
  same stable Engine and macro registration paths. Ordered map callbacks can
  now be proven to accept and return copied script map values without exposing
  Rust references or adding script generics.
- Added M14 Rust `f32` inbound signature conversion. Typed Engine natives and
  macro-generated functions can now accept copied script float values as `f32`,
  while finite script floats that overflow the Rust type report structured
  conversion errors instead of silently becoming infinity.
- Added M14 macro signature metadata for Rust `Option<T>`. Native function and
  native method macros now expose the copied inner value hint for nullable
  arguments and returns, while runtime conversion still uses `null`/value
  shapes and does not introduce script-language generics.
- Tightened M14 macro signature validation for Rust integer widths. Native
  function and native method macros now reject `i128`, `isize`, `u64`, `u128`,
  and `usize` anywhere in script-visible parameters or returns, including
  wrapper types, matching the Engine typed conversion boundary before
  generated registration code reaches Rust type-checking.
- Added M14 `ScriptHost` field helper generation. Host schema derives now
  generate per-field stable `FieldId` accessors and `HostPath` constructors
  for exposed fields through the stable Engine API, giving embedders a safe
  path-building surface without exposing Rust references or bypassing
  `PatchTx`.
- Tightened M14 host schema derive validation. `ScriptHost` and
  `ScriptReflect` now reject generic Rust host schema types with an explicit
  macro diagnostic instead of allowing generated code to fail later, preserving
  the no script-generics boundary at the embedding layer.
- Added `EngineBuilder::register_reflect_schema::<T>()` for
  `ScriptReflectSchema` output. Reflect-only derived schemas can now enter the
  Engine `TypeRegistry` through the same stable builder surface as host
  schemas, without requiring embedders to copy generated `TypeDesc` values by
  hand.
- Added `string.slice(start, end)` as a UTF-8 safe character-indexed string
  helper with inline and managed-heap runtime coverage, analysis stdlib facts,
  and completion metadata.
- Added `math.round(value)` as a deterministic numeric standard native that
  returns script `int` values for integers and finite floats, with VM,
  Engine-installed stdlib, analysis, and completion coverage.
- Added `PermissionSet::gameplay()` to the stable Engine API. The preset grants
  deterministic context time helpers while keeping controlled random and
  reflection capabilities explicit opt-ins.
- Added Engine-owned hot-reload runtime application. `Runtime` can now be
  constructed from a `ProgramVersion`, apply `HotUpdate` reports, preserve the
  active program on rejected updates, and expose hot-reload types through the
  stable Engine API.
- Added `array.join(separator)` for deterministic string assembly over script
  string arrays in inline and managed-heap execution, with analysis TypeFacts
  and completion metadata.
- Added `array.contains(value)` for direct collection membership checks using
  the same equality semantics as `==`. It works in inline and managed-heap
  execution, including heap-backed strings and nested aggregate values, and
  exposes boolean analysis TypeFacts plus completion metadata.
- Added `string.find(needle)` as a UTF-8 character-indexed string search
  helper that returns dynamic `Option.Some(index)`/`Option.None` in inline and
  managed-heap execution, with analysis TypeFacts and completion metadata.
- Added `string.strip_prefix(prefix)` and `string.strip_suffix(suffix)` for
  gameplay event/tag normalization. Both helpers return dynamic
  `Option.Some(stripped)`/`Option.None` in inline and managed-heap execution,
  with analysis TypeFacts and completion metadata.
- Extended M14 typed native signature conversion to five script arguments for
  pure, host, context-host, and native method callbacks, with Engine and macro
  tests proving copied conversions and PatchTx host effects.
- Extended M14 typed native signature conversion to six script arguments for
  pure, host, context-host, and native method callbacks. Engine and macro
  tests now prove copied six-argument conversion and PatchTx host effects
  without exposing Rust references to scripts.
- Added `EngineBuilder::register_host_methods::<T>()` as the stable M14 host
  method registration path. `#[script_methods]` now implements the trait hook
  so embedders can register metadata-only methods and callable native method
  bodies through EngineBuilder, with tests covering a five-argument callable
  method that records PatchTx effects.
- Extended Engine-derived compiler options so registered host schema fields
  lower natural field reads/writes into `HostPath` segments, and registered
  host methods remain callable on schema-derived field paths. The mapping now
  lives in a focused Engine module instead of growing `engine.rs`.
- Migrated the game server CLI demo runner onto the stable `Engine` and
  `Runtime` API. Demo scripts now compile through Engine-owned schema metadata
  and execute through Runtime call options while still recording host effects
  into `PatchTx`.
- Added non-mutating `set.union(other)`, `set.intersection(other)`, and
  `set.difference(other)` helpers. They preserve deterministic receiver-order
  output, work in inline and managed-heap execution, reject non-set operands,
  and expose analysis TypeFacts plus completion metadata.
- Added set predicate helpers `set.is_subset(other)`, `set.is_superset(other)`,
  and `set.is_disjoint(other)` for gameplay tag and requirement checks. They
  work for inline and managed-heap set values, preserve heap string identity
  through key comparisons, reject non-set operands, and expose analysis
  TypeFacts plus completion metadata.
- Added directional string trimming helpers `string.trim_start()` and
  `string.trim_end()` for gameplay event/tag normalization. They run in inline
  and managed-heap execution and expose analysis TypeFacts plus completion
  metadata.
- Added `string.parse_int()` for gameplay/config text parsing. It returns
  dynamic `Option.Some(int)` or `Option.None`, composes with
  `option.unwrap_or`, works in managed-heap execution, and exposes analysis
  TypeFacts plus completion metadata.
- Added `string.parse_float()` for gameplay/config numeric text parsing. It
  accepts finite `f64` input, returns dynamic `Option.Some(float)` or
  `Option.None`, works in managed-heap execution, and exposes analysis
  TypeFacts plus completion metadata.
- Added `string.parse_bool()` for gameplay/config flag parsing. It accepts
  exact `true` and `false` literals, returns dynamic `Option.Some(bool)` or
  `Option.None`, works in managed-heap execution, and exposes analysis
  TypeFacts plus completion metadata.
- Extended `map.any`, `map.all`, and `map.count` to support key-aware
  `(key, value)` callbacks while preserving one-argument value predicates.
  Analysis TypeFacts and completion metadata now advertise the key/value
  callback shape.
- Added key-aware `map.find()` for first-match map searches. It returns
  dynamic `Option.Some(MapEntry { key, value })` or `Option.None`, supports
  one-argument value callbacks and `(key, value)` callbacks, and exposes
  analysis TypeFacts plus completion metadata.
- Added non-mutating `map.merge(other)` for deterministic copied-map
  composition. The right-hand map wins duplicate keys, inline and managed-heap
  execution preserve the receiver, and analysis TypeFacts plus completion
  metadata expose the map-to-map signature.
- Added non-mutating `array.distinct()` for stable first-seen de-duplication
  of copied script arrays. It uses VM equality semantics, works in inline and
  managed-heap execution, preserves the receiver, and exposes analysis
  TypeFacts plus completion metadata.
- Added non-mutating `array.reverse()` for copied array ordering. It works in
  inline and managed-heap execution, preserves the receiver, and exposes
  analysis TypeFacts plus completion metadata.
- Added non-mutating `array.slice(start, end)` for deterministic half-open
  array slicing. It preserves the receiver, supports inline and managed-heap
  execution, reports out-of-bounds indexes through the VM error path, and
  exposes analysis TypeFacts plus completion metadata.
- Added Option/Result conversion helpers `option.ok_or(option, err)` and
  `result.to_option(result)` for propagation-oriented control flow. They use
  the existing dynamic enum shapes, compose with `?`, work in managed-heap
  execution, and expose analysis TypeFacts plus completion metadata.
- Aligned M14 macro metadata for Rust `Result<T, E>` returns with the dynamic
  script boundary. Native function and method macros now expose Result returns
  as `TypeHint::Any` while `VmResult<T>` and `HostResult<T>` continue to
  expose the successful copied value hint, with macro tests proving generated
  registrations return dynamic `Result.Ok`/`Result.Err` values.
- Tightened M14 macro generic-bound validation. Native function, context-host
  function, host function, and host method macros now reject Rust where-clauses
  through a shared signature helper instead of allowing hidden generic
  constraints to reach generated Engine registration code.
- Added `string.repeat(count)` for deterministic gameplay label and diagnostic
  text assembly. It works in inline and managed-heap execution, rejects
  negative counts, and exposes analysis TypeFacts plus completion metadata.
- Added `math.distance2d(x1, y1, x2, y2)` as a deterministic gameplay range
  helper over finite script numbers. It returns a float distance, rejects
  non-numeric values, and exposes analysis TypeFacts plus completion metadata.
- Added `math.distance3d(x1, y1, z1, x2, y2, z2)` as the matching
  deterministic 3D gameplay range helper. It runs through VM and
  Engine-installed standard natives, rejects non-numeric values, and exposes
  analysis TypeFacts plus completion metadata.
- Added `math.pow(base, exponent)` as a deterministic numeric helper for
  scaling formulas. Non-negative integer powers preserve integer results when
  they fit, other finite numeric powers return floats, invalid inputs/results
  are rejected, and analysis/completion expose non-generic numeric facts.
- Tightened M14 macro signature safety. Native function, context-host
  function, host function, and host method macros now reject unsafe Rust
  callbacks through a shared signature helper before generating Engine
  registration code.
- Tightened the same M14 macro signature boundary for non-Rust ABI callbacks.
  Native function, context-host function, host function, and host method macros
  now reject `extern` signatures before generating typed Engine registrations.
- Added M14 Rust fixed-array signature conversion. Typed Engine natives can
  accept copied `[T; N]` arguments from script arrays with exact-length
  validation, macro-generated function/method metadata reports Rust arrays as
  script arrays, host schema derives infer array field hints, and unsupported
  integer widths are rejected even when nested inside array signatures.
- Extended M14 Rust `Option<T>` inbound conversion so typed Engine natives and
  macro-generated native functions accept both the existing `null`/value
  embedding shape and script-visible dynamic `Option.Some`/`Option.None`
  values produced by the standard library, without changing `Option<T>` return
  conversion or adding script-language generics.
- Extended the game-server monster-kill demo to award inventory through a
  natural nested host path,
  `player.inventory.items["gold"].count += monster.reward_count`. The focused
  CLI demo schema/state modules now include Inventory and ItemStack metadata,
  and CLI integration coverage proves the nested keyed `HostPath` patch reaches
  the safe-point apply path through the stable Engine/Runtime demo runner.
- Strengthened M14 explicit Engine schema registration validation. Registered
  `TypeDesc` values now reject duplicate field IDs/names, duplicate enum
  variant IDs/names, and duplicate variant payload field IDs/names before
  schemas enter the `TypeRegistry`, extending the stable-ID duplicate checks
  beyond types, host types, methods, and native functions.
- Tightened M14 host schema derive validation for exposed field names.
  `ScriptHost` and `ScriptReflect` now reject duplicate script-visible field
  names during macro expansion, complementing duplicate field ID checks and
  preventing invalid generated schemas from reaching Engine registration.
- Tightened M14 host method macro validation for exposed method names.
  `#[script_methods]` now rejects duplicate script-visible method names during
  expansion, complementing duplicate method ID checks before generated method
  metadata reaches Engine registration.
- Extended M14 explicit Engine schema validation to trait metadata. Registered
  `TypeDesc` trait implementations now reject duplicate trait IDs/names and
  duplicate trait method IDs/names before entering the `TypeRegistry`, keeping
  reflected trait metadata stable for permissions, dispatch, and hot reload.
- Made M14 host schema derive hashes order-independent for exposed fields.
  `ScriptHost` and `ScriptReflect` schema hashes now sort fields by stable
  field ID/name before hashing, so equivalent macro-generated schemas survive
  Rust field reordering while still changing for member or metadata changes.
- Tightened M14 explicit Engine metadata validation for parameter names.
  Registered native functions, host methods, injected native method
  descriptors, and trait method metadata now reject duplicate reflected
  parameter names before entering `TypeRegistry` or hot-reload ABI metadata.
- Added HIR duplicate-parameter diagnostics for script function and lambda
  parameters, including previous/duplicate source labels. The bytecode
  compiler now rejects those semantic diagnostics before register allocation,
  and lambda parameter HIR lookup uses parameter spans instead of whole-lambda
  spans.
- Added HIR import-name stability diagnostics. Duplicate import aliases and
  imports that conflict with local declarations now report both source spans,
  and the bytecode compiler rejects those semantic diagnostics before code
  generation instead of letting import binding maps silently choose one name.
- Added HIR script schema member stability diagnostics. Duplicate struct
  fields, enum variants, enum payload fields, trait methods, and impl methods
  now report source spans and are rejected before bytecode generation can build
  ambiguous field slots, reflection metadata, or hot-reload ABI manifests.
- Preserved schema field default metadata from the planned grammar. Struct
  fields and enum record/tuple payload fields now parse default expressions,
  HIR keeps their source spans, and reflected script field metadata reports
  whether a field is defaulted without changing script type layout or exposing
  runtime schema mutation.
- Made schema field defaults executable for known script constructors. The
  bytecode compiler now fills omitted script struct fields plus enum record and
  tuple payload fields from declaration defaults, including pure const
  expressions and imported module constructors, while preserving the existing
  dynamic record path for undeclared shapes.
- Tightened script constructor shape validation before bytecode emission.
  Known script struct and enum constructors now reject missing required fields,
  unknown fields, duplicate record fields, invalid tuple arity, and unknown
  enum variants with source-spanned semantic diagnostics instead of silently
  producing mismatched runtime shapes.
- Fixed schema-member parsing for the grammar's newline-separated style.
  Struct fields, enum variants, and record variant fields can now appear on
  adjacent lines without commas instead of the parser treating the following
  member as trailing text.
- Extended M9 return lowering through direct expression contexts. Block, if,
  and match expressions that return on all reachable paths can now appear as
  operands or returned values and still exit the active function correctly,
  instead of being rejected after bytecode emission had already produced
  returns.
- Tightened M9 script-call argument validation for named and defaulted
  parameters. Invalid script calls now report source-spanned semantic
  diagnostics for unknown names, duplicate arguments, positional-after-named
  ordering, too many arguments, and missing required parameters instead of
  generic unsupported-syntax errors.
- Improved M12 reflection error reporting through the VM diagnostic boundary.
  Reflection runtime failures now keep stable `reflect::*` diagnostic codes,
  human-readable messages, ranked candidate names, and related schema source
  labels instead of being flattened into generic VM reflection errors.
- Extended attribute argument parsing toward the planned grammar surface.
  Structured attribute arguments with named values, paths, arrays, maps, and
  literals are now normalized through syntax, preserved in HIR, and exposed
  through reflected script metadata without changing runtime schema structure.
- Added an M13/M14 Engine context host schema helper. Embedders can opt into
  stable `Context` metadata for `ctx.now`, `ctx.tick`, `ctx.emit`, and
  `ctx.log`; compiler options lower those workflows to HostRef/HostPath
  operations and event/log calls remain PatchTx patches applied at host safe
  points.
- Reused the standard Engine `Context` schema in the game-server demo. The
  demo now uses the exported context `TypeDesc`, field IDs, host type ID, and
  method IDs for context time, event, and log workflows instead of maintaining
  parallel local context metadata.
- Split VM script `Value` and closure definitions into a focused value module,
  keeping the crate root centered on VM API wiring and execution dispatch while
  preserving the public `vela_vm::Value` re-export.
- Added a focused host `PathProxy` abstraction over `HostPath`. Proxy reads and
  mutations require an explicit `PatchTx` and state adapter, VM values can carry
  copied proxies without tracing host state, Engine argument conversion accepts
  them, and `ScriptHost` derives now generate per-field proxy helpers alongside
  existing `HostPath` helpers.
- Extended managed-heap aggregate storage for copied `PathProxy` values. Heap
  slots now preserve path proxies like external host refs without tracing or
  owning Rust host state, allowing native-returned proxies to round-trip through
  heap-backed arrays and maps.
- Extended `map.map_values` to support key-aware `(key, value)` callbacks
  while preserving existing one-argument value callbacks. Runtime behavior now
  matches the richer map lambda shape exposed by M13 analysis facts and works in
  both inline and managed-heap execution.
- Tightened M14 macro signature safety for script-visible Rust references.
  Native function and method macros now reject nested reference parameters and
  reference return types before generating Engine registrations, while keeping
  the explicit `NativeCallContext`, `HostExecution`, and `HostPath` boundary
  parameters available.
- Completed the first host-provided iterable slice for M9. `IteratorState` now
  exposes a copied-value constructor for native callbacks, and `for-in` accepts
  native-returned `Value::Iterator` values in inline and managed-heap execution
  without storing iterator state in the script heap.
- Tightened hot-reload function ABI checks. Update source that omits a
  previously loaded script function now fails with `reload.function.removed`
  before the runtime version changes, preventing stale function code from being
  silently retained across accepted reloads.
- Extended executable grammar support for `for pattern in expr` loops. Syntax
  now preserves loop patterns, HIR records destructured loop locals as
  `LocalBindingKind::For`, and bytecode reuses match-pattern checks so
  nonmatching iterator values are skipped while matching enum payload fields
  are bound for the loop body.
- Preserved statement attributes from the planned grammar instead of dropping
  them during parsing. `Stmt` nodes now carry copied attribute metadata and
  attributed statements continue through HIR, bytecode, and VM execution as
  inert metadata until specific statement-level policies are defined.
- Aligned map higher-order callbacks for M13 convenience. `map.filter` now
  accepts value-only callbacks in addition to `(key, value)` callbacks, and
  expression analysis infers value facts for single-argument map callbacks
  while preserving key/value facts for two-argument callbacks.
- Added Option/Result `.map` value methods for M13 propagation convenience.
  `Option.Some` and `Result.Ok` invoke script callbacks through the existing
  method runtime, `Option.None` and `Result.Err` pass through their dynamic
  enum shape, and expression analysis/completions expose matching non-generic
  method facts for `Option`/`Result` type facts.
- Extended M14 Engine native metadata with explicit attributes for registered
  native functions and callable native methods. Descriptor attrs now flow into
  reflected `FunctionDesc` and `MethodDesc` metadata, so host-provided APIs can
  carry the same controlled tags as script declarations without runtime schema
  mutation.
- Added Result `.map_err` as an M13 error-side propagation convenience.
  `Result.Err` invokes the callback through the existing method runtime,
  `Result.Ok` preserves the success payload, and analysis/completions expose
  matching non-generic facts for general, narrowed-Ok, and narrowed-Err result
  shapes.
- Added Option/Result `.and_then` value methods for M13 propagation chaining.
  `Option.Some` and `Result.Ok` invoke budgeted callbacks that must return the
  same dynamic enum family, while `Option.None` and `Result.Err` pass through;
  analysis/completions expose matching non-generic facts for general and
  narrowed Option/Result shapes.
- Added Option/Result `.or_else` value methods for M13 fallback chaining.
  `Option.None` invokes a zero-argument fallback callback that must return an
  Option-family value, `Result.Err` invokes an error-aware callback that must
  return a Result-family value, and success variants pass through with
  matching analysis/completion facts.
- Added Option `.filter(predicate)` for M13 value validation chains.
  `Option.Some` invokes a budgeted predicate callback and keeps the payload
  only when the predicate is truthy, `Option.None` passes through, and
  analysis/completion facts expose the predicate payload shape without adding
  script generics.
- Extended M14 native function and method macros with static descriptor attrs.
  `#[script_function]`, `#[script_context_function]`, `#[script_host_function]`,
  and `#[script_method]` now accept repeated `attr = "key=value"` metadata and
  generate `NativeFunctionDesc`/`NativeMethodDesc` attrs that flow through the
  existing Engine reflection pipeline.
- Added M13 Option/Result helper method parity. Dynamic Option values now
  support `.is_some()`, `.is_none()`, `.unwrap_or(value)`, and `.ok_or(error)`;
  Result values support `.is_ok()`, `.is_err()`, `.unwrap_or(value)`, and
  `.to_option()`. Inline and managed-heap execution share the focused
  `option_result_methods` implementation, and analysis/completion facts plus
  branch narrowing understand the method predicate forms without adding script
  generics.
- Extended M14 host schema derives with static descriptor attrs. `ScriptHost`
  and `ScriptReflect` now accept repeated `attr = "key=value"` metadata on
  host structs and script-exposed fields, emit those attrs into generated
  `TypeDesc`/`FieldDesc` values, and include them in the derived schema hash
  without allowing runtime schema mutation.
- Added M12 singular method metadata lookup. Scripts can now call
  `reflect.method(target, name)` to retrieve one copied method descriptor with
  the same access policy enforcement as `reflect.methods`, and unknown method
  names report ranked candidates without mutating reflected type structure.
- Added M12 policy-aware module/function presence checks. Scripts can now call
  `reflect.has_module(name)` and `reflect.has_function(name)` for non-throwing
  reflection guards, with function checks respecting the same visibility,
  privacy, and permission rules as `reflect.function` and `reflect.functions`.
- Added M12 type/trait presence guards. Scripts can now call
  `reflect.has_type(name)` and `reflect.has_trait(name)` before performing
  throwing metadata lookups with `reflect.type_info` or `reflect.trait_info`,
  preserving schema-safe read-only reflection behavior.
- Added M12 singular variant metadata lookup and guards. Scripts can now call
  `reflect.variant_info(value, name)` for one copied variant descriptor and
  `reflect.has_variant(value, name)` for non-throwing enum-schema checks;
  reflected variant fields respect the same field-read policy as
  `reflect.variants`.
- Added M13 Result error-side Option conversion. Scripts can now call
  `result.to_error_option(value)` or `value.to_error_option()` to turn
  `Result.Err(error)` into `Option.Some(error)` and `Result.Ok(_)` into
  `Option.None`; runtime, managed-heap execution, analysis facts, and
  completions all use the existing focused Option/Result modules.
- Added M13 Option/Result flattening helpers. Scripts can now call
  `option.flatten(value)`, `result.flatten(value)`, or `.flatten()` on nested
  dynamic Option/Result values, with inline and managed-heap execution,
  non-nested type errors, analysis facts, and completions covered without
  adding script-language generics.
- Added M13 `set.symmetric_difference(other)` as a deterministic, non-mutating
  gameplay tag-delta helper. It preserves receiver-only values before
  argument-only values, works in inline and managed-heap execution, rejects
  non-set operands, and is exposed through analysis facts and completions.
- Added M13 `set.filter(predicate)` for deterministic, non-mutating tag and
  requirement filtering. It invokes callback predicates in receiver order,
  preserves scalar set semantics in inline and managed-heap execution, rejects
  non-callback arguments, and exposes lambda parameter facts plus completions.
- Added M13 set higher-order predicates `set.find`, `set.any`, `set.all`, and
  `set.count`. They run predicate callbacks in deterministic receiver order,
  work in inline and managed-heap execution, return dynamic Option values for
  `find`, and expose non-generic lambda facts plus completions.
- Strengthened M15 function hot-reload ABI checks for event handlers. Reflected
  function `event` attrs now enter `FunctionAbi`, and updates that add, remove,
  or change event bindings are rejected with report details before a safe-point
  code swap can occur.
- Added M13 `set.map(transform)` as a deterministic, non-mutating set
  transform. Callback results are deduplicated through the existing scalar set
  element rules, work in inline and managed-heap execution, and expose
  non-generic analysis/completion facts for transformed element shapes.
- Added M13 `array.index_of(value)` as a deterministic position lookup helper.
  It uses the VM's dynamic equality semantics, returns
  `Option.Some(index)`/`Option.None`, works in inline and managed-heap
  execution including nested aggregate values, and exposes non-generic
  analysis/completion facts.
- Added M13 `array.remove_at(index)` for script-owned array mutation. It
  removes and returns a dynamic `Option.Some(value)` when the index exists,
  returns `Option.None` for out-of-range indexes, works in inline and
  managed-heap execution, and exposes non-generic analysis/completion facts.
- Added M13 `array.insert(index, value)` for script-owned array mutation. It
  inserts at existing positions or appends at `len`, rejects sparse
  out-of-bounds indexes, works in inline and managed-heap execution, and
  exposes non-generic analysis/completion facts.
- Added M13 `array.extend(other)` for script-owned array batching. It appends
  values from another dynamic array, rejects non-array arguments, works in
  inline and managed-heap execution, and exposes non-generic
  analysis/completion facts.
- Added M13 collection `.clear()` for script-owned arrays, maps, and sets. It
  empties the receiver in inline and managed-heap execution without touching
  host state, and exposes non-generic analysis/completion facts.
- Extended M13 collection batching with `map.extend(other)` and
  `set.extend(other)`. Maps overwrite existing keys from the extension map,
  sets add missing scalar values deterministically, both work in inline and
  managed-heap execution, and both expose non-generic analysis/completion
  facts.
- Added M13 `array.sort()` as a deterministic, non-mutating scalar sort helper.
  It preserves the source array, supports numeric and string arrays in inline
  and managed-heap execution, rejects mixed scalar domains, and exposes
  non-generic analysis/completion facts.
- Added M13 `array.min()` and `array.max()` as deterministic scalar extrema
  helpers. They return dynamic `Option.Some(value)`/`Option.None`, preserve the
  source array, support numeric and string arrays in inline and managed-heap
  execution, reject mixed scalar domains, and expose non-generic
  analysis/completion facts.
- Added the first M17 source fixture under
  `tests/fixtures/conformance/core_language.lang` plus a focused VM integration
  harness. The fixture compiles and executes through the public bytecode/VM
  APIs while covering constants, typed structs/enums, trait self-method
  dispatch, named/default arguments, collection higher-order methods,
  loops with break/continue, match guards, block values, and Option/Result
  propagation.
- Extended the M17 conformance fixture across HIR and analysis integration
  harnesses. The same source now resolves through `ModuleGraph`, validates
  struct/enum/trait/impl metadata, and checks analysis facts for schema
  declarations plus typed locals without duplicating inline fixture programs in
  crate-local tests.
- Extended M14 `NativeCallContext` budget controls. Context host natives can
  now charge native-side memory bytes and reserve PatchTx patch capacity before
  recording host mutations, with tests proving instruction, memory, and patch
  budget failures leave `PatchTx` unchanged.
- Strengthened M15 function descriptor ABI checks. Reflected/native
  `FunctionDesc` parameters now enter `FunctionAbi`, hot reload rejects
  deleted parameters, changed parameter names/order/type/default ABI, and new
  required parameters, while appended defaulted parameters remain compatible.
- Strengthened M15 method descriptor ABI checks. Reflected/native `MethodDesc`
  parameters now enter `MethodAbi`, and hot reload rejects method parameter
  deletions, changed parameter names/order/type/default ABI, and new required
  method parameters while accepting appended defaulted parameters.
- Strengthened M15 callable descriptor ABI checks. Reflected/native
  `FunctionDesc` and `MethodDesc` return type hints now enter
  `FunctionAbi`/`MethodAbi`, and hot reload rejects added, removed, or changed
  return hints with structured report details before a safe-point code swap.
- Strengthened M15 trait descriptor ABI checks. Registered `TraitDesc` method
  IDs, names, parameter metadata, return hints, and default-method status now
  enter `TraitAbi`; hot reload rejects removed traits, changed existing trait
  method ABI, and new required trait methods while allowing reordered methods
  and appended defaulted methods.
- Strengthened M15 reflected module ABI checks. Registered `ModuleDesc`
  exports now enter a focused module ABI manifest; hot reload rejects removed
  modules and removed or changed existing module exports while allowing
  appended exports, with structured diagnostics and report details.
- Strengthened M15 schema ABI checks beyond hash-only comparisons for
  registry-derived schemas. `TypeDesc` kind, field metadata, variant metadata,
  field access, and defaultability now enter a focused schema ABI manifest;
  hot reload accepts reordered members and appended defaulted fields while
  rejecting required field additions and changed existing members with
  structured report details.
- Extended the focused M15 schema ABI manifest to include type-level trait
  implementations. Reload compatibility now preserves existing trait impl
  metadata even when the schema hash is unchanged, allows added impls, and
  rejects removed or changed impl metadata with the existing structured schema
  ABI diagnostics.
- Added M15 Engine file/directory hot-reload source APIs. `Engine` can now
  compile initial and update `ProgramVersion`/`HotUpdate` values from single
  files or deterministic `.lang` module directories using registry ABI,
  compiler options, and configured hot-reload policy. Shared source loading
  lives in a focused loader module, and file/directory reload errors preserve
  both source IO/path failures and hot-reload compatibility failures.
- Routed the M17 CLI hot-reload demo through the stable Engine/Runtime
  embedding path instead of low-level reload helpers. The demo now compiles
  initial and updated scripts through `Engine` source APIs, applies the update
  through `Runtime`, and still proves old version handles keep old code while
  new calls enter the updated code.
- Started M16 CLI diagnostic rendering. Script and hot-reload source compile
  failures now route compiler diagnostics through the common source renderer
  when the CLI has the source file, producing code, span, line, and caret
  output instead of raw debug structs; an integration test covers invalid demo
  script output.
- Strengthened the M17 hot-reload CLI demo rejection path. ABI-compatible
  updates still apply through `Runtime`, while ABI rejections now go through
  the structured `HotReloadReport` renderer so the CLI shows the unchanged
  version, rejection code, target, reason, and repair hint instead of a debug
  dump.
- Extended M16 CLI diagnostic rendering to runtime failures. Demo script
  execution errors now route `VmError::to_diagnostic()` through the common
  renderer, producing stable diagnostic codes/messages instead of raw
  `VmErrorKind` debug output.
- Improved M16 compiled runtime diagnostic source context. The bytecode
  compiler now preserves spans on arithmetic/unary operations and call
  instructions, numeric VM failures attach the active instruction span, and
  the CLI runtime diagnostic test asserts source lines plus script call-site
  labels for division-by-zero failures.
- Tightened the M17 monster-kill reward demo proof. The runnable script now
  records a configured `player.add_reward(...)` host method call through
  `PatchTx` while still mutating nested inventory state through host paths, and
  the CLI integration expectation verifies the reward-call patch count.
- Added M12 direct reflected attribute lookups. Scripts can now call
  `reflect.attr(value, name)` and `reflect.has_attr(value, name)` for
  permissioned, read-only type attribute checks without copying or scanning the
  full attribute map; analysis facts expose the new helpers without script
  generics.
- Added M13 `string.split_lines()` for gameplay log and diagnostic text
  processing. It returns deterministic script arrays in inline and managed-heap
  execution, handles LF and CRLF through the focused string stdlib module, and
  exposes non-generic analysis/completion facts.
- Extended M12 copied metadata attribute/docs helpers. `reflect.attrs`,
  `reflect.attr`, `reflect.has_attr`, and `reflect.docs` now also accept
  copied `Reflect*` records such as field metadata, so scripts can use the same
  read-only helper surface for type, field, method, trait, variant, module, and
  function descriptors without mutating runtime schema structure.
- Split copied `Reflect*` metadata-record helper logic into a focused
  reflection module, keeping member queries responsible for dispatch while the
  metadata-record module owns copied descriptor `attrs`/`docs` extraction.
- Extended copied metadata-record reflection so `reflect.name(record)` can read
  the stable `name` field from copied `Reflect*` descriptors. This lets scripts
  use one read-only naming helper for both runtime targets and copied metadata
  records without exposing mutable schema handles.
- Normalized direct `reflect.module(name)` and `reflect.function(name)` lookups
  to return typed copied `ReflectModule`/`ReflectFunction` records, matching
  list-query results and allowing scripts to use descriptor field access plus
  `reflect.name`/`reflect.docs`/`reflect.attr` helpers consistently.
- Extended copied metadata-record reflection so `reflect.kind(record)` reports
  descriptor kinds for copied `Reflect*` records, while `ReflectType` records
  preserve their registered schema kind. This keeps runtime targets and copied
  descriptors on the same read-only helper surface.
- Added script-visible `reflect.source_span(value)` for runtime type targets
  and copied `Reflect*` metadata records. It returns a copied
  `ReflectSourceSpan` record or `null`, giving admin/debug scripts a uniform
  read-only way to locate reflected declarations.
- Extended M14 Engine native descriptors with optional source spans for
  registered native functions and host methods. The existing metadata mapper
  now carries those spans into reflected `FunctionDesc`/`MethodDesc` records,
  allowing registry-derived hot-reload ABI diagnostics to point back at
  embedder-provided native declaration locations.
- Filled an M12 function reflection metadata gap by exposing stable function
  IDs on copied `ReflectFunction` records. Direct function lookups and
  function metadata lists now preserve the registry's `FunctionId` alongside
  names, modules, signatures, effects, access metadata, attrs, docs, and source
  spans.
- Added script-visible `reflect.id(value)` for runtime type targets and copied
  metadata records that carry stable IDs. The helper mirrors the existing
  read-only `reflect.name`/`reflect.kind` surface, works for host type targets
  and copied field/function records, and has analysis facts as an integer
  metadata query.
- Normalized owner metadata on copied member descriptors. Direct
  `reflect.field`, `reflect.method`, `reflect.methods(value)`,
  `reflect.variants(value)`, and `reflect.variant_info` records now carry the
  same `owner` field as zero-argument member metadata lists, so scripts can use
  one descriptor shape for targeted and enumerated field/method/variant queries.
- Extended the M17 reflection demo script to prove stable reflected type and
  field IDs, owner-qualified field and method descriptors, method effect
  metadata, and the existing controlled `reflect.set`/`reflect.call` workflow
  through the stable Engine/Runtime CLI path.
- Added M13 `math.sign(value)` as a deterministic gameplay direction helper.
  It returns `-1`, `0`, or `1` for numeric inputs in inline, managed-heap, and
  Engine-installed standard native execution, and analysis/completion facts
  expose the helper without introducing script generics.
- Added M13 `math.move_towards(current, target, max_delta)` for deterministic
  gameplay movement/cooldown steps. It handles integer and finite float inputs,
  rejects negative deltas, clamps without overshoot, and is covered by VM,
  managed-heap, Engine, analysis, and completion tests.
- Added M12 `reflect.required_permissions(value)` for copied reflection
  metadata records and access records. Scripts can now read descriptor
  permission requirements without knowing each descriptor's nested access
  shape, and analysis exposes the helper as a non-generic string-array query.
- Added M12 `reflect.effects(value)` for copied function and method metadata.
  It returns a copied `ReflectEffectSet` record from descriptors or existing
  effect records, giving scripts a stable read-only helper for effect metadata.
- Added M12 `reflect.params(value)` and `reflect.returns(value)` for copied
  function, method, and trait-method metadata. Scripts can inspect descriptor
  signatures through stable read-only helpers instead of knowing each copied
  record's field layout.
- Added M12 `reflect.owner(value)` for copied field, method, and variant
  descriptors. Scripts can now read owner type names through the same
  controlled metadata-helper surface used for names, kinds, IDs, docs, attrs,
  effects, and signatures.
- Added M12 `reflect.access(value)` for copied field, method, and function
  descriptors. It returns a copied access record from descriptors or existing
  access records, giving scripts a stable read-only entry point for visibility,
  callability, writability, and required permission metadata.
- Extended the M17 reflection demo script to prove `reflect.access` on field
  and method descriptors through the stable CLI/Engine path, and refreshed the
  architecture reflection API list with the implemented type, trait, module,
  function, method, variant, and non-throwing `has_*` queries.
- Added M13 `string.split_whitespace()` for gameplay command and diagnostic
  tokenization. It returns deterministic word arrays for inline and
  managed-heap strings, exposes analysis/completion facts without script
  generics, and is now listed in the architecture standard-library contract.
- Added M12 `reflect.origin(value)` for copied descriptors that carry
  declaration-origin metadata. Function descriptors now have a stable helper
  for `host`/`script` origin strings, descriptors without origin report `null`,
  and analysis exposes the helper as a non-generic nullable string query.
- Normalized copied trait-method descriptors with owner metadata. Trait method
  records returned from `reflect.trait_info` and `reflect.traits` now carry
  the owning trait name, so scripts can use `reflect.owner` and `reflect.kind`
  on trait methods just like field, method, and variant descriptors.
- Extended M12 module declaration-origin metadata. `ModuleDesc` now records
  `host` or `script` origin, script module registration marks modules as
  script-declared, and `reflect.origin(module)` works for direct and listed
  `ReflectModule` records instead of reporting `null`.
- Normalized M12 module export reflection targets. `reflect.exports(value)` now
  accepts either a module name string or a copied `ReflectModule` descriptor,
  preserving policy-filtered export visibility while matching the descriptor
  helper style used by the rest of the reflection surface.
- Extended M12 trait declaration-origin metadata. `TraitDesc` now records
  `host` or `script` origin, script trait registration marks traits as
  script-declared, implemented trait copies preserve that origin, and
  `reflect.origin(trait_info)` works for copied `ReflectTrait` records.
- Extended M12 declaration-origin metadata across type and member
  descriptors. `TypeDesc`, fields, methods, trait methods, and variants now
  carry host/script origin, HIR script type registration stamps script
  descriptors accordingly, and copied `ReflectType`/member records expose the
  same `reflect.origin` helper surface.
- Normalized M12 type descriptor query targets. Copied `ReflectType` records
  returned by `reflect.type_info` can now be passed directly to
  `reflect.fields`, `reflect.field`, `reflect.methods`, `reflect.method`,
  `reflect.traits`, `reflect.variants`, and related `has_*`/variant helpers,
  matching the documented `reflect.*(type)` API without requiring a live value.
- Extended the same M12 copied type descriptor target handling to
  `reflect.implements(type_info, trait_name)`. Descriptor-target extraction now
  lives in a focused reflection helper module shared by member metadata queries
  and implements checks, keeping copied `ReflectType` parsing out of unrelated
  logic.
- Normalized M12 trait descriptor targets for implements checks.
  `reflect.implements(value_or_type, trait_info)` now accepts copied
  `ReflectTrait` records returned by `reflect.trait_info`/`reflect.traits`, and
  analysis completion facts expose the trait argument as either a string name
  or copied trait descriptor without introducing script generics.
- Extended the M17 reflection demo proof to exercise copied `ReflectType` and
  `ReflectTrait` descriptor targets directly. `reflect_debug.lang` now checks
  `reflect.fields(type_info)`, `reflect.traits(type_info)`, and
  `reflect.implements(type_info, trait_info)` through the stable CLI demo path
  while preserving the controlled `reflect.set`/`reflect.call` workflow.
- Fixed an M9/M11 lowering edge where indexing the result of an ordinary call
  could be mistaken for a host path. Indexed host paths now only grow from an
  already host-shaped receiver, while expressions such as
  `reflect.traits(type_info)[0].name` compile through normal index and record
  field reads.
- Split M9 assignment lowering out of the main bytecode compiler file into a
  focused compiler submodule. Local, index, record-field, and host-path
  assignment behavior remains unchanged, while the compiler file no longer
  owns the full assignment lowering implementation directly.
- Filled an M9 analysis gap for range expressions. Exclusive and inclusive
  ranges now produce an explicit non-generic `range` `TypeFact`, and analysis
  exposes the runtime-supported `range.len()` and `range.is_empty()` method
  facts instead of degrading those expressions to `unknown`.
- Filled an M9 analysis gap for index reads. Array and map indexing now infer
  element/value facts, including compatible union receivers, while incompatible
  index shapes still degrade conservatively to `unknown` instead of inventing
  static type errors.
- Filled an M9 analysis gap for Option/Result-style propagation. `expr?` now
  infers the unwrapped `Option.Some`/`Result.Ok` payload fact, treats known
  `Option.None`/`Result.Err` paths as `never`, and keeps the propagation policy
  in a focused analysis submodule.
- Filled an M9 executable/control-flow gap for expression-valued `if` without
  `else`. The compiler now lowers the untaken branch to `null`, analysis
  includes the null fallback in expression facts, and conformance covers the
  grammar-permitted form.
- Strengthened M13 map introspection coverage. `map.keys()`, `map.values()`,
  and `map.entries()` now have focused inline, managed-heap, and analysis
  assertions proving deterministic key order and copied `MapEntry` records.
- Promoted the M17 hot-reload ABI rejection proof into the game-server demo
  fixtures. The CLI rejection test now uses an incompatible
  `examples/game_server_demo` script update that removes `kill_exp`, matching
  the accepted hot-reload workflow's example-script coverage.
- Split M14 script-function macro emission into a focused child module. The
  derive crate now keeps script-function parsing/validation separate from
  generated descriptor and Engine registration token construction, preserving
  existing macro output while keeping the implementation reviewable.
- Split M14 script-method macro emission into a focused child module. Method
  macro parsing and signature validation now stay separate from generated
  method descriptor expressions and Engine registration chains, preserving the
  existing host-method API while reducing macro-file responsibility.
- Improved M16 unresolved-name diagnostics for function binding. Local name
  suggestions now include a related declaration label pointing at the candidate
  binding, and candidate ranking lives in a focused HIR binding helper module
  instead of expanding the main binding lowerer file.
- Improved M16 member-access diagnostics. Unknown field and method diagnostics
  now rank completion candidates by typo similarity and surface the closest
  `did you mean` label through the existing focused analysis diagnostics
  candidate helper.
- Normalized M12 target field reflection. `reflect.fields(value_or_type)` now
  returns copied owner-qualified `ReflectField` records with the same policy
  filtering as `reflect.fields()`, and analysis/demo coverage now treats the
  targeted field list as descriptor metadata rather than a string-name list.
- Normalized M12 type list reflection. `reflect.types()` now returns copied
  `ReflectType` records instead of string names, aligning list and direct type
  metadata queries plus analysis/demo coverage.
- Extended the M17 reflection demo proof across module and function metadata.
  The game-server demo now registers a reflected `game.reward.grant` native
  through the Engine API, and `reflect_debug.lang` verifies module exports,
  function lists, params, returns, docs, attrs, origins, and access metadata
  through the CLI path.
- Extended the shared M17 core-language conformance fixture with tuple enum
  variant construction and guarded tuple-pattern matching. The same fixture now
  resolves through HIR, carries analysis facts, and executes through the VM for
  both record and tuple enum payload shapes.
- Split M11 host-path push/remove lowering out of the main bytecode compiler
  file into the focused host-path compiler module. Host path recognition,
  root/segment materialization, and push/remove bytecode emission now live
  together while preserving the existing PatchTx execution behavior.
- Moved the M17 monster-kill reward demo closer to the architecture gameplay
  loop by adding a reflected `Config` host schema under `ctx.config`. The
  script now reads `ctx.config.exp_to_next_level` and filters
  `ctx.config.kill_rewards` through normal HostPath reads and stdlib arrays
  before recording inventory, reward, quest, and event patches.
- Extended the shared M17 core-language conformance fixtures across modules.
  The fixture now imports a public const, struct, enum, and function from a
  second module, executes imported constructor/default/function/match flows in
  the VM, and analysis now resolves imported schema aliases in type hints to
  qualified record/enum facts instead of degrading them to `unknown`.
- Split M12 copied member metadata construction into a focused reflection
  module. Field, method, trait, and variant record builders now live outside
  the member-query policy module, preserving the public reflection API while
  keeping future access/reporting work out of a single large file.
- Split bytecode const evaluation into a focused compiler module. Literal
  parsing plus scalar top-level const and constructor-default evaluation now
  live outside the main compiler coordinator, preserving behavior while making
  future grammar-surface work easier to keep modular.
- Aligned M12 `reflect.type_of(value)` with the copied metadata contract. The
  script-visible native now returns a copied `ReflectType` record or `null`,
  analysis facts expose that descriptor shape, and the reflection demo checks
  type names through `reflect.name(...)` instead of relying on raw strings.
- Split bytecode compiler API records into focused modules. `CompileError`,
  `CompileErrorKind`, `CompileResult`, and `CompilerOptions` now live outside
  the main compiler coordinator while preserving the existing public
  `vela_bytecode::compiler::*` import surface for callers.
- Extended M14/M15 Runtime embedding with active-version hot-reload compile
  helpers. `Runtime` can now compile source, file, and directory updates
  against its current `ProgramVersion`, then let the host apply or reject the
  resulting update report at an explicit safe point.
- Routed the M17 CLI hot-reload demo update compile path through
  `Runtime::compile_hot_reload_update_file`, so the runnable example no longer
  manually fetches and passes the current `ProgramVersion` before applying the
  update report at the safe point.
- Split bytecode call-expression lowering into a focused compiler module.
  Tuple enum constructor calls, host method calls, host-path helper calls,
  script method dispatch, closure calls, script calls, and native fallback
  calls now live outside the main compiler coordinator without changing the
  emitted bytecode behavior.
- Split bytecode path-expression lowering into a focused compiler module.
  Local/const path lookup, qualified path access, host-path reads, and
  typed record/enum slot lookup now live outside the main compiler
  coordinator without changing emitted bytecode behavior.
- Filled an M9 tuple-constructor surface gap. Tuple enum variants now accept
  named arguments and declared default values while still storing payloads in
  stable positional enum fields, and the shared conformance fixture executes
  the named/default constructor path.
- Filled an M9/M11 host compound-assignment gap. Host field and HostPath
  targets now lower and execute `*=`, `/=`, and `%=` through PatchTx
  Mul/Div/Rem patches with transaction overlay reads and safe-point apply.
- Filled an M12 active-variant field reflection gap. Enum values now expose
  the active variant's payload fields through `reflect.fields`,
  `reflect.field`, and `reflect.has_field`, with the same field access policy
  filtering and diagnostics as type-level fields.
- Split bytecode map literal lowering into a focused compiler module. Map
  entry key normalization and value lowering now live outside the main
  compiler coordinator, preserving emitted bytecode while keeping future
  collection-literal work modular.
- Extended the M13/M17 game-server demo proof to use Engine-installed
  deterministic context clock helpers. The demo Engine now grants gameplay
  permissions, registers `ctx.now()` and `ctx.tick()` as reflect-visible
  permissioned natives, and the CLI suite runs a dedicated context-clock script
  alongside the existing HostRef context event workflow.
- Extended the M12/M17 reflection demo proof to cover script-visible
  reflection permission metadata. `reflect_debug.lang` now verifies
  `reflect.permissions()` and `reflect.has_permission(...)` under the
  permissioned demo Engine policy before performing controlled reflective host
  writes and calls.
- Filled an M9 nested script record assignment gap. Record paths such as
  `player.stats.level += 1` now lower by updating the leaf record field and
  writing modified intermediate records back to the root local, with focused VM
  coverage and shared core-language conformance coverage.
- Filled an M9 indexed script record assignment gap. Local collection elements
  such as `players[0].stats.level += 1` now lower by reading the indexed
  record, updating its nested record field path, and writing the modified
  record back through `SetIndex`, with compiler, VM, and conformance coverage.
- Split bytecode match-pattern lowering into the focused compiler pattern
  module. Variant tag checks, nested pattern comparisons, and pattern-local
  binding now live outside the main compiler coordinator, preserving behavior
  while keeping grammar-surface work modular.
- Updated M17 validation docs to include the deterministic context-clock demo
  script alongside the other runnable game-server demo workflows.
- Split bytecode statement and control-flow lowering into the focused compiler
  control-flow module. Let/return/expression statements, for-in loop patching,
  break/continue handling, and value-producing block/if/match lowering now
  live outside the main compiler coordinator without changing emitted bytecode.
- Split bytecode expression lowering into the focused compiler expression
  module. Literal, path, field, index, call, lambda, aggregate, unary, binary,
  and try expression dispatch now live outside the main compiler coordinator
  while preserving emitted bytecode behavior.
- Split bytecode constructor lowering into a focused compiler module. Tuple
  enum constructor arguments, schema defaults, record field compilation, and
  constructor diagnostics now live outside the main compiler coordinator while
  preserving emitted bytecode behavior.
- Split bytecode lambda lowering into the focused compiler lambda module.
  Capture collection, nested closure compiler setup, script type fact
  propagation, and closure bytecode emission now live together outside the main
  compiler coordinator while preserving lambda runtime behavior.
- Split bytecode script call argument lowering into the focused compiler call
  argument module. Named/default argument resolution and register-or-missing
  argument emission now live together outside the main compiler coordinator
  while preserving script call bytecode behavior.
- Filled an M11 host variant-field path boundary gap. `HostPath` now has an
  explicit `variant_field(FieldId)` builder, and PatchTx overlay/apply coverage
  proves host enum payload field paths can read, patch, and commit through the
  same safe transaction model as fields, indexes, and keys.
- Extended the M11 host variant-field path through bytecode and VM execution.
  Compiler options can mark host enum payload fields, bytecode preserves them
  as `VariantField` path segments, and a focused VM host-path module routes
  those segments into PatchTx without growing the central dispatch file.
- Connected M11 variant-field host paths to the stable Engine API path.
  `compiler_options_from_registry` now installs registered `VariantDesc`
  payload fields as host variant fields, and Engine coverage proves a
  schema-registered `player.quest_progress.count += 1` script records a
  `HostPath::variant_field` PatchTx update.
- Added stable Engine module metadata registration for M12/M14. Hosts can
  register `ModuleDesc` docs and attributes before native function injection,
  duplicate module names are rejected, reflected module records expose docs,
  and the game-server reflection demo now verifies module docs/attrs through
  the CLI path.
- Filled an M9 script method call-argument gap. Statically resolved script
  method calls now support named arguments and default parameters through the
  same `CallArgument::Missing` path as script functions, and the shared
  conformance fixture exercises named/default method dispatch.
- Filled an M9 nested closure capture gap. Lambda capture discovery now walks
  nested lambda bodies so variables from an outer function can be carried
  through an intermediate closure into an inner returned closure, with
  compiler, VM, and shared conformance coverage.
- Tightened M13 `math.lerp` numeric safety. The helper now rejects
  non-finite interpolation results even when all inputs are finite, matching
  the finite-output policy used by the other deterministic math helpers.
- Tightened M9 string literal grammar conformance. Unsupported backslash
  escapes now produce `E_LEX_STRING_ESCAPE` diagnostics instead of silently
  accepting non-grammar escape forms, while supported escapes and Unicode
  escapes continue to lex normally.
- Tightened M9 radix integer grammar conformance. Hex and binary literals now
  require at least one digit after the `0x` or `0b` prefix, preserving token
  recovery while reporting `E_LEX_INT` for prefix-only forms.
- Tightened the same M9 radix path to reject uppercase `0X` and `0B`
  prefixes at lex time, matching the lowercase-only grammar and removing the
  compiler's old uppercase-prefix fallback.
- Added M13 `string.char_at(index)` as a Unicode-aware, Option-returning
  string helper for safe command and event parsing. Runtime and analysis
  coverage now expose `option<string>` results without out-of-bounds errors.
- Split string-only VM method dispatch into a focused module. The central
  script method dispatcher now keeps overloaded collection/string decisions,
  while dedicated string routing owns the growing M13 string helper surface.
- Moved array `push` and `pop` method implementations into the focused VM
  array method module. The central script method dispatcher now routes those
  calls without owning array mutation and managed-heap slot conversion logic.
- Split bytecode semantic source preparation into a focused compiler module.
  Syntax checking, HIR module graph setup, const evaluation, script function
  symbol discovery, script field slots, and schema-default collection now live
  outside the compiler coordinator while preserving emitted bytecode behavior.
- Added an M13/M17 gameplay helper demo script. The CLI suite now runs a
  parameterless game-server fixture that parses command text with string and
  Option helpers, builds deterministic tag sets, and uses gameplay math helpers
  through the same Engine-installed standard library path as the host demos.
- Split M14 macro metadata and host-schema generation into focused modules.
  Script function and method parsing now live beside their emission modules,
  and ScriptHost schema collection/hash logic is separate from token emission,
  keeping the stable Engine API derive surface modular without behavior
  changes.
- Split M13 VM callback method dispatch out of the central script-method
  router. Higher-order array/map/set and Option/Result method routing now
  owns its callback runtime plumbing in a focused module, reducing the shared
  dispatcher while preserving stdlib behavior across inline and managed-heap
  execution.
- Extended the M13/M17 demo permission proof for controlled random. The
  game-server demo Engine now registers permission-gated `math.random` without
  granting `std.random`, the reflection demo proves the hidden native is
  filtered to an empty `math` module export list, and a negative CLI fixture
  renders the source-spanned permission denial.
- Kept M13 string helper routing modular. String-specific `contains`, `find`,
  and `slice` dispatch now live with the dedicated string method dispatcher,
  leaving the central script-method router responsible for collection fallbacks
  and script-defined method lookup.
- Extended the M13/M17 gameplay helper demo proof across the remaining common
  string and math convenience APIs. The runnable script now exercises trimming,
  line splitting, affix checks, case conversion, slicing, repetition,
  `parse_float`, 3D distance, clamp/min/max/abs/sign, pow, ceil, and round
  while preserving its deterministic CLI result.
- Extended the shared M9/M17 conformance fixture with nested block comments at
  module and function scope. The fixture now proves grammar-level nested block
  comment layout survives HIR resolution, bytecode compilation, and VM
  execution without changing program semantics.
- Added a focused M15 Engine integration test for the runtime safe-point reload
  workflow. Compiling a compatible update from the active runtime now proves
  the current `ProgramVersion` and calls remain unchanged until the host invokes
  `Runtime::apply_hot_update`, then switch only after that explicit apply step.
- Extended the shared M13/M17 conformance fixture across map and set standard
  library workflows. The fixture now executes map `set`/`remove`/`get_or`/
  `merge` plus set construction, union, intersection, difference, symmetric
  difference, subset/superset, disjoint, and membership checks through the HIR,
  compiler, and VM path.
- Split M14 Engine argument-conversion coverage out of the large internal
  Engine test module into a focused integration test. `args!`, `host!`,
  Option/Result/set conversions, owned Rust extraction, and Runtime call
  argument plumbing now exercise only the public Engine API surface.
- Split core M12 reflection value-access coverage into a focused public
  integration test. Host field reads observe PatchTx overlays, reflective
  writes respect read-only metadata, and reflective host method calls remain
  deferred until safe-point transaction apply without relying on crate-private
  test access.
- Split M13 array ordering helpers into a focused VM module. `sort`,
  `sort_by`, `min`, and `max` keep their stable non-mutating behavior and
  managed-heap coverage while the main array method module no longer owns
  scalar ordering and extrema internals.
- Split M13 map higher-order helpers into a focused VM module. `map_values`,
  `filter`, `find`, `any`, `all`, and `count` now keep callback argument
  shaping and truthiness routing outside the general map mutation and
  introspection helpers.
- Split M13 set higher-order helpers into a focused VM module. Set
  `map`, `filter`, `find`, `any`, `all`, and `count` now keep callback
  dispatch and truthiness handling separate from set mutation, algebra, and
  scalar keying internals.
- Split M13 string parsing helpers into a focused VM module. `parse_int`,
  `parse_float`, and `parse_bool` keep their Option-returning behavior and
  finite-float policy while the broad string method module no longer owns
  gameplay/config text parsing internals.
- Split M13 string splitting helpers into a focused VM module. `split`,
  `split_lines`, and `split_whitespace` keep their deterministic array
  materialization behavior while general string search, affix, slice, and case
  helpers stay in the parent method module.
- Split M13 string search/index helpers into a focused VM module. `contains`,
  `starts_with`, `ends_with`, `find`, and `char_at` keep Unicode-aware
  character indexing and Option-returning lookup behavior separate from affix,
  slice, case, parsing, and splitting helpers.
- Split M13 string affix helpers into a focused VM module. `strip_prefix` and
  `strip_suffix` keep their Option-returning behavior and managed-heap coverage
  separate from search, trim, case, slice, parsing, and splitting helpers.
- Split M13 string text-transform helpers into a focused VM module.
  `to_upper`, `to_lower`, `trim`, `replace`, and `repeat` now keep their
  deterministic string materialization and repeat overflow guard separate from
  search, affix, slice, parsing, and splitting helpers.
- Split M13 Unicode-aware string slicing into a focused VM module. `slice`
  now keeps character-index to byte-index conversion and range diagnostics
  separate from search, affix, transform, parsing, and splitting helpers.
- Split M13 scalar math natives into a focused VM module. `math.max`,
  `math.min`, `math.clamp`, `math.sign`, rounding helpers, and `math.abs`
  now keep their finite-number and integer-preserving behavior separate from
  interpolation, movement, distance, and power helpers.
- Split M13 interpolation and movement math natives into a focused VM module.
  `math.lerp` and `math.move_towards` now keep finite-output validation,
  negative-delta rejection, and integer-preserving stepping separate from
  scalar, distance, and power helpers.
- Split M13 distance math natives into a focused VM module. `math.distance2d`
  and `math.distance3d` now keep coordinate validation and finite-distance
  checks separate from scalar, interpolation, movement, and power helpers.
- Split M13 power math native into a focused VM module. `math.pow` now keeps
  integer-preserving exponentiation, overflow rejection, and finite float
  fallback behavior separate from scalar, movement, and distance helpers.
- Split M13 array higher-order helpers into a focused VM module. Array
  `map`, `filter`, `find`, `any`, `all`, and `count` now keep callback
  dispatch and truthiness handling separate from mutation, slicing, grouping,
  and numeric aggregation helpers.
- Split M13 array mutation helpers into a focused VM module. Array `push`,
  `pop`, `remove_at`, `insert`, `extend`, and `clear` now keep managed-heap
  slot conversion and in-place mutation separate from read-only collection
  helpers.
- Split M13 array lookup helpers into a focused VM module. Array `first`,
  `last`, `contains`, and `index_of` now keep Option-returning lookup and
  equality search behavior separate from mutation, ordering, transforms, and
  aggregation helpers.
- Split M13 array transform helpers into a focused VM module. Array `join`,
  `distinct`, `reverse`, and `slice` now keep deterministic materialization,
  equality-based de-duplication, and range validation separate from lookup,
  mutation, ordering, grouping, and numeric aggregation helpers.
- Split M13 array aggregation helpers into a focused VM module. Array `sum`
  and `group_by` now keep numeric total accumulation, callback mapping, and
  protected group roots separate from lookup, mutation, ordering, and
  transform helpers.
- Split M13 map mutation helpers into a focused VM module. Map `set`,
  `remove`, `clear`, and `extend` now keep managed-heap slot conversion and
  in-place updates separate from lookup, introspection, merge, and
  higher-order helpers.
- Split M13 map lookup helpers into a focused VM module. Map `has`, `get`,
  and `get_or` now keep key normalization and Option/default-return behavior
  separate from mutation, introspection, merge, and higher-order helpers.
- Split M13 map introspection helpers into a focused VM module. Map `keys`,
  `values`, and `entries` now keep deterministic materialization and
  `MapEntry` construction separate from lookup, mutation, merge, and
  higher-order helpers.
- Split M13 map merge into a focused VM module. Non-mutating merge behavior now
  owns ordered copy materialization separately from lookup, introspection,
  mutation, and higher-order callback helpers.
- Split M13 set combination helpers into a focused VM module. Set union,
  intersection, difference, symmetric difference, subset, superset, and
  disjoint checks now keep algebra/relation behavior separate from mutation,
  construction, scalar keying, and higher-order callback helpers.
- Split M13 set mutation helpers into a focused VM module. Set `add`,
  `remove`, `clear`, and `extend` now keep managed-heap slot conversion and
  in-place updates separate from construction, lookup, values, combination,
  scalar keying, and higher-order callback helpers.
- Split M13 set construction and basic lookup helpers into a focused VM
  module. `set.from_array`, `set.has`, and `set.values` now keep scalar
  uniqueness and deterministic materialization separate from mutation,
  combination, scalar keying, and higher-order callback helpers.
- Split M13 set scalar keying into a focused VM module. `SetKey`, value key
  normalization, batch key extraction, and heap-slot key conversion now live
  separately from construction, lookup, mutation, combination, and
  higher-order helpers.
- Extended the M12/M13/M17 reflection demo proof for context helpers. The
  game-server `reflect_debug` script now verifies `Context.emit` and
  `Context.log` method metadata, parameters, callable access, and event-effect
  flags through the same CLI path that applies reflected event patches.
- Split M11 host bridge tests into focused proxy, transaction overlay,
  safe-point apply, and permission/freshness modules. HostPath, PathProxy, and
  PatchTx coverage now stays aligned with the production module boundaries
  instead of growing one catch-all host test file.
- Filled an M12 Engine reflection gap for compiled script enum metadata.
  Compiled `Program`s now carry HIR script declaration metadata, Runtime
  installs a program-specific reflection registry that includes script types,
  and the game-server reflection demo verifies `reflect.variants`,
  `reflect.variant_info`, `reflect.has_variant`, `reflect.variant`, and
  `reflect.variant_is` through the CLI path.

## Next

- Continue M12/M13 with remaining reflection access/reporting polish,
  standard-library gameplay conveniences, and string/math/context gaps.
- Continue targeted M14/M15 slices only where they unblock embedding and
  safe-point hot reload workflows.
- Keep M18 performance work limited to measurement scaffolding until M12-M17
  conformance and demo coverage are stable enough for optimization decisions.
- Use [performance.md](performance.md) as the performance-track entry point;
  only tracked benchmark sources and results define official performance
  evidence.
