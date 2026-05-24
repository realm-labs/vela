# Progress

## Current Milestone

M0: Workspace And Infrastructure.

## Completed

- Created the Cargo workspace layout with `crates/vela_common`.
- Added stable ID newtypes for common host, type, field, method, function,
  variant, object, and source identifiers.
- Added `SymbolInterner`, `SourceId`, `Span`, and a basic `Diagnostic` model.
- Added focused unit tests for symbol interning, span behavior, and diagnostic
  construction.

## Next

- Add the syntax crate skeleton and begin lexer/parser coverage for M1.
