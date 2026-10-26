# SF10 — Shared matrix source extraction

Date: 2026-10-26. Implementation handoff; **no commit**. Parent owns integration
and shared status/design documentation, after SF09.

## Scope and outcome

Reviewed the complete 218-JSON family inventory in
`docs/test-input-census-tooling.md`, together with targeted source bodies,
lifecycle consumers, direct readers and provenance. Implemented the selected
SF10 work from `docs/test-organization-plan.md`, not a zero-inline conversion.

- **94 JSON fixtures changed; 124 remain unchanged.**
- **515 source occurrences now reference 198 authored `.vela` files**, totaling
  **744,462 unique marked UTF-8 bytes**. The other 317 occurrences reuse those
  authored documents rather than keep independently editable identical strings.
- All **218 fully resolved JSON trees** equal their pre-extraction values,
  including every logical file path, fixture/phase/case/query ID, action order,
  schema/manifest, marker string and independent oracle. No expectations were
  regenerated from production providers.
- Every referenced document's original evaluated bytes equal the stored bytes;
  stripped text and every marker's byte/line/UTF-16 coordinates also match.
  Twenty-three extracted documents intentionally have **no final newline**.
- No edits to crates, `sources/local`, feature inventories, immutable baselines,
  previous evidence, shared status docs or existing archive reports. No new
  executable catalog, test-discovery mechanism, fixture framework or commit.
  One-off extraction/check outputs live only in ignored `target/sf10-review`.

Authored changes are confined to `tests/lsp_matrix/fixtures` JSON and non-local
sources, `tests/lsp_matrix/support`, `scripts/lsp-matrix`, the affected
`editors/vscode/test` reader callsites, the explicitly authorized review follow-up
`editors/vscode/scripts/check-removed-consumers.test.js`, and this report. The
source attributes file explicitly scopes only the ten SF10 directories; it does
not apply to SF09's `sources/local`.

## Whole-family dispositions

Paths in this table are relative to `tests/lsp_matrix/fixtures/`. Counts partition
all 218 fixtures. Selection was based on composite bodies and actual repeated
marked bytes, not merely JSON file size. The exact changed fixture and source
lists below make the selection auditable.

| Family | Total / changed JSON | Implemented selection and deliberate inline retention |
|---|---:|---|
| `call-argument-context`, `call-parameter-mapping`, `callable-imports` | 3 / 0 | Keep independently marked short call/import graphs beside their mapping oracles. No file per argument slot. |
| `completion-*` | 53 / 10 | Extract repeated root/dependency API modules from abrupt/local-exit, loop/match, receiver-assignment, return-flow, shared-method and package-member families. Extract callable/overlay/recovery lifecycle API versions across initial/phase files, action, disk and open states. Keep small query files, deletion nulls, schema facts, snippets and concise authoring/type/callback probes inline. `completion-callback-factories` applied documents remain independently authored paired expectations beside the edits; they are not replaced by generated output or merged with inputs. |
| `code-action-import-partitions`; `diagnostic-*` | 15 / 5 | Extract the composite import main, invalid control-flow/member/constructor documents, valid/invalid type-position documents and repeated invalid top-level declarations. Keep the small healthy companion programs and local repair/applied outputs (including the 563-byte member/constructor applied document) beside their assertions. Source/schema-state and unresolved/typo micrographs remain self-contained. |
| `document-open`, `document-change`, `document-symbol-*` | 6 / 5 | Share invalid declarations with diagnostics and the body document with reference/rename. Extract substantial full replacement/restoration step texts; partial edits stay inline. Extract declarations, ownership and sizeable lifecycle versions with distinct symbol trees in JSON. Keep recovery's small damaged/repaired cases and the short lifecycle origin/API base where not selected. |
| `folding-*` | 8 / 0 | Retain small independently marked region/comment/literal/recovery cases with folding expectations. Reviewed main/first-case/editor repetition, but fragmenting these short boundary documents does not improve them. Existing editor-to-partition comparison remains; its dynamic JSON reader now resolves references if the selected upstream corpus changes. |
| `hover-*` | 14 / 11 | Extract substantial s1/s2/s4/s5/s6/s7 bodies, s8 support/package documents, two s10 composite cases, lifecycle main/API versions and the full signature-coordinate main. Preserve short s3 per-type contexts, s9 recovery/call-site probes, small support modules, EOF probes and null/markdown/type-fact oracles inline. Schema lifecycle payloads remain JSON strings, not mislabelled Vela files. |
| `navigation-*` | 12 / 9 | Extract multi-declaration main/API documents for calls, constructors, declarations, dynamic, imports, members, namespaces, owned-declarations and type-positions. Keep short decoys, schema/recovery/source-lifecycle graphs and actions inline. |
| `reference-rename-*` | 31 / 25 | Extract substantial bodies/control/default/parameter/field/import/schema-owner documents. Share only byte-identical marked variant-import graphs; distinct ambiguity/capture/public/private schemas and assertions stay separate. Keep small import-boundary, lifecycle, recovery and schema-lookup probes inline. |
| `rename-declaration-collisions`, `rename-local-collisions`, `rename-local-shorthand` | 3 / 0 | Keep coherent source/name/edit triples beside collision and shorthand expectations, including their isolated longer rows. No source-only abstraction for a table whose purpose is that local relationship. |
| `selection-*` | 15 / 6 | Extract the bodies/calls/literals/members/patterns/types composite main/first-case duplicates and three substantial helper documents. Retain the 200/375/491-byte helper documents and individual ancestry/recovery/byte-sensitive cases. Lifecycle remains a short local sequence. Editor's upstream fixture/case references still select the resolved source, not copied editor documents. |
| `semantic-token-*` | 15 / 14 | Extract dense positive and negative bodies/calls/control/imports/literals/members/ownership/recovery/top-level/type-position streams, editor variants, repeated incremental queries and source/schema lifecycle state documents. Restore/repair uses the same exact authored positive version; negative streams retain their own token/diagnostic oracles. Keep coordinates, small dirty/equal-byte-length probes, tiny unrelated modules and short dependency edits inline. A tiny negative recovery row also remains inline. |
| `signature-*` | 6 / 4 | Extract s3 types, s5 methods, s8 package/main/action versions and editor main/recovery. Retain short s9/s10 positions, schema payloads, active-parameter/null expectations and small support API text. |
| `workspace-symbol-*` | 9 / 4 | Extract declarations/ownership/type-positions and sizeable lifecycle variants. Do **not** merge document/workspace sources on stripped equality: their marked bytes and missing-EOF policies differ. Keep incremental/source/recovery microcases and finite state mutations inline. Worker fixtures continue to select lifecycle state rather than acquire their own source copies. |
| `input-*` | 21 / 1 | Extract `input-semantic-tokens` initial, shifted and unknown-token documents. Its 38-byte decoy, replacement fragment and prefix remain inline. The actual fields are `shifted` and `unknown`; there is no separately authored damaged/applied field to invent. Other input contracts and their small sources stay self-contained; outline/document-picker/workspace-picker/selection reuse resolved upstream corpora. |
| `marker-golden` | 1 / 0 | Retain authored Unicode/EOL/invalid-marker unit data and independent coordinates, including its direct Rust `include_str!` and JS JSON require. |
| `shared-unicode-lifecycle` | 1 / 0 | Keep its two tiny files and eleven ordered transitions together. This is a specific justified inline lifecycle, not an exemption for large lifecycle families. |
| `request-cancellation`, `session-init`, `watched-files`, `workspace-configuration`, `workspace-packages` | 5 / 0 | Keep small source sentinels, malformed updates, manifests/configuration and wire operations inline; real filesystem/package/config behavior remains tested. |

### Source group ownership

All new documents are below `tests/lsp_matrix/fixtures/sources/`:

| Directory | Documents | Purpose |
|---|---:|---|
| `completion/` | 22 | Shared API graphs and distinct lifecycle versions; small probes remain in JSON. |
| `diagnostics/` | 9 | Composite invalid/valid inputs and full document-change versions. |
| `hover/` | 27 | Substantial marked bodies, packages, coordinates and lifecycle source versions. |
| `navigation/` | 9 | Composite declaration/navigation documents. |
| `references/` | 24 | Marked scope/owner/import graphs, including the body document reused by sync. |
| `selection/` | 9 | Six composite main/first-case sources and three sizeable helpers. |
| `symbols/` | 22 | Separate document/workspace marked declarations/ownership/lifecycle/type documents. |
| `tokens/` | 60 | Dense positive/negative, incremental, editor and source/schema lifecycle streams. |
| `signature/` | 13 | Types/methods, package versions and editor main/recovery. |
| `input/` | 3 | Initial/shifted/unknown physical token-render inputs. |

Canonical repeated origins include callable-lifecycle's API versions, the
`diagnostic-top-level-declarations` invalid source, the
`reference-rename-body-scopes` main, and each selection composite's main.
Sharing compares the **entire marked string**, never just stripped text.
Canonical names describe fixture origin and logical source or phase/variant;
they are storage names only and never become workspace URIs or module names.

## Reader contract and version decision

This is an additive **version 1 test-data document union**, not a v2 reader that
silently downgrades to v1. Each declared source slot is either an inline string
or exactly `{"sourceFile":"sources/<group>/<document>.vela"}`. Existing
`Spec.files`, `Action.source`, marker parsers and materializers still receive
ordinary strings. Different documents may deliberately use different forms;
there is no fallback/shadow string for a referenced document.

`tests/lsp_matrix/support/fixture_sources.rs::load_json` resolves the finite
schema routes before `support/mod.rs::load` deserializes `Spec`.
`scripts/lsp-matrix/fixture-sources.js::loadFixture` performs matching resolution
before JS marker/EOL/shift operations. It returns fresh objects rather than
mutating Node's cached `require` JSON objects. The existing public test helpers
and provider/oracle logic were not rewritten.

The internal slot traversal follows only these hardcoded schema routes. It is
not driven by JSON-provided path expressions and never recursively interprets
arbitrary objects as includes. `*` below denotes array/map entries in the
implementation's fixed route list, not fixture syntax.

| Owner / fields | Null handling |
|---|---|
| All: `files.*`; `actions[].source` | File strings required; absent/null action payload retained. |
| Three `completion-source-{callable-lifecycle,overlay-lifecycle,recovery}` fixtures: `oracle.sourceLifecycle[].files.*`, `.action.source`, `.diskSource`, `.openSource` | Preserve source deletion and closed/absent overlay nulls. |
| `hover-*`, `signature-*`: `oracle.phases[].actions[].source` | Optional action payloads. Small schema payload strings remain strings. |
| `signature-editor`: `oracle.recovery.source` | Source string/reference. |
| `document-change`: `oracle.cases[].steps[].text` | String/reference; partial edit strings are not extracted. |
| `selection-*`, `document-symbol-*`, `workspace-symbol-*`: `oracle.cases[].source`, `oracle.variants.*.source` | Source string/reference. |
| `semantic-token-*`: `oracle.negative.source`, `.negative.cases[].source`, `.states[].source`, `.states[].queries.*.source`, `.initial.*.source`, `.steps[].queries.*.source` | Source string/reference. |
| Tokens: `oracle.states[].diskSources.*`, `.openSources.*`, `.action.source`; `oracle.steps[].source` | Preserve absence/deletion nulls. |
| `input-semantic-tokens`: `oracle.shifted`, `oracle.unknown` | Source string/reference. |

Source storage resolution is separate from logical-workspace `safe_file` /
`safeFile` checks. Paths must start with `sources/` relative to their JSON owner,
with no empty/dot/parent components, absolute/drive paths, backslashes, colon or
NUL. Reject wrong reference types, missing files, directories, unknown keys and
conflicting inline/reference keys. Reject **all source-path symlink components**,
including internal aliases, because raw provenance walks regular files and would
not cover an aliased target. The caller's fixture root is canonicalized once per
read. This is a reader for trusted, non-concurrently-mutated test trees, not a
TOCTOU-resistant filesystem sandbox.

Rust `read_to_string` and JS fatal `TextDecoder("utf-8", {fatal:true,
ignoreBOM:true})` reject malformed bytes. `ignoreBOM:true` means the BOM remains
source content. Neither reader trims, normalizes EOL, appends a newline, parses
markers or resolves references embedded *inside source text*. JS also checks
`sourceFile.isWellFormed()` before source filesystem resolution; Rust rejects
unpaired surrogate escapes while decoding JSON. Null, inline/path Unicode
validity and source-object failures have matching accept/reject behavior;
OS-specific error wording is not an oracle.

## Consumers and target boundaries

Rust's shared entry is included by:

- `crates/vela_language_service/src/lib.rs` (`matrix_fixture`, library tests).
- `crates/vela_lsp_server/src/lib.rs` (`matrix_fixture`, library tests).
- `crates/vela_lsp_server/tests/stdio_transport.rs` (`stdio_fixture`, integration).

No crate callsite edits were necessary: early resolution preserves all existing
`.as_str()` oracle consumers, `source.rs` overlay materialization,
`signature_imports.rs`/`hover_recovery.rs` action parsing,
`semantic_tokens.rs` negative/repair processing, document open/change consumers,
and symbol/selection lifecycle readers. Existing LF→CRLF/prefix transforms still
run **after resolution** at their previous owners. `support/tests.rs`'s marker
JSON and `support/workspace_symbol_states.rs`'s small direct `include_str!` mutation
table remain inline; the latter's ownership base is obtained through `load`.

Thirty JS reader modules were coordinated, including the review correction
listed below. Direct selected
JSON `require`s now call `loadFixture(require.resolve(...))`; the existing
`path.join` dynamic readers call `loadFixture` directly. This includes:

- Dynamic selection-editor and selection-input fixture/case lookup.
- Dynamic outline and workspace-picker fixture lookup; document-picker continues
  to reuse the outline model rather than copy source.
- Token provider and render models, including nested editor states and physical
  shifted/unknown inputs.
- Symbol/selection Node tests and the selected installed-editor provider,
  workspace-materialization and input-runner callsites.
- `editors/vscode/scripts/check-removed-consumers.test.js`: both named-fixture
  loads and the all-fixture artifact inspection use `loadFixture`. The mixed-call
  removal scan asserts string values before joining, preventing silent
  `[object Object]` coercion from hiding a referenced source.

Unchanged direct readers of unselected small JSON documents need no source
interpretation: marker golden, folding/rename recovery microcases, input-driver,
completion/navigation/reference input contracts, installation/lifecycle/config
contracts and shared-unicode lifecycle remain self-contained. In particular,
`scripts/lsp-matrix/run.js` directly parses only the unchanged `input-driver`
fixture; `input/bridge.js` directly reads unchanged installation/lifecycle inputs.
The raw feature/evidence JSON readers remain raw evidence readers. The review
re-audit also checked the direct `include_str!` in
`crates/vela_lsp_server/src/tests/workspace_symbol_worker.rs`: its unchanged
`workspace-symbol-worker.json` is an interleaving contract, not a source-bearing
reference. The lifecycle base still arrives through the resolved shared loader.

No production dependency or distribution copy was introduced. These are existing
repository test routes and runtime copies into isolated editor workspaces, not a
claim that a separately packaged crate can execute repository-owned tests. No
isolated package/distribution gate was run.

## Byte, marker, identity and provenance proof

Before edits, captured the original raw fixture, script/editor and protected
inventory/local-source inputs. For all 218 fixtures, compared the complete
resolved object to the captured decoded original. For each of 515 references,
compared original string UTF-8 bytes directly to the new file, then compared the
old/new parsed text and full marker map. Equality checks preceded digest recording;
hashes are review evidence, not replacements for assertion review.

Aggregate hashes below use sorted `path + NUL + payload` entries. JSON tree
payloads are Node `JSON.stringify` of the decoded/resolved complete object.
Marker paths are `fixture basename + '/' + JSON.stringify(slot components)` and
payloads are `JSON.stringify(parseMarkers(source))`. Raw fixture JSON hashes use
repository-relative paths and unmodified raw bytes.

| Proof boundary | Before SHA-256 | After SHA-256 |
|---|---|---|
| 218 raw fixture JSON files | `946ca5860c991b1e8ad53c9bbfde36520d69d585f6099fee4ce8125c20dc61f9` | `99b3caa2359a097993d7e21b755cce93b8affad2b821ba0ad0f77e2491ea07ee` |
| 218 fully resolved JSON trees | `82179b6836515c1a4899926eddff74185025929c973fe22514afde5b5d1fc131` | same |
| 515 stripped text + marker maps | `6407cbd2af15e601bb7f5865e9a1340ce72332e5f9266eb508d79eb145e6429c` | same |
| Installed-input raw provenance (after review fixes) | `8529d991d135c2c0ce228369e7f7e98e0c3239069eea393451994fc7215a84b8` | `861c12acd6be30ae218cf99d28556cfb48969a7912418cbe5a3705a8d0aa77d5` |
| Installed server, `target/debug/vela_lsp_server` | `53139de0f93710dfa867db6a3bf2cbe6e343c5f6c94c6a87ab3f11d2aab64c23` | same |
| 80 protected inputs | `606dc1ab3d7eae206de6ebf4a4a116bff29ae7e7cc6f91a0718c29d939f896e4` | same |

The pre-extraction installed-input digest was reconstructed from the captured
raw inputs plus the unchanged extension/package/lock trio using the existing
provenance algorithm. An intermediate live digest taken after introducing the
reader is **not** labelled as the baseline. Protected inputs are all 68 SF09
local source files, four feature inventory files and eight top-level matrix JSON
files (`catalog`, `checkpoint`, `execution-baseline`, `execution-manifest`,
`gate-evidence`, `interactions`, `protocol-baseline`, `syntax-baseline`).

`provenance.js` already recursively covers the new source paths; no provenance
algorithm change was needed. Added a source-byte mutation and same-byte path-move
negative control: each invalidates input provenance while leaving server identity
unchanged. `source-identity.js` still owns tree/mode/untracked identity;
`model.js`/`execution.js` still own distinct contract digests. The inventory and
contract tests pass, and the **1,327 semantic / 1,477 execution IDs** remain intact.

**Old installed-editor evidence is stale under the new input fingerprint.** No
old capture, immutable baseline or checkpoint was rewritten/relabelled. B02–B08
remain reopened; physical Input/Render, Windows and the missing installed
observer prerequisite are not closed by these provider/Node/Rust results.
Platform for this run: `darwin`.

### Review blockers and renewed reader audit

The initial handoff missed the raw reader in
`editors/vscode/scripts/check-removed-consumers.test.js`. Reproduced its failure:
**7 passed / 1 failed**, with `parseMarkers` receiving a reference object in the
selection-source test. The signature scan also silently coerced a referenced
source to `[object Object]`. Both its named loader and whole-corpus artifact
inspection now resolve documents; the join has an explicit source-string guard.
This corrects the original reader-coverage claim rather than treating the
initial matrix-only Node result as editor-script coverage.

Added paired JS/Rust path-Unicode controls with an actual
`sources/\ufffd.vela` filename containing `fn replacement() {}`. The valid U+FFFD
reference must load successfully. Escaped lone high (`\ud800`) and low (`\udc00`)
surrogates must reject despite that file existing. The JS test failed before the
fix with **Missing expected exception**, proving the substitution bug instead of
merely testing a nonexistent path. JS now rejects before source filesystem
access; Rust already rejected these escaped paths during JSON decoding and gains
an explicit regression test. The original malformed source-byte tests remain.

Re-audited tracked and nonignored source files across the repository for matrix
fixture roots, raw JSON requires/parsers and Rust includes, including the newly
owned editor-script directory. **102 remaining literal raw JSON callsites refer
to 48 distinct fixtures; all 48 were decoded and checked to contain no
`sourceFile` references.** Reviewed dynamic selection/outline/workspace-picker
and editor-provider loaders, the input-driver parser, the three direct Rust
marker/state/worker JSON sources, and the corpus reader tests. Raw reads in
provenance intentionally hash bytes; raw reads in reader tests intentionally
compare unresolved and resolved forms. No additional missed selected-source
consumer was found. No additional file ownership was needed.

The earlier post-extraction raw input digest
`a86810aff228c7556a5d88721c929d12e234bb9ef14a90a4404c1645fc3849e4`
is superseded by the review-fixed digest in the table. The editor-script reader
itself is outside `provenance.js`'s installed-input walk (it is a repository
validation test, not installed-editor input); its SHA-256 is
`f4114d63e8089be7ee33f0040b38e003ed6e3d114b93bcad7c66717ad49145b0`.
The shared JS loader/test changes are inside that walk. No hashing algorithm,
old evidence or installed server binary was changed. Rechecked all 218 resolved
trees and all 515 original source-byte/marker comparisons after the fixes: equal.

## Validation and limits

All commands ran at the repository root with finite tool timeouts. No workspace
formatting command was run.

| Command / gate | Bound | Observed result |
|---|---:|---|
| Baseline `node --test "scripts/lsp-matrix/*.test.js" editors/vscode/test/session-paths.test.js editors/vscode/test/symbol-schema.test.js` | 120 s | 324 passed, 0 failed/skipped. |
| Baseline `cargo test -p vela_language_service -p vela_lsp_server --lib --locked -- --test-threads=4` | 180 s; retry 600 s | Both timed out. The retry reached LSP reference tests; neither run is an aggregate pass. |
| Baseline `cargo test -p vela_language_service --lib --locked -- --test-threads=16` | 600 s | 771 passed, 2 ignored, 0 failed; 331.02 s test runtime. |
| Baseline `cargo test -p vela_lsp_server --lib --locked -- --test-threads=16` | 600 s | 842 passed, 0 failed/ignored; 372.33 s. |
| Baseline `cargo test -p vela_lsp_server --test stdio_transport --target-dir target/lsp-matrix-stdio --locked` | 240 s | 10 passed; 1.19 s. |
| Initial post Node command above (before review corrections) | 120 s | 331 passed, 0 failed/skipped; 14.29 s. Original 324 plus seven reader/provenance tests; editor-script tests were not selected. |
| Initial post LS command above | 600 s | 777 passed, 2 ignored, 0 failed; 314.49 s. |
| Initial post LSP command above | 600 s | 848 passed, 0 failed/ignored; 357.72 s. |
| Initial post stdio command above | 240 s | 16 passed; 0.81 s. Four actual subprocess tests plus shared support/oracle tests. |
| `cargo test -p vela_language_service -p vela_lsp_server --lib source_files_ --all-features --locked` | 180 s combined with stdio/check | Six new tests passed in **each** library after Clippy-message cleanup. |
| `cargo test -p vela_lsp_server --test stdio_transport --target-dir target/lsp-matrix-stdio --all-features --locked` | same finite 180 s group | 16 passed again; 0.83 s. Installed server target preserved. |
| `cargo clippy -p vela_language_service -p vela_lsp_server --tests --all-features --locked -- -D warnings` | 240 s | Initial run caught `unwrap_used` in the new test helper. Replaced with `expect`; rerun passed (3.84 s). No unrelated lint edits. |
| `rustfmt --edition 2024 --config skip_children=true --check tests/lsp_matrix/support/fixture_sources.rs tests/lsp_matrix/support/fixture_sources_tests.rs` | 120 s / subsequent 180 s group | Passed after formatting only these two new files. Existing support modules were not recursively formatted. |
| `node --check` on loader + 29 changed JS modules | 30 s | All 30 parsed. |
| Scoped `git diff --check`; `git check-attr text eol` | 10 s / 120 s group | Diff check passed; representative ordinary/missing-EOF sources report `text: set`, `eol: lf`. All 198 source bytes were read directly; none has CR requiring a mixed-EOL attribute exception. |
| Complete JSON/byte/marker and protected-input comparisons | 10 s | All passed; exact hashes above and source appendix below. |
| Review `node --test "scripts/lsp-matrix/*.test.js" "editors/vscode/scripts/*.test.js" "editors/vscode/test/*.test.js"` | 120 s | **340 passed**, 0 failed/skipped; 11.98 s. Initial 331 + one path-Unicode regression + all eight editor-script tests. |
| Review `cargo test -p vela_language_service -p vela_lsp_server --lib source_files_ --all-features --locked` | 240 s group with stdio/format check | **Seven passed in each library**, 0 failed/ignored; 0.14 s each. |
| Review `cargo test -p vela_lsp_server --test stdio_transport --target-dir target/lsp-matrix-stdio --all-features --locked` | same 240 s group | **17 passed**, 0 failed/ignored; 0.72 s. |
| Review scoped Clippy command above | 240 s | Passed; 21.13 s including build-lock wait. |
| Review Rust formatting check on `fixture_sources_tests.rs`; `node --check` on loader, reader tests and editor-script reader; scoped diff check | 240 s / 10 s | Passed. Initial formatting check requested only multiline formatting of the new assertion; formatted that one file, then ran Rust tests and rechecked. No workspace formatting. |

Cargo metadata reports **no feature definitions** in either owner crate. The
full owner runs use the normal repository feature graph; the focused reader and
stdio lanes plus Clippy were also run with `--all-features`. This does not claim
a full workspace all-feature run. The initial post full-owner runs preceded the
new tests' `unwrap`→`expect` cleanup and the two review corrections described
above. Full LS/LSP suites were **not** rerun during review; all seven resolver
parity tests were rerun in both libraries, together with full stdio and the
expanded Node command. These are not relabelled full-owner or installed-editor
results.

### Discovered identities

Compared the actual `cargo test -p <owner> --lib --locked -- --list` output before
and after (same target names). Sorted identity hashes use newline-separated
names with a trailing newline, excluding Cargo build output. No old identity
was removed or renamed. The review reran discovery: the same seven new support
tests are discovered under
`matrix_fixture::fixture_sources::tests` in each library and under
`stdio_fixture::fixture_sources::tests` in stdio.

| Target | Before / after discovered | Before SHA-256 | After SHA-256 |
|---|---:|---|---|
| `vela_language_service --lib` | 773 / 780 | `7f4f2b5eb038f324a781c225f7a4bcc814e6b53c1d5e73aa21b106d14f3c0638` | `e5eba48cf69813492b687ed5a30d4d4de7b2972d25e8d1fd09d067178abbb367` |
| `vela_lsp_server --lib` | 842 / 849 | `581f81af2b395658cb21180e7de715d3c04636b79bd14c31e7bebd9e4a1e7c24` | `f17112af01baba998614418f61ecc4e928ada029380cc480c8fa3beb57fe1268` |

New Rust test suffixes:

- `source_files_preserve_exact_utf8_and_markers_before_transforms`
- `source_files_reject_wrong_conflicting_unknown_missing_and_unsafe_references`
- `source_files_reject_invalid_utf8_in_documents_and_json`
- `source_files_reject_unpaired_path_surrogates_with_existing_replacement_file`
- `source_files_reject_escaping_and_internal_symlinks`
- `source_files_resolve_lifecycle_slots_without_interpreting_metadata`
- `source_files_resolve_every_authored_reference_without_changing_oracles`

These tests and `scripts/lsp-matrix/fixture-sources.test.js` cover the same exact
BOM/NUL/Unicode/mixed-EOL/missing-EOF source and independently expected marker
point (byte 18, line 1, UTF-16 character 0). Malformed byte controls include invalid
lead, overlong encoding, surrogate encoding and truncated multibyte input, for
both source and JSON decoding. Negative reference controls cover null/nonobject,
array/empty object, nonstring path, unknown/conflicting keys, absent target,
directory target, absolute/drive/UNC/backslash/dot/traversal/empty/NUL paths,
unpaired path surrogates with an existing replacement-character file, and
escaping/internal/directory symlinks. Lifecycle nulls and untouched non-source
metadata have positive controls. JS additionally checks fresh-load isolation,
no orphan sources, no identical inline shadows and raw provenance mutation.
Symlink controls run on macOS here; Windows privileges/behavior were not tested
and are not silently claimed as passed.

No installed-editor recapture, native UI input/render, browser, Windows,
workspace-wide test/format or isolated package gate was executed. Existing local
library ignores remain ignores. These limits do not block the byte-preserving
organization handoff, but cannot be promoted to acceptance of external gates.

## Exact migrated JS reader paths

- `editors/vscode/scripts/check-removed-consumers.test.js`
- `scripts/lsp-matrix/document-symbol-oracle.test.js`
- `scripts/lsp-matrix/folding-editor.test.js`
- `scripts/lsp-matrix/outline-contracts.js`
- `scripts/lsp-matrix/selection-bodies.test.js`
- `scripts/lsp-matrix/selection-calls.test.js`
- `scripts/lsp-matrix/selection-editor-oracle.js`
- `scripts/lsp-matrix/selection-input-contracts.js`
- `scripts/lsp-matrix/selection-literals.test.js`
- `scripts/lsp-matrix/selection-members.test.js`
- `scripts/lsp-matrix/selection-pattern-recovery.test.js`
- `scripts/lsp-matrix/selection-patterns.test.js`
- `scripts/lsp-matrix/selection-types.test.js`
- `scripts/lsp-matrix/semantic-token-oracle.js`
- `scripts/lsp-matrix/token-render-contracts.js`
- `scripts/lsp-matrix/workspace-picker-contracts.js`
- `scripts/lsp-matrix/workspace-picker-contracts.test.js`
- `scripts/lsp-matrix/workspace-symbol-fixture.test.js`
- `scripts/lsp-matrix/workspace-symbol-lifecycle.test.js`
- `scripts/lsp-matrix/workspace-symbol-oracle.test.js`
- `scripts/lsp-matrix/workspace-symbol-ownership.test.js`
- `scripts/lsp-matrix/workspace-symbol-states.test.js`
- `scripts/lsp-matrix/workspace-symbol-types.test.js`
- `scripts/lsp-matrix/workspace-symbol-worker.test.js`
- `editors/vscode/test/document-symbol-provider.js`
- `editors/vscode/test/input/run.js`
- `editors/vscode/test/run.js`
- `editors/vscode/test/signature-provider.js`
- `editors/vscode/test/symbol-schema.test.js`
- `editors/vscode/test/workspace-symbol-provider.js`

## Changed JSON raw hashes

Paths are relative to `tests/lsp_matrix/fixtures/`. All resolved trees
are unchanged; these differing hashes describe raw storage, not semantic changes.

| JSON fixture | References | Before raw SHA-256 | After raw SHA-256 |
|---|---:|---|---|
| `code-action-import-partitions.json` | 1 | `29a61027d2c84dc1eb7f2f836b4826b4636b0e0ec7bbe8de69224bccc67c8c30` | `b5b59e5bbde4ca8e1c0a91286622cce17db2fb7d8e3bcadf2b1c1adc9c7afb6c` |
| `completion-abrupt-flow.json` | 3 | `69122458bef0f70da4ac8b8c92876e7ee482986c8801d831c98a3acd675c2a37` | `4f8d61192c3f8008f847e90ae3073783566c27d4878ca5ffe46d6c3b83e4b763` |
| `completion-local-exits.json` | 3 | `78539d7976d9e3579b4f36a2f09ba4e1f62f93b0057e9d9e6feb555c77efcdc0` | `75331cbe0a45b866fa95254bacff9829488a1550cec1cbfc88d9f2bca12895c7` |
| `completion-loop-match-flow.json` | 3 | `e3ebe72561b5c9759e499752c28da6961368f14fa0b4234efba7e4372e667b82` | `69981d32a60bfde0ad397c480638e8e5e47be299e1d94c0a39383afdfcea55a7` |
| `completion-package-members.json` | 3 | `cefe824058e1e984861d232a05f6d9b9244a53acf829b5bff0c33e1a314949fc` | `4083e4df90437651090dbca0b8754aa91f66a9ceb8c169d4ea6be589d5d5132f` |
| `completion-receiver-assignments.json` | 3 | `334643bf9363af65abc40522c28b353475caa7b3743f45fd41bf3c9bd797ffae` | `f9a0c4d52110d9b2156ace9b8a25b04160cae60e9e2da48f429f74be9cc828eb` |
| `completion-return-flow.json` | 3 | `20684d078dc3c2c402ab7e327191b53f571078d643d4273f54178c0075976379` | `0a3d87264044e60f348a4ffda9784a5fc1d9d7d9265fb2ee7e41d72951e58c9b` |
| `completion-shared-methods.json` | 3 | `c6b2f4b1cadf22ce58b779e528b9e53297a55b27cfb4ef71797ff87e6f58f95e` | `585b51aece3ac73d518ba56f280455736673aa04043860bb6ac370d4c05cb629` |
| `completion-source-callable-lifecycle.json` | 14 | `1eb14f99c9754e681db1c6161262663b79c992989146161be0399f4e986acc02` | `e19f978dd283035c6e14dbf740a4a2a3fd80093b5a643967f46265680473e16b` |
| `completion-source-overlay-lifecycle.json` | 63 | `4982dfaf36bd77071fb17bd2fbbf7df13feda422bda90cff56549c59f0a1c8ab` | `c2180209803209a299b06953ef8295cf41d5e052d21dfb00dbe3f8b0a0c8a8db` |
| `completion-source-recovery.json` | 29 | `1d70c8105865bafa3d36a6202fce3cf6ffcc29d440d32cf4b2b239574947e08e` | `df5a7d7a08f473f7947f6d7c6db2d2ef949cc6e99c8dfad0bb59c1cb76d92cdf` |
| `diagnostic-action-control-flow.json` | 1 | `33539115b9591f6144b57c6d727e3fa2d7e0410e48af748e4d05f15c16b7b0cb` | `6e88f21df3e6796cfca33e8df5eea56952696f6ad7656504824c51e0c05dc486` |
| `diagnostic-action-members-constructors.json` | 1 | `fbd466148d8ef4f971f496e535115b005cba0c2f2f23460bd2541cea0ee41667` | `e8a382eefbafbbc022f80748548856ea5b401f4b984170d2bb3dbcafda59227d` |
| `diagnostic-top-level-declarations.json` | 1 | `6d17fdeeef5575963fcd20341bc07fd67c7645c3fecb563e782e32d191877a85` | `db6342fc54ac5fb5fce2c96b351b624d13f7c2322f58a2623a4788e81d8ae710` |
| `diagnostic-type-positions.json` | 2 | `be3963c788da2c34a7093f5fb513464038091845c4d6a86dc454c9f2c3a46f67` | `0c7a331e4dad90739579fdc89f6e6650c2cb4c200627904cd5e2b84ff450e6c2` |
| `document-change.json` | 7 | `74247e8729eb7ba3466b27197a15752a9659b03a4310ed7437025dfef48b41ab` | `2adbdd11c2c8b8610fa4e8210553dd48d80c2384e6b6796770775abe288e78cc` |
| `document-open.json` | 1 | `2bc13194eb842cc31547bc2a11b626881b846b1eed6b4dc9f55e5e3f20a1767c` | `eb609586f9ac0e13eae5bec981fd0c1f28c7c793031026a8535bf7d0fc0244b8` |
| `document-symbol-declarations.json` | 1 | `6fd027a09fb94ba3a359357076b06cdffa1770fab1b97bf6b83734bc3f734663` | `34aafc1a552b2b40b94cb37d9731c992da7b45b431b5e59cbca3ef8109bb263f` |
| `document-symbol-lifecycle.json` | 7 | `1f347c1a5a3ca7c0761681a7f49117a03b850dfc9149d9e5c5f28432b81125bb` | `b789283e54c3b5417639fd5bea0dde6561d1d51c1c09904fff8056177d498fa4` |
| `document-symbol-ownership.json` | 3 | `c961c04d5d98dfffddef12a98b8aaac17d4ec1b9c2f01bc81a1292f2aeb5d852` | `bbbac59b754c7cbff3a164589b29ddc9ff346eebc256fed500b1c73f7829ea09` |
| `hover-lifecycle.json` | 14 | `f692a324f48b96994b421f30092416bb91b6656b57d1bf9126c16f2c45db5783` | `41232cbc5862e316a1e8a0331eca97305488b8ccbb97752a300863207a40a1f3` |
| `hover-s1.json` | 3 | `a6940089c17893c787965c328a383ec4baeb6f11b1b898f237d87bba8f1b2c43` | `155c7d9ea75cc170281cc66691bfc6dff4a5d4d595cf705079ea33ef383fa4f9` |
| `hover-s10.json` | 2 | `633e2e776862feb6b538c87f92d980dd9db6e5f3ad26ec0b06b1c38517cf88c6` | `cad40024990f6cad9a0df33207f88b04a25de1499c944d3cc92528ba552b8dcd` |
| `hover-s2.json` | 2 | `92896db464b9c5fc62e1761c93a6a47d14465cb04142d7ce107ac403e207f8ec` | `8f0282df9ebbb9d1a9a72f95a30bf8473c49a0c6f3e40ed32b049ba5d03f33c9` |
| `hover-s4.json` | 2 | `f87b82ff4e0ef5d7c49f88816224700a4a1cf500aa805f82779f4e63738d2757` | `086aa536db2efdeb007f56bdb3fa79207589b6293c8a733b64122a20eb3a05f2` |
| `hover-s5.json` | 2 | `74c42c12689a9ef9ad55227ed298f665ca6723a03e2a3c71c13a72a41e9e7194` | `14944d9ea97ba0d5b1decc4f127ef03c246d2ec3e8452d76b42981cf08f301bf` |
| `hover-s6.json` | 1 | `13fdfe05475170427f8d41507ab07df95e343bbd697129e84cd83a6a6fcca1ab` | `344fa76c92e4b48964be81342dcc9ab6956cc9195e8df9cfb79ea87356e421a8` |
| `hover-s7.json` | 1 | `8f740d6951e0a50540ba156fad17491b0d5a8d66cb54b53640697b3b2d611b27` | `dc3460b053daca711aae3a8ce02f2404729d497ac4b71d37d5a1fe6ad1b56f59` |
| `hover-s8-package.json` | 2 | `c402f8271a99b05b020264a40659919379c7b11c50399bba6577e27f943b7f46` | `77c3050d79ba0901f21983346e683858adc591eba592dad048e76cb8919abb0f` |
| `hover-s8.json` | 1 | `647db8caec18296fa580cf9cb0a904915afde98d942a9be4efa01bbcf4d5041e` | `7554f0a27271ff661a397cd2eacdb12a3976810a66b852c2c8627dc504e39742` |
| `hover-signature-coordinates.json` | 1 | `36443f43f18f9c5800c6a685e733c7835c6986fb2c199c2411470fde96e829c9` | `1c791283d493849cbb2aa682bcb5d7cfa16d01923c26a74f26789d4a40a6c563` |
| `input-semantic-tokens.json` | 3 | `02b5dc8ad8e75bc636fe0e8160edc90bb33d93a04821c2929339449dc5090c21` | `94d19608bb4ac5d2abd2f3d00e27e9c58a0fbfb5ecfa02f422a83c7ced7da589` |
| `navigation-calls.json` | 1 | `4328817b44ff8002314ce1e61c6a00316e0d21bf00f1e9528e8333d0c1e5463c` | `7ef6dd7ebe439c2ff5b4d4c17751d25a656fbd5618a5a5b49dbb9a86f4cab44f` |
| `navigation-constructors.json` | 1 | `e8f54bbbe258bec19420079c978424e666a73b24c1643fc7f4233dc14641b9a7` | `2d597e9dd2f750b18761529fe0be2af333c5c78f417f5f7b3891bcb6d1fa9654` |
| `navigation-declarations.json` | 1 | `ac944af14be66e9849c6455b35023c08f360698d7ec305649da9c3334413859f` | `ad0593c6ccb22f907b08e8c71c71f110267c9d0cdc5a7b7c1fe6b55c5072f3d0` |
| `navigation-dynamic.json` | 1 | `16974273aabfb749a5e69f3261d7417189ffc86d1754085b18c03ade831ef75a` | `25645a2f2f9aedef3e9f9ff37e0de6ce9c9c187045cfb8f44b88010ca3c7d329` |
| `navigation-imports.json` | 1 | `05e82448ac25b3002f888065aaa87f1bcb79e8ef30a7f4d32d480a59bae1a7f8` | `f539629ce3cfc5c332c8e1d15907dce3167cd6255026fbeb82a5841d888ada56` |
| `navigation-members.json` | 1 | `50789f035c3b7c3e89e6c1928389ea2d213d903d9b186325d37f2987ddd57083` | `20e2c24d41a0fa409b0cbf7e733e8e652075922bb182d00fed8a8b08cb5315bd` |
| `navigation-namespaces.json` | 1 | `bf132641667826f5afcbc74ad80be99783b768f4352c5b693f6ea645e44d9608` | `96d7868fe108f090195f0f42666efedea5f295f3af1b0e67d124ee62c787c8b0` |
| `navigation-owned-declarations.json` | 1 | `63ebbf64813431b574a6e496b7b11c722c0af746916b90998afc02aed9549c41` | `6fa615c2512ce4fbcafbb7c562c136d44b7a78ce71cf6d029d8873d6ed541c08` |
| `navigation-type-positions.json` | 1 | `f45c2352348b7e666d9afc0d1519c00f66558dae09e7b2c98e204e81ed910da9` | `7e85d9194267898276c9aba59fc7bf849258700a7a0bd48462983616ba592188` |
| `reference-rename-body-scopes.json` | 1 | `d3a60e2929a99e7764fe487c3ca1c5ae3463db6cf1d225be734ca60e8622185a` | `95b56b5987251ddd00906f5b0c258d94bf97bb3fe3b04801c2630b211d438657` |
| `reference-rename-control-flow.json` | 1 | `966bb2bdebbc5d7206f5906205010b850f678dd28818f6b7a33f8b427ba39f5f` | `044f81b0f2ed4af46df6b1c15b6b12181ad0bbdfbcbe488d013fe1d68e588540` |
| `reference-rename-coordinates.json` | 1 | `0e15520e4cbd192564a7695026a472b893fec4d88711dc81678511ee63357ef1` | `e8d087974591aa8bc5724cf81e51294f4f0b2ac41c21b2b307dfa4ffd5b8be7a` |
| `reference-rename-default-bindings.json` | 1 | `48aeaa19a7c0922feb1bc9c474d7f4f1a72f2ac8f29afed0511db816df3051fe` | `b3e33a7cf10a4c88f7406b552ba514de40fb02aea90831f53258260c3779c207` |
| `reference-rename-imported-types.json` | 2 | `841429a9b1a4f3cf7395e339f3f954c17f7d7ac23df456522590fe0ad39d199f` | `47bae89ee0062aa3b5898c82c22acf6dfd8dbcf25916482e197ebc99bf34fff2` |
| `reference-rename-imported-values.json` | 1 | `af61a0e4a68bbbe195a17e0a45e4033d4e083805256895d9e4437283be3220fa` | `ac77aa9a03b18bed4b3d62470711220da116186d752261c522faa4b38ec7ecdb` |
| `reference-rename-method-parameters.json` | 1 | `1f6d6f40dd8957648a628ef8aeff07c039d8061d09111fec3ec273b2a370d91e` | `76f5bae8ccb7a083643260a3c6cd1da9058c757cda48b86cd2a978f193975911` |
| `reference-rename-named-parameters.json` | 1 | `27a13a9e051a56bc4409e6bba14271eccb27e4d6e710f7f637698d0f320c3563` | `e4ba73acc5ac0296cb46728ed013340491ee81c9ff9e2b6325b1868f3b9c7e03` |
| `reference-rename-private-variant-imports.json` | 3 | `c585444ca7ad5f4c0d344dd0a598742a837159261857ca7d1ec976fe39eb8d14` | `59bc1ecf648f98e191e78435cd66f9e3d2f331f75cb6a029b5a8b7ba56a18373` |
| `reference-rename-record-fields.json` | 1 | `79deb2830815221f77ec597b3d9c68c967f67a71758e4cef62b4acefd1f0ce0b` | `ad22c827c42b17ec3394933cc777d057ac6401c78ad688fb234ad3c121d6dd96` |
| `reference-rename-required-parameters.json` | 1 | `e708207c5a2cd9b9dc0c54b20e2d12b99e5b58ccf7cbb4eaf7ea6ad98e754e6d` | `ac7834ffdc4722b03d1983ca7287396d7b750e4f01b8bf7e5085510850e69406` |
| `reference-rename-schema-capture.json` | 1 | `d1721b6629ca579e7414eb3d2ff78487718faf772c57091e144f915291cff5b8` | `52e64769d93ed8f4f22847a54f6d285a4fd2e7297517b20d1d86601106552dee` |
| `reference-rename-schema-fields.json` | 1 | `8c99e55216041e1e55c7b4a0af3fe929b42412dc5a5aa0891fe1393fbe0f63e7` | `68f54f468500b163b7ba46e102fd24a680134d3fe7819853eb5a5bb1e0e25ff0` |
| `reference-rename-schema-functions.json` | 1 | `6d7bfea10fb172de55d159399db19d64f494e809bcba125a5fc2066098269f75` | `6d6f326559904aafe2f536c848fc028dd7595474863071e2229b5bace87663e4` |
| `reference-rename-schema-imports.json` | 1 | `044b403e68930da5ff1f899c2219b5781202479de63a43c9008f81cca6f0906f` | `2e815a265b53327004f5d884dbfc7a17973d1d66a75cd57633884c8cba301e1f` |
| `reference-rename-schema-methods.json` | 1 | `4f8cbe2d17ff0954c854dd2c590421161ac454c5d219a608655fc2b5b381e356` | `848b0873769aac9c0f566917aa95e776c7f4034df00e73c7d01f03414d16adbd` |
| `reference-rename-schema-type-imports.json` | 1 | `60bf40edc2a2391f574af7edb8adfc021f561591aaee63c1f693ac4a00f965d7` | `c893b2189b82c1d338f75d1d6489dbe831595cd13f6a693373e74c0ae42dd076` |
| `reference-rename-schema-variant-ambiguity.json` | 1 | `1cd32beae08c3c14680d5b72e57b23480a644c720c481f847f012585fdfa8431` | `3cf7517ab3828176d2dc70f0ed2b35e8a2a6c691c103c2f08faa9422cf5fb58e` |
| `reference-rename-schema-variant-capture.json` | 1 | `2365ce607b7ef7565301260ac79fa755f9984c00d9639fcbc80cb3b4b7a0d68d` | `650cfa61ab3e2499b8435887031da996568cac4a24c5a65c74a7ac3ba726d36f` |
| `reference-rename-schema-variant-imports.json` | 1 | `b11d8e4ad7c25ada1582efccc19ef4cf3ff15d2a30b82f6e11cdfc8d41b68d0e` | `bd1e858528372736448efa783abe89c150d9a9fe2dd96b9f3da1a8a0017cdb68` |
| `reference-rename-schema-variant-lookup.json` | 1 | `f1888d59b36fe68796e657b4b08ed7591c5d78bdf5578b54a28d300a630c931b` | `60cb832856675b0f4a30be600740c7c4e180a130bd8aad8012a0c6786fe0354e` |
| `reference-rename-source-variant-imports.json` | 2 | `954141468c96a7f3ed979eae6effc2807ee2d8c19ce2d79aa9a648ff83780196` | `0b140ffda27e83545f58073ec7390679528dc7d35f83687485a1835c96e566fe` |
| `reference-rename-tuple-fields.json` | 1 | `a66a4e447ffec105673fb18915695374cfd3db306e3734798eff2021a40ec389` | `bd77903ed90d594e44456dc7f3b7d8f278fc0b82c55185ac21e342f98cc4eb5c` |
| `reference-rename-variant-ambiguity-removal.json` | 3 | `02a13fab0895429df11370157affda40eb48f910e5b4a06d009b62a44f7998aa` | `798021e1a9c344329b7bcf7b09545774302682f716c532e6e5333d91399a1a7e` |
| `reference-rename-variant-fields.json` | 1 | `6b764210d00ee71cbafd42ea4d02f72f0597dbbe2cbab6396e0b9be20e8e3386` | `394d2b2907dbe8579b3e77076ecb23af402ed2304b1f8461683efadd0767c216` |
| `selection-bodies.json` | 3 | `b18b9f5c6f85827375ebc46cc3e289b7d1636d233d3e2018fbb6be90f804383b` | `a1cc4ed0bb40ba2158f0c5154f560c83da2e1afafd077a832c04edabf1860229` |
| `selection-calls.json` | 2 | `7870ca5fc5caa158411fbb3ba4e192cd8b859f250183878f74dacd9c6d489655` | `4f70a6903502656141b34e3c30e93dd21e9fa8336754c9c80fbac3a36b1ece10` |
| `selection-literals.json` | 3 | `cf2f7e8fb8afbf65345d3d89274138e23d61be878d4f35e4bd106c4c859c1fbc` | `1630cdcca37cb64a97278b61e53e5aad1ea74f1545bf4d434b2e983d7eb07502` |
| `selection-members.json` | 3 | `209917873d2c5b0739243431fa3f3f6b9fc4db38cd7a877a8614822c1d3faeee` | `165cbd8c65c62937befd324bce82d2fedbd8c17825d4d01377a5aa18a51ffe64` |
| `selection-patterns.json` | 3 | `0d6973649c377143f4cd8e9b25978b8780b5c5f8d255a2b117dea3fb866f95b8` | `ac41c174dcdcbda439bbb1950ad7c09617687d9afa07dd77f247b6d75fb239e1` |
| `selection-types.json` | 2 | `d5fd4a311c5dc21a620b73afb19343eb88a0fa9ec3580f7b63730d2c5077a2d3` | `e4d41f6c7c964aa70f082f84e5d6690df68d8eba3f1454f1bceae60b06f8a65c` |
| `semantic-token-bodies.json` | 2 | `f84435ef01d48a579f7f66967395ea62ad25b430e87efc145d20203801d02b88` | `9533d47e7fdf71b7bdae6de1652621039971c98971e1353d1a04f77f4f2ea5b9` |
| `semantic-token-calls.json` | 2 | `15e9f681b8e7c0c7cfdc561c6f97526da02431c89dfa067ed667654d87c85eeb` | `2390d14e034f0c18d77756bc0a5985985738147b7490d3c994b1631679e1cbd2` |
| `semantic-token-control.json` | 2 | `94f810fa46f96c836cb01b4bda0f0da0bc44590b13481baba8c52e393d157444` | `0cafa40623bbde094ca0988ecc07114217e28f64720fec5602d6e9649a818870` |
| `semantic-token-editor.json` | 4 | `a13a4541ba16f21df425611ed48736f509c179ee3be57a9ff4e675d4b7722acd` | `50730eab8ff3c715b2776214d7fdcf4a2290294ae4c4b58f0337bb7227dc5dea` |
| `semantic-token-imports.json` | 2 | `de29b188511d37f11318ea11fcd025e353b7c86eb9e64718e930c36e5a765bcc` | `e8503cb82ed47dfc7585c618077481c5c4b829d1e4e87aaf2207c8a8e3703fd8` |
| `semantic-token-incremental.json` | 38 | `020f89b108d61e1ea93d4619733c0f5135f6d7431f14e23989146f1f6ecbd5b6` | `3fb9880934fe75bda1f78c116f1b19d068d3f2f40615d42a23854625db8f3e8f` |
| `semantic-token-literals.json` | 2 | `866146257bcbea8afa2b1f388402e5447063c817a84a8b95476ca56ba80540ea` | `37fd73d8e51fd196c3587ff34cad08e27b883ca4ff142041d34b62e84c8fcedb` |
| `semantic-token-members.json` | 2 | `e952301eb6a4ace893741e9db09e2f6d50165cb5cb34077d9886111c726eea49` | `316c1cf354fb8fb0118c53b2a457b738902e5d62e28695d4b1ee67426f39eb76` |
| `semantic-token-ownership.json` | 4 | `d4409ef2cc3f9edcf6a55e307767854b40d71bb9f00d3bca7857e72a66c70fd1` | `d38c5322430b245bf5c189f0c0ff3b19aa9b616ff9a07f4f09474775c0a4aeec` |
| `semantic-token-recovery.json` | 25 | `a613da704fc5d36589599fa8e5ffd02c9e7252b316b6f7b8860fc2f56548f248` | `ec3c2205897b393e68b61911e524a0706158a27df4a7ccea6dcdf41156e94deb` |
| `semantic-token-schema-lifecycle.json` | 71 | `10dd243ecada3ad34d61b3864df80b32b5b38583f72ccef0863d42c9b4c7614d` | `030c9b4352ecbfec1df098f1f8005f17895c6135c7febf1e49a0560c8748bbbe` |
| `semantic-token-source-lifecycle.json` | 85 | `4cc7e50ec91a7f51e9f89275de7671578989673020e11eb1c90a70ca4e9dc765` | `9383bcd2f926ce108d33f3cab2ea90d9b4dde70666a162d8d785c594d546efe9` |
| `semantic-token-top-level.json` | 2 | `df20c5eec7f9450ef21b63eac60951d73cb38b076e17c73b9d36e3c767d821cd` | `b47502e0cb0163b4520954901ee7dbb1758a625e93198b2419959cefb0ed820a` |
| `semantic-token-type-positions.json` | 2 | `555bc165581940fe64187eec2201ee46f6d6719656452bda87a58e69f4103aef` | `300f5609de399ac7d17f9ee73d579f1b7bfd7678b22eb99df36d2e5d95361cef` |
| `signature-editor.json` | 2 | `2475e24ca8107ca6fb2fd655ea1ab537aee5d5ea97e8c96c934d227a40c06f6c` | `cbcfb6f7f25d0fdc00bd0a7812d36d1841934db97d4e47082549e53e7ea4de8a` |
| `signature-s3.json` | 1 | `69cd7921b9fc52602282dfd54f0efc62fcc6f664bc90b3efaa8d0eae7ce9aee6` | `d449b3aa69523801e6c3a0bb5e982d6fba98b1446db803d9f4c67a088f66b828` |
| `signature-s5.json` | 1 | `d94207089f0c302f12cc167acc20399a680f05814b2fc9910a253562c3c9dc04` | `81c3914cdfd178bfb02cb7777be6f1ae66e8ac6016be721e630d5aaea0bf9085` |
| `signature-s8.json` | 12 | `fc140fc32c972d73e7654e551a62939403d3bc1fbf949e005be42b0c6c4391d4` | `a54287854c813f545db8f455639508cc3fea3a6422abbc61317bbd994504b9c9` |
| `workspace-symbol-declarations.json` | 1 | `31099bce72dc13df75ff9ecf30954445d29eca9d00f45ab656ae57f7aaa0658b` | `85da6412323e656b2a6697be131e7506f791fe434ae140cc33d21b9bca5c8f75` |
| `workspace-symbol-lifecycle.json` | 9 | `1881229de3aa1089c8d48e470d50a521f23adc4b8031af4d14201730b09e98ed` | `bdf80181aff2dfafdaf49d21a0b2e0448ba1493ab35ffcb856ccda0ab3f13011` |
| `workspace-symbol-ownership.json` | 3 | `3b76b1792793f2abe8b177175fbeef7fa00bdd52775bbcde6d6ad79e1f89ed00` | `53ea3e6d1a04d74b3ea23b3616f9b0cc14eca05a691352886e9df51e7bf26a70` |
| `workspace-symbol-type-positions.json` | 1 | `26c3d697b2531e1336daf0f15ab06ae31e672ba6c1dd7f256c55f4bdefd67291` | `6f1a40f951333aaf7195cc1eaefeffbff5ed8082ceadea03ddf863f06ebacbee` |

## Exact source-byte ledger

Paths are relative to `tests/lsp_matrix/fixtures/`. For every row, the old
SHA-256 of the evaluated marked source string **equals** the new SHA-256 of the
raw file bytes shown. Uses count all authored JSON references, not test executions.
No hash is a generated provider expectation. This is a review ledger only, not
an executable catalog or runtime source-discovery input.

| Authored source path | Bytes | Uses | Old = new SHA-256 |
|---|---:|---:|---|
| `sources/completion/completion-abrupt-flow-files-deps-external-scripts-api.vela` | 703 | 2 | `fe07fd7382b200c90e7c92c7c25d85dc974a65b312ad1d83d87f681a3734e12a` |
| `sources/completion/completion-abrupt-flow-files-deps-hidden-scripts-api.vela` | 739 | 2 | `a18a3fc548f98305400258cb20cc7b5278a810e1de75a205a6969da4f0234b91` |
| `sources/completion/completion-abrupt-flow-files-scripts-api.vela` | 684 | 2 | `36e8da4a0e750a613ff8025d3f4dedbc2f0ff0313898f3c468bcba9171a6216d` |
| `sources/completion/completion-loop-match-flow-files-deps-external-scripts-api.vela` | 387 | 2 | `6f8258916e75c96b56ff9fd7db80f994e67a52e6b9f543762ebc67189b34b048` |
| `sources/completion/completion-loop-match-flow-files-deps-hidden-scripts-api.vela` | 408 | 2 | `c08fb09d268373285296c99d096eab1d9ddc28c3478b2bd733fad5d88bb54520` |
| `sources/completion/completion-loop-match-flow-files-scripts-api.vela` | 471 | 2 | `f0f7288a803e0f2748002e0aefccda9062e561e42120719ae2d676db5c17dbc2` |
| `sources/completion/completion-package-members-files-deps-external-scripts-api.vela` | 577 | 1 | `fb5b6f5cc76f9f843a50ad6c24ff11f2129fa67eba79a3d9f203ea09547154dc` |
| `sources/completion/completion-package-members-files-scripts-api.vela` | 566 | 1 | `2b64b6d928d2e70191b7b73b158d85bfe308f3e38eb641d1bf0254d08bb3e3c4` |
| `sources/completion/completion-receiver-assignments-files-deps-external-scripts-api.vela` | 376 | 1 | `4ba5e51a0cbbb7204a82a5e644351b61c8c5745b9dc52ed2766ad0bb0de00557` |
| `sources/completion/completion-receiver-assignments-files-deps-hidden-scripts-api.vela` | 407 | 1 | `0dfeb3613f41ac3269c0b3dc2e7970c8a7eae8c1fed2a4d6cef040f23ad98f43` |
| `sources/completion/completion-receiver-assignments-files-scripts-api.vela` | 371 | 1 | `0d1030b9b2d17a4e6e80d42ca0a52d4680f5de7b678f7f6278b4d3bacebd8eee` |
| `sources/completion/completion-return-flow-files-deps-external-scripts-api.vela` | 705 | 1 | `130f3874a89accfcbc57598adf884686a6a6c8c141fa2138d3cf7434b160f5a7` |
| `sources/completion/completion-return-flow-files-deps-hidden-scripts-api.vela` | 741 | 1 | `92b566381fed12314c6606e7622e4cb3cc18534489b84765dbbad0e815ec1d90` |
| `sources/completion/completion-return-flow-files-scripts-api.vela` | 686 | 1 | `224847dfedb41c3a6d043dfbe5eb754ec827a4977d4830e6c1bfb93437fd232d` |
| `sources/completion/completion-source-callable-lifecycle-files-deps-external-scripts-api.vela` | 703 | 34 | `e5cb50295e13f089f223a36007160ca7d706d931b83f1aa1434f3ed7e3e03b6a` |
| `sources/completion/completion-source-callable-lifecycle-files-deps-hidden-scripts-api.vela` | 613 | 4 | `6b8a03c6c88a56d2f52027bfb806d3437a07edc80ff201c948ec69dac0775f2e` |
| `sources/completion/completion-source-callable-lifecycle-files-scripts-api.vela` | 687 | 28 | `0b93caa77d8369062f33fb8f087fc680dd8aa59bf5d253d95f46d8a8d8e73c20` |
| `sources/completion/completion-source-callable-lifecycle-oracle-sourceLifecycle-dependency_changed-files-deps-external-scripts-api.vela` | 800 | 12 | `621bbe3c0e9f24e1c7545fa07c800e1b2ce1972c677af9422cd71ffe797761cf` |
| `sources/completion/completion-source-callable-lifecycle-oracle-sourceLifecycle-dependency_recreated-files-deps-external-scripts-api.vela` | 734 | 17 | `bdd4c977c18e213573c383c0274120094d1160ac338e1c3e2e58c7aacf81eb72` |
| `sources/completion/completion-source-callable-lifecycle-oracle-sourceLifecycle-root_changed-files-scripts-api.vela` | 772 | 3 | `4a0d375bfcf87f3f768e88bf1e9ad5d23f8268ce8f96ed1cd6dd7979520a99dd` |
| `sources/completion/completion-source-recovery-oracle-sourceLifecycle-broken_neighbor-files-deps-external-scripts-api.vela` | 721 | 6 | `5890f00f4e24ffb3c25efbc6a83ddeb5a94ec66024eebb11dc86978a2b9070a9` |
| `sources/completion/completion-source-recovery-oracle-sourceLifecycle-repair_neighbor-files-deps-external-scripts-api.vela` | 745 | 3 | `b71531f520e30a42650071a6a453ee22e5b4ea9e3e149fab856e519350a9141f` |
| `sources/diagnostics/code-action-import-partitions-files-scripts-game-main.vela` | 1570 | 1 | `b3941822e12881cabb7f09e8afd595ceb21616cbc48beb82c799b1d91e7be8c7` |
| `sources/diagnostics/diagnostic-action-control-flow-files-scripts-invalid.vela` | 807 | 1 | `0dd0557f5bc83dbb5f6ccd3896596500d51af17838d1dfb7e8ac4e919a2fea46` |
| `sources/diagnostics/diagnostic-action-members-constructors-files-scripts-invalid.vela` | 824 | 1 | `695a36f55c6037721560ee375c4dea9fcdae4b3d491ffd891ca7bb63df23cebe` |
| `sources/diagnostics/diagnostic-top-level-declarations-files-scripts-invalid.vela` | 857 | 4 | `c773ea8f2fe295fa0061b047739b6c19259c3a8019481b05a8f3647bb9ac53a6` |
| `sources/diagnostics/diagnostic-type-positions-files-scripts-invalid.vela` | 926 | 1 | `21e0cd3c3172accc78b3e4d5ce247c0b366cd305b95006f07e09f20847889ba0` |
| `sources/diagnostics/diagnostic-type-positions-files-scripts-valid.vela` | 810 | 1 | `0056801557dd3bf5aa9f06a7b7664a3084ef1de821a202851c41fb0f1c3c9648` |
| `sources/diagnostics/document-change-oracle-cases-15-steps-0-text.vela` | 1205 | 1 | `87d05b8f03118c3d6e9a293433edaf8637eb4612569917d92ca69e553615b1d5` |
| `sources/diagnostics/document-change-oracle-cases-15-steps-1-text.vela` | 1211 | 1 | `179b97fce8754e7052ff708f342045699eaa0589ea5c3169791c346eee77f467` |
| `sources/diagnostics/document-change-oracle-cases-2-steps-0-text.vela` | 877 | 1 | `b0ea5fcc9f944feddda186a52790927260faaae99e0bf512d4e6c9c638496073` |
| `sources/hover/hover-lifecycle-files-scripts-api.vela` | 679 | 4 | `d713afb929f9e23dd6e42ac272584dfc87f8998e1267333e428281028356aea3` |
| `sources/hover/hover-lifecycle-files-scripts-main.vela` | 1979 | 1 | `8dab090d3da26bd51b530f8ab355a459eff23b8c617d09291a4fdbab49427bb9` |
| `sources/hover/hover-lifecycle-oracle-phases-definition-body-edit-actions-0-source.vela` | 681 | 1 | `2399efc2b3287c0499952ca37e91c178fcc30beae5204836d5554f892ac043bc` |
| `sources/hover/hover-lifecycle-oracle-phases-definition-recovery-actions-0-source.vela` | 724 | 1 | `88963e893f9a514475b162b25bdcc544755da19674ef7cdeb04f03183dcbb703` |
| `sources/hover/hover-lifecycle-oracle-phases-definition-return-any-actions-0-source.vela` | 679 | 1 | `f6cc389fe9b582bcba2e665047f485369cc4699b2f81a1a9f10a21e80a6a203f` |
| `sources/hover/hover-lifecycle-oracle-phases-definition-signature-docs-actions-0-source.vela` | 690 | 1 | `696fb2cc70b8152e3171c2e2a8c0de0a7ef036584bb18bb67a6f431c5e462715` |
| `sources/hover/hover-lifecycle-oracle-phases-dirty-caller-shift-actions-0-source.vela` | 2002 | 2 | `fac19fac2d1526080d59b4adc376c35c968e5f9436129893e25440920aba64d6` |
| `sources/hover/hover-lifecycle-oracle-phases-dirty-definition-again-actions-0-source.vela` | 694 | 1 | `c862bfd4b6d05310d18a4d77009cf9f54ef716665ef63716e569921adc10130b` |
| `sources/hover/hover-lifecycle-oracle-phases-disk-write-behind-overlay-actions-0-source.vela` | 652 | 1 | `5c45023420830b2ed1670188597a74302f7e2304073afddf159bee1c7e85ce9f` |
| `sources/hover/hover-lifecycle-oracle-phases-unresolved-caller-actions-0-source.vela` | 2004 | 1 | `106e9098fbbc98862a96ede9c61e199f9d12c844a8d7d7cb6af87d9dece5118c` |
| `sources/hover/hover-s1-files-scripts-api.vela` | 3425 | 1 | `8d68272840e4f91efe7deda856756611e0965bf2769ef57b66f6017fd4a5625d` |
| `sources/hover/hover-s1-files-scripts-main.vela` | 1480 | 1 | `977164d9697f9d5464c01b892f61c197d3a8162ba7c0fed7050903ba1f415670` |
| `sources/hover/hover-s1-files-scripts-negative.vela` | 1255 | 1 | `65f7e5cd14595d619ace39e9b993b049295dc62ff16843fefb78691525554e8c` |
| `sources/hover/hover-s10-files-scripts-case-lexical.vela` | 1744 | 1 | `ed7312018702064e0ff839b6ea582ba154839450f2a71ff26f95a37d42c46d16` |
| `sources/hover/hover-s10-files-scripts-case-source-owners.vela` | 1301 | 1 | `4d8af9766f0cc8f3e8f677be0064d722ba103583f21f3fdc7005e1ec2db5b19f` |
| `sources/hover/hover-s2-files-scripts-api.vela` | 2294 | 1 | `dbb9b7f7ddb32a3f93f9d3c96875b2bb08eb7185466ef9dbb33f5ec82656547d` |
| `sources/hover/hover-s2-files-scripts-main.vela` | 7787 | 1 | `fe6973ec9d41ab1f735b5a943e61e1ec5ba8910331ffed02d2976b0c748e3244` |
| `sources/hover/hover-s4-files-scripts-api.vela` | 1950 | 1 | `859e0c7ccccfb3059f2a16f5a375942d592936eac0103f1a2112ecc37839fce8` |
| `sources/hover/hover-s4-files-scripts-main.vela` | 12382 | 1 | `53c5eef7f96129c2925901415b198c86bc1d7a6e66e2583dab6b8bb500b7496e` |
| `sources/hover/hover-s5-files-scripts-api.vela` | 1700 | 1 | `db7b301c28d3a02d468da387a8aaf61c6de1a08aab2ffb52b032f28428315653` |
| `sources/hover/hover-s5-files-scripts-main.vela` | 10186 | 1 | `e31aee0e57a7730e9d979e0766e5615bd8cfe353477fba01249699821ae0f850` |
| `sources/hover/hover-s6-files-scripts-main.vela` | 14136 | 1 | `3d536d75f325372a74efb58ad6bebf0962cefa7b89629629428041f79172324e` |
| `sources/hover/hover-s7-files-scripts-main.vela` | 25885 | 1 | `87f2918826c8291901ad4a23c819d49bb0e14b398a4eb73ba7faacb6a6f30701` |
| `sources/hover/hover-s8-files-scripts-api.vela` | 1158 | 1 | `769b9e0fc73740cc5090d27914758f7abcaac7914c19396d0356fe4e9c2a04ee` |
| `sources/hover/hover-s8-package-files-deps-external-scripts-consumer.vela` | 1095 | 1 | `a206032684d953ee88fa9162919d734a2f396214b6bae6f195bf5e028cb5cfe4` |
| `sources/hover/hover-s8-package-files-scripts-main.vela` | 1948 | 1 | `03ea4dd542af3a340de80696a50b968e8e7d6587de1dbce8ed4e4c1647aad3b4` |
| `sources/hover/hover-signature-coordinates-files-scripts-main.vela` | 1071 | 1 | `742c9c646b84e06ae6085307f9194647588cf9758186961044622877e640387e` |
| `sources/input/input-semantic-tokens-files-scripts-tokens-input.vela` | 3240 | 1 | `99be05f547f93d6033fdc5bc08433491456d8d3c8ffb4894f8a089ab80748a9b` |
| `sources/input/input-semantic-tokens-oracle-shifted.vela` | 3272 | 1 | `1938b8b8d98d3f274cd285a5b72ffb8c72d4a8b81dc37ee391628354cf1b03b8` |
| `sources/input/input-semantic-tokens-oracle-unknown.vela` | 3243 | 1 | `6b3f7226ed98e4594539b01b40777cf763cb92e0fb0a6d3e2f19fdd957a524b3` |
| `sources/navigation/navigation-calls-files-scripts-main.vela` | 3715 | 1 | `fae020b48c4ebb2f42bc63ca794c77fe8464747d1c6425a28d77e732ba9979bf` |
| `sources/navigation/navigation-constructors-files-scripts-main.vela` | 2726 | 1 | `ecf5197a46d364b8353b969c260ba65cffc3bd61b47f10c2476bca167014cdf7` |
| `sources/navigation/navigation-declarations-files-scripts-main.vela` | 2265 | 1 | `ca58927e494b4f9061679b4610cf61beb96da315f82c331db3922a6911760411` |
| `sources/navigation/navigation-dynamic-files-scripts-main.vela` | 1215 | 1 | `3063af1a8531ca652e3e25163a838bfad19daddd43aa42dff2e8d8cd0f7c9ebc` |
| `sources/navigation/navigation-imports-files-scripts-main.vela` | 1914 | 1 | `1a989a9bf3357a93d29ed20bf4fc1d89ce958c4b0ce8bb57efda3aa6f6c2f1e8` |
| `sources/navigation/navigation-members-files-scripts-main.vela` | 1862 | 1 | `2650b5c4bda02bb3d02583d490147ef7c0452c60bdc438969ab9ee3285e6b035` |
| `sources/navigation/navigation-namespaces-files-scripts-main.vela` | 1120 | 1 | `9aa96844507998c9722d27feb6793c77a447197d98aa3eab75857f592af86c92` |
| `sources/navigation/navigation-owned-declarations-files-scripts-main.vela` | 3033 | 1 | `646bbada01f44c12165893caf7e83e0ba2b0122e4d61b4dbcfbca755ad1ddbef` |
| `sources/navigation/navigation-type-positions-files-scripts-main.vela` | 2750 | 1 | `9083bf9d7b6b58520d6c94e3a27ea443af80b99343c4bb8112c354d1b6ce4785` |
| `sources/references/reference-rename-body-scopes-files-scripts-main.vela` | 1185 | 3 | `0352ea6bd310dabe8a2f7f269c0290eb51d695db08827c2dfb795e71173f74cb` |
| `sources/references/reference-rename-control-flow-files-scripts-main.vela` | 957 | 1 | `497bfe78190bc8182a100993b5a1204d77f058df1a9af109a8031ccdee45b31c` |
| `sources/references/reference-rename-coordinates-files-scripts-main.vela` | 906 | 1 | `8400ac8836879e7e0558e5de78c0809cdb88926f9572999d2ca5afa3268cd613` |
| `sources/references/reference-rename-default-bindings-files-scripts-helpers.vela` | 1320 | 1 | `3b18854a403703970ada6abebdac2faedfda4ae5e092d843cb22524e4b8b3370` |
| `sources/references/reference-rename-imported-types-files-scripts-helpers.vela` | 1272 | 1 | `2bd7e0b029350445214766f85349f09b074472438a9d290b07d42377dabfe36b` |
| `sources/references/reference-rename-imported-types-files-scripts-main.vela` | 773 | 1 | `c59b24d66133510d957cfccec99db0e27ff9ee53407a3d336ca3c6f3b70a18af` |
| `sources/references/reference-rename-imported-values-files-scripts-helpers.vela` | 774 | 1 | `9538a3a04deb594f75989d5e07ada5e23a41890ec3ed56f79fb1402fc5972399` |
| `sources/references/reference-rename-method-parameters-files-scripts-main.vela` | 1166 | 1 | `aa9cb091313d57dd4e1940a5bc5c18a6eb9e954beabbca8b9b59c8a161441d60` |
| `sources/references/reference-rename-named-parameters-files-scripts-main.vela` | 786 | 1 | `52eb769fdd66e1844280cb47fc392abe22b47056524d98ea64a19bb4584c1437` |
| `sources/references/reference-rename-private-variant-imports-files-scripts-imports.vela` | 888 | 4 | `81d95442114f81af197ccaf4de610e566d6c932840436a579a29cfb5a853d274` |
| `sources/references/reference-rename-private-variant-imports-files-scripts-secret-defs.vela` | 1029 | 2 | `39148094ced08987bfc954ef2f9e10c88c6c640afdc25a16b66b4f4ad917196d` |
| `sources/references/reference-rename-private-variant-imports-files-scripts-source-imports.vela` | 1062 | 3 | `525fba5461dbcc738f6474c21ae93973af091b42f233da116d43daf78acf771b` |
| `sources/references/reference-rename-record-fields-files-scripts-main.vela` | 996 | 1 | `aa769bfab19db8956685fff79d1be18da2f4947fc6fd8308c47fa2bb509d74ab` |
| `sources/references/reference-rename-required-parameters-files-scripts-main.vela` | 917 | 1 | `ce7fce8871c0f1d62fa1f8e2abbd6ae8b07dbdd9dad6923ac832cc07b72b440b` |
| `sources/references/reference-rename-schema-capture-files-scripts-consumer.vela` | 725 | 1 | `b72e8a6cfa2c9de5c89ef8e62a556ce07e137edc992b78226b23cfeb8169aa10` |
| `sources/references/reference-rename-schema-fields-files-scripts-main.vela` | 1201 | 1 | `97da2dd1473cddd105ded401436e0aa8602e509be7f8e89fbca224b585a6aa27` |
| `sources/references/reference-rename-schema-functions-files-scripts-main.vela` | 970 | 1 | `28309744e8d2051eaec1baf28acd19407326c7d11baff40cb3662f6def2822b5` |
| `sources/references/reference-rename-schema-imports-files-scripts-main.vela` | 1342 | 1 | `9bd3d4898bcaed206be8c934b03f1dd18e1f94bca0b0d35d885527c0f4328195` |
| `sources/references/reference-rename-schema-methods-files-scripts-main.vela` | 1699 | 1 | `9333ecee78324e9c910a68ea2f9c75c9182898a136134248d22ea8094a0be8bd` |
| `sources/references/reference-rename-schema-type-imports-files-scripts-main.vela` | 911 | 1 | `8270eb5b34c87c10d6a2ec34d49da61d449b16c87b8832c3f108aa85ec81d82d` |
| `sources/references/reference-rename-schema-variant-ambiguity-files-scripts-imports.vela` | 1108 | 2 | `06ab8f433cb64e34cc647314f7e83c0dcecf584aab879e17d23ba0666d6d9c5c` |
| `sources/references/reference-rename-schema-variant-capture-files-scripts-imports.vela` | 1190 | 1 | `5c2cd9f954b61657ca789ba37d59b3a0355652429aaeb3c579d3002b593c1e36` |
| `sources/references/reference-rename-tuple-fields-files-scripts-main.vela` | 1342 | 1 | `821f43fb28b8061600bf74221be28e3b566c822f993681d5dacb9ae5cbdf6cbf` |
| `sources/references/reference-rename-variant-fields-files-scripts-main.vela` | 1400 | 1 | `46cacdd535a99644a5a9cca8ad5ca6369b531fb6f2c58e9045f8991dc1bca4e6` |
| `sources/selection/selection-bodies-files-scripts-main.vela` | 755 | 3 | `9254f65174162119e45ff558749d1b875f6dd2015f40229c9cec69381382c34e` |
| `sources/selection/selection-calls-files-scripts-main.vela` | 1212 | 2 | `82eb9faa036b89ec6815e4ace4fde3575fd2a5a24092cc0132501518ee036d78` |
| `sources/selection/selection-literals-files-scripts-helper.vela` | 632 | 1 | `42dd14c5dc9c8c680d9e96a02ed165a3cdef8c68a5ea6f86ceb78e290a040bab` |
| `sources/selection/selection-literals-files-scripts-main.vela` | 1498 | 2 | `9c5728f2e333cb86ef452407a239f81c5e930e78b5f5b916db829d9b223e628c` |
| `sources/selection/selection-members-files-scripts-helper.vela` | 654 | 1 | `8fb058ec0ec1581b5d68abdb4c5305f1b3f577abcf32ea048a83ba106e610e7f` |
| `sources/selection/selection-members-files-scripts-main.vela` | 1256 | 2 | `e4e22444431bd524674970dad356de8712912d994dd23419be7a28df1c3cc403` |
| `sources/selection/selection-patterns-files-scripts-helper.vela` | 727 | 1 | `901206e03dd13a334004b0e0569095452157c98a7889fb21f52cd1931a455bb9` |
| `sources/selection/selection-patterns-files-scripts-main.vela` | 2334 | 2 | `8eb96600bc82f1f86bb1b2b8e82799fd024f98b31057c8fb206003b77fdfbfc3` |
| `sources/selection/selection-types-files-scripts-main.vela` | 1128 | 2 | `3960d053d9613d6128b81e1ef6cac02e41b28a5925147173e1131845133b8d76` |
| `sources/signature/signature-editor-files-scripts-signature-main.vela` | 1614 | 1 | `28b52ef80599a1d1f184f7b3fd08480fba64bbe1aef6da788b7a86ef3c5bb71e` |
| `sources/signature/signature-editor-oracle-recovery-source.vela` | 1689 | 1 | `f5936441fcf5e2436372f041414572d5c2df248cfa63d3249b685c511cd50caf` |
| `sources/signature/signature-s3-files-scripts-types.vela` | 5031 | 1 | `4bc7b06e0c8868f6c274eebd38484e953ca5462bd2b76405a98d981c4fc27ce4` |
| `sources/signature/signature-s5-files-scripts-methods.vela` | 1504 | 1 | `91349fc4f9b979f4fbbd22f50e48ad776571d0f65cfea036a9e76729ed6f877f` |
| `sources/signature/signature-s8-files-deps-external-scripts-api.vela` | 808 | 2 | `7e7d52606db816d9c250a14857772eaa10ff6986a5f75e642c4c78965bc73b53` |
| `sources/signature/signature-s8-files-deps-hidden-scripts-api.vela` | 799 | 1 | `c5c7b247185ebc23b1d1b3524b5ab62b79b4fb449c6d0f60c12cb07f977331b4` |
| `sources/signature/signature-s8-files-scripts-api.vela` | 773 | 1 | `7a97adefc0908e94592351a25201209b1e4085b830f48513a0aac98d90af3d65` |
| `sources/signature/signature-s8-files-scripts-main.vela` | 3374 | 1 | `a38397e1b8dfb13e36823965858325adba006f74e0003d4f6322a70794ee534b` |
| `sources/signature/signature-s8-oracle-phases-dependency-change-actions-0-source.vela` | 846 | 3 | `1b986530219dc315c4b22eb3142e8aec9b4a05f6f995d25b2e1bf563093c28c2` |
| `sources/signature/signature-s8-oracle-phases-importer-dirty-actions-0-source.vela` | 3438 | 1 | `d04e11e1e38cfc277104f9f16e0b09afb282c00366f7c75a46ae9ed3047d7f25` |
| `sources/signature/signature-s8-oracle-phases-peer-dirty-actions-0-source.vela` | 852 | 1 | `0290416b739a4576b1b0e0203393e4a5e9c609b447e734367b5250d49518f4e1` |
| `sources/signature/signature-s8-oracle-phases-renamed-alias-actions-1-source.vela` | 3376 | 1 | `ebe9a0112f80918bebc65d10be7c5255495c5f200b7bffce6f1258d1b32c0820` |
| `sources/signature/signature-s8-oracle-phases-root-dirty-actions-0-source.vela` | 791 | 1 | `292c130b94e2eda5ed7d39b11cb12fe3519964f7f0ae72fd163b01b655d15c3d` |
| `sources/symbols/document-symbol-declarations-files-scripts-api.vela` | 4224 | 1 | `0867d8f00af5aa05e3a7d29c5cbc80239caf167453fbf673c1894f4560c8b625` |
| `sources/symbols/document-symbol-lifecycle-files-scripts-main.vela` | 709 | 2 | `f58738931f0daa8db625777ad9237ebb360b8d7a42bbc899dc741f13e5e19917` |
| `sources/symbols/document-symbol-lifecycle-oracle-variants-api-new-source.vela` | 627 | 1 | `babdddb6dac463f5929a96fc67c40c775aeaceba22a05a9a0e4f5b0f5b832bcc` |
| `sources/symbols/document-symbol-lifecycle-oracle-variants-api-replaced-source.vela` | 624 | 1 | `59389f16df37f74dbfca4a6c8b4d48b6323849759fe38d0db57ac0fe3fe0130b` |
| `sources/symbols/document-symbol-lifecycle-oracle-variants-main-dirty-source.vela` | 859 | 1 | `fe638b8d0f5007067468f8eae220c64eef629d4f36c185d7fdcc71ad4eb035e1` |
| `sources/symbols/document-symbol-lifecycle-oracle-variants-main-disk-source.vela` | 671 | 1 | `928b2e241abac3b0db63e7eeb166b9ab2873e17a94de24463db280f9cd41b2eb` |
| `sources/symbols/document-symbol-lifecycle-oracle-variants-main-second-source.vela` | 733 | 1 | `b2575db95154b23b29387ebc1e2e3c674540622cdb174bf1300f097e8fe02d2a` |
| `sources/symbols/document-symbol-ownership-files-scripts-main.vela` | 2315 | 1 | `4e202f0c4f12aac8a05640819182059daa903eef9cddc1e6ec2f14c9fd7cd518` |
| `sources/symbols/document-symbol-ownership-files-scripts-origins.vela` | 881 | 1 | `b84a229ddf700590285dd5c7d9db57705078effa7f7d50e318acdd05204c0b98` |
| `sources/symbols/document-symbol-ownership-files-scripts-source.vela` | 2449 | 1 | `e715facb8c6b66da5e05c0ad3e3db0e82bdeacb9d898748a808a6f556ff61b8a` |
| `sources/symbols/workspace-symbol-declarations-files-scripts-api.vela` | 4478 | 1 | `1c10d035d58a87a8431f73f8b92439b14b04d874aad0e5c4f199efe66b119a4a` |
| `sources/symbols/workspace-symbol-lifecycle-files-scripts-api.vela` | 622 | 2 | `4f3e3ae41211278197b27b5982c27ab2cd76411f3b1182a34b80f8f85c9db112` |
| `sources/symbols/workspace-symbol-lifecycle-files-scripts-main.vela` | 735 | 2 | `ca4d9e8ec194b00ef10b2bb8da0fec4a318d29e742501b192284cdf9c41bd801` |
| `sources/symbols/workspace-symbol-lifecycle-oracle-variants-api-new-source.vela` | 653 | 1 | `fa00c0bd5af8de10860a65f2d93a07e00f4b5814618ba7b8f9b6b6a85d42f234` |
| `sources/symbols/workspace-symbol-lifecycle-oracle-variants-api-replaced-source.vela` | 650 | 1 | `83750cfffa22c89b769f054cff1be02aeeeaf34979669b85f614e16de22fc997` |
| `sources/symbols/workspace-symbol-lifecycle-oracle-variants-main-dirty-source.vela` | 885 | 1 | `d96433ee5ecc57311cd68dd993d7a9d8ae99e709793eb54b7328c835e20193cd` |
| `sources/symbols/workspace-symbol-lifecycle-oracle-variants-main-disk-source.vela` | 697 | 1 | `f54c134fe39c4622ef93e831d5aa2402445f45f15bce915ccc60b3aa296eb803` |
| `sources/symbols/workspace-symbol-lifecycle-oracle-variants-main-second-source.vela` | 759 | 1 | `004feec388bc21c1d71ca6642461df8356f77f6880e4deab0c0f40315a327ed8` |
| `sources/symbols/workspace-symbol-ownership-files-scripts-main.vela` | 2341 | 1 | `83c5c829bc2b8135b182421ae9f1c7b47bc58602261f37d6f23d2acfcac34f5e` |
| `sources/symbols/workspace-symbol-ownership-files-scripts-origins.vela` | 907 | 1 | `21e8d8bf20d7df6508001bb2ae10396facc8029393b7cd7a2b4060a28408d151` |
| `sources/symbols/workspace-symbol-ownership-files-scripts-source.vela` | 2475 | 1 | `03c9c5cbf41e2749f06d33074d29450db14ac90505fafe32b97cbc450c95e94f` |
| `sources/symbols/workspace-symbol-type-positions-files-scripts-types.vela` | 7144 | 1 | `5d2024e40a95c514d50ec87eeb5e19a9272a5a05524a67ae7e57fb041962057b` |
| `sources/tokens/semantic-token-bodies-files-scripts-main.vela` | 15547 | 1 | `25c127f9f4b7957f9557ad0744e5a41af7f86bc5f2de5d2855402efc0837ecef` |
| `sources/tokens/semantic-token-bodies-oracle-negative-source.vela` | 11367 | 1 | `5e4f176dcc16e29401bd910e67006260da7ef02865e8b26f98e88943ea353025` |
| `sources/tokens/semantic-token-calls-files-scripts-main.vela` | 27674 | 1 | `92e619a57980116021c0fddf14683ed245e2b8139c8b17c366dea71aa5251ba2` |
| `sources/tokens/semantic-token-calls-oracle-negative-source.vela` | 21742 | 1 | `83dca78655e6def2c48075c1b57abbebc6919f95f22273043aa44d5712a0dd8a` |
| `sources/tokens/semantic-token-control-files-scripts-main.vela` | 28952 | 1 | `4770833b2d7a2338562c336d5f0f3a5db11eb443a4dc19b85c449b4944470a13` |
| `sources/tokens/semantic-token-control-oracle-negative-source.vela` | 14508 | 1 | `50a432c66443bae73b4ff7b3b5f980861d75e9848a9b48f6104a1f09d65ddccd` |
| `sources/tokens/semantic-token-editor-files-scripts-tokens-view.vela` | 1749 | 2 | `af1e2d00a1bd69dc88b7591574b9cfddf8bc0def1bc2ebb31bf3189114b43910` |
| `sources/tokens/semantic-token-editor-oracle-states-dynamic-demotion-source.vela` | 2034 | 1 | `ac07a27ec1f1befa3fcb3970e32951292b1f34ddd94b626ce311a00ab838c57e` |
| `sources/tokens/semantic-token-editor-oracle-states-unicode-shift-source.vela` | 2033 | 1 | `a8326b92c3489a953a9fff6eae72983386c42f97ada66f2c7eed23c921c217f6` |
| `sources/tokens/semantic-token-imports-files-scripts-main.vela` | 22246 | 1 | `3a070dddfad376d43d29b292640d1605c161a5e422cf2e0f29bd9b0bcf72a02f` |
| `sources/tokens/semantic-token-imports-oracle-negative-source.vela` | 8499 | 1 | `01dd708114702e372fe43ece69b21d9514576a683b7ef0178fe3015bfeb7c463` |
| `sources/tokens/semantic-token-incremental-files-scripts-main.vela` | 2228 | 2 | `952f614a95aed856da7c73e1463b4fafbda359bf9a62b625982f62dd3f20662d` |
| `sources/tokens/semantic-token-incremental-files-scripts-relay.vela` | 973 | 16 | `1c9b76c84d744130656ea84da67fa1f58531fd1750ae2e995531de6e749a9099` |
| `sources/tokens/semantic-token-incremental-oracle-steps-body-erases-member-source.vela` | 1574 | 2 | `4a848eb53483b2b7881444c97d4dd91ff27d4fdcc7d0596c8e0662f7b974e199` |
| `sources/tokens/semantic-token-incremental-oracle-steps-body-restores-member-source.vela` | 1572 | 8 | `f1ca882eb28b6fe317ced016dc27941ab1230a0e6e7dc27e21095acbbf7559e1` |
| `sources/tokens/semantic-token-incremental-oracle-steps-declaration-return-erases-transitive-member-source.vela` | 1575 | 2 | `03f385374609e7a4d37e809d10dcd3e6a53aaefd27a1cf45ddaecbc17f29526d` |
| `sources/tokens/semantic-token-incremental-oracle-steps-import-switches-owner-source.vela` | 1574 | 8 | `c6c31c3998c2d75e26ee4647c03b6986e8761173de4c139c3a361f508895214f` |
| `sources/tokens/semantic-token-literals-files-scripts-main.vela` | 22693 | 1 | `854688690db4216b5cc6aa42cbcba3921973981b7551614f6dda8e99ab82bb93` |
| `sources/tokens/semantic-token-literals-oracle-negative-source.vela` | 5289 | 1 | `2810c4dad5d5cea4c28ac87b2d58a2c6e908bc139d3ffcf6d9d847e23a0a8a4f` |
| `sources/tokens/semantic-token-members-files-scripts-main.vela` | 20133 | 1 | `77109f4b93356890544953267b87aea29db954fc45ec92135e9e313b25da53a2` |
| `sources/tokens/semantic-token-members-oracle-negative-source.vela` | 11707 | 1 | `62ade87f42ea8b7127122bf58683e8ef1900d512ca3cd89088f7ab5cb327d4ca` |
| `sources/tokens/semantic-token-ownership-files-scripts-main.vela` | 15669 | 1 | `11504cfe80911ec46fe4a8cbd9442a1a7ceb3e1471eeb6ac5bd14e945dbf40be` |
| `sources/tokens/semantic-token-ownership-oracle-negative-cases-0-source.vela` | 30040 | 1 | `6df47c9c0667ed008d9fa9189c1fa8f746d3255c0576ad4c8bf1d0edc9340cbf` |
| `sources/tokens/semantic-token-ownership-oracle-negative-cases-1-source.vela` | 21659 | 1 | `0c1c9e3e230c2c75ccad30385e765729908feeae04e85ba9225e5890298d1ba6` |
| `sources/tokens/semantic-token-ownership-oracle-negative-cases-2-source.vela` | 27674 | 1 | `f8b8ec87f8326c119b7f395a61f9e6f2a4727cae3286630b3421c188f2bd1caf` |
| `sources/tokens/semantic-token-recovery-files-scripts-main.vela` | 3556 | 1 | `a4b36d0b35c9af7e1f2c0259f99d0f24aef4d4a473fbfd03935efc8f7012375c` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-0-source.vela` | 3382 | 1 | `3fa9f3a49ae6bc907cd96dfec44aabc411a964ab12ca3411700e60b1cc83e0ca` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-1-source.vela` | 5809 | 1 | `25f9558b56060eec403909b0d495c363ff85670379baaab2fe6e0ec8155bfd40` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-10-source.vela` | 3612 | 1 | `d1360adb8176bc8d4ea8b1f7d71f3fa82318b79b1aa9edebc42124164d02cbf1` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-11-source.vela` | 3139 | 1 | `f7af63d026d350cb22760b0482b6d5e0169c1f6b6e66591a58b16c0a61fe4d1a` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-12-source.vela` | 3016 | 1 | `89f5042af7fb966568efa667290f9f4c85d990df8dfa5e1929f1190cbcd9c392` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-13-source.vela` | 4027 | 1 | `5d2a6b9999e31259c9abab34e99026b6045a13a6a799c0b21b0a3866d1a2e660` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-14-source.vela` | 4810 | 1 | `3c8bc9e88105800f369cac4303a42138cd40380c66dfacec6211ec9b86cf3af5` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-15-source.vela` | 3772 | 1 | `24190e9ae4df1d74e73e65ec7378fbfacbe0a6529f1f851c26707cdeb3d5c6cf` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-16-source.vela` | 3640 | 1 | `08c155a2fffb0ebc5794658743d15942dc0191d753e4ad060942b54229313dcf` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-17-source.vela` | 4709 | 1 | `4782d404760d16f05bdfe77896856b26ea575200ac94bb05f10aa15c97663611` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-18-source.vela` | 5319 | 1 | `765ce60aea24e8bafe55f4b09985984de3a5161e6007be3511d3ce0f7d873890` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-19-source.vela` | 4147 | 1 | `65bd7f625f07823279259bd5082e527a8cc3ad650d0ed25efbe5d856a7cf0038` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-2-source.vela` | 4326 | 1 | `01f43ec2d98d2de76ab75e6556ae2ec0010e9767019037796804ec67eba009ab` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-20-source.vela` | 4636 | 1 | `567d52cfbc5905542cbee28933dfe4272c51b74f6005ee5edbef6afa4a0c6191` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-21-source.vela` | 5403 | 1 | `4a2f628ea85f79d2b0943cd0e471b0d802f43c5ba60a4d51048653b659960800` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-22-source.vela` | 3846 | 1 | `081ef021cc3be113a4ad96c9e64a7d8eedf18aa94b98c809825334082c2d65af` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-24-source.vela` | 4078 | 1 | `f42a551737dc4973103c0db2e0f306568649efe8f5aba599296dbb315090c8ce` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-3-source.vela` | 4559 | 1 | `9e9e39532ee685c13847d965054509c7ed128b30662b8ae6f8a9f64bb44f9b9a` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-4-source.vela` | 7917 | 1 | `8a014880304df1dfab2f339a45dd1b2ac002881ec33f7c3e65c94781ab8b79d1` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-5-source.vela` | 4258 | 1 | `d6850b1536473eecf92d0d554a6d003251beb0bca83fd25548f91cd2b91243f9` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-6-source.vela` | 3146 | 1 | `6caffe2353ee6536e8adcb75b8c266a1c8549b154382138c0093f2464ae8295c` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-7-source.vela` | 2995 | 1 | `0c63b9bb63e731c8cfddc43b54fd365b96c76c9baefcffc5e52e49fc5f9ac3ec` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-8-source.vela` | 4135 | 1 | `b050e9c356843b188fc001181fc377371b0511bd8accefc02e97a9f37696b366` |
| `sources/tokens/semantic-token-recovery-oracle-negative-cases-9-source.vela` | 2373 | 1 | `4d0b45ac635fc37b29b0fabfcc70003081df0610c30b139073341b83048fdd0d` |
| `sources/tokens/semantic-token-schema-lifecycle-files-scripts-main.vela` | 4257 | 71 | `e91d3230f721fbea668aa6ba6252d84b697f7663ba096a36e0c2235494ff5144` |
| `sources/tokens/semantic-token-source-lifecycle-files-scripts-main.vela` | 3388 | 63 | `9b83584da6ceeff757a7b102cab5f7f089749a9d30b0cca10f4411d8b5ed8d92` |
| `sources/tokens/semantic-token-source-lifecycle-oracle-states-incomplete-member-action-source.vela` | 3346 | 3 | `796b8813c402e37389b9835893b325c24b14490afee31787e5952c1d1a7b444b` |
| `sources/tokens/semantic-token-source-lifecycle-oracle-states-missing-body-brace-action-source.vela` | 3352 | 7 | `a4cdacc77ae36e58e4a273828022de6768b548987d8fe8f9119fd5895cedbb62` |
| `sources/tokens/semantic-token-source-lifecycle-oracle-states-unicode-multiline-dirty-prefix-action-source.vela` | 3440 | 9 | `f3246718618e7951eed34d595fa6c0661ede574c0c328006364e6c9b097bc964` |
| `sources/tokens/semantic-token-source-lifecycle-oracle-states-unresolved-callee-action-source.vela` | 3546 | 3 | `2c6145b638f1fa11d4412a55bf0ba3309f876a6548f48f936c8edf010dd2ea4c` |
| `sources/tokens/semantic-token-top-level-files-scripts-main.vela` | 8075 | 1 | `ba8cbb9c3fa007e11097f8e3f6a61e8c1cd65a2cdba0906632ca56b14e80da2d` |
| `sources/tokens/semantic-token-top-level-oracle-negative-source.vela` | 8314 | 1 | `fe163d07ab0d622f66954c60cba88d43441451ce0c434c2e657e6fc101d0788a` |
| `sources/tokens/semantic-token-type-positions-files-scripts-main.vela` | 19543 | 1 | `4a74a5b265bc61f2876f86cde1e031366db7d7bdb45d42dd613f01dfdc99eed9` |
| `sources/tokens/semantic-token-type-positions-oracle-negative-source.vela` | 9813 | 1 | `2385b63e609c9d0afbfd201cd2ec63c4d365cd44b81ddbe8cf0753dbc7b87ce6` |
