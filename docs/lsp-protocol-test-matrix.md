# Native LSP Protocol Test Matrix

> **Document status:** planning matrix for future LSP test coverage.
> **Scope:** advertised LSP protocol behavior plus the Vela syntax and symbol
> surface each protocol must cover before it is considered complete.

This document turns the LSP coverage target into a protocol-first matrix. It is
not a claim that the current test suite already covers every row. Future LSP
tests should start from this matrix, choose one protocol row, then cover the
applicable Vela syntax dimensions through both `vela_language_service` tests
and `vela_lsp_server` JSON-RPC fixtures.

Execution progress for auditing and filling this matrix is tracked in
[lsp-implementation-plan.md](lsp-implementation-plan.md), section
"Protocol Test Matrix Coverage Tracker".

The matrix preserves the standing LSP constraints:

- `vela_language_service` owns editor-neutral analysis, query construction,
  symbol identity, display parts, edit plans, and semantic classification.
- `vela_lsp_server` owns JSON-RPC, lifecycle, capability advertisement,
  protocol projection, position/range conversion, cancellation, progress,
  workspace folders, file watching, and configuration transport.
- Editor packages stay thin. They provide launch/configuration and fallback
  syntax or scope metadata only.
- No LSP query may execute Vela code, run host code, read live host state,
  mutate `TypeRegistry`, change runtime semantics, or introduce new language
  semantics.

## Coverage Rules

Every advertised protocol capability needs these proofs:

1. Capability advertisement is pinned in lifecycle tests, including trigger
   characters, resolve support, dynamic registration settings, and provider
   options where applicable.
2. The protocol method has at least one JSON-RPC fixture that exercises request
   parsing, service call, LSP projection, and response shape.
3. The backing service query has focused tests for editor-neutral behavior and
   symbol identity before protocol projection.
4. Applicable Vela syntax dimensions from this document are covered, including
   negative and degraded cases.
5. Open overlays, stale generations, missing files, syntax errors, and missing
   or stale schema facts do not panic and do not publish stale results.
6. Unsupported protocols are not advertised. If an editor is likely to invoke
   the method anyway, add a negative fixture proving a stable method-not-found
   or explicit rejection response.

If a protocol consumes declaration, binding, module, type, or reference facts,
it must include at least one cross-file workspace fixture unless the method is
purely lexical or purely local by design. Cross-file coverage is mandatory for
navigation, hover, completion, signature help, references, rename, symbols,
semantic tokens, inlay hints, diagnostics, code actions, and call hierarchy.

For protocol rows that mutate source text, such as rename, code actions, and
formatting, coverage must also prove that edits are source-owned, versioned
where practical, conflict-checked, and deterministic.

## Vela Syntax And Symbol Dimensions

Use these dimensions as row references in the matrix.

| ID | Dimension | Required surface |
|---|---|---|
| S0 | Workspace/source state | Open overlay, disk snapshot, scratch file, configured roots, multi-file modules, missing/deleted/renamed files, schema absent/stale/invalid. |
| S1 | Top-level declarations | `pub`, `use`, `const`, `global`, `fn`, parameters, default parameters, `struct`, fields, `enum`, variants, `trait`, default/interface methods, inherent and trait `impl`. |
| S2 | Function and method bodies | Locals, explicit type hints, assignments, compound assignments, returns, nested blocks, `if`, `match`, loops, lambdas, closures, callbacks. |
| S3 | Type positions | Primitive hints, builtin containers, `Option`/`Result`, source types, schema host types, traits, missing type facts, dynamic `Any` and unknown. |
| S4 | Members and constructors | Source/schema fields, methods, trait methods, enum variants, tuple/record/unit variants, record constructors, shorthand fields, field labels, member writes. |
| S5 | Calls and arguments | Source functions, source methods, stdlib functions/methods, schema functions/methods, named arguments, defaulted parameters, active parameter tracking, dynamic or unresolved calls. |
| S6 | Patterns and control flow | `match` enum patterns, record-variant fields, binding patterns, guards where supported, `for` iteration, `break`, `continue`, control-flow keywords. |
| S7 | Literals and operators | Strings, bytes, numbers, booleans, null, arrays, maps, sets, records, unary/binary/logical operators, ranges, indexing, punctuation families. |
| S8 | Modules and imports | Qualified module paths, imported declarations, import aliases if supported, private/public visibility, source-backed schema spans, unresolved imports. |
| S9 | Error recovery | Parser recovery, incomplete member/call/type contexts, malformed declarations, unresolved names, diagnostics with candidates, partial stale facts. |
| S10 | Symbol ownership | Local, parameter, source declaration, source member, source variant, schema/host fact, stdlib fact, builtin fact, module, dynamic `Any`, unresolved. |
| S11 | Incrementality and cancellation | Repeated queries, body-only edits, declaration/import fingerprint changes, reverse dependency invalidation, request cancellation, generation rejection. |
| S12 | Trivia and formatting | Comments, shebang trivia, blank-line groups, indentation, top-level item spans, nested member spans, malformed-source fallback. |

## Workspace And Cross-File Required Coverage

`S0` and `S8` require more than "there is more than one file". For common LSP
features, the fixture must prove that workspace module facts are used instead
of same-file or text-only fallbacks.

Use a small two or three file workspace as the minimum fixture shape:

```text
scripts/rewards.vela
  pub const BASE_REWARD = 4
  pub global reward_scale = 2
  pub struct RewardConfig { item: String, count: i64 }
  pub enum RewardOutcome { Granted { item: String, count: i64 }, Skipped }
  pub fn reward_bonus(amount, scale = reward_scale) { ... }
  impl RewardConfig { pub fn total(self) { ... } }

scripts/main.vela
  use rewards::{BASE_REWARD, reward_scale, RewardConfig, RewardOutcome, reward_bonus}
  fn main() {
    let cfg: RewardConfig = RewardConfig { item: "gold", count: BASE_REWARD }
    let total = reward_bonus(cfg.total())
    return RewardOutcome::Granted { item: cfg.item, count: total }
  }
```

Where import-list syntax differs, keep the same semantic shape with the
project's supported import syntax.

| ID | Cross-file case | Required proof |
|---|---|---|
| W0 | Workspace setup | Configured root, disk snapshots, open overlay in the importing file, open overlay in the defining file, scratch fallback, and deleted/renamed imported file behavior. |
| W1 | Cross-file navigation | `definition`, `declaration`, and `typeDefinition` from imported function calls, const/global uses, type hints, constructors, enum variants, member access through imported source types, and schema facts with source spans. |
| W2 | Cross-file hover/completion/signature | Imported globals, functions, types, variants, fields, methods, and defaulted parameters surface the defining-file facts and docs. |
| W3 | Cross-file references/highlights | References include defining file plus use files for functions, const/global symbols, source types, field/member uses, enum variants, and imported aliases where supported. Same-document highlight remains local while references spans the workspace. |
| W4 | Cross-file rename | Rename of private source-owned functions, const/global symbols, source types, fields, methods, and variants edits defining and use files; public or schema-only renames report the right rejection or risk metadata. |
| W5 | Cross-file call hierarchy | Incoming and outgoing calls cross module boundaries for imported source functions and typed source methods. Dynamic or unresolved calls are not guessed. |
| W6 | Cross-file diagnostics and actions | Missing import, broken import after delete/rename, unused import, typo candidate from another module, and safe missing-import code action all use the workspace graph. |
| W7 | Cross-file semantic tokens and inlay hints | Imported function calls, const/global uses, type hints, constructors, enum variants, and member uses keep source/schema/builtin provenance and type facts across files. |
| W8 | Cross-file incrementality | Editing a function body in the defining file does not rebuild unrelated modules, but declaration/import fingerprint changes invalidate dependents and refresh open-file results. |

Protocol fixtures should name the cross-file case explicitly, for example
`lsp_definition_cross_file_function_call`,
`lsp_definition_cross_file_global_use`,
`lsp_references_cross_file_imported_function`, or
`lsp_rename_cross_file_struct_field`.

## Protocol Matrix

`Service proof` means an editor-neutral test in `vela_language_service` or the
owning lower crate. `Protocol proof` means a JSON-RPC fixture or lifecycle test
in `vela_lsp_server`.

| Protocol method or behavior | Capability or provider | Syntax dimensions | Required positive coverage | Required negative/degraded coverage |
|---|---|---:|---|---|
| `initialize` | Server lifecycle and capability object | S0, S11 | Exact advertised capability keys, provider options, trigger characters, semantic-token legend, server info, workspace folder support. | Unsupported providers are absent or null; client capability variations do not change service semantics. |
| `initialized` | Lifecycle notification | S0 | Notification has no response and may trigger watcher/config setup. | Repeated or minimal initialization stays stable. |
| `shutdown`, `exit` | Lifecycle termination | S0, S11 | Shutdown response, clean exit behavior, no pending background publication after shutdown. | Requests after shutdown are rejected consistently. |
| `$/cancelRequest` | Cancellation | S11 | Stale queued or expensive requests are discarded by generation/cancellation tokens. | Unknown or already-completed request IDs do not panic. |
| `textDocument/didOpen` | Text document sync | S0, S1, S9, S11 | Open overlay wins over disk and publishes diagnostics for syntax/HIR/analysis/schema facts. | Missing workspace config, scratch file mode, malformed source, missing schema. |
| `textDocument/didChange` | Incremental text sync | S0, S1, S2, S9, S11 | Full and incremental edits update overlays, versions, line indexes, diagnostics, completions, hovers, semantic tokens. | Out-of-order or stale versions do not publish stale facts; malformed incremental edits reject cleanly. |
| `textDocument/didClose` | `textDocumentSync.openClose` | S0, S11 | Closing removes overlay or restores disk snapshot, clears or refreshes diagnostics, and restores disk-backed query behavior for features such as definition. | If unsupported, stop advertising `openClose`; otherwise add a protocol fixture. |
| `textDocument/didSave` | Save is currently false | S0 | No provider dependency on save events. | Save notifications should not be required for correctness while `save` is false. |
| `textDocument/publishDiagnostics` | Server notification | S0, S1, S3, S8, S9, S11 | Parser, HIR, analysis, schema, config, missing import, unused import, and structured repair metadata project to LSP diagnostics. | One-file syntax errors do not block unrelated modules; stale schema degrades to `Any`; deleted files clear diagnostics. |
| `textDocument/completion` | `completionProvider` | S1-S11 | Item, statement, expression, type, member, record field, map key, module path, call argument, lambda parameter, schema, stdlib, builtin, and cross-file imported declaration completions. | Dynamic receivers suppress member guesses; unknown constructors suppress record fields; stale/cancelled queries discard; malformed cursor contexts recover. |
| `completionItem/resolve` | Completion resolve | S3, S4, S5, S10 | Lazy docs/details for schema, stdlib, and source-backed items where supported; items without lazy payloads pass through unchanged. | Unknown resolve payloads return an invalid-request error without panics; initial completion list stays lightweight. |
| `textDocument/signatureHelp` | `signatureHelpProvider` | S3, S5, S8-S10 | Source functions, source methods, schema functions/methods, trait methods, stdlib functions/methods, active parameter, named/default args, and imported function/method calls. | Unknown calls and dynamic `Any` receiver calls return null; incomplete calls resolve only when target facts exist; stale schema. |
| `textDocument/hover` | `hoverProvider` | S1-S10 | Locals, params, declarations, fields, methods, variants, modules, type hints, schema facts, stdlib facts, docs, effects, permissions, and imported source facts. | Unresolved names and dynamic `Any` member targets return null; missing-schema type hints degrade to `Any`; schema facts without source spans and parser recovery remain explicit audits. |
| `textDocument/definition` | `definitionProvider` | S1, S3-S5, S8-S10 | Local bindings, source declarations, cross-file imported declarations, imported const/global uses, imported function calls, source fields/methods/variants, schema facts with source spans. | Schema facts without source spans return no false enclosing declaration; dynamic/unresolved targets return no location. |
| `textDocument/declaration` | `declarationProvider` | S1, S3-S5, S8-S10 | Source declaration targets, including cross-file imported declarations, where declaration and definition are the same or explicitly distinct. | Must not silently alias unrelated definition behavior for members or type facts; dynamic/unresolved targets return no location. |
| `textDocument/typeDefinition` | `typeDefinitionProvider` | S1, S3, S4, S8, S10 | Variables, parameters, and member expressions with source/schema type facts jump to source/schema type declarations when source-backed, including imported source types. | Field values such as `cell.value` must not jump to the enclosing function by fallback; builtin/dynamic/unknown types use an explicit null policy. |
| `textDocument/implementation` | Not advertised | S1, S3, S4, S10 | No positive coverage until trait/impl implementation semantics are specified. | Capability remains absent/null and direct requests return method-not-found or an explicit unsupported response. |
| `textDocument/references` | `referencesProvider` | S1-S6, S8-S11 | Locals, parameters, source declarations, imports, functions, const/global uses, fields, methods, variants, schema-backed source spans, read/write classification, and cross-file uses. | Shadowed locals stay separate; schema-only, builtin, dynamic, unresolved, and missing-schema targets are classified or rejected consistently. |
| `textDocument/documentHighlight` | `documentHighlightProvider` | S1-S6, S8-S10 | Same-document highlights for locals, params, functions, fields, methods, variants, schema member calls, read/write/text kind. | Parser recovery, dynamic members, unresolved names, shadowing. |
| `textDocument/prepareRename` | `renameProvider.prepareProvider` | S1-S6, S8-S10 | Editable ranges for source-owned locals, private declarations, source members, variants, and source-backed schema spans where allowed. | Reject keywords, literals, schema-only host facts, builtin facts, dynamic `Any`, unresolved names, public ABI risk without metadata, collisions. |
| `textDocument/rename` | `renameProvider` | S1-S6, S8-S11 | Versioned workspace edits for all references of an editable source-owned symbol across defining and importing files, conflict checks, hot-reload risk metadata. | Overlapping edits, stale versions, visibility conflicts, name collisions, schema-only/builtin/dynamic/unresolved targets. |
| `textDocument/codeAction` | `codeActionProvider.quickfix` | S1, S3, S4, S6, S8-S10 | Diagnostic-backed typo fixes, missing imports, unused import removal, missing match arms, missing record fields. | Ambiguous imports, dynamic receiver typo fixes, speculative semantic rewrites, stale ranges. |
| `textDocument/prepareCallHierarchy` | `callHierarchyProvider` | S1, S4, S5, S8-S10 | Source functions, source methods, trait defaults/interface methods, schema-backed methods with source spans where available. | Non-callable targets, dynamic calls, unresolved targets, schema-only targets without source spans. |
| `callHierarchy/incomingCalls` | `callHierarchyProvider` | S1, S4, S5, S8-S11 | Incoming source and typed receiver method calls across modules, source/schema method call ranges. | Dynamic/unresolved calls are not guessed; stale indexes are rejected. |
| `callHierarchy/outgoingCalls` | `callHierarchyProvider` | S1, S4, S5, S8-S11 | Outgoing source function/method/schema method calls from selected callable body. | Parser recovery, dynamic calls, incomplete bodies. |
| `textDocument/documentSymbol` | `documentSymbolProvider` | S1, S4, S8, S9 | Top-level declarations and nested type/impl/trait members with names, kinds, details, ranges, selection ranges. | Malformed declarations recover where possible without bogus symbol ranges. |
| `workspace/symbol` | `workspaceSymbolProvider` | S0, S1, S3, S4, S8, S10, S11 | Module-qualified source declarations and schema facts, query filtering, workspace roots. | Deleted files, stale indexes, missing schema, duplicate short names. |
| `textDocument/foldingRange` | `foldingRangeProvider` | S1, S2, S4, S6, S7, S8, S9, S12 | Imports, declarations, functions, blocks, match arms, multiline literals, nested members. | Malformed braces or incomplete items degrade without panics. |
| `textDocument/selectionRange` | `selectionRangeProvider` | S1-S9, S12 | Syntax ancestry ranges for declarations, expressions, members, calls, types, patterns, literals. | Parser recovery and incomplete nodes still return stable parent chains where possible. |
| `textDocument/semanticTokens/full` | `semanticTokensProvider.full` | S1-S10 | Lexical and resolved tokens for declarations, uses, literals, operators, punctuation, provenance modifiers, unresolved references. | Parser recovery, missing schema fallback, client token/modifier fallback projection. |
| `textDocument/semanticTokens/full/delta` | Semantic-token delta | S1-S11 | Stable result IDs and deterministic full-replacement or delta behavior after edits. | Stale result IDs, edits that change line indexes, cancelled/stale generations. |
| `textDocument/semanticTokens/range` | Semantic-token range | S1-S11 | Requested range filters full service tokens and encodes valid relative ranges; empty requested ranges return an empty token stream. | Malformed sources and client capability fallback. |
| `textDocument/formatting` | `documentFormattingProvider` | S1, S2, S4, S6-S9, S12 | Full-document deterministic formatting, comment/blank-line preservation, final newline, idempotence. | Malformed source, unresolved HIR, syntax-only fallback. |
| `textDocument/rangeFormatting` | `documentRangeFormattingProvider` | S1, S2, S4, S6-S9, S12 | Selected top-level item, nested members, field groups, whitespace-padded selections, trailing whitespace cleanup. | Partial unsupported ranges return conservative edits only. |
| `textDocument/onTypeFormatting` | `documentOnTypeFormattingProvider` | S1, S2, S4, S6-S9, S12 | `}` and newline triggers for completed items, methods, enum record variants, current construct cleanup. | Unsupported triggers and incomplete constructs fall back safely. |
| `textDocument/inlayHint` | `inlayHintProvider` | S2-S6, S9-S11 | Parameter names, local type facts, lambda parameter facts, host-path type facts, tuple-variant payload names, requested range. | Explicit annotations, unknown/Any facts, dynamic boundaries, missing schema suppress hints. |
| `workspace/didChangeWatchedFiles` | Watched files | S0, S8, S9, S11 | Create/change/delete/rename `.vela`, `vela.toml`, and schema artifact events update project/schema state and diagnostics. | Coalesced duplicate events, missing files, invalid config/schema, deleted schema. |
| `workspace/didChangeConfiguration` | Configuration | S0, S9, S11 | Editor settings map to service-owned workspace config, roots, schema path, reload behavior. | Invalid settings degrade to diagnostics; no protocol types leak into service APIs. |
| `workspace/didChangeWorkspaceFolders` | Workspace folders | S0, S8, S11 | Added/removed roots reindex modules and republish open diagnostics. | Removed roots clear stale disk facts; open overlays remain authoritative where applicable. |
| `workspace/configuration` | Server request to client, when used | S0, S11 | Settings request/response is projected only in `vela_lsp_server` and maps to `WorkspaceConfig`. | Missing or malformed client response falls back to launch/config defaults. |
| Native stdio transport | Launch/transport smoke | S0 | `--stdio`, default stdio, `--root`, `--schema`, `--version`, server info. | Broken args, missing schema path, package launchers remain behavior-free. |

## Fixture Design

Prefer small, targeted fixtures over one huge assertion, but keep at least one
canonical "broad syntax" source file for cross-protocol smoke coverage.

Suggested naming:

```text
lsp_<method>_<syntax>_<expected>
service_<feature>_<syntax>_<expected>
```

Examples:

```text
lsp_definition_source_member_field_use
lsp_type_definition_local_source_type
lsp_completion_record_constructor_fields
lsp_references_shadowed_local_bindings
service_hover_schema_method_effects
service_rename_rejects_dynamic_member
```

Each protocol fixture should declare:

- Initial workspace roots and optional schema artifact.
- Open documents and disk snapshots.
- Request position/range in source text.
- Expected response shape and important locations or edits.
- Expected diagnostics or absence of diagnostics when relevant.
- Any client capabilities needed to exercise fallback projection.

## High-Priority Coverage Gaps To Audit First

These are the first places to compare current tests against the matrix:

1. Navigation semantics must stay separate per protocol. Current focused
   fixtures cover `typeDefinition` type-fact targets and null fallback for
   local source values, dynamic local values, source/schema member values,
   schema types without source spans, imported source type aliases used by
   locals and source fields, and imported source function calls whose return
   type is source-owned; the remaining audit is broader cross-file
   type-definition coverage across W1.
2. `textDocument/implementation` remains intentionally unadvertised until
   trait/impl implementation semantics are specified; lifecycle coverage now
   pins both the absent capability and direct request rejection.
3. `textDocumentSync.openClose` is advertised and `textDocument/didClose`
   behavior is covered for scratch diagnostics, disk-backed diagnostic
   restoration, and disk-backed definition queries after closing an overlay;
   remaining audits should focus on other cross-feature close/open
   interactions rather than basic capability support.
4. Capability-to-handler consistency should be audited for every advertised
   provider. A capability is incomplete if the lifecycle test advertises it but
   there is no method fixture and no service proof.
5. Dynamic boundaries need explicit negative tests. Current focused fixtures
   pin `typeDefinition` null results for dynamic local values,
   `signatureHelp` null results for unresolved calls and dynamic receiver
   calls, and `hover` null results for unresolved names plus dynamic receiver
   members, plus `prepareCallHierarchy` empty results for unresolved calls,
   dynamic receiver calls, and non-callable symbols; broader `Any`, missing
   schema, stale schema, unresolved name, and parser recovery cases should
   degrade by returning null, empty results, diagnostics, or suppressed hints,
   not guessed semantic facts.
6. Multi-file and overlay behavior should be present in each cross-file
   feature family: completion, hover, navigation, references, rename, symbols,
   semantic tokens, diagnostics, and call hierarchy.

## Completion Criteria For This Matrix

A protocol row is complete when:

- The advertised capability is pinned.
- A protocol fixture covers the JSON-RPC method.
- A service test covers the editor-neutral behavior.
- Applicable syntax dimensions have positive and negative coverage.
- Dynamic, missing-schema, parser-recovery, and stale-generation behavior is
  explicit.
- The relevant focused command passes.

The LSP protocol matrix is complete only when every advertised row above meets
that bar, every unsupported row is negatively pinned, and the full validation
set for LSP docs or implementation changes passes:

```bash
cargo test -p vela_language_service
cargo test -p vela_lsp_server
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
