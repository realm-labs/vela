# Clean LSP Architecture Refactor Plan

> **Track:** native LSP architecture cleanup, before MVP editor tooling
> hardening
> **Document status:** Codex execution plan
> **Compatibility policy:** breaking pre-release language-service and LSP
> internal APIs are allowed. Do not preserve the current coarse completion
> model, thin completion item shape, protocol projection shape, or feature
> specific cursor scanners as compatibility shims. Preserve product contracts:
> analysis-only LSP behavior, no runtime execution, no live host-state reads,
> no `TypeRegistry` mutation, no Rust `&mut` exposure, HostAccess safety,
> reflection permissioning, source-spanned diagnostics, and no editor feature
> that changes language or runtime semantics.

---

## 0. Codex Goal

```text
/goal Execute the complete clean LSP architecture refactor plan in
docs/lsp-clean-architecture-refactor-plan.md from the first unchecked
phase/task through final acceptance. This goal is complete only when every
phase checklist item in this execution document, every acceptance criterion in
Section 15, and the required Phase 9 cleanup/docs updates are complete and
validated; it is not complete after Phase 1, after fixing completion alone, or
after any single checkpoint. On each turn or resume, read docs/goal.md,
docs/architecture.md, docs/architecture/*.md, docs/architecture/lsp.md,
docs/lsp-implementation-plan.md, docs/progress.md, docs/decisions.md, and this
execution document, inspect the current git diff, then choose the smallest
verifiable task that advances the earliest incomplete phase. Implement that
task, validate it with the focused tests named in this document plus any
relevant workspace checks, update this plan's checklist/progress notes and
durable docs when status or decisions change, commit a small Conventional
Commit checkpoint, and continue to the next incomplete task rather than
shrinking the goal to the checkpoint just finished. Use
~/CLionProjects/rust-analyzer as the local rust-analyzer reference root for
architecture comparison, especially its split between completion context
construction, feature-specific producers, rich editor-neutral completion item
models, and LSP protocol projection. Treat completion as the first visible
failure to fix, but keep the refactor scoped to the full LSP surface:
diagnostics, completion, signature help, hover, definition, symbols, semantic
tokens, references, rename, code actions, formatting, inlay hints, workspace
snapshots, cancellation, configuration, and completion-specific scale behavior
near the one-million-line workspace target. Preserve standing product
constraints: no general script-language generics, no Rust &mut exposed to
scripts, all host mutation through HostRef, HostPath, PathProxy, and
HostAccess, reflection without runtime type-structure mutation or monkey
patching, analysis-only editor tooling, no runtime script execution for LSP
queries, no live host-state reads, no TypeRegistry mutation, no new language
semantics, and no custom full IDE product. Replace obsolete
language-service/LSP-server internals instead of carrying compatibility shims.
If a real external decision blocks progress, update docs/blocked.md and leave
the goal active or blocked explicitly; otherwise keep advancing the next
unchecked task until the entire plan is complete.
```

---

## 1. Purpose

The current native LSP track has enough feature coverage to be useful, but the
internal model is still too coarse. The most obvious symptom is completion:
typing `f` at an item boundary can rank stdlib function facts above the `fn`
keyword, and typing `st` does not reliably surface `struct` as the primary
choice. That is not only a sorting bug. It shows that the service does not yet
have a strong cursor-context model, typed completion producers, rich completion
items, or protocol projection boundaries.

This refactor should solve completion first because it is the most visible
editor failure, then carry the same architecture through the rest of the LSP.
The target is a language-service model that can answer each editor request
from shared source, syntax, HIR, analysis, schema, and workspace facts, while
keeping LSP transport and protocol details isolated in `vela_lsp_server`.

This is a cleanup track, not a compatibility track. Existing internal APIs can
be deleted when their replacements are verified.

---

## 2. Current Problems

- [ ] Completion uses broad contexts such as global, member, type hint, and
  module path instead of item, statement, expression, pattern, type, use,
  record, call, and keyword-sensitive contexts.
- [ ] Top-level and statement-position completion mixes declarations,
  modules, stdlib functions, schema facts, and keywords without a relevance
  model that understands what can syntactically appear there.
- [ ] Completion items are too thin for high-quality editors: source
  replacement ranges, lookup/filter text, label details, snippet shape,
  relevance, preselect, deprecation, documentation, and resolve payloads are
  not first-class service concepts.
- [ ] LSP conversion is forced to infer protocol behavior from a lossy service
  item instead of projecting a rich editor-neutral item into LSP types.
- [ ] Feature handlers still grow their own cursor and display decisions.
  Completion, hover, signature help, definition, semantic tokens, references,
  rename, and code actions need shared source identity, symbol identity, range,
  display, and edit-plan concepts.
- [ ] Tests prove individual happy paths but do not pin the model boundaries:
  cursor-context classification, item-vs-expression completion, replacement
  ranges, relevance ordering, LSP projection, and stale-result behavior.

---

## 3. rust-analyzer Ideas To Borrow

Borrow the architecture ideas, not Rust-specific implementation complexity.
Use this local checkout as the source reference root:

```text
~/CLionProjects/rust-analyzer
```

Useful reference areas in rust-analyzer, with paths relative to that root:

- `crates/ide-completion/src/lib.rs`: completion is split into context
  construction and completion production. The engine produces candidates and
  metadata; filtering by the already typed substring is left to the editor or
  projection layer.
- `crates/ide-completion/src/completions.rs`: completion dispatch is driven
  by syntax-aware name-reference and path contexts such as expression, type,
  item, pattern, visibility, use, dot access, and record fields.
- `crates/ide-completion/src/completions/item_list.rs`: item-list keyword
  completions are context-specific producers. `fn`, `struct`, and related
  snippets are not generic globals.
- `crates/ide-completion/src/item.rs`: completion items carry source ranges,
  text edits, lookup/filter identity, details, documentation, deprecation,
  relevance, imports, and resolve-time payload.
- `crates/rust-analyzer/src/lsp/to_proto.rs`: LSP conversion is a projection
  layer that maps editor-neutral item metadata into `CompletionItem`,
  `TextEdit`, `filterText`, `labelDetails`, `sortText`, `preselect`, docs,
  tags, and snippet formats.

Do not borrow rust-analyzer's macro expansion model, full Salsa setup,
Rust-specific type inference machinery, import insertion semantics, or trait
resolution complexity unless a later Vela-specific problem justifies it.

---

## 4. Target Architecture

```text
LSP request
  -> vela_lsp_server protocol handler
  -> WorkspaceSnapshot query
  -> QueryContext construction
  -> feature classifier
  -> feature producers
  -> editor-neutral feature result
  -> LSP projection
  -> JSON-RPC response
```

Shared service concepts:

- `QueryContext`: stable request inputs, document identity, snapshot
  generation, cursor offset/range, source text, syntax root, module facts,
  HIR/analysis facts, optional schema facts, cancellation generation, and
  client capability hints that do not leak LSP protocol types.
- `CursorContext`: syntax-aware cursor classification shared by completion,
  hover, signature help, definition, references, rename, and code actions.
- `SymbolRef`: editor-stable identity for source declarations, schema-owned
  host facts, builtin facts, local bindings, fields, variants, methods, and
  modules.
- `DisplayParts`: editor-neutral display text for signatures, hover labels,
  completion detail, symbol labels, inlay labels, and diagnostics.
- `EditPlan`: source-owned edit representation with ranges, replacement text,
  snippet intent, import insertion intent, and conflict metadata.
- `Relevance`: feature-owned scoring metadata that can be projected into LSP
  `sortText`, `preselect`, and secondary grouping without hard-coding protocol
  behavior in producers.

Million-line completion requirements:

- Completion must not rebuild the whole workspace on a keystroke. It may use
  the current open-document overlay, the last complete project snapshot, and
  stale-but-valid background indexes while newer background work is pending.
- Item, module, type, function, field, method, variant, local, import, stdlib,
  and schema candidates need precomputed or incrementally maintained indexes.
  A completion request should join the small set of indexes relevant to the
  `CursorContextKind`, not scan every parsed file.
- Editing a function body should avoid invalidating module-level declaration
  indexes unless the edit changes declarations, imports, public signatures, or
  other module fingerprints.
- Prefix/lookup filtering should run against indexed candidate identities. The
  producer should avoid materializing full documentation or expensive display
  strings until an item survives context and prefix narrowing or is resolved
  lazily.
- Member completion must be receiver-scoped. For known receivers, query the
  source/schema/builtin member tables for that type or trait set; do not scan
  global declarations.
- Top-level and type-position completion must use item/type indexes directly
  and exclude expression-only stdlib facts before ranking.
- Completion ranking should be bounded and deterministic. Relevance scoring
  can sort the final candidate set, but producer selection must keep that set
  small enough for interactive latency.
- Long-running index refreshes must be cancellable or generation-checked so an
  old completion result is never published over a newer buffer state.
- Scale tests must include completion-specific checkpoints against synthetic
  many-file workspaces near the one-million-line target, not only diagnostics
  or indexing checkpoints.

Feature ownership:

- `vela_language_service` owns all query construction, cursor classification,
  feature production, and editor-neutral result models.
- `vela_lsp_server` owns JSON-RPC, LSP request/response structs, position and
  range conversion, client capability mapping, request cancellation, progress,
  workspace folders, file watching, and configuration transport.
- `vela_syntax`, `vela_hir`, `vela_analysis`, and `vela_reflect` remain the
  source of truth for syntax, module graph, semantic facts, and schema facts.

---

## 5. Non-Goals

- [ ] Do not execute Vela scripts to answer editor queries.
- [ ] Do not run the Rust host application to discover schema metadata.
- [ ] Do not inspect or mutate live host state.
- [ ] Do not mutate `TypeRegistry` or runtime type structure.
- [ ] Do not add script-language generics, new overload semantics, monkey
  patching, JIT behavior, async/coroutine behavior, or runtime hot-reload
  semantics as part of LSP cleanup.
- [ ] Do not build a custom full IDE product.
- [ ] Do not keep old service APIs alive only to avoid updating tests or LSP
  callers.
- [ ] Do not let code actions, rename, or formatting invent type facts or
  choose ambiguous semantic rewrites.

---

## Phase Status

Use this checklist as the durable execution tracker. Mark a task only after
its focused tests and validation command pass.

```text
[ ] not started
[~] in progress
[x] complete
```

---

## 6. Phase 1: Pin The Broken Model With Tests

Purpose: write the failing tests before replacing the model.

- [x] Add language-service completion fixtures for item-boundary keyword
  completion:
  - typing `f` at a top-level item boundary ranks `fn` before stdlib symbols.
  - typing `st` at a top-level item boundary ranks `struct` before unrelated
    globals.
  - item-boundary completion does not offer expression-only stdlib functions.
- [x] Add statement, expression, type-hint, member, record-field, module-path,
  call-argument, and lambda-parameter cursor-context classification fixtures.
- [x] Add LSP completion conversion fixtures for replacement ranges,
  `filterText`, `sortText`, snippets, label details, and preselect behavior.
- [x] Add regression fixtures showing that existing member, type, module,
  record, and schema completions still work in their proper contexts.
- [x] Add at least one stale-generation or cancellation fixture if existing
  request plumbing can expose it cheaply.
  Covered by language-service stale/cancelled background diagnostics and the
  native LSP stale-request cancellation fixture.

Tests:

```bash
cargo test -p vela_language_service completion
cargo test -p vela_lsp_server completion
```

---

## 7. Phase 2: Build Shared Query And Cursor Context

Purpose: classify the cursor once and reuse it across features.

- [x] Add a focused query-context module in `vela_language_service` that
  builds request-local facts from a `WorkspaceSnapshot`.
- [x] Add `CursorContextKind` variants for item, statement, expression, type,
  pattern, use/import, module path, member access, record expression field,
  record type field, call argument, lambda parameter, map key, rename target,
  and unknown/error recovery.
- [x] Make cursor classification syntax-owned. Prefer parser/token structure
  and source spans over ad hoc substring checks.
  Record expression fields, record type fields, map keys, for/match pattern
  contexts, complete member-access receivers, call-argument callee contexts,
  lambda-parameter receiver contexts, and type-hint completion contexts now
  use shared cursor spans/classification. Incomplete member access now uses
  parser recovery for empty field nodes when available and a syntax-token
  recovery helper for trailing-dot receivers, including call/index/dot-chain
  receivers, instead of the old top-level substring fallback.
- [x] Include expected syntactic role, local scope, module scope, receiver
  expression facts, path qualifier facts, callable facts, and replacement
  range where available.
  `QueryContext` now exposes request source id and module path facts directly;
  call-argument contexts now expose callee ranges; `QueryContext` exposes
  receiver, callee, and lambda-method ranges plus text from shared cursor
  facts; completion member context now consumes the shared receiver range,
  named-argument completion now carries the shared call callee range, and
  lambda-parameter completion now consumes the shared receiver and method
  ranges. Top-level function local binding facts are request-local, and
  `QueryContext` now exposes locals visible before the cursor for completion;
  module path qualifiers now carry type-vs-expression role in `CursorContext`
  instead of completion re-scanning source text. Inlay parameter-hint
  signature lookup now requires syntax-owned member receiver ranges for shared
  signature candidate construction instead of keeping feature-local receiver
  recovery. Type-hint completion now
  consumes `CursorContextKind::Type` rather than running its own type-context
  scanner. Call-argument contexts now expose the active call opening offset,
  and `QueryContext` exposes the active argument prefix text, so
  named-argument completion and signature help no longer slice that range
  independently. `QueryContext` also exposes the active call parameter index,
  so signature help no longer owns top-level argument counting. Call-argument
  queries now expose one shared `CallArgumentFacts` object with callee,
  callee range, call opening offset, argument prefix, active parameter index,
  and member receiver facts; named-argument completion and signature help both
  consume that shared model instead of collecting those pieces separately.
  Lambda
  callback contexts now expose the callback call opening offset alongside the
  shared receiver and method facts, so lambda-parameter completion no longer
  reconstructs member callees locally. Completion no longer has a global
  fallback that reclassifies named arguments outside
  `CursorContextKind::CallArgument`. Record-field and map-key completion now
  only run their producer-specific fact extraction after shared cursor
  classification selects the matching kind. Incomplete call recovery now
  attaches fallback callee ranges in `CursorContext`, so named-argument
  completion no longer has a producer-local callee scanner. Signature help now
  consumes the shared call-argument callee range and active call opening
  offset instead of reconstructing active calls locally, and lambda body
  expressions remain call-argument contexts instead of being reclassified as
  lambda parameters. Call-argument contexts now expose member receiver ranges
  for method callees, and signature help consumes that shared call receiver
  range for method signature lookup. Definition and hover now consume the
  shared member receiver range from `QueryContext` for schema/member lookup
  instead of re-scanning member receivers locally. `QueryContext` now exposes
  shared range-to-`TypeFact` lookup with schema type-hint fallback; member
  completion, lambda-parameter completion, hover, definition, and signature
  member-call lookup consume it for receiver semantic facts instead of keeping
  separate receiver fact walkers. The shared range-to-fact path now handles
  function bodies, trait default bodies, and impl method bodies from the same
  binding-map lookup. `QueryContext` now exposes source function callable facts
  with parameter names, default markers, type facts, and return facts; named
  argument completion and source function signature help consume that shared
  callable model instead of rebuilding source function facts separately.
  Parameter inlay hints now consume AST-owned member receiver spans and shared
  callable facts directly, deleting the feature-local member callee
  source-slice scanner while leaving the broader feature-local cursor scanner
  audit open. Semantic token member-use classification now consumes a parsed
  field-expression receiver range map instead of recovering member receivers
  with a backwards text scanner for each identifier token. Semantic token
  function-call classification now consumes shared parsed path-call sites
  instead of reconstructing call paths from token text. Call hierarchy
  preparation now resolves method-call targets from `QueryContext` member
  receiver ranges and shared receiver type facts instead of recovering the
  selected call receiver locally. Call hierarchy incoming/outgoing method-call
  ranges now consume shared AST member-call sites instead of tokenizing source
  text and scanning backward for receivers. References and document highlights
  now resolve the selected source/schema member target from `QueryContext` member
  receiver ranges and shared receiver type facts instead of running
  feature-local member receiver recovery for the cursor token. Source method
  reference scans now consume shared AST member-call sites instead of lexing
  all identifiers and scanning backward for receivers, and source field
  reference scans consume shared AST member-access sites the same way. Schema
  method and field reference scans now consume the same shared AST member-call
  and member-access sites with query-owned receiver facts instead of
  feature-local receiver scanning. Prepare-rename and rename now resolve the
  selected source/schema member target from the same query-owned receiver ranges
  and type facts, preserving explicit
  `self.method()` handling without local receiver scanning; source method
  rename edit collection now also consumes shared AST member-call sites with
  explicit `self.method()` owner recovery, and source field rename edit
  collection consumes shared AST member-access sites. Schema member rename edit
  collection now consumes shared AST member-call/access sites with query-owned
  receiver facts instead of feature-local member receiver scanning. Schema
  function rename edit collection now consumes shared parsed path-call sites
  instead of tokenizing identifiers and reconstructing callees from source
  text, and schema function rename target selection consumes the same shared
  parsed path-call sites without legacy call-name reconstruction. Source and
  schema enum variant rename edit collection now
  consumes shared parsed expression and pattern path sites for
  constructor-like and pattern uses instead of falling back to legacy token
  range scanners. Source enum variant rename target selection consumes parsed
  expression and pattern path sites without legacy path reconstruction. Schema
  variant rename target selection consumes expression path sites without legacy
  path reconstruction. Source and schema enum variant reference target
  selection and reference scans now consume the same shared parsed expression
  and pattern path sites without a feature-local token range fallback.
  Schema variant go-to-definition now resolves constructor-like expression paths
  only from the shared parsed path sites, without legacy token path
  reconstruction.
  `CursorContext` now exposes the identifier token range under the request
  cursor, and rename target selection consumes that shared range instead of
  running its own first-step token scanner. Lambda-parameter contexts now
  expose the current parameter list range, and lambda completion consumes that
  shared text instead of finding the `|` pipe locally. Definition and hover now
  consume the shared identifier token range from `QueryContext` instead of
  recomputing the cursor token locally, and references plus call hierarchy
  preparation use the same shared identifier token for their initial request
  target. Broader callable semantic facts now include direct source, schema,
  and stdlib function callables, source and schema enum tuple variant
  callables, and source inherent/trait/default, schema host/trait, and stdlib
  method callables shared by signature help and inlay hints, while
  named-argument completion continues to consume the source function subset;
  schema record/named variant constructor families remain deferred until the
  schema contract carries constructor order.
- [x] Keep classification tolerant of incomplete source and parser recovery.
- [x] Route completion, hover, signature help, definition, and rename prepare
  through the shared context where practical.
  Completion, signature help, hover, definition, prepare-rename, and rename
  now build request-local source, parse, cursor, module, and generation facts
  through `QueryContext`; references and call hierarchy preparation now use
  it for source and cursor-token ownership as well.

Tests:

```bash
cargo test -p vela_language_service cursor_context
cargo test -p vela_language_service completion
```

---

## 8. Phase 3: Rewrite Completion Around Producers

Purpose: replace coarse global completion with context-specific producers.

- [x] Split completion into focused modules such as context, item, relevance,
  producers, render, and tests when file size or responsibility requires it.
  Item, statement, local-binding, expression/global, type-hint, record-field,
  map-key, named-argument, lambda-parameter, and pattern producers now live in
  focused modules, module-path candidate construction now lives in its own
  focused producer module, completion context construction now lives in a
  focused context module, analysis-backed item rendering now lives in a shared
  helper module, and the editor-neutral completion model now lives in a
  focused model module. Relevance ranking, match ranking, sort-text
  construction, and accumulator ordering now live in a focused relevance
  module.
- [x] Replace `CompletionContextKind::Global` style dispatch with producers
  selected by `CursorContextKind`.
  Pattern cursor contexts now route to a dedicated source/schema enum-variant
  producer and statement cursor contexts now route to statement keyword
  completions; expression cursor contexts now route through an explicit
  expression completion context while preserving the current expression
  candidate set. The obsolete `CompletionContextKind::Global` variant is
  removed, and fallback completion uses expression recovery instead. Pattern
  and statement completions have native LSP projection fixtures.
- [x] Add keyword and snippet completions as first-class completion kinds.
  Keyword items are first-class service `CompletionKind::Keyword`, and
  callable snippets now carry explicit editor-neutral insert-format metadata
  through service items. Item-boundary declaration snippets now use first-class
  service `CompletionKind::Snippet` with snippet insert-format metadata and
  LSP snippet-kind projection.
- [x] Add item-boundary producers for `fn`, `struct`, `enum`, `trait`, `impl`,
  `let`, `const`, imports, modules, and source declarations that are legal in
  that context.
  Top-level declaration keywords now come from an item-boundary producer with
  declaration snippets for `fn`, `struct`, `enum`, `trait`, `impl`, `use`,
  `const`, `global`, and `pub`. Top-level `let`, existing source declarations,
  and module names are not offered because Vela's syntax does not support them
  as items; module and source declaration completions stay in module-path,
  expression, and type contexts where they are syntactically meaningful.
- [x] Add expression producers for locals, parameters, functions, methods,
  variants, builtin values, stdlib functions, and schema facts that are legal
  in expression position.
  Module-path expression completion now includes source and schema enum
  variants for `Enum::Va` style constructors. Expression producer separation
  now has focused local-binding, builtin-value, source const/function/type,
  schema type/function, stdlib function, source module, and expression
  coordinator producer modules. Method completion remains receiver-owned in
  member contexts, while enum variants remain qualified module-path, pattern,
  record/map-key, and constructor-context candidates instead of leaking as
  unqualified expression globals.
- [x] Add type producers for builtin types, script types, schema types,
  modules, traits, and type aliases when those exist.
  Type-position completion now offers builtin type hints, source and schema
  types, source and schema traits, module path segments, and qualified
  type-path segments while excluding function-only candidates. Vela has no
  type aliases yet.
- [x] Keep member and record producers receiver-aware. Do not leak unrelated
  globals into member contexts.
  Member completion resolves host and schema trait receiver facts without
  falling back to global candidates, and record-field completion requires a
  known source or schema constructor before offering fields.
- [x] Add a `CompletionAccumulator` that accepts candidates, de-duplicates by
  lookup identity and edit range, and applies relevance metadata without
  client-specific fuzzy filtering.
  Completion aggregation now uses a focused accumulator that accepts service
  candidates, de-duplicates by lookup identity plus replacement range, and
  applies deterministic service-owned relevance ordering while leaving fuzzy
  filtering to the editor/projection layer.
- [x] Add a rich editor-neutral `CompletionItem` model with label, lookup,
  detail, documentation, kind, source range, text edit, snippet intent,
  filter text, label details, relevance, deprecation, symbol identity, and
  optional resolve payload.
  The service item model now carries editor-neutral metadata fields for lookup
  identity, source range, text edit, filter text, label details,
  documentation, relevance, deprecation, symbol identity, and resolve payload.
  The accumulator populates derived lookup/filter/range/text-edit and
  relevance metadata; schema-backed type, field, method, and enum-variant
  producers now populate docs and schema symbol identity. Source declaration
  producers now preserve fully qualified source symbol identity while keeping
  current-module display labels relative. Symbol-bearing producers now attach a
  documentation resolve payload in the service model, and the item model has
  explicit producer-owned deprecation and resolve-payload metadata; current
  source/schema facts do not yet carry deprecated-candidate input.

Tests:

```bash
cargo test -p vela_language_service completion
```

---

## 9. Phase 4: Rewrite LSP Projection

Purpose: keep LSP protocol behavior out of producers while giving editors
high-quality metadata.

- [x] Convert service completion items to LSP completion items using source
  ranges and text edits supplied by the service item.
  Service snippet completion kinds now project to LSP `CompletionItemKind`
  `Snippet`, while insert text format still comes from service-owned snippet
  intent. LSP projection consumes service-owned text edits when present,
  preserves label/filter/sort/preselect metadata, and keeps service resolve
  payloads in `CompletionItem.data`.
- [x] Set `filterText` from lookup identity when label text and inserted text
  differ.
- [x] Set `labelDetails` for signatures, modules, receiver types, and return
  types where the client supports it.
- [x] Set `insertTextFormat` from snippet intent rather than hard-coded
  function heuristics.
- [x] Project relevance into stable `sortText` and `preselect`.
  LSP projection preserves producer-owned `sortText` when present, derives
  stable fallback sort keys from service relevance otherwise, and marks the
  first service-ranked item as `preselect`.
- [x] Project deprecation into LSP tags.
  Service completion deprecation metadata projects to LSP `CompletionItemTag`
  value `1`; producer-owned deprecated candidates remain separate from the
  protocol projection path.
- [x] Add or preserve lazy resolve support for docs and expensive detail when
  it becomes useful.
  Schema-backed completion documentation is now projected eagerly from
  service-owned metadata, and service-owned documentation resolve payloads are
  preserved in LSP `data` while `completionItem/resolve` remains unadvertised
  until a concrete resolve handler is needed.
- [x] Add protocol fixtures for common editor clients, including Zed's
  completion behavior.
  Native LSP completion fixtures now include a Zed-shaped client capability
  request that preserves snippet, replacement-range, label-detail, sort, and
  preselect projection without moving client-specific behavior into producers.

Tests:

```bash
cargo test -p vela_lsp_server completion
```

---

## 10. Phase 5: Unify Symbol And Display Models

Purpose: make hover, signature help, definition, symbols, references, rename,
and semantic tokens use the same identities and display primitives.

- [x] Introduce `SymbolRef` or equivalent identity for local bindings, source
  declarations, modules, fields, methods, variants, builtin facts, stdlib
  facts, and schema-owned facts.
  A shared `SymbolRef` identity now exists in `vela_language_service`, and the
  completion model reuses it through the existing `CompletionSymbol` export.
  Hover and definition now build a shared cursor `SymbolTarget` that carries
  schema and builtin `SymbolRef` classification for schema member, variant,
  type, trait, function, and stdlib targets, and definition results now expose
  `SymbolRef` identity for local bindings, source declarations, and schema
  facts while keeping LSP projection protocol-neutral; inlay hints now carry
  shared `SymbolRef` identity for callable parameter hints, inferred local and
  lambda type hints, and schema-backed host-path type hints. Diagnostics now
  carry optional shared `SymbolRef` identity, with service-owned unused import
  diagnostics stamped with the resolved source declaration symbol. Local symbol
  identity now carries an optional source document plus declaration name range,
  and local-backed definition, hover, references, rename edits, and inferred
  local/lambda inlay hints stamp that precise identity when the binding span is
  available. Source declaration/member/variant symbol construction now lives in
  the shared `SymbolRef` boundary and is consumed by references and rename;
  diagnostics, hover, and definition now route source declarations, struct fields,
  impl/trait methods, and enum variants through those shared constructors;
  signature and inlay callable facts now use the same source constructors for
  script functions, methods, and enum tuple-variant constructors;
  module-path, map-key, and pattern source enum variant completions now use the
  shared enum variant symbol constructor;
  import-module references, cursor targets, and workspace module symbols now
  use the shared module symbol constructor;
  schema hover type, trait, function, member, and variant facts now use shared
  schema symbol constructors;
  schema reference identities for fields, methods, and variants now use the
  shared schema member/variant symbol constructors;
  schema completion identities for members, record fields, and enum variants
  now use the shared schema member/variant symbol constructors;
  shared cursor targets now use the shared schema constructors for schema
  symbols, members, and variants;
  callable facts and inlay hint parameter/host-path symbols now use the shared
  schema constructors;
  prepare-rename target symbols for schema facts now use the shared schema
  constructors;
  document/workspace schema symbols now use the shared schema constructors,
  leaving only tests and the constructor helpers with direct
  `SymbolRef::Schema` construction;
  schema type/trait/function completion adapters now use the shared schema
  symbol constructor, leaving no production `CompletionSymbol::Schema`
  construction outside the shared helper boundary;
  builtin/stdlib hover, callable, cursor-target, and inlay parameter symbols
  now use shared builtin symbol constructors, leaving only tests and helper
  pattern matches with direct `SymbolRef::Builtin` construction;
  source file symbols, source completion adapters, and source inlay parameter
  symbols now use shared source constructors, leaving production source,
  schema, and builtin symbol construction centralized in `symbol_ref`;
  document and workspace symbols now use the shared constructors for source
  declarations and nested source-owned members;
  document symbols now expose
  `SymbolRef` identity for source declarations and nested source-owned
  members, and workspace symbols expose `SymbolRef` identity for source files,
  modules, source declarations, and schema facts while keeping LSP projection
  protocol-neutral. Hover results now carry `SymbolRef` identity for source
  declarations, source-owned members, locals, schema facts, and builtin
  symbols. Prepare-rename results now carry `SymbolRef` identity for local,
  source-owned, and source-backed schema targets without changing LSP protocol
  projection. Reference results now carry `SymbolRef` identity for locals,
  source declarations, source-owned members and variants, source modules, and
  schema-owned fields, methods, and variants while keeping LSP projection
  protocol-neutral. Rename workspace edits now preserve the selected
  `SymbolRef` target through the service edit model without changing LSP
  projection.
- [x] Route hover labels, completion details, signature labels, symbol labels,
  inlay labels, and diagnostics through `DisplayParts` or equivalent
  structured display helpers.
  A shared `DisplayParts` segment model now exists in `vela_language_service`.
  Signature labels and parameter labels, parameter/type inlay labels, selected
  hover member/module labels, script/schema symbol names and signature details,
  and type-shaped completion details now render through it while preserving the
  existing user-visible strings. Completion items now carry structured detail parts
  alongside their rendered `detail`, with the accumulator defaulting older
  prose details to plain display parts and typed local/analysis-backed details
  preserving type display parts. Hover results now carry structured detail
  parts alongside their rendered detail; local, declaration, schema type,
  schema variant, source field, source method, and schema field/function/method
  details preserve type or signature parts while permission/effect metadata is
  structured as trailing detail text; stdlib function and method hover details
  now preserve type-shaped detail parts. Document and
  workspace symbol names/details now carry `DisplayParts` alongside their
  rendered strings while keeping LSP projection stable. Diagnostic messages,
  labels, candidates, and repair hint titles/replacements now carry
  `DisplayParts` alongside their rendered strings while keeping LSP projection
  stable; builtin value and lambda-parameter completion details now preserve
  type-shaped `DisplayParts`; record-field and map-key completion details now
  preserve type-shaped `DisplayParts`; module-path, pattern, and type-hint
  completion details now preserve type-shaped or qualified-symbol
  `DisplayParts`; named-argument and local completion details now preserve
  `DisplayParts`, and the obsolete string-only type detail helper has been
  removed; keyword and snippet completion details now carry explicit plain
  `DisplayParts`; unresolved import and qualified-path hover details now carry
  structured `DisplayParts`, and the generic string-only hover detail helper
  has been removed.
- [x] Make go-to-definition and hover resolve symbols through the shared
  cursor context before falling back to feature-local logic.
  Hover and definition now consume a shared `SymbolTarget` built from
  `QueryContext` identifier ranges, member receiver facts, schema source-span
  lookups, and schema/builtin `SymbolRef` classification. `SymbolTarget` now
  resolves local binding, local declaration, source declaration, and source enum
  variant identities from the query binding context, resolves import path
  segments to source declaration/module identities, and local hover/definition
  plus import module and script-owned member hover consume that shared identity
  before falling back to feature-local logic.
- [x] Ensure schema-owned symbols can be displayed and completed without
  pretending they have source definitions.
  Completion edit metadata now uses editor-neutral `edit_range` naming instead
  of source-definition terminology, and schema-backed record field completions
  carry schema documentation plus `SymbolRef::Schema` identity through LSP
  projection. Schema-backed completion, hover, references, rename prepare, and
  workspace symbol surfaces now carry schema identities without source-backed
  locations unless schema source spans are present; definition/type-definition
  now avoid falling back to the enclosing script declaration for schema-owned
  targets without source spans.
- [x] Preserve source-span accuracy for script-owned symbols.
  Source enum variant completions in module-path, map-key, and pattern
  contexts now carry `SymbolRef::Source` identities through service and LSP
  projection without changing their display labels. Definition, hover,
  references, rename, document/workspace symbols, and inlay hints now use
  source declaration/member spans or name ranges for source-owned symbols while
  carrying shared `SymbolRef::Source` identities through service results and
  LSP projection.

Tests:

```bash
cargo test -p vela_language_service hover
cargo test -p vela_language_service definition
cargo test -p vela_language_service signature
cargo test -p vela_lsp_server hover
cargo test -p vela_lsp_server definition
```

---

## 11. Phase 6: References, Rename, And Semantic Tokens

Purpose: make cross-reference features use explicit symbol identity and
source-owned edit plans.

- [x] Build references from symbol identity plus syntax/HIR binding facts, not
  plain text matching.
  Source/local references now route through `BindingResolution` and all
  reference result paths carry shared `SymbolRef` identities; the
  `references_keep_shadowed_local_bindings_separate` fixture pins that
  same-named local bindings do not collapse through text matching.
- [x] Distinguish source-owned references, schema-owned facts, builtin facts,
  dynamic `Any` facts, and unresolved names.
  `reference_query()` now returns a `ReferenceQueryResult` with
  `ReferenceResolution` categories for source-owned, schema-owned, builtin,
  dynamic `Any`, and unresolved targets while preserving the LSP-facing
  location projection.
- [x] Make prepare-rename reject schema-owned, builtin, dynamic, unresolved,
  and ambiguous targets.
  Prepare-rename now has fixtures for host-schema targets, builtin stdlib
  functions, dynamic `Any` member access, unresolved names, and ambiguous
  schema short-name calls; all reject instead of producing editable ranges.
- [x] Make rename produce an `EditPlan` with conflict checks and source-owned
  ranges only.
  `WorkspaceEdit` now owns an `EditPlan`; rename producers route through the
  shared checked builder that sorts, deduplicates, versions source documents,
  and rejects overlapping edit ranges before LSP projection.
- [x] Ensure semantic tokens are generated from syntax/HIR classification and
  stay stable under parser recovery.
  Semantic tokens already layer lexer/syntax tokens with HIR/schema
  classifications; recovery fixtures now cover both lexical degradation and
  retained HIR-backed function/parameter classifications through an incomplete
  body expression.
- [x] Add fixtures for shadowing, modules, methods, fields, and failed rename
  targets.
  Reference fixtures now cover local shadowing, source/schema/builtin/dynamic/
  unresolved resolution categories, failed prepare-rename targets, checked
  edit-plan conflict rejection, semantic-token parser recovery stability, plus
  existing module, method, and field cases.

Tests:

```bash
cargo test -p vela_language_service references
cargo test -p vela_language_service rename
cargo test -p vela_language_service semantic_tokens
cargo test -p vela_lsp_server references
cargo test -p vela_lsp_server rename
```

---

## 12. Phase 7: Code Actions, Formatting, And Inlay Hints

Purpose: keep editor mutations structured, local, and source-owned.

- [x] Route quick fixes through structured diagnostics and `EditPlan`.
  Quick fixes now continue to originate from structured diagnostics,
  candidates, and repair hints, and all code-action edits are built through the
  checked `WorkspaceEdit::try_new`/`EditPlan` path before LSP projection.
- [x] Reject ambiguous imports, dynamic receiver typo fixes, and semantic
  rewrites without a proven local pattern.
  Ambiguous import and dynamic receiver typo fixes are covered at service and
  LSP layers; semantic rewrite helpers now have a regression test proving they
  stay silent unless the diagnostic range contains the local syntax pattern
  they know how to edit.
- [x] Keep formatting syntax-owned and trivia-preserving. Do not rely on
  successful HIR or analysis.
  Formatting remains routed through `vela_syntax::formatting` and syntax parse
  facts only. Regression coverage now proves document formatting still works
  with unresolved HIR diagnostics and preserves semicolonless `use` item
  newline boundaries.
- [ ] Add AST-aware range and on-type formatting only after token/trivia rules
  are stable.
- [~] Generate inlay hints from stable type and signature facts, not ad hoc
  string parsing.
  Parameter inlay hints now use semantic `SignatureParameter` names carried by
  signature facts instead of parsing formatted signature labels; broader inlay
  and display-part unification remains open.

Tests:

```bash
cargo test -p vela_language_service code_action
cargo test -p vela_language_service formatting
cargo test -p vela_language_service inlay
cargo test -p vela_lsp_server code_action
cargo test -p vela_lsp_server formatting
```

---

## 13. Phase 8: Scale, Cancellation, And Configuration

Purpose: make the new model viable for large workspaces and real editors.

- [ ] Audit request paths for avoidable per-keystroke full-workspace rebuilds.
- [ ] Add completion-specific scale tests for item, expression, type, member,
  and module-path contexts in synthetic many-file workspaces near the
  one-million-line target.
- [ ] Add or reuse incremental declaration, import, type, member, stdlib,
  schema, local-scope, and reference indexes so completion producers can query
  context-relevant candidate sets without scanning all files.
- [ ] Track module fingerprints so body-only edits preserve declaration and
  import indexes.
- [ ] Bound eager completion rendering. Defer expensive docs/detail formatting
  until after context filtering, prefix narrowing, or resolve.
- [ ] Keep open-document queries prioritized over disk-only modules.
- [ ] Use workspace generation IDs to discard stale results.
- [ ] Keep cancellation checks at query construction and expensive producer
  boundaries.
- [ ] Ensure configuration is loaded through `vela.toml`, launch flags, and
  LSP settings without putting protocol types in the service.
- [ ] Add stress fixtures or benchmarks for many files when a cheap
  representative test can be maintained.

Tests:

```bash
cargo test -p vela_language_service workspace
cargo test -p vela_lsp_server workspace
```

---

## 14. Phase 9: Delete Old Paths And Update Docs

Purpose: complete the breaking cleanup.

- [ ] Delete obsolete completion context variants, compatibility conversion
  helpers, stale tests, and dead feature-local cursor scanners.
- [ ] Update `docs/architecture/lsp.md` with the final query/result/projection
  boundary.
- [ ] Update `docs/lsp-implementation-plan.md` if phase status or capability
  ownership changes.
- [ ] Update `docs/progress.md` only when milestone status changes.
- [ ] Update `docs/decisions.md` for any new durable architecture decision.

Validation:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 15. Acceptance Criteria

- [ ] At an item boundary, typing `f` ranks the `fn` keyword/snippet before
  stdlib functions, and typing `st` ranks `struct` before unrelated globals.
- [ ] Expression-position completion still offers legal stdlib functions,
  locals, declarations, modules, builtins, and schema facts with sensible
  relevance.
- [ ] Type-position completion does not offer expression-only functions.
- [ ] Member completion is receiver-aware and does not leak top-level globals.
- [ ] LSP completion uses correct replacement ranges, filter text, snippets,
  label details, sorting, preselect, docs, and deprecation tags where
  supported.
- [ ] Hover, signature, definition, symbols, semantic tokens, references,
  rename, code actions, formatting, and inlay hints route through shared
  query, symbol, display, and edit models where relevant.
- [ ] Obsolete APIs are deleted rather than kept as compatibility shims.
- [ ] The architecture remains analysis-only and does not change runtime
  semantics.

---

## 16. First Execution Tasks

Use the repository task template when starting implementation.

```text
Task: Add completion context regression tests for item-boundary keywords.
Context: This belongs to the clean LSP architecture refactor. The visible bug
is that typing `f` at top level can rank stdlib facts above `fn`, and typing
`st` does not reliably surface `struct`.
Expected behavior:
  - top-level `f` completion includes and ranks `fn` first.
  - top-level `st` completion includes and ranks `struct` first.
  - item-boundary completion does not include expression-only stdlib functions.
Tests:
  - cargo test -p vela_language_service completion
Do not change:
  - Do not add compatibility shims for the old global completion model.
  - Do not change language syntax or runtime semantics.
Validation:
  cargo test -p vela_language_service completion
```

```text
Task: Introduce shared CursorContext classification for LSP queries.
Context: Completion, hover, signature help, definition, and rename currently
use feature-local cursor logic. The refactor needs one syntax-aware
classification boundary.
Expected behavior:
  - item, statement, expression, type, member, record, call, module, and
    lambda contexts are classified from parser/token spans.
  - incomplete source and parser recovery still produce useful contexts.
Tests:
  - cargo test -p vela_language_service cursor_context
  - cargo test -p vela_language_service completion
Do not change:
  - Do not expose LSP protocol types from vela_language_service.
  - Do not read or mutate live host state.
Validation:
  cargo test -p vela_language_service cursor_context completion
```

```text
Task: Replace completion items with a rich editor-neutral item model.
Context: LSP projection needs service-owned source ranges, text edits,
lookup/filter text, snippet intent, detail, docs, relevance, symbol identity,
and resolve payloads.
Expected behavior:
  - producers return editor-neutral items with replacement ranges and
    relevance metadata.
  - vela_lsp_server projects those items into LSP CompletionItem fields.
Tests:
  - cargo test -p vela_language_service completion
  - cargo test -p vela_lsp_server completion
Do not change:
  - Do not hard-code editor-specific ranking inside producers.
  - Do not preserve the old thin CompletionItem shape for compatibility.
Validation:
  cargo test -p vela_language_service completion
  cargo test -p vela_lsp_server completion
```
