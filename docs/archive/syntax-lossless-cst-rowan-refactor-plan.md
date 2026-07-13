# Lossless CST Rowan Refactor Plan

Track: syntax foundation, parser, formatter, and downstream analysis migration

Document status: close-out complete for the Rowan hard-switch and bytecode
control-flow close-out cleanup

Compatibility policy: this is an intentionally breaking pre-release syntax
infrastructure refactor. Old owned AST structs, the old non-lossless parser API,
the token-gap formatter, and compatibility shims may be removed. The refactor
must not change Vela language semantics, VM behavior, compiler/runtime host
boundary rules, hot reload semantics, reflection mutation policy, or LSP
analysis-only constraints.

Hard-switch policy: this plan is intended to be run by goal-mode loops using
large deletion-first slices, not one-fallback micro-commits. At the start of
each slice, delete the obsolete surface for that subsystem first, then use
compiler errors and focused failing tests as the migration queue. It is
acceptable for the working tree to be temporarily uncompilable while a
hard-switch slice is in progress, but every committed checkpoint must compile
and pass the relevant focused validation. Do not add or extend fallback code,
CST-to-owned adapters, duplicate parser APIs, alias types, migration helpers,
temporary test-only syntax stacks, or temporary dispatch paths only to keep the
project compiling during the switch. The final state must not contain the
legacy parser, owned AST bodies, fallback payloads, migration-only names,
transitional tests, or token-gap formatter production paths.

Canonical naming policy: CST is a valid implementation term only where the code
is explicitly about the concrete syntax tree itself, such as the lossless parser,
syntax tree construction, trivia/token preservation, tree-shape tests, and
architecture documentation. Downstream crates should not need to care whether a
payload came from a CST once the old owned AST is gone. After the hard switch,
rename migration-era `cst_*`, `*Cst*`, and `CST ... payload` names in compiler,
HIR, analysis, language-service, LSP, runtime-facing diagnostics, and behavior
tests to canonical syntax/domain names such as `syntax`, `body`, `statement`,
`expression`, `param_default`, or a more specific semantic term. Do not require
global `cst` zero results: internal `vela_syntax` parser modules such as
`cst_parser`, `CstParser`, and tests that directly assert concrete tree shape
may keep CST in their names.

Commit policy: prefer large subsystem checkpoints. A valid checkpoint removes
an entire obsolete subsystem class, such as the legacy syntax feature, old
bytecode expression inputs, transitional fallback tests, or token-state
formatter. Do not commit one old fallback removal at a time unless that single
removal is the last blocker for a subsystem checkpoint.

## 0. Codex Goal

Use this prompt to execute the full refactor:

```text
goal Execute the full lossless CST rowan refactor hard-switch from
docs/syntax-lossless-cst-rowan-refactor-plan.md. Treat docs/goal.md,
docs/architecture.md, docs/architecture/*.md, docs/progress.md,
docs/decisions.md, docs/grammar.ebnf, and this plan as required context. Also
read docs/lsp-implementation-plan.md and
docs/lsp-rust-analyzer-main-loop-refactor-plan.md before changing LSP-facing
syntax behavior because the language server depends on the syntax model. This
is a breaking internal refactor: the priority is to delete old syntax surfaces
and finish the CST-only architecture, not to preserve compatibility.

At the start of each execution turn, inspect the current git diff, then inspect
remaining references to legacy-body-parser, legacy_body_parser,
parse_owned_body_blocks_for_tests, parse_body_blocks_at_spans, old parser
entrypoints, old owned AST structs, Expr, ExprKind, Stmt, StmtKind, Block,
Argument, RecordField, SourceFile, ItemKind, `.fallback()`, token-gap
formatting paths, CST-to-owned adapters, migration-only names, and downstream
or user-visible `cst_*`/`*Cst*` naming that only exists to distinguish the new
syntax path from the deleted old AST path. In `vela_bytecode`, also inspect
`missing CST`, `unsupported CST`, `CST ... payload`, `cst_payload`,
`cst_lowering_covers`, `syntax_only`, and `is_syntax_only`; these are likely
hard-switch residue, not final architecture names. Work with any existing user
changes and do not revert them.

Use a hard-switch strategy with large subsystem slices. Delete the obsolete
fallback/API at the start of a slice, then use compiler errors and focused
failing tests as the migration queue. It is acceptable for the working tree to
be temporarily uncompilable during the turn, but do not commit until the
changed slice has focused tests passing. Do not add new compatibility shims,
CST-to-owned adapters, duplicate parser APIs, alias types, optional fallback
paths, migration-only dispatch, or temporary replacement test scaffolds just to
keep both syntax stacks alive.

Current execution order:
1. Delete the legacy syntax and owned-AST feature boundary: remove
   legacy-body-parser, legacy_body_parser, parse_owned_body_blocks_for_tests,
   old owned AST structs, and the dev-dependency feature that keeps them alive.
   Fix downstream compile errors against typed CST wrappers, HIR facts, or
   compiler-owned facts. Do not replace the feature with another compatibility
   facade.
2. Hard-switch bytecode to CST-only compiler inputs. Remove old Expr, ExprKind,
   Stmt, StmtKind, Block, Argument, and RecordField dependencies from compiler
   production and test helpers. Replace fallback payload tests with
   source-to-bytecode, source-to-runtime, or source-to-diagnostic behavior
   tests. Then remove migration-era compiler scaffolding such as `syntax_only`,
   `is_syntax_only`, `*_cst_lowering_covers`, `cst_payload`, and `missing CST`
   or `unsupported CST` `UnsupportedSyntax` messages.
3. Delete transitional fallback payload scaffolding: `.fallback()` accessors,
   paired old/CST payload constructors, source-less fallback fixtures, and
   tests whose only purpose is proving that legacy fallback is ignored.
4. Audit HIR, analysis, language service, and LSP after the deletion. Keep
   behavior stable, but remove old parser imports, migration-only names,
   unnecessary re-exports, more-than-one-super imports, and touched active files
   over 1200 lines.
5. Replace the formatter token-state machine with CST/typed-AST layout rules in
   this plan, then delete token-gap formatter production paths and any tests
   that exist only for the old token-gap model.
6. Rename surviving syntax APIs to concise canonical names only after old
   fallbacks are gone; do not keep long migration names like parse_syntax_* or
   names that only existed to distinguish old/new stacks unless they are still
   the best final API. Keep CST terminology only in syntax-tree implementation
   boundaries, parser/tree-shape tests, and architecture docs; rename compiler,
   HIR, analysis, language-service, LSP, and user-visible diagnostics to
   syntax/domain terminology.
7. Before final acceptance, run zero-result audits for legacy-body-parser,
   legacy_body_parser, parse_owned_body_blocks_for_tests, old owned AST types,
   `.fallback()`, CST-to-owned helpers, token-gap formatter paths, and
   migration-only names. Separately audit surviving `cst` names and classify
   each one as either syntax-tree implementation terminology or a naming bug to
   remove before close-out. Treat bytecode `UnsupportedSyntax` messages as
   potentially user-visible because CLI rendering can fall back to debug output
   when no diagnostic exists.
8. Update docs/progress.md, docs/decisions.md, and this checklist only when
   milestone state, final architecture, or remaining gaps materially change.

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

For each checkpoint, choose the largest deletion-first subsystem slice that can
be restored to focused green validation in one execution turn. Avoid
one-fallback commits. Validate with the narrowest relevant tests first, usually
cargo test -p vela_syntax, cargo test -p vela_bytecode --no-fail-fast,
cargo test -p vela_hir, cargo test -p vela_analysis, cargo test -p
vela_language_service completion formatting semantic_tokens inlay, and cargo
test -p vela_lsp_server completion formatting semantic_tokens lifecycle as the
touched surface expands. Close out with
cargo fmt --all -- --check,
cargo clippy --workspace --all-targets -- -D warnings, and
cargo test --workspace when practical. Update
docs/progress.md only when milestone state changes, update docs/decisions.md for
durable architecture decisions, and commit large coherent Conventional Commit
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
when the phase-level task remains open. During hard-switch work, the local tree
may be red between edits; use the resulting compiler errors as the task list,
but do not commit a checkpoint until the relevant focused validation passes.

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

- [x] Task: Delete the old parser/owned-AST feature boundary and leave rowan CST
      as the only syntax model.

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
- [x] Hide old owned AST structs from normal `vela_syntax` production builds
  behind `legacy-body-parser`, leaving only shared literal/operator/visibility
  facts available to CST consumers.
- [x] Delete unused top-level owned AST declarations (`SourceFile`, item
  kinds, item payload structs, and impl/trait wrapper structs) that are no
  longer needed by the temporary bytecode legacy body parser.
- [x] Delete old owned `Attribute` statement payload storage from the temporary
  legacy body parser and remove the obsolete attribute normalizer.
- [x] Delete old owned lambda parameter default-value fallback storage; CST/HIR
  parameter-default payloads are the remaining default source.
- [x] Delete the `legacy-body-parser` feature, `legacy_body_parser` module,
  `body_parser_support` re-export, and `parse_owned_body_blocks_for_tests`
  entrypoint before downstream cleanup.
- [x] Delete old owned `Expr`, `ExprKind`, `Stmt`, `StmtKind`, `Block`,
  `Argument`, `RecordField`, `Pattern`, `MatchExpr`, and related old parser
  body structs, then fix downstream compile errors against CST/HIR directly.
- [x] Remove the dev-dependency feature path that keeps old parser structs alive
  for bytecode tests.
- [x] Ensure no production parser path returns the old owned AST.
- [x] Ensure no test-only parser path returns the old owned AST or constructs
  owned AST bodies for fallback assertions.

Expected behavior:

- The parser uses events/markers/tree sink or an equivalent structured rowan
  construction model.
- `parse_source` returns `Parse<ast::SourceFile>`.
- Every parse returns a source-file root, even with syntax errors.
- Error recovery keeps useful tree shape for incomplete items, expressions,
  types, patterns, calls, containers, and blocks.
- Typed AST wrappers cover the current language surface needed by downstream
  crates.
- Old owned AST structs and the feature that exposes them are deleted at the
  start of the hard-switch checkpoint; downstream call sites are then fixed
  against CST/HIR directly.

Do not change:

- Do not keep the old `SourceFile { items, diagnostics }` model as a compatibility
  layer.
- Do not reintroduce owned AST aliases, adapters, or parser entrypoints to make
  intermediate compilation easier.
- Do not keep test-only owned AST parser support as a transitional assertion
  boundary.
- Do not make AST wrappers compute HIR facts implicitly.

Validation:

```bash
cargo test -p vela_syntax parser
cargo test -p vela_syntax ast
cargo check -p vela_bytecode --lib
```

### Phase 4: Migrate HIR and module graph lowering

- [x] Task: Move HIR/module graph construction from owned AST to typed CST wrappers.

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
- [x] Resolve HIR compile errors from the hard switch by consuming CST/HIR
  directly; do not restore old AST conversion helpers.

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

### Phase 5: Hard-switch compiler and analysis callers

- [x] Task: Delete bytecode fallback payload scaffolding and make compiler
      tests/production consume CST/HIR-only syntax inputs.

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
- [x] Delete old-AST fallback sides from `CompilerBodyPayload`,
  `CompilerStatementPayload`, `CompilerExpressionPayload`, and child payload
  structs instead of leaving them as test-only fixtures.
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
- [x] Remove `crates/vela_bytecode/src/compiler/legacy_payloads.rs`.
- [x] Remove runtime default-expression fallback.
- [x] Remove temporary old-AST body fallback helpers, `.fallback()` accessors,
  paired payload constructors, and source-less legacy assertion paths.
- [x] Remove all imports and type signatures for old `Expr`, `ExprKind`,
  `Stmt`, `StmtKind`, `Block`, `Argument`, `RecordField`, `ItemKind`, and
  `SourceFile` from `vela_bytecode`.
- [x] Remove production imports of old expression AST types from
  `vela_analysis`.
- [x] Replace tests whose only purpose is "does not use legacy fallback" with
  source-driven bytecode, runtime, or diagnostic behavior tests; delete tests
  that have no behavior value after the old fallback path is gone.
- [x] Rename bytecode-facing `cst_*`, `*Cst*`, and `CST ... payload` helper,
  test, and internal diagnostic names that only existed to contrast the new
  syntax path with old AST fallback inputs. Keep behavior-oriented names such
  as `param_default_lowering_*`, `body_payload`, `syntax_payload`, or more
  specific compiler-domain names.
- [x] Clean bytecode compiler hard-switch residue, not just names:
  `UnsupportedSyntax("missing CST ...")`, `UnsupportedSyntax("unsupported CST
  ...")`, `is_syntax_only`, `syntax_only`, `*_cst_lowering_covers`, and
  `cst_payload` helpers/tests are migration-era scaffolding unless the code is
  directly implementing concrete syntax tree behavior. Replace them with
  semantic compiler concepts such as missing statement data, unsupported
  expression form, syntax payload, lowering support, or behavior tests.
- [x] Treat `CompileErrorKind::UnsupportedSyntax` messages as potentially
  user-visible because CLI rendering may fall back to debug output when no
  diagnostic is available. Remove CST/internal payload terminology from those
  messages or convert the path into a proper source-spanned diagnostic.
- [x] Refactor hard-switch readability debt introduced by incremental CST
  migration. Replace long probe-and-return chains such as "try one syntax
  shape, return `Ok(Some(...))`, otherwise return `Ok(None)`" with a stable
  dispatch structure based on `SyntaxExpressionKind`, `SyntaxStatementKind`, or
  a small typed classifier. Prefer `match` expressions, table-like dispatch,
  or focused helpers over accumulated `if let` ladders when the branch key is
  already an enum or can be classified once.
- [x] Refactor boolean coverage predicates such as
  `param_default_cst_lowering_covers` into final semantic support checks.
  Avoid sequences of `if !condition { return false; }` when the same logic can
  be expressed as a match, `Option`/iterator combinators, or named helper
  predicates that describe the supported language shape.
- [x] Split or reorganize migration-dense compiler files after the hard switch.
  `control_flow/syntax_statement_values.rs` was split by lowering ownership:
  statement-level lowering remains in that file, expression dispatch moved to
  `syntax_expression_dispatch.rs`, assignment lowering moved to
  `syntax_assignments.rs`, call lowering moved to `syntax_calls.rs`, index
  lowering moved to `syntax_indexes.rs`, and array/map container lowering moved
  to `syntax_containers.rs`. `param_defaults.rs` remains under the ordinary
  line guideline after its support predicate cleanup.
- [x] Remove "fallback-shaped" control flow after the fallback is gone. Branches
  that repeatedly return `Ok(None)`, `return false`, or generic unsupported
  errors should either be a deliberate optional fast path with a clear caller
  contract, a source-spanned unsupported-language diagnostic, or a match arm in
  the canonical lowering dispatch.
- [x] Close any remaining pattern and control-flow expression lowering gaps
  exposed by deleting the old AST, using typed CST wrappers or HIR facts only.
- [x] Prove compile-dir and checked examples pass with CST/HIR-only syntax
  inputs.

Expected behavior:

- `vela_bytecode` no longer depends on old owned AST types.
- Compiler payload structures do not pair CST nodes with old-AST fallback nodes
  in production or tests.
- Expression, pattern, statement, literal, type, const-eval, and semantic
  lowering behavior matches the pre-refactor behavior.
- `vela_analysis` diagnostics and symbol facts remain behavior-compatible.
- Compile-dir semantics and examples still pass.

Do not change:

- Do not change VM opcodes unless a separate milestone explicitly requires it.
- Do not change host boundary rules, reflection, or hot reload behavior.
- Do not add fallback lowering paths only to preserve old owned-AST behavior.
- Do not keep transitional cst_payload tests, helpers, or names after they no
  longer test user-visible behavior.
- Do not expose `CST` in compiler diagnostics or error messages that can reach
  users; report the missing or unsupported language construct instead.
- Do not leave `syntax_only` or `cst_lowering_covers` as final architecture
  concepts in `vela_bytecode`; after old AST deletion there is only the
  production syntax path and the question is what language shapes the compiler
  supports.
- Do not perform style-only churn that obscures behavior. The cleanup should
  remove migration scaffolding, clarify branch ownership, reduce repeated
  syntax probes, or make source-spanned failure behavior more explicit.
- Do not replace early returns mechanically. Keep guard clauses where they
  enforce preconditions cleanly; refactor the cases where many guards emulate a
  missing enum dispatch or fallback chain.

Validation:

```bash
cargo test -p vela_bytecode
cargo test -p vela_analysis
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
cargo run --manifest-path examples/Cargo.toml --bin level_up
cargo run --manifest-path examples/Cargo.toml --bin modules
rg -n "legacy-body-parser|legacy_body_parser|parse_owned_body_blocks_for_tests|\\.fallback\\(|\\bExprKind\\b|\\bStmtKind\\b|vela_syntax::ast::(Expr|Stmt|Block|Argument|RecordField)|vela_syntax::ast::\\{[^}]*\\b(Expr|Stmt|Block|Argument|RecordField)\\b" crates/vela_bytecode crates/vela_syntax
rg -n "missing CST|unsupported CST|CST .*payload|cst_payload|cst_lowering_covers|syntax_only|is_syntax_only" crates/vela_bytecode
rg -n "return Ok\\(None\\)|return Ok\\(Some|return false;|if let Some\\(|else if let" crates/vela_bytecode/src/compiler/control_flow/syntax_statement_values.rs crates/vela_bytecode/src/compiler/param_defaults.rs crates/vela_bytecode/src/compiler/param_defaults
```

Code review findings to close in the next bytecode slice:

- [x] User-visible error leakage risk: `vela_cli` renders `CompileError` with
  `format!("{error:?}")` when no diagnostic exists, while
  `CompileErrorKind::UnsupportedSyntax` intentionally returns no diagnostic.
  Any bytecode `UnsupportedSyntax("missing CST ...")` or
  `UnsupportedSyntax("unsupported CST ...")` string can therefore reach users
  through CLI/debug output. Fix the bytecode errors to use semantic names or
  emit proper diagnostics; do not leave CST/internal payload wording.
  Evidence:
  - `crates/vela_cli/src/diagnostics.rs:25`
  - `crates/vela_bytecode/src/compiler/control_flow/statements.rs:36`
  - `crates/vela_bytecode/src/compiler/control_flow/syntax_statement_values.rs:605`
- [x] `CompilerBodyPayload` and `CompilerStatementPayload` still encode the
  deleted old/new syntax distinction through `syntax_only` and
  `is_syntax_only()`. `is_syntax_only()` always returns `true`, so callers that
  branch on it are carrying hard-switch residue instead of final compiler
  architecture. Remove the concept or rename it to the actual semantic
  condition being tested.
  Evidence:
  - `crates/vela_bytecode/src/compiler/body_payloads.rs:57`
  - `crates/vela_bytecode/src/compiler/body_payloads.rs:154`
  - `crates/vela_bytecode/src/compiler/control_flow/statements.rs:86`
- [x] `compile_syntax_expression` is an ordered probe chain over syntax shapes,
  returning `Ok(Some(_))` for the first matching helper and `Ok(None)` when no
  helper accepts the expression. This reads like a remaining fallback search
  rather than a canonical lowering dispatch. Refactor toward a
  `SyntaxExpressionKind` match plus focused helper dispatch, preserving any
  deliberately ordered special cases with names that explain why ordering
  matters.
  Evidence:
  - `crates/vela_bytecode/src/compiler/control_flow/syntax_statement_values.rs:255`
- [x] `control_flow/syntax_statement_values.rs` is still over the ordinary
  1200-line guideline and mixes
  expression lowering, assignment lowering, host path lowering, calls,
  container literals, logical chains, numeric special cases, and shape/type
  probes. Split by ownership after removing the fallback-shaped probes; do not
  accept this as the final architecture without a documented exception.
  Evidence:
  - `crates/vela_bytecode/src/compiler/control_flow/syntax_statement_values.rs`
    is now 230 lines.
  - New focused modules are all under the 1200-line guideline:
    `syntax_assignments.rs` 714 lines, `syntax_calls.rs` 322 lines,
    `syntax_expression_dispatch.rs` 302 lines, `syntax_indexes.rs` 56 lines,
    and `syntax_containers.rs` 55 lines.
- [x] Parameter-default lowering keeps a second support predicate tree named
  `param_default_cst_lowering_covers` beside the real lowering functions. This
  duplicates shape knowledge and risks drift as language forms are added.
  Rename it to a semantic support predicate or fold support checking into the
  lowering path so unsupported cases have one source of truth and one
  source-spanned error behavior.
  Evidence:
  - `crates/vela_bytecode/src/compiler/param_defaults.rs:57`
  - `crates/vela_bytecode/src/compiler/param_defaults.rs:667`
- [x] Parameter-default tests still assert CST payload/scaffolding concepts
  instead of behavior-oriented source-to-bytecode/source-to-diagnostic results.
  Rewrite names and assertions around supported defaults, unsupported defaults,
  diagnostics, and runtime behavior.
  Evidence:
  - `crates/vela_bytecode/src/compiler/param_defaults/tests.rs:7`
  - `crates/vela_bytecode/src/compiler/param_defaults/tests.rs:49`
- [x] `docs/progress.md` is stale relative to the code: it still describes the
  rowan refactor as "started" and contains legacy fallback/body-parser status
  that is no longer true after the hard-switch deletions. Close-out must update
  progress to current state; otherwise goal mode can claim code completion
  while roadmap status remains misleading.
  Evidence:
  - `docs/progress.md:125`
  - `docs/progress.md:280`
  - `docs/progress.md:398`

### Phase 6: Migrate language service features

- [x] Task: Make editor features use CST, typed AST wrappers, HIR, and analysis facts.

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
- [x] Audit completion, hover, definition, references, rename, semantic
  tokens, inlay hints, selection range, folding range, document symbols, and
  code actions after old AST deletion.
- [x] Remove remaining editor test/helper usage of the old parser when it is
  not intentionally testing the legacy removal boundary.
- [x] Prove native LSP protocol behavior remains unchanged after old AST
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

- [x] Task: Delete the token-state formatter and build formatting from the
      CST/typed-AST layout model.

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
- [x] Replace the remaining token/layout state machine with CST/typed-AST
  layout rules in this refactor plan, not a later track.
- [x] Delete obsolete `extract_format_elements`, token-gap `Formatter`, token
  adjacency state, delimiter-stack layout inference, and related production
  paths.
- [x] Add CST-rule coverage for item, statement, expression, pattern, type,
  trivia, and error-recovery formatting decisions.
- [x] Prove full-document, range, and on-type formatting are all CST-rule
  backed.
- [x] Prove formatting diagnostics and skipped-error behavior are explicit.
- [x] Delete formatter tests that only lock in old token-gap implementation
  details, replacing them with idempotent behavior fixtures where needed.

Expected behavior:

- Full-document, range, and on-type formatting use CST/typed-AST layout rules,
  not token adjacency or parser-gap reconstruction.
- Container type hints format like Rust-style generics without spaces around
  angle brackets: `Map<String, i64>`, `Array<i64>`, `Result<Map<String, i64>,
  String>`.
- Formatter handles comments, attributes, item bodies, struct literals, maps,
  sets, arrays, calls, match arms, loops, and multiline type hints through
  syntax structure rather than token-gap inference.
- Formatting diagnostics and skipped-error behavior are explicit and tested.
- Obsolete `extract_format_elements`, token-gap `Formatter`, and related
  production paths are removed.
- Formatter tests describe source formatting behavior and trivia preservation,
  not internal token-state transitions.

Do not change:

- Do not use formatter rules to hide parser recovery bugs.
- Do not silently drop comments or trivia.
- Do not keep a token-state formatter as a fallback or compatibility path.

Validation:

```bash
cargo test -p vela_syntax formatting
cargo test -p vela_language_service formatting
cargo test -p vela_lsp_server formatting
rg -n "extract_format_elements|struct Formatter|token-gap|delimiter_stack|previous_token" crates/vela_syntax crates/vela_language_service crates/vela_lsp_server
```

### Phase 8: Remove obsolete APIs and close out docs

- [x] Task: Finish the breaking cleanup and document the new syntax architecture.

Checkpoint checklist:

- [x] Prove old owned AST structs, old parser feature gates, and old body parser
  test support are deleted from production and tests.
- [x] Prove the old non-lossless parser API and any test-only old parser entry
  points are deleted.
- [x] Delete transitional CST-to-owned fallback helpers and paired CST/owned
  payload fixtures.
- [x] Delete or rename migration-only identifiers such as `legacy_*`,
  `parse_syntax_*`, `*_fallback*`, and verbose new-vs-old disambiguation names.
- [x] Rename the final CST API to concise canonical names after old fallbacks
  are gone.
- [x] Audit all remaining `cst`, `Cst`, and `CST` names. Keep them only in
  concrete-syntax-tree implementation boundaries such as `vela_syntax`
  parser/tree construction modules, syntax tree-shape tests, and architecture
  documentation. Rename downstream compiler/HIR/analysis/language-service/LSP
  helpers, tests, APIs, and user-visible messages to canonical syntax/domain
  names.
- [x] Delete the token-gap formatter production path.
- [x] Delete transitional tests that exist only to prove old fallback paths are
  ignored; preserve or rewrite behavior tests that still matter.
- [x] Audit public `vela_syntax` exports and remove re-exports that are not a
  deliberate scoped public API.
- [x] Audit import paths touched by this track for the "no more than one
  `super`" rule.
- [x] Audit touched active source/test files for the 1200-line rule and split
  by ownership when needed.
- [x] Audit migration-dense control flow in touched downstream crates. Replace
  fallback-shaped `if let`/early-return ladders with canonical enum dispatch,
  typed classifiers, or focused helper modules when that makes branch ownership
  clearer. Record any intentionally retained large file or guard-heavy
  function with a short rationale near the module or in the close-out notes.
- [x] Update `docs/architecture.md` and subsystem architecture docs only for
  durable architecture changes.
- [x] Update `docs/progress.md` when milestone state changes.
- [x] Update `docs/decisions.md` for durable syntax architecture decisions.
- [x] Run focused syntax/downstream validation.
- [x] Run final zero-result legacy audit searches before declaring the plan
  complete.
- [x] Run full formatting, clippy, and workspace tests when practical.

Close-out notes:

- Rowan hard-switch and legacy cleanup audits are complete. The final forbidden
  legacy/parser/fallback formatter searches return zero hits for the Rowan
  track.
- Downstream `cst` naming audit is complete. Remaining CST terminology is
  limited to `vela_syntax` concrete-syntax-tree parser/tree implementation,
  syntax tree-shape tests, and architecture documentation.
- Public `vela_syntax` re-exports are the deliberate syntax facade for typed
  AST wrappers, parse records, rowan text types, syntax kinds, syntax nodes,
  lexer, token, and formatting APIs.
- No multi-level `super::super` imports remain in the touched syntax,
  bytecode, HIR, analysis, language-service, or LSP crates.
- The bytecode control-flow close-out is complete. The former mixed
  `syntax_statement_values.rs` module now contains statement-level syntax
  lowering only and is 230 lines. Expression dispatch, assignment lowering,
  call lowering, index lowering, and array/map container lowering live in
  focused sibling modules, all under the ordinary 1200-line guideline. Future
  bytecode compiler work should add new syntax families to the focused module
  that owns the lowering behavior, or create a new focused module when no
  current owner fits.

Expected behavior:

- No production or test code imports old owned AST structs.
- No production or test code uses the old parser output, old body parser, or
  token-gap formatter.
- No public, production, or test API keeps migration-only naming that existed
  only to distinguish the new CST path from the old fallback path.
- Final syntax structures and functions use concise canonical names appropriate
  for a single production syntax stack.
- Remaining CST terminology is intentional and local to concrete syntax tree
  implementation, syntax tree-shape tests, or architecture docs. Downstream
  compiler, HIR, analysis, language-service, LSP, runtime-facing diagnostics,
  and behavior tests use syntax/domain names rather than CST migration names.
- Public `vela_syntax` API exposes a deliberate scoped syntax facade.
- Module layout follows the file-size, scope, `super`, and re-export constraints
  from the Codex goal.
- Migration-era control flow is not left as the final architecture: repeated
  syntax-shape probes, `Ok(None)` fallback chains, `return false` support
  ladders, and generic unsupported branches are either refactored into
  canonical dispatch/support predicates or explicitly justified as deliberate
  fast-path probes.
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
rg -n "legacy-body-parser|legacy_body_parser|parse_owned_body_blocks_for_tests|\\.fallback\\(|\\bExprKind\\b|\\bStmtKind\\b|vela_syntax::ast::(Expr|Stmt|Block|Argument|RecordField)|vela_syntax::ast::\\{[^}]*\\b(Expr|Stmt|Block|Argument|RecordField)\\b|parse_syntax_|fallback payload|token-gap|extract_format_elements|struct Formatter" crates examples editors
rg -n "\\bcst\\b|Cst|CST|cst_" crates/vela_bytecode crates/vela_hir crates/vela_analysis crates/vela_language_service crates/vela_lsp_server
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
- Old owned AST structs, old body parser support, fallback payload helpers, and
  transitional fallback tests are removed from production and tests.
- Migration-only fallback naming is removed from final public, production, and
  test APIs; the surviving CST API uses concise canonical structure and
  function names.
- HIR, analysis, compiler, language service, and LSP tests pass against the new
  syntax API.
- Formatting is CST/typed-AST based and no production or test path depends on
  the old token-gap formatter.
- CST naming is not globally forbidden. It is allowed where the code directly
  implements or verifies the concrete syntax tree. It is not allowed as a
  migration label in downstream compiler, HIR, analysis, language-service, LSP,
  behavior-test, or user-facing diagnostic names after the old AST path is gone.
- Vela language semantics, VM behavior, HostAccess boundaries, reflection
  mutation rules, and hot reload behavior remain unchanged.
- Active source and test files touched by this track stay below 1200 lines unless
  an exception is documented next to the module decision.
- New code avoids import paths with more than one `super`.
- Re-exports are limited to deliberate scoped public APIs.
- The final close-out includes focused tests, workspace tests, formatting, and
  clippy when practical.
- Final zero-result audits prove the forbidden legacy syntax and formatter
  symbols are gone from `crates/`, `examples/`, and editor package sources.

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

Final legacy audit validation:

```bash
rg -n "legacy-body-parser|legacy_body_parser|parse_owned_body_blocks_for_tests|\\.fallback\\(|\\bExprKind\\b|\\bStmtKind\\b|vela_syntax::ast::(Expr|Stmt|Block|Argument|RecordField)|vela_syntax::ast::\\{[^}]*\\b(Expr|Stmt|Block|Argument|RecordField)\\b" crates/vela_bytecode crates/vela_syntax
rg -n "extract_format_elements|struct Formatter|token-gap|delimiter_stack|previous_token" crates/vela_syntax crates/vela_language_service crates/vela_lsp_server
```

Final canonical naming audit:

```bash
rg -n "\\bcst\\b|Cst|CST|cst_" crates/vela_bytecode crates/vela_hir crates/vela_analysis crates/vela_language_service crates/vela_lsp_server
rg -n "missing CST|unsupported CST|CST .*payload|cst_payload|cst_lowering_covers|syntax_only|is_syntax_only" crates/vela_bytecode
```

This audit is not a global zero-result rule. Every remaining hit outside
`vela_syntax` must be reviewed and either renamed to syntax/domain terminology
or justified as directly handling concrete syntax tree representation. The
bytecode hard-switch residue audit should be zero unless a remaining hit is
documented as a concrete syntax tree implementation boundary, which is unusual
outside `vela_syntax`. User-visible diagnostics and `UnsupportedSyntax` debug
fallback output must not mention CST.

Full close-out:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run package:release
```

Run `npm run package:release` from `editors/vscode` only when this track changes
LSP/editor package behavior or needs a VSIX verification pass.
