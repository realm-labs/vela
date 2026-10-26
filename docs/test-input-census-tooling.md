# SF00 Test-Input Census: Language Service, LSP And Matrix Readers

## Scope And Review Status

This is a review/documentation handoff for [the test-input organization plan](test-organization-plan.md),
not an SF09/SF10 implementation or acceptance report. The current request authorizes
staged implementation overall, but this assignment writes **only this file**, with
no code changes or commits. The parent owns commits and shared plan/status updates.
The existing changes to shared documentation were inspected and left untouched.

Owned review scope:

- `crates/vela_language_service/`: feature tests, tests inside production modules,
  support functions, lifecycle versions and `benches/incremental_latency.rs`.
- `crates/vela_lsp_server/`: library/protocol/global-state tests and the real stdio
  integration target, including support and filesystem/schema/watcher scenarios.
- `tests/lsp_matrix/`: fixture JSON, Rust support and evidence/discovery linkage.
- `scripts/lsp-matrix/` and `editors/vscode/test/`: fixture readers, source-bearing
  setup, oracle transformations, materialization and provenance consumers.

Method: tracked-file inventory, source-argument/literal/helper searches, raw and
cooked multiline-string searches, `format!`/`concat!`/replacement/IO inspection,
recursive decoded JSON field inventory, duplicate-string comparison, then targeted
bodies and their readers/assertions. Marker-bearing source was also examined after
removing markers for **triage only**: `fn [[name]]...` defeats ordinary declaration
regexes. Neither a lexical match nor a file-size count establishes a test or an
extraction decision. JSON diagnostics, signatures, schemas and expected labels also
produce false source matches. No executable inventory or framework was added.

The inventory covered 328 tracked Rust files in language service (including its
bench), 268 in LSP (including integration support), 41 shared Rust support files,
218 fixture JSON files, 144 matrix JavaScript files, and 79 editor-test files
(including 68 JavaScript, two existing Vela sources and one manifest). These are
**inventory reach**, not claims that every line was manually read or every test
was executed. Families below account for local source, shared-source routes,
generation and non-source matches; specific bodies substantiate the choices.
Future extraction must still prove evaluated-byte and assertion equivalence.

Disposition vocabulary follows the plan: `file-backed`, `shared-input`,
`retain-inline`, `unit-data`, `generated`, `specialized-corpus`,
`distribution-copy`, `not-test-input`. A proposed `file-backed`/`shared-input`
entry below is work for the later owner, not a claim that files already exist.
There is no blanket tooling/JSON/lifecycle exemption and no zero-inline target.

## Targets, Proof Boundaries And Helper Ownership

| Owner / existing target | Inputs and proof that must remain at this boundary |
|---|---|
| `vela_language_service --lib`, wired by `crates/vela_language_service/src/lib.rs` and feature modules | Static analysis, source/schema/package ownership, byte-column service queries, diagnostics, completion edits, identities, incremental/fresh equivalence and snapshot isolation. No live script execution. |
| `vela_lsp_server --lib`, wired by `crates/vela_lsp_server/src/lib.rs` and `src/tests.rs` | Actual production message dispatch, typed LSP responses, UTF-16 projection, client capabilities, notifications, request IDs, cancellation/publication, watcher and schema/config changes. Service tests cannot replace this proof. |
| `vela_lsp_server --test stdio_transport`, `crates/vela_lsp_server/tests/stdio_transport.rs` | Real binary startup/exit, frame IO, fragmented UTF-8, diagnostic repair, stale update rejection, malformed headers/JSON/EOF. Not equivalent to an in-memory `TestServer`. |
| `vela_language_service --bench incremental_latency` | Harness-free benchmark declared in `crates/vela_language_service/Cargo.toml`; generated workspace, edited module, schema-free support module, cold assembly and per-edit/query/snapshot timings. Build is not latency acceptance. |
| Node matrix tests and installed editor runners | Independent fixture/oracle validity; installed provider/command observations; separately scoped native Input/Render. Node self-tests do not execute installed-editor or native-input contracts. |

Neither crate declares a feature-specific fixture target. Preserve existing `--lib`
module paths, ordinary tests and named parameterized cases; do not move everything
into a new integration target. The LSP binary remains exercised by the existing
stdio integration target. `scripts/lsp-matrix/rust-suites.js` explicitly lists both
libraries and stdio; stdio uses `--target-dir target/lsp-matrix-stdio` to avoid
replacing the dev-profile binary fingerprinted by installed-editor evidence.

### Existing Helpers And Freshness Contracts

- `crates/vela_language_service/src/completion/member_tests.rs`:
  `completions_for` / `completions_for_with_schema` build fresh sources, workspace
  snapshot and databases, then locate the cursor by `find(needle)` and `LineIndex`.
  The schema/non-schema route and negative global-member assertions stay separate.
  `src/completion/tests.rs::project_for_completion` likewise assembles caller-owned
  sources, not a shared mutable database.
- `crates/vela_language_service/src/definition/source_return_tests.rs`:
  `assert_navigation` shares source setup between definition and declaration but
  retains the selected query, qualified symbol and exact target coordinates.
  `databases_for` helpers in hover/references/inlay families construct fresh state;
  same bytes do not make those feature observations redundant.
- `crates/vela_language_service/src/incremental/tests.rs`: `file`, `project`,
  `project_with_roots` construct snapshots/graphs; the test deliberately retains a
  database across changes and checks parse counts, invalidation and rebuild metrics.
  Do not replace this with independently compiled final-state rows.
- `crates/vela_language_service/src/hover/fixture_layout.rs::Layout` uses a logical
  root for non-package tests, but materializes and loads a real package graph for
  package cases; `Drop` checks and removes the owned root. `provider_tooling_tests.rs`
  has `TraitToolingFixture`, a separate two-package IO route with explicit cleanup.
  Its name/PID temporary layout is not the atomic allocator used by newer LSP tests;
  do not introduce concurrent shared setup or claim every existing helper has the
  same isolation contract.
- `crates/vela_lsp_server/src/tests/support.rs::TestServer` owns fresh `GlobalState`
  and channels, calls production dispatch, rejects request errors in typed success
  helpers and exposes `request_response` for deliberate error assertions. Queue,
  receive and publish are separate so lifecycle tests can interleave real changes.
  Task waits are five seconds. `unique_temp_root` atomically creates a name using
  PID/time/sequence and has a frozen-clock parallel uniqueness test.
- `crates/vela_lsp_server/src/tests/document_sync_matrix_support.rs`,
  `src/tests/watched_files_matrix/support.rs`, and
  `src/tests/workspace_configuration_matrix/support.rs` own fresh layouts, capability
  profiles, message sending, exact publication/progress sets and live/fresh comparisons.
  Changing storage must not bypass physical writes/watch events or root precedence.
- `crates/vela_language_service/src/semantic_tokens/test_support/scenarios.rs` and
  `crates/vela_lsp_server/src/tests/semantic_tokens/support/scenarios.rs` consume
  full positive/negative/repair phases. Preserve complete streams, range queries,
  actual delta application, unchanged IDs and fresh-database comparison.
  `crates/vela_lsp_server/src/tests/semantic_tokens/lifecycle_support.rs::Driver`
  also checks effective sources, open overlays, physical disk, versions and watcher
  notifications. Saving is followed by a watcher event because save sync is not advertised.
- `tests/lsp_matrix/support/mod.rs`: independently authored marker/edit oracles,
  `Spec`, `Action`, `FixtureWorkspace`, and `load`. Included under `cfg(test)` as
  `matrix_fixture` by both libraries and as `stdio_fixture` by stdio. Each load/model
  is fresh; no global mutable cache. Materialization requires a new root.
- `tests/lsp_matrix/support/signature_imports.rs::Layout` materializes fresh package
  fixtures, applies disk write/delete separately from overlays, verifies disk bytes,
  and cleans its owned root. Source/source-overlay/schema helpers do not execute Vela.
- `crates/vela_lsp_server/tests/support/stdio.rs`: existing bounded child ownership,
  ten-second process deadline, finite pipe waits, 1 MiB limit per output stream and
  kill/reap-on-drop. Keep original framed JSON fields and stderr/status assertions.
  Do not replace this helper with unbounded process collection during extraction.

## SF09: Whole Local-Family Inventory

For compact path lists below, **LS** means `crates/vela_language_service/src/` and
**LSP** means `crates/vela_lsp_server/src/`. Braces and `*` denote reviewed filename
families, not new runtime discovery or proposed test registration. Candidate
locations are spelled out further below. Submodules remain owned by their current
library target unless explicitly identified as stdio/bench.

| Family and locations | Source route, disposition and retained observations |
|---|---|
| Completion: LS `completion/` test modules (`tests`, `analysis`, `authoring_surface`, `call_expression`, `call_parameter`, `callable_import`, `enum_alias`, `expression_ownership`, `import_alias`, `incremental`, `member_index`, `member`, `member_matrix`, `named_argument_matrix`, `record_field`, `struct_field`, `tuple_destructuring`, `type_label`, `type_matrix`, `type_ownership`, `service_path`); tests inside `item.rs`, `lambda_parameter.rs`, `record_field.rs`, `source_module.rs`, `statement.rs`, `struct_field.rs`, `type_hint.rs`. LSP `tests/completion*.rs`, `completion_resolve/`, `completion_type/`, `completion_type_ownership/`, `call_argument_context.rs`, `call_parameter_mapping.rs`, `callable_imports.rs` | Local partial documents plus shared JSON loaded into snapshots or opened through the protocol. `retain-inline` for concise context/negative probes, snippet fills and short version edits; `shared-input` for repeated returned-receiver documents; substantial member/index arrangements merit `file-backed`. Matrix source storage belongs to SF10. Preserve label/details/lookup/filter/sort/resolve identity, full text edits, insertion parsing and negative results, not just label presence. |
| Cursor/query/expression facts: LS `cursor_context.rs`, `cursor_context/tests.rs`, `cursor_context/tuple_field.rs`, `query_context.rs`, `query_context/call_argument_tests.rs`, `query_context/hir_cursor.rs`, `expression_facts/tests.rs` | Inline caret/tuple/partial-expression rows and matrix call arguments. `retain-inline` / `unit-data` for tiny syntax slots, literal spellings and cursor markers. The tuple-destructuring document in `expression_facts/tests.rs` is a real multi-scope extraction candidate, not a lexical false positive. Preserve exact occurrence selection and typed facts. |
| Navigation/hover: LS `definition/` tests including dynamic, types, tuples, source/schema returns and lifecycle; tests in `definition/source_callables.rs`; `hover/` tests including cross-file, coordinates, all matrix partitions and lifecycle/recovery states. LSP `tests/definition.rs`, `tests/definition/` (including `type_definition/`), `tests/hover.rs`, `tests/hover/` | `shared-input` for repeated function/method/default-trait receiver programs; `retain-inline` for short dynamic/null/type-binding and URI-coordinate cases. Large JSON matrices are SF10 candidates, not exempt because tests only load them. Preserve definition versus declaration versus type-definition, package-qualified identities, null ownership, schema-source locations, full markdown and original URIs. |
| References/highlights/rename: LS `references/` tests and `support.rs`, `rename/` tests including `tests/schema_{collision,record}_tests.rs`; LSP `tests/references.rs`, `tests/references/`, `tests/rename.rs`, `tests/rename/`, `rename_collisions.rs`, `rename_schema.rs`, `rename_source_return.rs` | Returned-receiver and larger enum/field programs are `shared-input` candidates. Short collision/name-validity rows remain `retain-inline`; expected symbol names/edit tokens are `unit-data`. Shared JSON collision, recovery and lifecycle corpora are reviewed in SF10. Keep declaration inclusion, reference kinds, highlights, complete edit sets, ambiguity/capture rejection, stale request publication and disk/open distinctions. |
| Call hierarchy: LS `call_hierarchy/tests.rs`, `call_hierarchy/tests/{returned_receivers,schema_method_return_receivers}.rs`; LSP `tests/call_hierarchy.rs`, `tests/call_hierarchy/{cross_file,returned_receivers,source_method_return_receivers,schema_method_return_receivers,tasks,values}.rs` | Repeated multi-declaration returned-receiver sources are `shared-input`; short dynamic/unknown/no-call probes remain `retain-inline`. Preserve incoming/outgoing call edges, exact `fromRanges`, item data and stale item/request rejection. Same source as references does not authorize merging assertions or targets. |
| Signature/inlay: LS `signature.rs`, `signature/call_matrix_tests/`, `signature/import_tests/`, `inlay/{tests,suppression_tests}.rs`; LSP `tests/signature.rs`, `tests/signature/`, `tests/inlay.rs`, `tests/inlay/`, `inlay_suppression.rs` | `shared-input` for repeated Any-suppression/lambda/receiver programs, `file-backed` for substantial nonidentical receiver arrangements. Short argument-index, dynamic receiver, null and suppression rows remain inline. Schema JSON factories are metadata, not Vela. Keep active parameter mapping, exact hint positions/kinds/labels and missing-hint negatives. |
| Diagnostics/actions: LS `diagnostics/` tests and `code_action.rs`, `code_action/` tests; LSP `tests/diagnostic_*.rs`, `tests/code_action.rs`, `tests/code_action/` including dependency/schema lifecycle | Small local errors and repairs remain `retain-inline`; typed diagnostic/edit expectations remain in Rust. Larger shared multi-error/repair sources belong to SF10, including applied documents hidden in `oracle`. Retain codes/severity/labels/candidates, stale-diagnostic clearing, edit application, reparsing and dependency/schema transition effects. |
| Semantic tokens: LS `semantic_tokens/` tests, `test_support.rs`, `test_support/scenarios.rs`; LSP `tests/semantic_tokens.rs`, `tests/semantic_tokens/`, `semantic_tokens_{degradation,schema,schema_trait,source}.rs`, `semantic_tokens_schema/` | `shared-input` for repeated local member/enum/source-return programs; `retain-inline` for focused range/coordinate/delta/degradation cases. Dense positive/negative/repaired JSON streams are strong SF10 candidates. Keep token kinds/modifiers, source slices, scalar boundaries, legends, capabilities, result IDs, cache eviction, cancellation and full/range/delta distinctions. |
| Symbols/folding/selection: LS `symbols.rs`, `symbol_target.rs`, `folding.rs`, `selection.rs`, `document_symbol_*_tests.rs`, `workspace_symbol_*_tests.rs`, `folding_*_tests.rs`, `selection_*_tests.rs`; LSP `tests/symbols.rs`, `tests/folding.rs`, `tests/selection.rs`, `tests/document_symbol_*.rs`, `tests/workspace_symbol_*.rs`, `tests/folding_*.rs`, `tests/selection_*.rs` | Local small tree/range checks remain inline; the repeated item/block folding program is `shared-input`. Full symbol documents and marked selection composites are SF10 candidates; tiny recovery/EOL/ancestry cases stay self-contained. Preserve hierarchy, ownership, exact whole sets/ranges, folding capability projection, selection parent chains, all requested positions and worker lifecycle. |
| Formatting: LS `formatting/tests.rs`; LSP `tests/formatting.rs` | `shared-input` source/expected pair for the repeated container-type-hint example. `retain-inline` for compact range/on-type examples, incomplete syntax, EOL/trailing-space and `concat!` probes where spelling is the assertion. Retain edit ranges/counts and untouched-neighbor checks, not only formatted text. SF02 owns any new executable formatter equivalence selection. |
| Incremental/project/workspace/sync: LS `incremental/tests.rs`, tests in `incremental/{parse_summary,schema_sources}.rs`, `project.rs`, `workspace.rs`, `text.rs`, `document_{open,change,close}_tests.rs`, `document_sync_test_support.rs`, `shared_fixture_tests.rs`; LSP `global_state/tests/{edits,queries,state,tasks}.rs`, `tests/{incremental,document_sync,document_sync_dependents,close_overlay,document_open_matrix,document_change_matrix,document_close_matrix,shared_fixture}.rs` | Tiny adjacent source versions are `retain-inline`; scale inputs are `generated`; shared lifecycle JSON belongs to SF10. Example: `snapshot_captures_read_only_global_state` changes `fn main() { 1 }` to `{ 2 }` while checking the old snapshot's many captured fields. Extraction would hide that local contrast. Preserve source versions/IDs, Arc/snapshot ownership, rebuild counts, stale edits and diagnostic publication. |
| Workspace IO/config/schema/watching: LSP `tests/{file_watching,file_watching_coalescing,workspace_folders,schema_reload,session_init,lifecycle,cancellation_matrix}.rs`, `tests/watched_files_matrix/`, `tests/workspace_configuration_matrix/`, `transport/config_tests.rs`; LS `provider_tooling_tests.rs`, `schema/tests.rs`, `schema/signature/tests.rs` | Small Vela sentinels remain inline; metadata/config tables remain `unit-data`. Real multi-package tree in `TraitToolingFixture` is a file-tree candidate even though individual sources are short. Schema replacement/delete/invalid/repair and roots/package reload are retained sequences, not one final result. Repeated schema JSON is not automatically a `.vela` candidate. |
| Removed boundary and architecture checks: LS `removed_boundary_tests.rs`, `removed_execution_tests.rs`, completion service-path tests; LSP `architecture_tests.rs`, `tests/removed_execution.rs`, `tests/service_paths.rs` and feature rejection delegates | Concise removal sources remain inline or in existing matrix JSON; schema mutation loops are `unit-data`/`generated`. Rust forbidden-import/dispatcher needles are `not-test-input`. Keep current rejection rather than restoring features suggested by historical fixture names. `assert_removed_resource_surface` proves old schema facts clear across several query APIs; do not split away the transition. |
| Protocol/text conversion microtests: LSP `line_index.rs`, `lsp/from_proto.rs`, `lsp/to_proto/tests.rs`, `protocol.rs`, `rpc.rs`, `semantic_tokens.rs`, `task.rs`, `transport.rs`, lifecycle/config/path modules | `unit-data` for wire fields, IDs, UTF-16/byte boundaries and payload corruption; `retain-inline` for tiny documents needed to project a location. Production protocol strings and capability names are `not-test-input`. These are not reasons to extract arbitrary JSON or create source fixtures. |
| Stdio and benchmark | Stdio's `document-change` JSON and nested repair text are shared reader dependencies; inline `fn poisoned(` is intentionally a tiny stale update, and malformed frame byte arrays are `unit-data`. Bench `module_text` / `module_document`, incremental `scaled_file` and completion `completion_scale_file` are genuine `generated` inputs. See generator details below. |

### Concrete SF09 Extraction/Sharing Candidates

These are meaningful starting groups, not a quota. The proposed origin for
**cross-library plain-source** sharing is `tests/lsp_matrix/fixtures/sources/local/`:
inside the already hashed tooling fixture subtree, without changing the matrix
JSON schema. Use direct test-only `include_str!` at consumers; do not add a loader
or catalog for these local Rust strings. Single-owner sources can instead live in
that crate's `tests/fixtures/`. Proposed directories/files do not yet exist.

| Candidate and exact current locations | Why / proposed authored origin and constraints |
|---|---|
| Four navigation/hover returned-receiver documents: `crates/vela_language_service/src/definition/source_return_tests.rs` (source function → method, source method → method, source function → default trait, source method → default trait); `crates/vela_language_service/src/hover/tests.rs`; `crates/vela_lsp_server/src/tests/definition/source_return_receivers.rs`, `src/tests/definition.rs`; `crates/vela_lsp_server/src/tests/hover/source_return_receivers.rs`, `src/tests/hover.rs` | Four exact raw-string groups (239, 320, 295, 376 ASCII bytes) recur across feature and transport observations. One file per program under proposed `sources/local/returned-receivers/`, seeded from the LS definition helper. Preserve no-final-newline spelling, line constants, `game::main::Player.grant` / `Inventory.grant` / `Rewardable.preview`, and all eight definition/declaration entry tests. |
| References/highlights/call edges: `crates/vela_language_service/src/references/source_return_tests.rs`, `src/call_hierarchy/tests/returned_receivers.rs`; `crates/vela_lsp_server/src/tests/references/returned_receivers.rs`, `src/tests/references/source_method_return_receivers.rs`, `src/tests/call_hierarchy/source_method_return_receivers.rs` | Substantial cooked multiline programs include repeated two-call function and method-return variants. `open_source_method_return_method_fixture` and `open_source_method_return_trait_fixture` embed the same arrangements used by references. Choose LS references as origin under a **separate** `sources/local/reference-receivers/`; verify evaluated cooked bytes before sharing. Do not substitute the similar navigation programs: visibility, method returns, blank lines and call counts differ. |
| Completion source-return receivers: `crates/vela_language_service/src/completion/member_tests.rs` and `crates/vela_lsp_server/src/tests/completion_member.rs` | Exact 246/322-byte raw strings, including leading newline and incomplete trailing member expression, are shared-input candidates under `sources/local/completion/`. Keep the shorter LS `member_completion_uses_source_function_return_facts` independent: it lacks the global decoy and has different visibility/assertions. The 418-byte impl/trait arrangement and `src/completion/member_index_tests.rs` are owner-local file-backed candidates. |
| Inlay suppression and lambda facts: `crates/vela_language_service/src/inlay/suppression_tests.rs`, `src/inlay/tests.rs`; `crates/vela_lsp_server/src/tests/inlay_suppression.rs`, `src/tests/inlay.rs` | Exact raw groups include 247/271/373/349/314/392-byte source programs and the 326-byte lambda program. One authored file per distinct arrangement under proposed `sources/local/inlay/`, seeded from LS. Keep typed schema registrations and hint expectations local; do not unify Any-return rejection with known-return positive control. |
| Rename source-default return: `crates/vela_language_service/src/rename/source_return_tests.rs` and `crates/vela_lsp_server/src/tests/rename_source_return.rs` | Exact 316-byte raw program can be one source under `sources/local/rename/`; preserve private/public eligibility and complete multi-location edit observations. Larger enum/record programs in LS `references/tests.rs`, `references/variant_field_tests.rs`, `rename/tests.rs` and matching LSP reference/rename modules warrant the same byte-pair review; they are not exempt as “unit tests.” |
| Local semantic-token members/returned receivers: `crates/vela_language_service/src/semantic_tokens/source_call_tests.rs`, `src/semantic_tokens/tests.rs`, `src/semantic_tokens/variant_use_tests.rs`; `crates/vela_lsp_server/src/tests/semantic_tokens_source.rs`, `src/tests/semantic_tokens.rs`, `src/tests/semantic_tokens_schema.rs` | Repeated cooked member, enum, function-return and method-return sources are shared-input candidates under `sources/local/tokens/`. Keep independent source/schema token ownership, suppression and wire projection; exact cooked equality is an implementation check, not inferred from similar names. |
| Container formatting pair: `crates/vela_language_service/src/formatting/tests.rs::formatting_formats_container_type_hint_example` and `crates/vela_lsp_server/src/tests/formatting.rs::lsp_document_formatting_formats_container_type_hint_example` | Long compact two-function input and multiline expected output are repeated. Proposed `sources/local/formatting/container-hints.input.vela` / `.expected.vela`; decode Rust line continuation and escaped quotes exactly. The example refers to `score` without defining it: it is formatting proof, **not** a ready executable-result equivalence fixture. Coordinate SF02 rather than inventing semantics here. |
| Multi-scope tuple facts, item trees and folding: `crates/vela_language_service/src/expression_facts/tests.rs::expression_facts_include_tuple_destructuring_bindings`, `src/hover/tests.rs`, `src/symbols.rs::document_symbols_include_nested_type_members`, `src/folding.rs::folding_ranges_cover_items_and_blocks`; `crates/vela_lsp_server/src/tests/folding.rs` | The tuple body has let/for/match scopes and exact nth-occurrence checks: owner-local `file-backed`, not automatically the differently formatted hover/definition tuple source. The repeated 335-character cooked item/block folding program merits shared origin under `sources/local/folding/`. Short malformed literal recovery remains adjacent to assertions. |
| Ordinary trait package: `crates/vela_language_service/src/provider_tooling_tests.rs::TraitToolingFixture::new` | Proposed LS-local `tests/fixtures/packages/ordinary-trait/` with `api/vela.toml`, `api/src/api.vela`, `plugin/vela.toml`, `plugin/src/plugin.vela`. A coherent tree clarifies the actual `../api` dependency and package identities despite tiny sources. Continue copying to fresh writable roots; do not use checked-in paths as document identity. Keep the single removed-provider-attribute probe inline. |
| Signature source receiver arrangements: `crates/vela_language_service/src/signature.rs`; `crates/vela_lsp_server/src/tests/signature.rs` | Multi-declaration function/method/default-trait cases are file-backed candidates. They are not byte-identical to the navigation or completion sources, and LS/LSP variants have different formatting. Separate owner-local files initially; do not normalize them to manufacture a duplicate group. |

Exact raw-string equality above was checked during inventory; it does not imply
that all surrounding environments are equal. Cooked candidates still require a
Rust-evaluated byte comparison at implementation. No test/helper deletion is
proposed. Schema factories adjacent to these programs (for example long JSON in
LSP hover/inlay/completion files) are **metadata**, not Vela source; never give them
a `.vela` extension or count them as source extraction.

### Generators And Intentional Inline Versions

- `crates/vela_language_service/benches/incremental_latency.rs::module_text` varies
  module index, function count and edit salt; `module_document` varies logical path.
  Defaults are 128 modules × six functions, two warmups and eight measured rounds.
  `EDITED_MODULE = 17`, caret line 4/column 17 and imported `support::helper` are
  semantic inputs. Keep generation, finite environment-controlled runs and timing
  boundaries; do not turn scale into hundreds of static files or add timed file IO.
  `SUPPORT_TEXT` is only a constant plus one helper function: `retain-inline` is
  justified here, rather than extracting merely because it is static.
- LS `incremental/tests.rs::scaled_file` and
  `completion/tests.rs::completion_scale_file` generate uniquely named modules and
  comment padding. Scale/cache/invalidation coverage requires generation. Retain
  the small fixed edited document near the metrics and completion assertions.
- LF→CRLF and shifted Unicode-prefix variants throughout shared support are
  `generated` transformations of one authored document. Shebang placement,
  marker offsets, missing EOF newline and fixed-width changes must remain intact.
- `tests/lsp_matrix/support/workspace_symbol_states.rs::cases` loads the ownership
  base plus finite `imports`/`body` edits; it asserts exactly one occurrence of each
  replacement anchor. Keep this explicit mutation coverage, not static files for
  every combination. Its direct `include_str!` reader is part of SF10.
- Tiny old/new pairs in `close_overlay.rs`, global-state snapshots, schema reload
  and incremental tests are `retain-inline`: source differences and event order
  are readable together. This does **not** exempt the sizeable matrix lifecycle
  versions enumerated next.

## SF10: Shared JSON Families And Actual Selections

All paths in the following table are under `tests/lsp_matrix/fixtures/`. Counts
partition the **218 JSON files**, not documents, phases or test identities.
The `files` maps may include manifests, schemas, expected `.txt` documents and
non-Vela sentinels; classification follows the consumer, not just the extension.

| Inventory partition | Count | Disposition after whole-family inventory and targeted reader/body review |
|---|---:|---|
| `call-argument-context.json`, `call-parameter-mapping.json`, `callable-imports.json` | 3 | `retain-inline`: short independently marked argument/call/import documents alongside each expected mapping; dependency fragments stay with the small graph. No file per parameter slot. |
| `completion-*.json` | 53 | Mixed. Retain concise authoring/type/literal/operator/call/snippet/callback rows, alias queries, tiny schema-state probes and deliberate removed-execution inputs. Extract substantial repeated API modules and versions in `completion-source-callable-lifecycle.json`, `completion-source-overlay-lifecycle.json`, `completion-source-recovery.json`; share meaningful API duplicates in `completion-abrupt-flow.json` / `completion-local-exits.json`. Review `completion-{loop-match-flow,receiver-assignments,return-flow,shared-methods,package-members}.json` API modules with those groups, not their many tiny probe files. `completion-callback-factories.json` includes complete applied-source expectations under `files/expected/*.txt`: source/golden candidates, not disposable duplicate outputs. |
| `code-action-import-partitions.json`; `diagnostic-*.json` | 15 | Larger import/member/constructor/control-flow/type/top-level documents and applied outputs are file-backed candidates (`code-action-import-partitions`, `diagnostic-action-control-flow`, `diagnostic-action-members-constructors`, `diagnostic-top-level-declarations`, `diagnostic-type-positions`). Retain small method-typo, unresolved/recovery, source-state and schema/dependency-change probes inline. Keep independent applied outputs and diagnostic fields; an output must not be recomputed by production repair code. |
| `document-open.json`, `document-change.json`, `document-symbol-{declarations,ownership,recovery,lifecycle}.json` | 6 | Extract substantial declaration/ownership documents and repeated invalid/body documents; review all nested change/repair text and symbol variants. Small malformed incremental edit sequences and short recovery cases remain inline. Symbols have separate independent trees, not a generic declaration snapshot. |
| `folding-{bodies,declarations,editor,imports,literals,members,recovery,trivia}.json` | 8 | Mostly `retain-inline`: per-case marked sources make exact folded regions, comments/literals and malformed boundaries readable with their oracles. Duplicate `files/main` / first-case source and selected editor copies are genuine sharing review points, but do not mandate dozens of tiny files. `folding-editor.test.js` already checks selected sources/ranges against their partition origins; preserve that relationship. No independent-distribution requirement was found to justify new editable copies. |
| `hover-{s1,s2,s3,s4,s5,s6,s7,s8,s8-package,s9,s9-recovery,s10,lifecycle,signature-coordinates}.json` | 14 | `file-backed` for large marked documents, especially s1/s2/s4/s5/s6/s7, substantial s8 package sources and lifecycle action versions. s3/s8/s9/s10 are mostly smaller per-position documents: retain small probes, extract only substantial support sources. Coordinate fixture and tiny recovery sequences remain self-contained when that keeps position/null expectations readable. |
| `navigation-*.json` | 12 | Extract multi-declaration main/API source trees in calls, constructors, declarations, dynamic, imports, members, namespaces, owned-declarations and type-positions. Retain concise schema/recovery/source-lifecycle probes and actions. Shared tiny decoys do not need a universal file. |
| `reference-rename-*.json` | 31 | Extract substantial marked body/control/default/method/field/import/schema documents and meaningful repeated variant-import graphs. Keep short import-boundary, recovery and lifecycle transitions inline. Preserve per-family ambiguity/capture/public/private assertions even when graph source is shared. |
| `rename-{declaration-collisions,local-collisions,local-shorthand}.json` | 3 | `retain-inline`: small source/name/edit triples in `oracle.cases` directly expose the collision or shorthand distinction. Longer isolated rows are still local coherent documents; do not extract the entire family solely for uniformity. |
| `selection-{bodies,brackets,calls,declarations,editor,imports,lifecycle,literals,members,parens,pattern-recovery,patterns,recovery,trivia,types}.json` | 15 | Mixed: extract dense repeated composite main/first-case inputs in patterns, literals, members, calls, types and bodies; retain tiny ancestry/recovery/byte-sensitive tables and query chains. Consider substantial lifecycle variants independently. `selection-editor.json` already references fixture/case IDs through its reader: preserve this reuse rather than copying its upstream source. |
| `semantic-token-{bodies,calls,control,coordinates,editor,imports,incremental,literals,members,ownership,recovery,schema-lifecycle,source-lifecycle,top-level,type-positions}.json` | 15 | Strong `file-backed` candidates for dense positive and negative marked streams, including documents inside `oracle.negative.cases`, state queries and repair sequences. Keep compact coordinate/byte-width variants and tiny incremental edits inline when clearer. Do not extract only `files` and miss the larger negative documents. |
| `signature-{editor,s3,s5,s8,s9,s10}.json` | 6 | Extract s3's type source, s5's multi-call main, s8's package/changed versions and editor's full main/recovery documents where useful. Retain short s9/s10 call-site rows and local updates. Keep exact labels, active-parameter and null cases independently authored. |
| `workspace-symbol-{declarations,incremental,lifecycle,ownership,recovery,sources,states,type-positions,worker}.json` | 9 | Extract substantial declarations/ownership/type-position sources and sizeable lifecycle variants. Retain small incremental/source/recovery cases and the finite mutation table in states. Worker JSON identifies interleavings rather than supplying a standalone program; it depends on lifecycle source. Document/workspace symbol sources can have equal stripped text but **different markers**: do not merge on stripped equality. |
| `input-*.json` | 21 | Most are concise self-contained physical-input contracts or references to other corpora; retain them. `input-semantic-tokens.json` has a substantial 98-line marked source and shifted/damaged/applied documents: explicit extraction candidate across native driver, token-render oracle and provider consumers. Outline/document-picker/workspace-picker/selection/folding configurations indirectly select corpus source even when they contain no declaration strings. |
| `marker-golden.json` | 1 | `unit-data` / existing specialized marker corpus: retain hand-authored Unicode/LF/CRLF text, byte/UTF-16 coordinates and malformed-marker cases in JSON. Extracting each tiny string obscures escape semantics. |
| `shared-unicode-lifecycle.json` | 1 | `retain-inline`: two tiny files and eleven ordered transitions make overlay/save/close/delete/recreate behavior immediately visible. This is a justified self-contained lifecycle, not a rule that all lifecycles stay inline. |
| `request-cancellation.json`, `session-init.json`, `watched-files.json`, `workspace-configuration.json`, `workspace-packages.json` | 5 | `retain-inline` for short source sentinels, malformed updates, configuration/manifests and ordered operations. Metadata/schema/wire profiles are `unit-data`; preserve real package/watcher/config IO. Full source extraction is not justified merely to force these through a common schema. |

### Specific SF10 Candidate Origins

Use the plan's proposed `tests/lsp_matrix/fixtures/sources/<family>/...` subtree,
with one authored marked source for a selected group. The following are actual
locations, not inferred candidates from file size alone:

1. `hover-s7.json` → `files["scripts/main.vela"]`: 25,885 decoded UTF-8 bytes,
   135 newlines, many independently marked literals/operators/containers and
   multiple functions. A separate marked `.vela` document is markedly easier to
   inspect. Preserve its query IDs and all markdown/null/type-fact oracles in JSON.
   Similarly s2/s4/s5/s6 have sizeable multi-feature documents, not local one-liners.
2. `semantic-token-calls.json` → main (27,674 marked UTF-8 bytes) and negative source
   (21,742); `semantic-token-control.json` → main (28,952); ownership negative cases
   reach 30,040 bytes. These sizes include markers, which are important input.
   Proposed positive/negative files per scenario; preserve positive→negative→repair
   execution and full token/diagnostic oracles. Do not prettify the one-line bodies.
3. `completion-source-callable-lifecycle.json`,
   `completion-source-overlay-lifecycle.json`, `completion-source-recovery.json`:
   repeated root/dependency API versions occur in initial `files`,
   `oracle.sourceLifecycle[].files`, `.action.source`, `.diskSource`, `.openSource`.
   Seed the shared base from the callable-lifecycle fixture and distinct version
   files from their current owning phase. Keep the logical paths
   `scripts/api.vela` and `deps/external/scripts/api.vela`, package manifests,
   null deletion/closed states and ordered actions. Small query files stay inline.
4. `diagnostic-top-level-declarations.json` → `files["scripts/invalid.vela"]` is
   exactly repeated in `document-open.json`, `document-change.json` and the latter's
   `oracle.cases[2].steps[2].text`. Seed one marked invalid-declarations source from
   the diagnostic fixture. `document-change.json`'s body source also repeats
   `reference-rename-body-scopes.json` main and a restored step; seed from the
   reference/rename fixture. Preserve each distinct diagnostic/sync/reference route.
5. `selection-patterns.json` → main and `oracle.cases[0].source` are identical
   2,334-byte marked strings. Similar main/first-case duplication exists in
   `selection-{literals,members,calls,types,bodies}.json`. One composite source per
   family, reused by its two declared fields, is preferable to two editable copies;
   keep the small individual cases inline. Selection editor resolves upstream case
   IDs and must receive the resolved source too.
6. `reference-rename-schema-variant-{ambiguity,lookup}.json` share the substantial
   `scripts/imports.vela`; private/source variant-import and ambiguity-removal
   fixtures share `scripts/source_imports.vela` and other modules. Proposed shared
   variant-import source group, seeded from schema-variant-ambiguity and
   private-variant-imports respectively. Compare **marked** bytes before sharing;
   keep different schema registries and collision oracles independent.
7. `document-symbol-declarations.json` → `scripts/api.vela` (4,224 marked bytes),
   `workspace-symbol-declarations.json` counterpart (4,478), and
   `workspace-symbol-type-positions.json` → `scripts/types.vela` (7,144) are
   file-backed candidates. Different marker sets mean separate canonical sources
   unless full marked equality is established. Lifecycle initial sources duplicated
   in `oracle.variants.*.source` may share within their own family.
8. `signature-s8.json` carries source in `oracle.phases[].actions[].source`, including
   sizeable 49/51-line versions and repeated restorations. Use version files for
   these documents, not one file per event. Keep ordering, overrides, source/schema
   owners and disk verification. `input-semantic-tokens.json` similarly needs its
   initial, shifted, damaged and post-edit documents reviewed together.

These findings rule out a blanket SF10 “no worthwhile extraction” conclusion.
They do **not** authorize broad JSON rewrites now. Small families retain their
inline form; selected fields/readers require the coordinated change below.

## Reader And Provenance Dependency Map

### Source-Bearing Fields, Not Arbitrary Recursive Includes

Current Rust `Spec.files` is `BTreeMap<String, String>` and `Action.source` is
`Option<String>`; `Spec.oracle` is an untyped `serde_json::Value`. `load(id)` reads
`../../tests/lsp_matrix/fixtures/<id>.json` relative to `CARGO_MANIFEST_DIR` with
`read_to_string`. JavaScript readers mostly `require` the JSON directly.
`scripts/lsp-matrix/fixtures.js::FixtureWorkspace` parses file/action values as
**text**, not paths. Replacing a string with a filename would compile that filename.
No source-file reference format is implemented today.

Declared source fields needing explicit treatment **when selected**, including
consumers that bypass `FixtureWorkspace`:

| Fields / route | Existing reader owners |
|---|---|
| `files`, `actions[].source` | `tests/lsp_matrix/support/mod.rs`; `scripts/lsp-matrix/fixtures.js`; all library consumers; `crates/vela_lsp_server/tests/stdio_transport.rs`; editor materializers. |
| `oracle.sourceLifecycle[].files`, `action.source`, `diskSource`, `openSource` | `tests/lsp_matrix/support/source.rs`; LS `completion/type_ownership_tests.rs` and its `source_overlays.rs`; LSP `tests/completion_type_ownership.rs` and `source_lifecycle.rs` / `source_overlays.rs`. |
| `oracle.phases[].actions[].source` | `tests/lsp_matrix/support/signature_imports.rs`, `hover_recovery.rs`; LS `signature/import_tests.rs`, `hover/lifecycle_states.rs`, `hover/recovery_states.rs`; LSP `tests/signature/imports.rs`, `tests/hover/{lifecycle_states,recovery_states}.rs`. |
| `oracle.cases[].source`, `oracle.variants.*.source`, phase variant references | Shared `document_symbols.rs`, `document_symbol_lifecycle.rs`, `workspace_symbols.rs`, `selection.rs`, `selection_lifecycle.rs`, `selection_*` and `folding_*` modules. JS document/workspace-symbol, selection and folding oracle modules and their `*.test.js` files read these directly. |
| Token `oracle.negative.source` / `.cases[].source`, state query documents, `diskSources`, `openSources`, coordinate/dirty/same-byte-length sources | `tests/lsp_matrix/support/semantic_tokens.rs`; LS `semantic_tokens/test_support/scenarios.rs`, `coordinate_tests.rs`, `incremental_tests.rs`; LSP `tests/semantic_tokens/{support/scenarios,lifecycle_support,incremental_support,coordinates}.rs`; `scripts/lsp-matrix/semantic-token-oracle.js`, `token-render-contracts.js`. |
| `oracle.cases[].steps[].text`, full/range-edit initial/final documents | `tests/lsp_matrix/support/document_open.rs`; LS document open/change/close tests; LSP corresponding matrix modules and stdio. Partial `newText` or invalid edit ranges remain ordinary inline edit data. |
| `applied`, `applied*`, `repaired*`, `damaged`, `dirty`, `changedDependency`, `recreatedDependency`, `overlay`, `expected`, snippet `expanded`, `files/expected/*.txt` | Owning diagnostics/code-action/completion readers, not a universal include walker. Some are whole source documents, some edit fragments or expected values: migrate only fields explicitly selected and preserve independent outputs. |
| `workspace-symbol-states.json` finite imports/body edits | `tests/lsp_matrix/support/workspace_symbol_states.rs` uses direct `include_str!`; Node `workspace-symbol-states.test.js` uses direct `require`. Keep tiny fragments inline; resolving a referenced base is still required if ownership source moves. |
| Marker golden | Direct `include_str!` in `tests/lsp_matrix/support/tests.rs`, direct `require` in `scripts/lsp-matrix/fixtures.test.js`. Keep as authored unit data; do not route diagnostic strings, labels or marker names through source resolution. |

Other shared support files (`schema.rs`, `references.rs`, `rename_collisions.rs`,
`rename_shorthand.rs`, `signature_calls.rs`, `workspace_symbol_incremental.rs`,
`workspace_symbol_sources.rs` and the folding/selection partition modules) were
included in the reader inventory. They build typed schema facts, derive coordinates
from authored markers or transform explicit case documents. Schema/type metadata,
marker labels and expected result arrays remain data, not executable source.

### JavaScript And Editor Fan-Out

- `scripts/lsp-matrix/{document-symbol-oracle,workspace-symbol-oracle,selection-oracle,
  selection-editor-oracle,folding-editor-oracle,semantic-token-oracle}.js` and
  specialized selection literal/import/trivia oracles consume source or marker
  documents. Partition `*.test.js` files directly require JSON and assert exact
  sources/oracles. Updating only `fixtures.js` will not migrate them.
- `scripts/lsp-matrix/selection-editor-oracle.js::resolve` loads another fixture
  by `ref.fixture` and selects `ref.case`. `outline-contracts.js::outlineModel`
  loads fixtures named in `input-outline.json`; `document-picker-contracts.js`
  remaps that model's logical files. Workspace picker and gutter/keyboard folding
  and selection-input contracts similarly reuse corpora. These are existing
  concrete readers, not permission to add a generic recursive include language.
- Direct installed-provider readers:
  `editors/vscode/test/{completion-fixture,diagnostic-action-provider,
  document-symbol-provider,folding-provider,navigation-providers,
  reference-rename-providers,shared-fixture,signature-provider,
  selection-provider,selection-lifecycle-provider,workspace-symbol-provider}.js`.
  `semantic-token-provider.js` obtains documents through the token model.
  `document-symbol-session.js`, `editor-document-session.js` and
  `selection-session.js` preserve buffer/disk isolation and fresh wire observations.
- `editors/vscode/test/run.js` copies the existing
  `editors/vscode/test/fixture/{vela.toml,scripts/main.vela,scripts/helpers.vela}`
  into an isolated workspace, appends shared fixture files, and starts separate
  workspace-symbol/selection sessions. These existing authored files are
  `file-backed`; temporary workspace copies are not another authored origin.
- `editors/vscode/test/input/run.js` materializes/merges `input-*.json`, diagnostic
  quick-fix and derived outline/document-picker files. `input/bridge.js` also reads
  installation/lifecycle fixtures directly; `input/workspace-trust-session.js`
  materializes its own fixture. Companion input drivers consume the contract models
  for completion/navigation/references/rename/diagnostics/tokens/hover/signature,
  folding/selection, workspace files/roots/schema/trust and lifecycle/install.
- Node/source microinputs remain inline: `scripts/lsp-matrix/disk-file.test.js`
  contains tiny CRLF sentinels; `editors/vscode/test/selection-runner.js` writes a
  one-line startup function; `document-symbol-provider.js` and `symbol-schema.test.js`
  use a one-line schema probe. `input/hover-layout.js`'s reviewed source line is a
  layout contract, not a substantial hidden program. Trace/wire strings in
  `*-trace*.js`, `stdio.js`, model/infrastructure/evidence tests and audit needles
  are `unit-data` or `not-test-input`. Do not extract JavaScript's own declarations.

### Bytes, Logical Identity And Evidence

For any later selected JSON extraction, use the plan's explicit `sourceFile` form
relative to the owning JSON, with no inline fallback copy. Resolve only declared
source fields **before** marker parsing, CRLF/shift transforms and materialization.
This requires a coordinated Rust/JS fixture-data transition, not a product API or
executable artifact-format change.

Required implementation proof:

1. Compare old/new decoded marked UTF-8 bytes, length, stripped text and every
   marker's byte/line/UTF-16 position; preserve leading newline, indentation,
   escaped quotes, BOM/NUL, mixed EOL and absent final newline. No trim/reformat or
   reauthored coordinate goldens. Existing Rust string reads are strict UTF-8;
   JS source-byte decoding must become explicitly strict if file references are
   added. `parseMarkers`'s well-formed-string check is not a strict byte decoder.
2. Missing/reference-type/unknown-field/conflicting-form errors, absolute/drive/
   traversal/backslash paths, symlink escape and invalid UTF-8 must reject with
   matching Rust/JS behavior. The current `safe_file`/`safeFile` validates logical
   workspace names; it does **not** by itself implement source-storage containment.
3. Keep logical document paths and URI roots in Rust/JSON/JS. `/workspace/scripts`,
   `file:///workspace/scripts/game/main.vela`, scratch/untitled documents, Unicode
   and percent-encoded roots, package IDs/module names, document versions and request
   IDs must not become physical fixture paths. Source/schema SourceIds are rebound
   to the current graph where that is the tested route; do not hardcode storage IDs.
4. Keep JSON fixture IDs, phase/case/query IDs and existing Rust test names. Preserve
   `tests/lsp_matrix/features/` links and `gate-evidence.json` references. Compare
   actual default/affected-feature `--list` output; `rust-suites.js` rejects empty
   discovery, duplicate identities and results absent from discovery. Listed-only
   tests are not executed evidence.
5. `scripts/lsp-matrix/provenance.js` hashes sorted **path + NUL + raw bytes** for
   editor extension/package/lock files and recursively for `editors/vscode/test`,
   `tests/lsp_matrix/fixtures`, `scripts/lsp-matrix`; it separately hashes the server
   binary and records platform. Extraction changes this fingerprint even if
   resolved Vela bytes are identical. Keep proposed shared sources in that subtree.
6. `scripts/lsp-matrix/source-identity.js` hashes tracked and nonignored untracked
   tree entries, modes, deletions and symlinks, excluding only `checkpoint.json`.
   Its tree identity is different from installed input/server provenance.
   `model.js`/`execution.js` contract digests and reviewed old/new migrations are
   another layer; do not mechanically replace them with source-file hashes.
   `editor-results.js`, `local-evidence.js`, `archive-evidence.js`, `checkpoint.js`
   and their tests retain validation of the appropriate evidence boundary.
7. Preserve all 1,327 semantic and 1,477 execution obligation IDs, immutable
   `execution-baseline.json`, original captures and pending proof. B02–B08 remain
   reopened. Recapture affected installed evidence with matching input/server/
   platform provenance; never update old captures to make new sources appear proven.
   Provider results do not close native Input/Render or Windows obligations.
8. Test-only cross-crate includes must remain reachable in the repository test
   route. No production dependency on repository fixtures. Any independently
   packaged test route must demonstrate source/support inclusion from its packaged
   tree; no distribution copy is needed or approved by this census. Editor workspace
   materialization is a runtime copy, not a second editable fixture or symlink fix.

## Baselines And Handoff Gates

Commands are rooted at the repository and follow [validation.md](validation.md).
Use finite command timeouts and require nonempty selections. No fixture migration,
new test catalog, shared-doc status update or commit was made in this assignment.

### Actually Executed During This Census

| Command | Observed result |
|---|---|
| `node --test scripts/lsp-matrix/fixtures.test.js scripts/lsp-matrix/provenance.test.js scripts/lsp-matrix/rust-suites.test.js` | 24 passed, zero failed/skipped. Existing marker/edit/path/materialization/provenance/target-mapping baseline only; no proposed reference-reader tests exist yet. |
| `cargo test -p vela_language_service --lib shared_fixture --locked -- --list` | Two discovered identities: `matrix_fixture::tests::shared_fixture_paths_actions_and_materialization_are_isolated`; `shared_fixture_tests::shared_fixture_service_queries_use_independent_unicode_lifecycle_ranges`. |
| `cargo test -p vela_lsp_server --lib shared_fixture --locked -- --list` | Two discovered identities: same library helper identity; `tests::shared_fixture::shared_fixture_protocol_queries_follow_unicode_disk_overlay_lifecycle`. They are distinct owner targets, not duplicate cases to delete. |
| `cargo test -p vela_language_service --lib shared_fixture --locked` | Passed the two selected tests. |
| `cargo test -p vela_lsp_server --lib shared_fixture --locked` | Passed the two selected tests. |
| `cargo test -p vela_lsp_server --test stdio_transport --target-dir target/lsp-matrix-stdio --locked` | All 10 passed: four process tests plus six shared support/oracle tests included in this integration target. |

Each test/discovery invocation was bounded by a 120-second tool timeout; none
reached it. These are focused baselines, not a fresh complete owner/workspace,
all-features, installed-editor or native-input acceptance claim.

### SF09 Baseline And Exit Commands

```sh
cargo test -p vela_language_service --lib --locked -- --list
cargo test -p vela_lsp_server --lib --locked -- --list
cargo test -p vela_language_service --locked
cargo test -p vela_lsp_server --locked
cargo test -p vela_language_service --all-features --locked
cargo test -p vela_lsp_server --all-features --locked
cargo test -p vela_lsp_server --test stdio_transport --target-dir target/lsp-matrix-stdio --locked
cargo bench -p vela_language_service --bench incremental_latency --no-run --locked
cargo fmt --all -- --check
cargo clippy -p vela_language_service -p vela_lsp_server --all-targets --all-features --locked -- -D warnings
```

Before each candidate group, list/run the affected module filter (for example
`source_return`, `member_completion`, `inlay`, `formatting`, `call_hierarchy`) and
verify the selected names rather than assuming a filter is nonempty. Preserve
lifecycle/cancellation suites even when only initial bytes move. Default and
all-feature owner runs are not substitutes for the aggregate workspace gate and
its feature-unification context. Build the bench only if its input/helper changes;
run any measurement separately with explicit finite settings and no performance
claim from a build.

### SF10 Baseline And Exit Commands

```sh
node --test "scripts/lsp-matrix/*.test.js"
node --test editors/vscode/test/session-paths.test.js editors/vscode/test/symbol-schema.test.js
cargo test -p vela_language_service --lib matrix_fixture --locked
cargo test -p vela_lsp_server --lib matrix_fixture --locked
cargo test -p vela_lsp_server --test stdio_transport --target-dir target/lsp-matrix-stdio --locked
node scripts/lsp-matrix/run.js --run
```

Also run both complete owner lanes above and every selected feature's service/
protocol cases. Add matching invalid-reference/strict-decoder/byte-marker parity
proof to the existing readers only when implementing selected references. Direct
reader Node tests and editor source guards must use the resolved source, not a
shadow inline copy. Run the full matrix command with an appropriate finite bound;
its output/report is not permission to use `--accept` or change checkpoint state.

Affected installed-provider gates remain `npm --prefix editors/vscode ci` and
`npm --prefix editors/vscode test` from the validation guide; dependency installation
and editor download require network, and editor execution requires the pinned
profile/environment. None was run here. Preserve raw timeout/partition evidence,
require matching provenance to merge partitions and leave native observer-extension,
strict/native and platform gaps with the parent. Fixture reorganization cannot
resolve those prerequisites or certify physical input.

### Parent Handoff

- SF09 has concrete sharing/file-backed work plus justified inline/generated cases;
  no test-target move, orchestration collapse or new helper framework is needed.
- SF10 has worthwhile selections and a known multi-reader migration surface. Assign
  **one writer** for shared source origins/readers and coordinate both crates and
  editor consumers before moving JSON fields. Do not independently migrate `files`
  while leaving phase/oracle readers interpreting objects as strings.
- SF11 receives the bench and editor-seed dispositions above; this census does not
  authorize unrelated site/grammar/fuzz work or close those owners.
- Exact new filenames, source-field schema/version details, packaged-tree validation
  and byte/assertion equivalence remain implementation obligations. This document
  completes the owned family census, not those later stages. Shared progress/design/
  blocked documentation and commits remain parent-owned.
