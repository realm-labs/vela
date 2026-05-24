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
