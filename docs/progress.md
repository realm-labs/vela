# Progress

## Current Milestone

M1: Syntax Frontend.

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

## Next

- Expand M1 parsing from declaration-level items into statements and
  expressions: `let`, `return`, `if/else`, `for-in`, `match`, field access,
  method calls, array/map literals, lambdas, and assignments.
