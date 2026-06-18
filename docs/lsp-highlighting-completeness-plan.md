# LSP Highlighting Completeness Plan

> **Track:** native LSP semantic highlighting plus editor fallback highlighting
> completeness
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release language-service, LSP-server,
> and editor-package internals are allowed. Preserve product contracts:
> analysis-only editor tooling, no runtime script execution, no live host-state
> reads, no `TypeRegistry` mutation, no Rust `&mut` exposure, no script-language
> generics, no monkey patching, and no editor feature that changes language or
> runtime semantics.

---

## 0. Codex Goal

```text
/goal Execute the complete LSP highlighting completeness plan in
docs/lsp-highlighting-completeness-plan.md from the first unchecked phase/task
through final acceptance. This goal is complete only when every phase checklist
item in this execution document and every acceptance criterion in Section 12 is
complete and validated; it is not complete after adding one token kind, after
fixing Zed syntax colors alone, or after any single checkpoint. On each turn or
resume, read docs/goal.md, docs/architecture.md, docs/architecture/lsp.md,
docs/lsp-implementation-plan.md, docs/lsp-clean-architecture-refactor-plan.md,
docs/progress.md, docs/decisions.md, and this execution document, inspect the
current git diff, then choose the smallest verifiable task that advances the
earliest incomplete phase. Implement that task, validate it with the focused
tests named in this document plus any relevant workspace checks, update this
plan's checklist/progress notes and durable docs when status or decisions
change, commit a small Conventional Commit checkpoint, and continue to the next
incomplete task rather than shrinking the goal to the checkpoint just finished.
Use C:\Users\dairc\CLionProjects\rust-analyzer on this Windows machine, or
~/CLionProjects/rust-analyzer on Unix-like shells, as the local rust-analyzer
reference root for architecture comparison. Borrow rust-analyzer's split
between editor-neutral highlight classification, LSP semantic-token projection,
standard/custom token fallback, and VS Code semantic-token contribution
metadata. Do not borrow Rust-specific macro expansion, borrow checking, trait
resolution, or Rust syntax behavior unless a later Vela-specific problem
requires it. Preserve standing product constraints: no general script-language
generics, no Rust &mut exposed to scripts, all host mutation through HostRef,
HostPath, PathProxy, and HostAccess, reflection without runtime type-structure
mutation or monkey patching, analysis-only editor tooling, no runtime script
execution for LSP queries, no live host-state reads, no TypeRegistry mutation,
no new language semantics, and no custom full IDE product. Keep Zed and VS Code
as thin editor packages: semantic classification belongs in
vela_language_service, LSP protocol projection belongs in vela_lsp_server, and
editor packages only provide launch/configuration plus fallback syntax
highlighting or theme-scope metadata. If a real external decision blocks
progress, update docs/blocked.md and leave the goal active or blocked
explicitly; otherwise keep advancing the next unchecked task until the entire
plan is complete.
```

---

## 1. Purpose

Vela already has a native semantic-token path, but the visible editor result is
still much coarser than Rust in rust-analyzer. In Zed, many functions and
members can appear with the same color because the Tree-sitter fallback query is
basic, the semantic token taxonomy collapses several language concepts into
generic `type`, `function`, `method`, or `variable`, and editor themes may not
differentiate the current modifier set.

This plan makes highlighting complete across three layers:

- `vela_language_service`: editor-neutral classification from syntax, HIR,
  analysis facts, stdlib facts, and schema facts.
- `vela_lsp_server`: LSP legend, full/range/delta projection, client capability
  handling, and standard/custom token fallback policy.
- Editor packages: Zed Tree-sitter highlight queries and VS Code TextMate plus
  semantic-token contribution metadata as fallback and theme integration.

The target is rust-analyzer-style layering: one semantic classification model,
multiple protocol/editor projections, and editor-specific fallback grammar or
scope mapping. Zed and VS Code must not grow separate semantic analysis
implementations.

---

## 2. Current Problems

- [ ] The semantic token taxonomy is too small. Script structs, enums, traits,
  type aliases, constants, builtins, booleans, labels, punctuation families,
  and unresolved references are not represented as distinctly as the language
  can support.
- [ ] `DeclarationKind::Struct`, `Enum`, `Trait`, `TypeAlias`, and `Impl`
  currently collapse into `type`, so editor themes cannot reliably style them
  like rust-analyzer styles `struct`, `enum`, `interface`, and `typeAlias`.
- [ ] Const/global declarations collapse into `variable` with `readonly`; this
  is serviceable but less expressive than a dedicated `const` or `static`
  custom token with a standard `variable` fallback.
- [ ] `true`, `false`, and `null` are keyword tokens. Booleans should be able to
  style as `boolean` with a standard fallback; `null` should have an explicit
  policy.
- [ ] Operators and punctuation are too coarse. Arithmetic, comparison,
  logical, negation, path/dot, comma, colon, semicolon, brace, bracket, and
  parenthesis classes cannot be styled separately.
- [ ] Builtin and stdlib concepts are represented mostly by `defaultLibrary`;
  there is no dedicated builtin type/token fallback policy like
  rust-analyzer's `builtinType -> type`.
- [ ] Source-owned, schema-owned, host-owned, and stdlib-owned symbols rely on
  sparse modifiers. Public/private, mutable/immutable, trait-associated,
  callable, control-flow, library/default-library, and source/schema/host
  provenance need a deliberate modifier policy.
- [ ] The LSP server advertises full semantic tokens and delta, but not range
  semantic tokens. Large editors can request smaller ranges when the service can
  answer them cheaply.
- [ ] VS Code does not declare Vela semantic token types, modifiers, or fallback
  TextMate scopes in `editors/vscode/package.json`.
- [ ] Zed's `highlights.scm` covers common syntax but remains a fallback query,
  not a complete semantic substitute. It needs stronger capture coverage for
  declarations, calls, members, variants, types, attributes, literals,
  operators, and punctuation.
- [ ] Tests cover many current LSP token cases, but there is no single
  high-signal highlighting fixture that aligns Tree-sitter captures, TextMate
  scopes, LSP semantic tokens, and rust-analyzer-inspired taxonomy decisions.

---

## 3. rust-analyzer Ideas To Borrow

Borrow the architecture ideas, not Rust-specific language rules. Use this local
checkout as the source reference root:

```text
C:\Users\dairc\CLionProjects\rust-analyzer
```

Useful reference areas in rust-analyzer, with paths relative to that root:

- `crates/ide/src/syntax_highlighting/tags.rs`: editor-neutral `HlTag`,
  `HlMod`, operator, and punctuation taxonomy.
- `crates/ide/src/syntax_highlighting/highlight.rs`: syntax plus semantic
  classification that maps definitions, builtins, literals, operators,
  punctuation, unresolved references, and modifiers into editor-neutral
  highlights.
- `crates/rust-analyzer/src/lsp/semantic_tokens.rs`: LSP semantic token type and
  modifier legend, custom token names, and standard fallback mapping.
- `crates/rust-analyzer/src/lsp/to_proto.rs`: projection from editor-neutral
  highlights to LSP semantic tokens.
- `crates/rust-analyzer/src/lsp/capabilities.rs`: capability advertisement for
  full, delta, and range semantic tokens.
- `editors/code/package.json`: VS Code semantic-token type, modifier, and
  TextMate scope contribution metadata.

Vela should not copy rust-analyzer's full symbol universe. Instead, define the
smallest Vela-owned taxonomy that gives themes enough separation for Vela
source, schema-backed host facts, stdlib facts, and dynamic-language literals.

---

## 4. Target Architecture

```text
source text
  -> vela_syntax lexer/parser
  -> HIR, binding map, TypeFacts, stdlib facts, optional schema facts
  -> vela_language_service editor-neutral SemanticTokenType + modifiers
  -> vela_lsp_server LSP legend, capability-aware projection, full/range/delta
  -> editor package fallback:
       Zed Tree-sitter highlights.scm
       VS Code TextMate grammar + semanticTokenTypes/scopes metadata
```

Feature ownership:

- `vela_language_service` owns all semantic classification, including lexical
  fallback, resolved symbols, declarations, member uses, imports, type hints,
  schema facts, stdlib facts, and unresolved references.
- `vela_lsp_server` owns JSON-RPC, LSP capability negotiation, token legend
  ordering, standard/custom token fallback, relative token encoding,
  full/range/delta responses, and result-id lifecycle.
- `editors/zed` owns extension configuration, grammar packaging, Tree-sitter
  query fallback, outline/indent queries, and package validation only.
- `editors/vscode` owns extension configuration, TextMate fallback scopes, VS
  Code semantic token contribution metadata, launcher boundaries, and package
  validation only.
- No editor package may reimplement source analysis, schema lookup, stdlib
  lookup, or semantic token classification.

Recommended service taxonomy shape:

```text
SemanticTokenType:
  standard LSP types where available:
    namespace, type, class/struct, enum, interface, typeParameter, parameter,
    variable, property, enumMember, function, method, macro, keyword,
    modifier, comment, string, number, regexp, operator, decorator
  Vela custom types with standard fallbacks:
    boolean -> keyword or number policy to decide
    null -> keyword
    bytes -> string
    builtinType -> type
    typeAlias -> type
    const -> variable
    global/static -> variable
    label -> variable
    unresolvedReference -> variable
    arithmetic/comparison/logical/negation/bitwise -> operator
    punctuation/brace/bracket/parenthesis/angle/comma/dot/colon/semicolon -> operator

SemanticTokenModifiers:
  standard LSP modifiers where available:
    declaration, definition, readonly, static, deprecated, defaultLibrary,
    documentation
  Vela custom modifiers:
    host, schema, source, public, mutable, callable, controlFlow, associated,
    trait, unresolved, builtin/library policy as needed
```

The exact final list must be smaller than rust-analyzer's list when Vela does
not need a concept. Every custom token must have an explicit standard fallback
or an explicit "no fallback" decision.

---

## 5. Non-Goals

- [ ] Do not execute Vela scripts or host code to compute highlighting.
- [ ] Do not run the Rust host application to discover schema metadata.
- [ ] Do not inspect or mutate live host state.
- [ ] Do not mutate `TypeRegistry` or runtime type structure.
- [ ] Do not add script-language generics, overload semantics, Rust-like borrow
  checking, macros, or new runtime behavior for editor colors.
- [ ] Do not build separate Zed and VS Code semantic classifiers.
- [ ] Do not make a custom IDE product or editor UI beyond the native LSP server
  and thin editor integrations.
- [ ] Do not require a particular user theme. The plan should expose enough
  token information for capable themes while preserving useful fallback scopes.

---

## 6. Phase Status

Use this section as the resume point. Each phase should end with a verified
checkpoint commit. Leave a checklist item unchecked until its tests or
validation commands have passed locally.

| Phase | Status | Notes |
|---|---|---|
| 1. Baseline inventory and fixtures | Complete | Shared showcase fixture and baseline service/LSP/editor validator coverage now pin current collapse points before taxonomy changes. |
| 2. Token taxonomy and fallback policy | Complete | Expanded service token/modifier names, deterministic legend ordering, fallback policy, and direct taxonomy tests are in place. |
| 3. Service semantic classification | Complete | Declaration-kind, resolved-use provenance, builtin-type, literal, operator/punctuation, control-flow keyword, and unresolved-reference classification are covered. |
| 4. LSP projection and capabilities | In progress | Legend projection remains service-owned; range semantic tokens are now advertised and served by filtering service tokens in the server. |
| 5. Zed Tree-sitter fallback | Not started | Improve `highlights.scm` without semantic analysis. |
| 6. VS Code fallback and scopes | Not started | Add grammar and semantic-token contribution coverage. |
| 7. Cross-editor consistency fixtures | Not started | Align service, Zed, and VS Code behavior. |
| 8. Docs and final validation | Not started | Update setup docs and run final validation. |

---

## 7. Phase 1 - Baseline Inventory And Fixtures

Goal: capture the current behavior and make the visible color gaps testable
before changing implementation.

- [x] Add or identify a single comprehensive Vela highlighting fixture that
  includes functions, methods, structs, enums, enum variants, traits, impl
  methods, fields, properties, modules/imports, const/global declarations,
  locals, parameters, attributes, comments, strings, bytes, numbers, booleans,
  null, control-flow keywords, operators, punctuation, unresolved names, stdlib
  calls, schema-backed host calls, and builtin type hints.
- [x] Add service-level semantic token snapshot coverage for the fixture in
  `crates/vela_language_service` or `crates/vela_lsp_server`, whichever keeps
  the assertion closest to the behavior under test.
- [x] Add Zed Tree-sitter query validation coverage for the same fixture or for
  a smaller query-specific fixture set.
- [x] Add VS Code TextMate grammar validation coverage for the same fixture or
  document the temporary validation gap before Phase 6 fills it.
- [x] Record the current LSP semantic-token legend and visible collapse points
  in test names or fixture comments, not in long progress docs.

Phase 1 notes:

- Shared fixture: `tests/fixtures/lsp_highlighting/showcase.vela`.
- Baseline service/LSP tests pin current behavior without changing taxonomy.
- Current visible collapse points include source structs/enums/traits all using
  `type`, const/global declarations using `variable`, booleans/null using
  `keyword`, broad punctuation/operators using `operator`, and unresolved
  showcase identifiers remaining plain `variable`.
- Zed and VS Code validators now check the shared fixture and fallback
  capture/scope metadata while keeping editor packages thin.

Focused validation:

```bash
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server semantic_tokens
node editors/zed/scripts/validate-package.js
node editors/vscode/scripts/validate-package.js
```

---

## 8. Phase 2 - Token Taxonomy And Fallback Policy

Goal: define a Vela-owned taxonomy that is rich enough for complete
highlighting and stable enough for editor packages.

- [x] Expand `SemanticTokenType` with distinct source-level type-family tokens
  for structs, enums, traits/interfaces, type aliases, enum variants, builtin
  types, constants/statics/globals, booleans/null policy, labels, unresolved
  references, bytes/string escape policy if supported, operator families, and
  punctuation families.
- [x] Expand `SemanticTokenModifiers` with a deliberate provenance and behavior
  policy for source, schema/host, stdlib/default-library, public/private,
  mutable, callable, associated, trait-related, control-flow, unresolved, and
  documentation where supported by source trivia.
- [x] Implement a rust-analyzer-style fallback table for every custom token
  type and custom modifier that might be removed for clients or editor themes
  that only understand standard LSP names.
- [x] Keep legend ordering deterministic. Existing standard token names should
  stay stable where possible; new custom names must have tests pinning legend
  indexes indirectly through encoded token assertions.
- [x] Update service-level tests so each new token type and modifier has at
  least one direct fixture assertion.

Phase 2 notes:

- `SemanticTokenType` now includes distinct source type-family, builtin type,
  const/global, boolean/null, unresolved-reference, operator-family, and
  punctuation-family tokens, with existing legend entries kept first.
- `SemanticTokenModifiers` now has explicit source/schema/host/provenance and
  behavior modifier names plus a modifier fallback table.
- Direct semantic-token fixtures cover high-signal classified token names, and
  taxonomy policy tests enumerate every custom token and modifier fallback,
  including future/no-syntax entries such as labels and type aliases.

Focused validation:

```bash
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server semantic_tokens
```

---

## 9. Phase 3 - Service Semantic Classification

Goal: make `vela_language_service` classify the richer taxonomy from real Vela
syntax and semantic facts instead of editor-specific heuristics.

- [x] Classify declarations by `DeclarationKind`: function, const/global,
  struct, enum, trait, type alias, impl/self type, and module/import.
- [x] Classify uses through resolved binding, declaration, member, import, and
  schema facts: source functions, source methods, trait methods, source fields,
  enum variants, module path segments, schema fields, schema methods, schema
  variants, schema functions, stdlib functions, and stdlib methods.
- [x] Distinguish source-owned, schema/host-owned, and stdlib/default-library
  symbols through modifiers without changing runtime semantics.
- [x] Classify builtin type hints separately from source and schema type hints.
- [x] Classify literals: strings, bytes, numbers, booleans, null, and comments.
  If escape sequences or format specifiers are not yet represented in syntax,
  record that as a later syntax-token enhancement instead of faking it.
- [x] Classify control-flow keywords and operators with `controlFlow` where the
  token role is clear, following rust-analyzer's modifier idea but with Vela
  syntax rules.
- [x] Split operator and punctuation families where the lexer exposes enough
  information: arithmetic, comparison, logical, negation, bitwise, assignment,
  dot/path, comma, colon, semicolon, braces, brackets, parentheses, and angle
  punctuation if Vela syntax uses it.
- [x] Keep parse-error and unresolved-name behavior stable. Tokenization should
  degrade gracefully and still return lexical tokens where possible.
- [x] Keep semantic-token production analysis-only and independent from VM,
  runtime, host-state, and TypeRegistry mutation.

Phase 3 notes:

- Escape sequences and format specifier sub-token classification remain a later
  syntax-token enhancement because the current syntax token stream does not
  expose them separately.
- Unresolved import leaves now classify as `unresolvedReference` with the
  `unresolved` modifier while preserving module classification for earlier
  import path segments.
- Source-owned declarations, local bindings, source declaration uses, and
  script member uses now carry `source`, while schema/host facts continue to
  carry `host` and stdlib facts continue to carry `defaultLibrary`.
- Source-owned struct, enum, and trait type hints now carry `source`, keeping
  them distinct from builtin type hints and schema/host type hints.
- Schema-backed type hints, fields, methods, trait methods, and functions now
  carry both `host` and `schema`, while stdlib facts remain
  `defaultLibrary`.
- Source and schema enum variant constructor/path-expression/pattern uses now
  classify as `enumMember` with `source` or `host` plus `schema` provenance
  when existing HIR or schema facts prove the target.
- Source member uses on locals initialized from source record constructors now
  classify through an editor-only local fact side table, keeping HIR, analysis,
  runtime, and language semantics unchanged.
- Unresolved plain names with HIR diagnostics and unresolved call names that do
  not resolve as source, schema, or stdlib calls now classify as
  `unresolvedReference` with the `unresolved` modifier.

Focused validation:

```bash
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server semantic_tokens
```

---

## 10. Phase 4 - LSP Projection And Capabilities

Goal: project the service taxonomy through LSP completely while keeping protocol
details out of `vela_language_service`.

- [x] Update `vela_lsp_server` legend construction for the expanded token type
  and modifier lists.
- [ ] Add or update tests for initialize capability advertisement, encoded full
  semantic tokens, full/delta result IDs, and fallback behavior for custom
  tokens.
- [x] Add `textDocument/semanticTokens/range` support when the service can
  produce or cheaply filter tokens for the requested range. Until then, keep
  `range: false` and leave this item unchecked.
- [x] Preserve full semantic-token behavior and deterministic result IDs.
- [ ] Keep client capability handling in the LSP server. If the server needs to
  suppress or remap non-standard tokens for a client, that belongs in
  projection, not in service classification.

Focused validation:

```bash
cargo test -p vela_lsp_server semantic_tokens
cargo test -p vela_lsp_server lifecycle
```

Phase 4 notes:

- `vela_lsp_server` builds its legend from the service taxonomy, so expanded
  token type and modifier names project without editor-side classification.
- `textDocument/semanticTokens/range` is advertised and served by filtering
  full service tokens to the requested LSP range before encoding.

---

## 11. Phase 5 - Zed Tree-sitter Fallback

Goal: make Zed's syntax fallback much closer to the LSP semantic token model
while keeping it a grammar query, not a semantic engine.

- [ ] Expand `editors/zed/languages/vela/highlights.scm` for declaration names,
  function calls, method calls, field/property access, enum variants, type
  identifiers, builtin type identifiers where syntactically recognizable,
  attributes, comments, literals, operators, punctuation, and module/import
  path segments.
- [ ] Keep captures aligned with Zed/Tree-sitter conventions where possible:
  `@keyword`, `@function`, `@function.method`, `@type`, `@type.builtin`,
  `@property`, `@variable.parameter`, `@constant`, `@constant.builtin`,
  `@string`, `@number`, `@boolean`, `@comment`, `@operator`, and punctuation
  captures when supported.
- [ ] Add or update checked-in fixture coverage for query captures.
- [ ] Keep package validation strict that the extension launcher does not
  implement semantic highlighting itself.

Focused validation:

```bash
node editors/zed/scripts/validate-package.js
cd editors/tree-sitter-vela
npx --yes tree-sitter-cli@0.25.10 generate
npx --yes tree-sitter-cli@0.25.10 parse --quiet ../../site/src/syntax/fixtures/complete.vela
```

If `complete.vela` is not the right fixture for highlighting coverage, add a
dedicated highlighting fixture and validate that instead.

---

## 12. Phase 6 - VS Code Fallback And Semantic Scopes

Goal: make VS Code useful with TextMate fallback alone and richer with LSP
semantic tokens.

- [ ] Expand `editors/vscode/syntaxes/vela.tmLanguage.json` for declaration
  names, calls, method calls, fields/properties, enum variants, type names,
  builtin types, attributes, literals, comments, operators, punctuation, and
  module/import paths where regex grammar can safely identify them.
- [ ] Add `semanticTokenTypes`, `semanticTokenModifiers`, and
  `semanticTokenScopes` contributions to `editors/vscode/package.json` for all
  custom Vela tokens and modifiers that need theme fallback.
- [ ] Keep VS Code contribution names aligned with the LSP legend strings from
  `vela_lsp_server`.
- [ ] Keep `editors/vscode/extension.js` a thin launcher/configuration layer.
  The package must not compute semantic classifications.
- [ ] Update `editors/vscode/scripts/validate-package.js` if needed so it
  validates contribution metadata without allowing semantic logic in the
  launcher.

Focused validation:

```bash
node editors/vscode/scripts/validate-package.js
```

---

## 13. Phase 7 - Cross-Editor Consistency Fixtures

Goal: prove that LSP semantic tokens, Zed fallback captures, and VS Code
fallback scopes describe the same Vela concepts even though their precision
differs.

- [ ] Keep one canonical highlighting showcase fixture in a path shared by
  service and editor validations, or mirror it through a small script that
  prevents drift.
- [ ] Add an assertion table or snapshot that maps showcase concepts to
  service semantic token type/modifier, Zed capture, and VS Code TextMate or
  semantic scope.
- [ ] Document intentional differences, such as regex grammar limitations,
  Tree-sitter syntax-only limitations, or theme-dependent semantic-token
  styling.
- [ ] Add tests that prevent editor fallback changes from silently diverging
  from the service taxonomy for common concepts.

Focused validation:

```bash
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server semantic_tokens
node editors/zed/scripts/validate-package.js
node editors/vscode/scripts/validate-package.js
```

---

## 14. Phase 8 - Docs And Final Validation

Goal: document how Vela highlighting works and validate the completed track.

- [ ] Update `docs/lsp-editor-setup.md` with the final Zed and VS Code
  highlighting model: Tree-sitter/TextMate fallback plus LSP semantic tokens
  from the native server.
- [ ] Update `docs/lsp-implementation-plan.md` or `docs/architecture/lsp.md`
  only if the durable LSP architecture contract changes.
- [ ] Update `docs/progress.md` only when milestone status, current focus,
  available capability coverage, validation expectations, or remaining gaps
  change. Do not append routine implementation notes.
- [ ] Update `docs/decisions.md` if the final taxonomy introduces a durable
  design decision, such as a custom-token fallback policy.
- [ ] Run focused validation for LSP and editor packages.
- [ ] Run default full validation when completing the entire plan or before a
  final merge checkpoint.

Focused validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node editors/zed/scripts/validate-package.js
node editors/vscode/scripts/validate-package.js
```

---

## 15. Acceptance Criteria

- [ ] Vela has a documented semantic token taxonomy with explicit standard LSP
  names or standard fallback names for every custom token.
- [ ] Functions, methods, stdlib calls, schema/host calls, source calls, and
  unresolved calls are distinguishable in service semantic tokens.
- [ ] Structs, enums, traits/interfaces, type aliases, enum variants, fields,
  properties, modules, imports, constants/globals, parameters, locals, builtin
  types, source types, and schema types are distinguishable where analysis facts
  allow it.
- [ ] Literals, comments, attributes, control-flow keywords, operators, and
  punctuation have complete lexical or semantic coverage and degrade under
  parse errors.
- [ ] The LSP server advertises and serves a legend that matches the service
  taxonomy, preserves full/delta behavior, and supports range tokens if that
  phase is completed.
- [ ] Zed fallback highlighting covers the main Vela syntax surface through
  Tree-sitter queries without duplicating semantic analysis.
- [ ] VS Code fallback highlighting and semantic-token scope contribution
  metadata cover the same main concepts without duplicating semantic analysis.
- [ ] Cross-editor fixtures show which concepts are handled semantically by LSP
  and which are fallback-only in Zed or VS Code.
- [ ] Editor packages remain thin launchers/configuration packages around the
  native server.
- [ ] No runtime behavior, host access behavior, reflection behavior, TypeRegistry
  structure, script syntax semantics, or VM behavior changes solely for
  highlighting.

---

## 16. First Execution Tasks

Use these as the first concrete tasks when starting the goal.

```text
Task: Add a canonical highlighting showcase fixture and baseline semantic-token
snapshots.
Context: This belongs to M20.5 LSP/editor tooling. Current semantic tokens pass
focused tests, but coverage is scattered and does not pin all visible editor
highlighting gaps.
Expected behavior:
  - The fixture contains representative declarations, uses, literals, members,
    schema/stdlib concepts, and unresolved references.
  - Existing behavior is captured without changing taxonomy yet.
Tests:
  - cargo test -p vela_language_service semantic_tokens
  - cargo test -p vela_lsp_server semantic_tokens
Do not change:
  - Do not change token taxonomy in this task.
  - Do not change Zed or VS Code package behavior in this task.
Validation:
  cargo test -p vela_lsp_server semantic_tokens
```

```text
Task: Expand the editor-neutral semantic token taxonomy and fallback policy.
Context: This follows the baseline fixture and mirrors rust-analyzer's
editor-neutral tag plus LSP projection split.
Expected behavior:
  - New Vela token types and modifiers have explicit standard fallback policy.
  - Existing semantic token tests still pass after updating expected names.
Tests:
  - cargo test -p vela_language_service semantic_tokens
  - cargo test -p vela_lsp_server semantic_tokens
Do not change:
  - Do not add editor-specific semantic classification.
  - Do not change runtime or language semantics.
Validation:
  cargo test -p vela_language_service semantic_tokens
  cargo test -p vela_lsp_server semantic_tokens
```

```text
Task: Bring Zed Tree-sitter fallback captures up to the semantic taxonomy.
Context: Zed needs good syntax fallback even when the active theme or client
does not visibly differentiate all LSP semantic tokens.
Expected behavior:
  - highlights.scm captures common declarations, calls, members, types,
    literals, operators, punctuation, and attributes.
  - The Zed package validator still proves the extension remains a thin
    launcher/package.
Tests:
  - node editors/zed/scripts/validate-package.js
  - tree-sitter query or parse fixture command for the highlighting showcase
Do not change:
  - Do not implement semantic lookup in the Zed extension.
Validation:
  node editors/zed/scripts/validate-package.js
```
