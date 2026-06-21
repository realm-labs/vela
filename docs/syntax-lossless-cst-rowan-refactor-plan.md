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
Temporary names used only to distinguish the new CST path from the old fallback
path during migration must not become the final API. At close-out, delete the
old fallback API completely, then rename the new syntax structures and
functions to concise canonical names that make sense when there is only one
syntax stack left.

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

Checklist rule: a phase is complete only when every item in its checkpoint
checklist is checked. Keep these items updated as each small commit lands, even
when the phase-level task remains open.

### Phase 1: Add rowan syntax foundation

- [x] Task: Add the syntax tree primitives without changing production parsing yet.

Checkpoint checklist:

- [x] Add `rowan` to the workspace and `vela_syntax`.
- [x] Define `SyntaxKind` for node, token, trivia, EOF, error, and unknown kinds.
- [x] Define `VelaLanguage` and raw rowan kind conversion.
- [x] Add `SyntaxNode`, `SyntaxToken`, `SyntaxElement`, and text-range aliases.
- [x] Add a minimal `Parse<T>` green-tree shell.
- [x] Cover syntax kind classification and raw kind round trips with tests.

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

- [x] Task: Make lexical output preserve all source text.

Checkpoint checklist:

- [x] Keep a parser-facing significant-token stream for existing parsing.
- [x] Add a lossless token stream that preserves whitespace.
- [x] Preserve line comments, block comments, and shebangs.
- [x] Preserve unknown characters and malformed token fragments as source text.
- [x] Preserve exact literal spelling for later AST/lowering accessors.
- [x] Preserve existing lexical diagnostics.
- [x] Prove lossless token text can reconstruct the original source.

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

Checkpoint checklist:

- [x] Add a rowan `parse_source` path returning a source-file root.
- [x] Preserve lexical diagnostics in the rowan parse record.
- [x] Add a typed `SyntaxSourceFile` wrapper.
- [x] Add source-file item iteration.
- [x] Wrap top-level declarations as item CST nodes.
- [x] Add typed wrappers for `use`, `const`, `global`, and function items.
- [x] Expose use paths, use-path text, aliases, and alias tokens.
- [x] Expose const/global names, type hints, and const value expressions.
- [x] Expose function names, parameter lists, parameter names, type hints,
  type arguments, defaults, return types, and body blocks.
- [x] Expose struct field lists, field names, type hints, and defaults.
- [x] Expose type-hint path text, nested type arguments, and delimiter tokens.
- [x] Expose enum variant lists, tuple payloads, record payloads, and defaults.
- [x] Expose trait and impl method headers, signatures, and optional bodies.
- [x] Preserve leading item, field, variant, method, and statement attributes.
- [x] Expose typed block and direct statement wrappers.
- [x] Expose let, return, break, continue, for, if, and else statement tokens.
- [x] Expose for-loop index/value patterns, iterable expressions, and bodies.
- [x] Expose if/else-if condition expressions and branch blocks.
- [x] Expose expression wrappers for let initializers, return values,
  expression statements, and assignments.
- [x] Expose binary, unary, field, call, argument-list, named-argument, path,
  literal, postfix, index, and try-expression wrappers.
- [x] Expose operator tokens/kinds for binary, range, assignment, and unary
  expressions.
- [x] Expose array, map, record, lambda, argument, and parameter list
  delimiters and separators.
- [x] Expose map-entry and record-field labels, values, colons, and shorthand
  classification.
- [x] Keep the rowan map-vs-block split for bare braced expressions.
- [x] Expose match expressions, arm lists, guards, separators, and arm bodies.
- [x] Expose wildcard, literal, binding, path, tuple-variant, and
  record-variant pattern wrappers.
- [x] Expose record pattern fields, labels, nested patterns, colons, and
  shorthand classification.
- [x] Split rowan-backed typed wrappers into focused syntax, attribute, item,
  statement, expression, and pattern modules.
- [ ] Close the remaining pattern coverage called out in `docs/progress.md`.
- [ ] Close the remaining control-flow expression coverage called out in
  `docs/progress.md`.
- [ ] Delete old owned `SourceFile`, `ItemKind`, `ExprKind`, and the old parser
  output after downstream callers are migrated.
- [ ] Ensure no production parser path returns the old owned AST.

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

Checkpoint checklist:

- [x] Add a `ModuleSource`-based module graph insertion API.
- [x] Make HIR `add_source` consume rowan parse records directly.
- [x] Lower module spans, imports, and top-level declaration indexes from CST
  item headers.
- [x] Lower declaration attributes from CST wrappers.
- [x] Lower const/global metadata from CST/HIR declarations.
- [x] Lower function signatures and parameter defaults from CST wrappers.
- [x] Lower struct fields and enum variants from CST wrappers.
- [x] Lower trait and impl method metadata from CST wrappers.
- [x] Bind function and method bodies from CST statement/expression wrappers.
- [x] Bind local scopes and pattern names from CST pattern wrappers.
- [x] Route top-level const initializer diagnostics through the CST summary.
- [x] Remove old HIR type and attribute conversion helpers.
- [x] Stop reparsing module graph sources through the old owned `SourceFile`
  API.
- [x] Audit remaining HIR-facing tests and helpers for direct old-parser usage.
- [ ] Keep this phase open until compiler and analysis migration no longer
  require old AST body fallbacks.

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

Checkpoint checklist:

- [x] Make the bytecode semantic parse gate read CST parse diagnostics first.
- [x] Read typed-let contracts from HIR local binding type hints.
- [x] Read schema type, variant, constructor, field fact, and default-presence
  metadata from HIR/CST declarations.
- [x] Discover schema default-expression payloads from rowan struct/enum field
  wrappers.
- [x] Evaluate constant defaults from rowan CST expressions where supported.
- [x] Read function and method signatures/default flags from HIR metadata.
- [x] Discover function, method, and trait-default parameter default payloads
  from rowan parameter lists.
- [x] Introduce a shared compiler body payload carrying rowan CST bodies plus
  a temporary legacy fallback.
- [x] Route top-level statement dispatch through rowan statement categories
  when payloads align.
- [x] Route expression statement, assignment, call, let, and return payloads
  through rowan expression categories when payloads align.
- [x] Route array, map, record, literal, path, field, index, unary, binary,
  try, call, and block value payloads through CST-aware lowering where covered.
- [x] Route for, if, block, and match statement bodies through nested rowan body
  payloads where covered.
- [x] Route match and for-loop pattern payloads through rowan pattern wrappers
  where covered.
- [x] Prefer rowan labels/paths for record constructors, named arguments,
  tuple enum constructors, method calls, host paths, and host index checks
  where covered.
- [x] Prefer rowan expression payloads for script type/fact extraction, value
  type inference, shape inference, and binary comparison checks where covered.
- [x] Route language-service analysis diagnostics for unknown members,
  non-exhaustive matches, and missing record fields through the CST parse
  record.
- [ ] Remove `crates/vela_bytecode/src/compiler/legacy_payloads.rs`.
- [ ] Remove temporary old-AST body and runtime default-expression fallbacks.
- [ ] Remove production imports of old `Expr`, `ExprKind`, `ItemKind`, and
  `SourceFile` from `vela_bytecode`.
- [ ] Remove production imports of old expression AST types from
  `vela_analysis`.
- [ ] Close remaining pattern lowering coverage in compiler and analysis.
- [ ] Close remaining control-flow expression lowering coverage in compiler
  and analysis.
- [ ] Prove compile-dir and checked examples pass with CST/HIR-only syntax
  inputs.

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

Checkpoint checklist:

- [x] Store rowan parse records in the language-service parse/index layer.
- [x] Read parse diagnostics from the CST parse record.
- [x] Read module-summary fingerprints from CST traversal.
- [x] Remove the legacy owned `SourceFile` from parse database records.
- [x] Preserve missing-delimiter diagnostics through CST parse diagnostics.
- [x] Serve unknown-member diagnostics from CST-backed analysis.
- [x] Serve non-exhaustive-match diagnostics from CST-backed analysis.
- [x] Serve missing-record-constructor-field diagnostics from CST-backed
  analysis.
- [x] Remove the old owned-AST aggregate analysis diagnostics facade.
- [x] Use syntax pointers/typed wrappers for map-key and record-field
  completion contexts where already migrated.
- [x] Use syntax parse records for formatting range selection.
- [ ] Audit completion, hover, definition, references, rename, semantic
  tokens, inlay hints, selection range, folding range, document symbols, and
  code actions after old AST deletion.
- [ ] Remove remaining editor test/helper usage of the old parser when it is
  not intentionally testing the legacy removal boundary.
- [ ] Prove native LSP protocol behavior remains unchanged after old AST
  deletion.

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

Checkpoint checklist:

- [x] Feed formatter input from the rowan CST token/trivia stream.
- [x] Preserve explicit EOF as formatter state.
- [x] Remove old lexer-gap reconstruction from the production formatting input
  boundary.
- [x] Preserve compact container type hints such as `Map<String, i64>` and
  `Array<i64>`.
- [x] Preserve comments, shebang trivia, spans, blank-line groups, and final
  newline insertion through the current formatter path.
- [x] Serve full-document formatting through the native language-service/LSP
  boundary.
- [x] Serve conservative range formatting for selected top-level items and
  selected impl/trait methods.
- [x] Serve on-type reflow for top-level items, impl/trait methods, and enum
  record variants.
- [ ] Replace the remaining layout state machine with CST/typed-AST layout
  rules.
- [ ] Delete obsolete `extract_format_elements`, token-gap `Formatter`, and
  related production paths.
- [ ] Add CST-rule coverage for item, statement, expression, pattern, type,
  trivia, and error-recovery formatting decisions.
- [ ] Prove full-document, range, and on-type formatting are all CST-rule
  backed.
- [ ] Prove formatting diagnostics and skipped-error behavior are explicit.

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

Checkpoint checklist:

- [ ] Delete the old owned AST production structs after all call sites migrate.
- [ ] Delete the old non-lossless parser production API after all call sites
  migrate.
- [ ] Delete transitional CST-to-owned fallback helpers.
- [ ] Delete or rename migration-only identifiers such as `legacy_*`,
  `parse_syntax_*`, and verbose new-vs-old disambiguation names.
- [ ] Rename the final CST API to concise canonical names after old fallbacks
  are gone.
- [ ] Delete the token-gap formatter production path.
- [ ] Audit public `vela_syntax` exports and remove re-exports that are not a
  deliberate scoped public API.
- [ ] Audit import paths touched by this track for the "no more than one
  `super`" rule.
- [ ] Audit touched active source/test files for the 1200-line rule and split
  by ownership when needed.
- [ ] Update `docs/architecture.md` and subsystem architecture docs only for
  durable architecture changes.
- [ ] Update `docs/progress.md` when milestone state changes.
- [ ] Update `docs/decisions.md` for durable syntax architecture decisions.
- [ ] Run focused syntax/downstream validation.
- [ ] Run full formatting, clippy, and workspace tests when practical.

Expected behavior:

- No production code imports old owned AST structs.
- No production code uses the old parser output or token-gap formatter.
- No public or production API keeps migration-only naming that existed only to
  distinguish the new CST path from the old fallback path.
- Final syntax structures and functions use concise canonical names appropriate
  for a single production syntax stack.
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
- Migration-only fallback naming is removed from final public and production
  APIs; the surviving CST API uses concise canonical structure and function
  names.
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
