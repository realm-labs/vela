# Progress

## Current Milestone

M2: Minimal Bytecode VM Loop.

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

## Next

- Add script-function call support and a small compiler from parsed AST
  expressions/statements into bytecode.
- Extend the VM value model with arrays/maps as executable bytecode operations.
