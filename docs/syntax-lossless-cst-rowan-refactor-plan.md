# Lossless CST Rowan Refactor Plan

Track: syntax foundation, parser, formatter, and downstream analysis migration

Document status: Codex execution plan

Compatibility policy: this is an intentionally breaking pre-release syntax
infrastructure refactor. Old owned AST structs, the old non-lossless parser API,
the token-gap formatter, and compatibility shims may be removed. The refactor
must not change Vela language semantics, VM behavior, compiler/runtime host
boundary rules, hot reload semantics, reflection mutation policy, or LSP
analysis-only constraints.

## 0. Codex Goal

Use this prompt to execute the full refactor:

```text
goal Implement the lossless CST + rowan syntax refactor from
docs/syntax-lossless-cst-rowan-refactor-plan.md. Treat docs/goal.md as the
product roadmap, docs/architecture.md and docs/architecture/*.md as the
architecture contract, docs/grammar.ebnf as the language grammar reference,
docs/progress.md as the current milestone state, and docs/decisions.md as the
durable design decision log. Also read docs/lsp-implementation-plan.md and
docs/lsp-rust-analyzer-main-loop-refactor-plan.md before changing LSP-facing
syntax behavior because the language server depends on the syntax model.

At the start of each execution turn, read those documents, inspect the current
git diff, inspect or run the most relevant failing test, and choose the smallest
verifiable task that advances the earliest incomplete phase in this plan. This
track is allowed to be a breaking internal refactor: remove the old owned AST,
old non-lossless parser API, old token-gap formatter, and transitional
compatibility shims instead of keeping two syntax stacks alive.

Use the local rust-analyzer checkout at ~/CLionProjects/rust-analyzer as the
main architecture reference when it is available. Inspect the relevant files
before changing parser or syntax architecture:
- crates/syntax/src/lib.rs
- crates/syntax/src/syntax_node.rs
- crates/syntax/src/parsing.rs
- crates/syntax/src/ast.rs
- crates/syntax/src/token_text.rs
- crates/parser/src/lib.rs
- crates/parser/src/event.rs
- crates/parser/src/grammar.rs
- crates/parser/src/syntax_kind.rs

Borrow the editor and syntax-tree model, not Rust-only semantics. Do not add
Rust macro expansion, proc macros, Cargo project modeling, Rust editions, borrow
checking, Rust trait solving, or Rust-specific name resolution. Do not introduce
script-language generics, do not expose real Rust &mut T references to scripts,
do not mutate TypeRegistry/RegistryFacts at runtime, do not add monkey
patching, and do not change VM/compiler runtime behavior except where call sites
must consume the new syntax API.

Organize code by ownership scope rather than flattening files. Syntax work
should keep lexer, token kinds, parser events, grammar, tree sink, typed AST,
formatting, diagnostics, and tests in focused modules once those pieces become
non-trivial. Avoid import paths with more than one `super`; prefer `crate::...`
or a clearer module boundary. Avoid re-exports unless they define a deliberate
scoped public API. Do not use re-exports just to shorten imports or hide
unclear file placement. Keep active source and test files under 1200 lines
unless a documented exception explains why splitting would make ownership or
logic materially worse.

Validate each checkpoint with focused tests for the changed crates, then close
out with formatting, clippy, and workspace tests when practical. Update
docs/progress.md only when milestone state changes, update docs/decisions.md for
durable architecture decisions, and commit small Conventional Commit
checkpoints.
```

## 1. Purpose

The current `vela_syntax` parser produces an owned AST with spans and
diagnostics, but it is not a lossless concrete syntax tree. Whitespace, comments,
shebangs, and exact token text are not represented as first-class syntax tree
data. Formatting is currently a separate token/trivia reconstruction pass, so it
cannot reliably preserve source structure or reason over recovered syntax.

This plan replaces that model with a rowan-backed lossless CST and rowan-backed
typed AST wrappers. The result should give formatting, completion, semantic
tokens, selection ranges, rename, diagnostics, HIR lowering, and compiler entry
points one shared syntax source of truth.

## 2. Current Problems

- The lexer skips trivia for normal parsing, so the parser cannot build a
  lossless tree.
- AST nodes store semantic fields and spans instead of syntax node/token
  structure, which makes source-preserving edits and formatting fragile.
- Literal, path, type, attribute, and item shapes are represented as owned Rust
  structs, forcing downstream crates to depend on parser implementation details.
- Formatting is implemented as a token/trivia state machine in
  `vela_syntax::formatting`, with additional range/on-type selection in the
  language service.
- `vela_language_service` stores `SourceFile` in parse records and query
  contexts, so editor features cannot share a CST cursor model.
- `vela_hir`, `vela_analysis`, and `vela_bytecode` consume the old AST directly,
  which makes a compatibility layer tempting but would keep two syntax models
  alive.

## 3. Goals

- Add `rowan` as the syntax tree foundation.
- Define a complete `SyntaxKind` covering node kinds, token kinds, trivia kinds,
  EOF/error/unknown kinds, and helper classification methods.
- Define `VelaLanguage`, `SyntaxNode`, `SyntaxToken`, `SyntaxElement`,
  `SyntaxNodePtr`, and text range aliases in `vela_syntax`.
- Make the lexer lossless: whitespace, line comments, block comments, shebangs,
  unknown text, and malformed token fragments remain represented in the tree.
- Build a parser that always returns a root syntax tree and diagnostics, even
  for incomplete or invalid source.
- Replace owned AST structs with typed AST wrapper traits and wrapper structs
  over rowan nodes/tokens.
- Keep semantic extraction in explicit accessors and lowering code, not in the
  raw CST.
- Migrate HIR lowering, analysis, bytecode compilation, and language-service
  features to the new typed AST wrappers or HIR.
- Replace token-gap formatting with CST/typed-AST layout rules.
- Delete obsolete parser, old AST, and old formatter code once migrated.

## 4. Non-Goals

- Do not change Vela syntax or runtime semantics as part of this refactor.
- Do not introduce script-language generics.
- Do not introduce Rust macro expansion, proc macro support, borrow checking,
  Rust trait solving, or Cargo project modeling.
- Do not require Salsa in this track. The CST model should be compatible with a
  future query engine, but this refactor does not depend on one.
- Do not keep an old-`SourceFile` compatibility facade after downstream crates
  have been migrated.
- Do not rewrite the VM, HostAccess, reflection, hot reload, or standard library
  semantics.

## 5. Target Architecture

`vela_syntax` should become the only crate that owns raw source syntax. A
representative module shape is:

```text
crates/vela_syntax/src/
  lib.rs
  syntax_kind.rs
  syntax_node.rs
  parse.rs
  diagnostics.rs
  lexer/
    mod.rs
    cursor.rs
    token.rs
    literal.rs
  parser/
    mod.rs
    event.rs
    marker.rs
    tree_sink.rs
    grammar/
      mod.rs
      attributes.rs
      expr.rs
      items.rs
      lists.rs
      patterns.rs
      recovery.rs
      statements.rs
      types.rs
  ast/
    mod.rs
    support.rs
    attributes.rs
    expr.rs
    items.rs
    literals.rs
    patterns.rs
    statements.rs
    types.rs
  formatting/
    mod.rs
    layout.rs
    rules/
      mod.rs
      expr.rs
      items.rs
      patterns.rs
      statements.rs
      trivia.rs
      types.rs
```

This structure is a target, not a mandate to create every file on day one.
Create modules when ownership becomes real. Avoid dumping unrelated parser,
formatter, and typed AST logic into one large file.

The public parse boundary should look conceptually like this:

```rust
pub struct Parse<T> {
    green: rowan::GreenNode,
    diagnostics: Vec<SyntaxDiagnostic>,
    _ty: std::marker::PhantomData<fn() -> T>,
}

pub fn parse_source(text: &str) -> Parse<ast::SourceFile>;
```

`Parse<ast::SourceFile>` owns the green tree and diagnostics. Typed AST nodes are
views over syntax nodes created from the parse tree. No downstream crate should
own or mutate raw parser state.

Downstream ownership should become:

- `vela_hir` lowers from typed AST wrappers into HIR/module graph facts.
- `vela_bytecode` compiles through HIR or typed AST wrappers while semantic
  behavior remains unchanged.
- `vela_analysis` queries HIR and syntax wrappers for diagnostics and symbol
  facts.
- `vela_language_service` stores parse trees and summaries in its parse/index
  layer, then serves editor features from syntax pointers, typed AST wrappers,
  HIR, and analysis facts.
- `vela_lsp_server` remains protocol-only and never parses source directly.

## 6. Phased Execution Plan

### Phase 1: Add rowan syntax foundation

- [x] Task: Add the syntax tree primitives without changing production parsing yet.

Expected behavior:

- `rowan` is added to the workspace and `vela_syntax`.
- `SyntaxKind`, `VelaLanguage`, syntax aliases, text aliases, and a minimal
  `Parse<T>` shell exist.
- Syntax kinds distinguish nodes, tokens, trivia, EOF, error, and unknown text.
- Syntax kind conversion to/from raw rowan kinds is tested.

Do not change:

- Do not migrate downstream crates in this phase.
- Do not add a partial old/new adapter that becomes a permanent API.

Validation:

```bash
cargo test -p vela_syntax syntax
cargo test -p vela_syntax parser
```

### Phase 2: Replace lexer with lossless tokenization

- [ ] Task: Make lexical output preserve all source text.

Expected behavior:

- Whitespace, comments, shebangs, unknown text, and malformed token fragments are
  represented as tokens/trivia.
- Existing token classification and diagnostics are preserved or deliberately
  mapped to the new diagnostic model.
- Literal helpers preserve exact source text and only parse semantic values in
  accessor/lowering code.
- Concatenating token text from a lexed source reproduces the original source.

Do not change:

- Do not normalize string escapes or numeric literal spelling in the lexer.
- Do not use formatter-specific token hacks in the lexer.

Validation:

```bash
cargo test -p vela_syntax lexer
cargo test -p vela_syntax parser
```

### Phase 3: Build rowan parser and typed AST wrappers

- [ ] Task: Replace the parser output with a lossless CST and rowan-backed AST views.

Expected behavior:

- The parser uses events/markers/tree sink or an equivalent structured rowan
  construction model.
- `parse_source` returns `Parse<ast::SourceFile>`.
- Every parse returns a source-file root, even with syntax errors.
- Error recovery keeps useful tree shape for incomplete items, expressions,
  types, patterns, calls, containers, and blocks.
- Typed AST wrappers cover the current owned AST surface needed by downstream
  crates.
- Old owned AST structs are deleted as soon as call sites are migrated in the
  same checkpoint series.

Do not change:

- Do not keep the old `SourceFile { items, diagnostics }` model as a compatibility
  layer.
- Do not make AST wrappers compute HIR facts implicitly.

Validation:

```bash
cargo test -p vela_syntax parser
cargo test -p vela_syntax ast
```

### Phase 4: Migrate HIR and module graph lowering

- [ ] Task: Move HIR/module graph construction from owned AST to typed CST wrappers.

Expected behavior:

- `vela_hir` consumes rowan-backed typed AST wrappers.
- Module item discovery, attributes, exports/imports, function signatures,
  struct declarations, trait/impl declarations, constants, and type hints lower
  to the same HIR facts as before.
- Parse summaries used by editor indexing come from CST traversal.
- Diagnostics retain stable locations through syntax text ranges.

Do not change:

- Do not add new type-system behavior.
- Do not change module graph semantics outside the syntax API migration.

Validation:

```bash
cargo test -p vela_hir
cargo test -p vela_language_service module
```

### Phase 5: Migrate compiler and analysis callers

- [ ] Task: Update bytecode compilation and analysis to consume the new syntax/HIR
shape.

Expected behavior:

- `vela_bytecode` no longer depends on old owned AST types.
- Expression, pattern, statement, literal, type, const-eval, and semantic
  lowering behavior matches the pre-refactor behavior.
- `vela_analysis` diagnostics and symbol facts remain behavior-compatible.
- Compile-dir semantics and examples still pass.

Do not change:

- Do not change VM opcodes unless a separate milestone explicitly requires it.
- Do not change host boundary rules, reflection, or hot reload behavior.

Validation:

```bash
cargo test -p vela_bytecode
cargo test -p vela_analysis
cargo run -p vela_cli -- examples/game_server_demo/scripts/level_up.vela
```

### Phase 6: Migrate language service features

- [ ] Task: Make editor features use CST, typed AST wrappers, HIR, and analysis facts.

Expected behavior:

- `ParseDb`, `ParseRecord`, and query contexts store parse trees, diagnostics,
  summaries, and syntax-aware pointers instead of old owned AST values.
- Completion, hover, definition, references, rename, semantic tokens, inlay
  hints, selection range, folding range, document symbols, and code actions keep
  current behavior while gaining CST-backed cursor/context handling.
- LSP protocol crates continue to consume language-service results only; they do
  not parse source directly.

Do not change:

- Do not mix protocol conversion with syntax parsing.
- Do not hide parser dependencies behind unrelated re-exports.

Validation:

```bash
cargo test -p vela_language_service completion hover definition references rename
cargo test -p vela_language_service semantic_tokens inlay selection folding
cargo test -p vela_lsp_server completion semantic_tokens lifecycle
```

### Phase 7: Replace formatter with CST layout rules

- [ ] Task: Delete the token-gap formatter and build formatting from the CST/typed AST
layout model.

Expected behavior:

- Full-document, range, and on-type formatting use CST/typed-AST layout rules.
- Container type hints format like Rust-style generics without spaces around
  angle brackets: `Map<String, i64>`, `Array<i64>`, `Result<Map<String, i64>,
  String>`.
- Formatter handles comments, attributes, item bodies, struct literals, maps,
  sets, arrays, calls, match arms, loops, and multiline type hints through
  syntax structure rather than token-gap inference.
- Formatting diagnostics and skipped-error behavior are explicit and tested.
- Obsolete `extract_format_elements`, token-gap `Formatter`, and related
  production paths are removed.

Do not change:

- Do not use formatter rules to hide parser recovery bugs.
- Do not silently drop comments or trivia.

Validation:

```bash
cargo test -p vela_syntax formatting
cargo test -p vela_language_service formatting
cargo test -p vela_lsp_server formatting
```

### Phase 8: Remove obsolete APIs and close out docs

- [ ] Task: Finish the breaking cleanup and document the new syntax architecture.

Expected behavior:

- No production code imports old owned AST structs.
- No production code uses the old parser output or token-gap formatter.
- Public `vela_syntax` API exposes a deliberate scoped syntax facade.
- Module layout follows the file-size, scope, `super`, and re-export constraints
  from the Codex goal.
- `docs/architecture.md`, `docs/architecture/lsp.md`, `docs/progress.md`, and
  `docs/decisions.md` are updated only where the architecture or milestone state
  materially changed.

Do not change:

- Do not bundle unrelated LSP main-loop, VM, host, or reflection refactors into
  the final cleanup commit.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 7. Acceptance Criteria

- `vela_syntax` uses rowan-backed lossless syntax trees as the production parse
  representation.
- Parser output preserves exact source text through CST token/trivia text.
- Whitespace, comments, shebangs, malformed tokens, and unknown source text are
  represented in the syntax tree.
- Parsing invalid or incomplete source always returns a source-file root plus
  diagnostics.
- Old owned AST structs and old non-lossless parser production APIs are removed.
- HIR, analysis, compiler, language service, and LSP tests pass against the new
  syntax API.
- Formatting is CST/typed-AST based and no production path uses the old
  token-gap formatter.
- Vela language semantics, VM behavior, HostAccess boundaries, reflection
  mutation rules, and hot reload behavior remain unchanged.
- Active source and test files touched by this track stay below 1200 lines unless
  an exception is documented next to the module decision.
- New code avoids import paths with more than one `super`.
- Re-exports are limited to deliberate scoped public APIs.
- The final close-out includes focused tests, workspace tests, formatting, and
  clippy when practical.

## 8. Validation Matrix

Focused syntax validation:

```bash
cargo test -p vela_syntax lexer
cargo test -p vela_syntax parser
cargo test -p vela_syntax ast
cargo test -p vela_syntax formatting
```

Downstream validation:

```bash
cargo test -p vela_hir
cargo test -p vela_analysis
cargo test -p vela_bytecode
cargo test -p vela_language_service completion formatting semantic_tokens inlay
cargo test -p vela_lsp_server completion formatting semantic_tokens lifecycle
```

Full close-out:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run package:release
```

Run `npm run package:release` from `editors/vscode` only when this track changes
LSP/editor package behavior or needs a VSIX verification pass.

## 9. First Execution Tasks

- [x] Task 1: Add rowan syntax primitives.

Context: This establishes `SyntaxKind`, `VelaLanguage`, aliases, and `Parse<T>`
without migrating callers.

Tests:

```bash
cargo test -p vela_syntax syntax
```

- [ ] Task 2: Make lexer output lossless.

Context: The parser cannot become lossless until trivia and unknown text survive
lexing.

Tests:

```bash
cargo test -p vela_syntax lexer
cargo test -p vela_syntax parser
```

- [ ] Task 3: Build source-file CST parsing and typed AST wrappers.

Context: Replace the parser output for the root source file first, then migrate
item/expression/type/pattern wrappers by call-site pressure.

Tests:

```bash
cargo test -p vela_syntax parser
cargo test -p vela_syntax ast
```

- [ ] Task 4: Migrate parse summaries and HIR top-level discovery.

Context: Downstream crates should move through stable typed AST accessors rather
than direct CST walking at every call site.

Tests:

```bash
cargo test -p vela_hir
cargo test -p vela_language_service module
```

- [ ] Task 5: Replace formatter with CST layout.

Context: The concrete UX bug that motivated this track is formatting drift for
container type hints and multiline collections.

Tests:

```bash
cargo test -p vela_syntax formatting
cargo test -p vela_language_service formatting
cargo test -p vela_lsp_server formatting
```
