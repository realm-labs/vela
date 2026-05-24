# Progress

## Current Milestone

M0-M6 runnable prototype loop complete. Current milestone: M7 runtime safety,
budgets, and GC.

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

## Next

- Start M7 with `ExecutionBudget` and budget charging in the VM dispatch loop.
- Add call-depth and patch-count limit tests before adding new looping
  execution paths.
- Plan the non-moving heap and root model for strings, arrays, maps, records,
  enums, closures, and temporary VM values.
