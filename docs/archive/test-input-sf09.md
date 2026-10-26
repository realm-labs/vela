# SF09 — Local Language-Service And LSP Source Inputs

## Scope, Parent State And Result

Implemented the local Rust-input stage of [the plan](../test-organization-plan.md)
and [the tooling census](../test-input-census-tooling.md). This is source-storage
organization, not a language/tooling feature or a new acceptance claim for editors.

- Initial HEAD: `dc6483eb9` (`test(vm): organize execution and reflection source fixtures`),
  with a clean working tree. Preceding parents: `d77450af1` (SF04), `a9bfbd97d`
  (SF03), `5c6b70f2d` (SF02), `bdc23b480`, `51145cdfa`.
- Write ownership: `crates/vela_language_service/` excluding benches,
  `crates/vela_lsp_server/`, selected direct-include sources under
  `tests/lsp_matrix/fixtures/sources/local/`, and this report only.
- **196 literal occurrences in 63 Rust files now use 97 exact-byte fixtures:**
  68 cross-library sources, 24 language-service-local files (including two package
  manifests), and five LSP-local sources. The fixture ledger below records every
  length, SHA-256, and original consumer location.
- All existing tests, assertions, query offsets, logical paths, SourceId derivation,
  capability/schema setup, helper contracts and lifecycle sequences are retained.
  No test registration, Cargo manifest, production behavior, reader/schema, existing
  matrix JSON, benchmark, or other shared document was changed by SF09.
- Parent/concurrent SF06–SF08 Engine/hot-reload/macros/CLI/WASM/example changes and
  their reports appeared during validation. They were left untouched. HEAD was
  still `dc6483eb9` at the handoff check. **No commit or staging was performed.**
  The parent owns commits and shared stage/status updates.

## Review Method And Complete Local-Family Disposition

Reviewed the census against the current **595 Rust files** in the two owners
(excluding the language-service bench), including tests embedded in production
modules, helpers, integration support and generated/assembled source routes.
A temporary lexical inventory evaluated 627 source-like literals using `rustc`;
those are triage signals, not 627 distinct tests or an exhaustive count of small
strings. Inspection included duplicate groups, source-construction/IO call sites,
selected source bodies and adjacent assertions, and the remaining substantial
literal exceptions below. No length threshold drove extraction. The inventory,
byte evaluator and comparison scripts were temporary and removed before handoff;
there is no new loader, framework, catalog, cache or test dependency.

Here **LS** means `crates/vela_language_service/src/`; **LSP** means
`crates/vela_lsp_server/src/`. The census's whole-family filename lists remain the
review map, not a request to migrate JSON in this stage.

| Reviewed local family | Final disposition and preserved boundary |
|---|---|
| Completion (`completion/`, LSP completion/member/type/resolve/call-argument families; embedded item/lambda/record/module/statement/type tests) | `shared-input`: exact function-/method-return documents with global decoys. `file-backed`: LS impl/trait arrangement and member index; distinct LSP impl/trait arrangement. `retain-inline`: short context/cursor/negative probes, insertion fragments, named-argument slots and short edits. The decoy-free LS function-return case is deliberately not replaced by the similar shared source. Shared JSON remains SF10. Completion details, resolve ownership and full edits stay local. |
| Cursor/query/expression facts (`cursor_context*`, `query_context*`, `expression_facts/`) | `file-backed`: multi-scope tuple destructuring. `retain-inline`/`unit-data`: caret/partial-expression/type/range rows and compact source/schema/builtin callable probes, whose operands are adjacent to range assertions. No cursor normalization or SourceId replacement. |
| Definition/declaration/type-definition and hover | `shared-input`: four exact returned-receiver programs, cross-file hover main/rewards pair, and method-return-type declarations. `file-backed`: distinct tuple-destructuring documents and LSP Cell field-navigation document. `retain-inline`: small dynamic/null/type-binding/schema-span/Unicode URI probes and short version changes. Definition, declaration and type-definition remain independent tests; full hover markdown and qualified owners are unchanged. |
| References/highlights/rename | `shared-input`: four two-call returned-receiver programs (distinct from navigation); record fields/shorthand/imports, enums/variant fields, trait impls, schema-owned uses, read/write/call sources, private rename/collision programs and default-trait-return rename. `file-backed`: LS tuple scope program and marked schema-record main/unrelated fragments. `retain-inline`: concise collision/invalid-name/ambiguous-owner rows and simple schema-member calls. Typed expectations and complete edits remain in Rust. |
| Call hierarchy | `shared-input`: method-return documents also used by references, multi-declaration trait impl/default/interface sources and cross-file caller/owner sources. `retain-inline`: short unknown/non-callable/Any/schema-return probes and tiny helper modules. Incoming/outgoing edges, exact fromRanges, stale item/request rejection and scenario helper contracts are unchanged. |
| Signature/inlay | `shared-input`: Any-parameter controls, function/method/trait-return suppression, variant suppression, lambda facts and matching method/function-return parameter documents. `file-backed`: seven LS signature arrangements and three distinct LSP signature arrangements; LS tuple hint scopes. No whitespace normalization to manufacture equality. `retain-inline`: small argument-index/null/Any/tuple-contract and schema-only probes. Schema registrations, active parameters, exact hint positions and missing-hint negatives stay local. |
| Diagnostics/code actions | `retain-inline`: concise errors/repairs, typo rows, missing Option/Result arms, tuple mismatch and selected edit spellings. Generated applied-text comparisons remain explicit. Larger multi-error, recovery, dependency/schema lifecycle and oracle sources are the unchanged SF10 JSON corpus, not silently declared extracted here. Codes/severity/labels/edits/reparse and stale clearing remain covered. |
| Semantic tokens | `shared-input`: members, typed member uses, enum uses/import owner, function/method/default-trait returned receivers and Any-return rejection. `file-backed`: LS constructor-member, source-enum and source-type-hint documents. `retain-inline`/`unit-data`: focused coordinate/range/token/legend/degradation/short delta probes. Dense matrix streams remain SF10. Full/range/delta application, result IDs, cache/cancel checks, origins and fresh comparisons are untouched. |
| Symbols/folding/selection | `shared-input`: repeated item/block folding source. `file-backed`: LS nested type-member symbol document. `retain-inline`: small symbol/range/tree cases, malformed literal folding, EOL/ancestry and bracket/selection probes. Whole symbol/selection/folding matrices remain SF10. All requested positions, parent chains, capability projection and worker lifecycle remain at existing targets. |
| Formatting | `shared-input`: container-hints input/expected pair. `retain-inline`: small document/range/on-type cases, malformed source, EOL/trailing spaces and concat probes whose spelling is the assertion. Exact edit ranges, untouched neighbors and idempotence tests stay local. The extracted example references undefined `score`; it remains formatting proof, not executable-result equivalence. SF02's separate execution selection is not altered. |
| Incremental/project/workspace/open/change/close/snapshots | `retain-inline`: short old/new and disk/overlay pairs, including `fn main() { 1 }` versus `{ 2 }`. `generated`: scaled modules/padding. JSON lifecycle documents remain SF10. Snapshots/Arc ownership, SourceIds, versions, invalidation/rebuild metrics, stale publication and current-versus-fresh comparisons are unchanged. |
| Workspace IO/config/schema/watchers/session/cancellation | `file-backed`: LS ordinary-trait two-package tree, still copied by the same helper to fresh writable temporary roots with the same identities/dependency path and cleanup. `retain-inline`: small filesystem sentinels and disk/overlay contrasts. `unit-data`: schemas/config/capabilities. No watcher, physical IO, root precedence, invalid/delete/repair transition or cancellation sequence was collapsed. |
| Removed boundaries and architecture audits | `retain-inline`: removal/Service-path/async/task/resource rejection snippets and tiny probes. `unit-data`/`generated`: schema mutation tables. `not-test-input`: forbidden Rust imports/dispatcher needles. No obsolete capability was restored or historical fixture name treated as support. |
| Protocol/text conversion (`line_index`, lsp conversion, protocol/rpc/task/transport/config/path modules) | `unit-data`: UTF-16 boundaries, wire fields, IDs, malformed payloads and protocol tables. `retain-inline`: tiny documents needed for one location conversion. Production strings are `not-test-input`. No arbitrary metadata extraction. |
| Stdio integration/support | Existing `specialized-corpus`: document-change/shared JSON remains SF10. `retain-inline`: poisoned stale update. `unit-data`: malformed headers/JSON/EOF/framing byte arrays. Existing real child-process deadlines, output limits, kill/reap ownership and all four process scenarios plus six support tests remain intact. |
| Assembly/generators and excluded bench | `generated`: completion/incremental scale inputs, Unicode/CRLF/prefix variants, explicit snippet insertion and finite source mutations. No generated combination was frozen into a fixture. The bench and its support source are excluded from SF09 writes and validation; SF11 owns that review/action. |

### Deliberate Residual Substantial Literals

The final residual scan was reviewed rather than blindly extracting the last
strings over a numeric threshold:

- LS `completion/member_tests.rs`: the trait-only two-method completion contrast
  (203 bytes) and decoy-free source-function-return positive (200 bytes) remain
  adjacent to their narrow assertions; neither is the shared 246-byte decoy case.
- LS `definition/source_callables.rs`: the 239-byte, four-line local impl/required
  trait/schema-span probe stays beside dynamically derived source spans and the
  independently authored schema mapping.
- LS `diagnostics/tests.rs`: the 209-byte Option/Result pair stays beside its two
  missing-arm assertions; it is not a large independent program.
- LS `query_context/hir_cursor.rs` (215 bytes) and `query_context.rs` (251 bytes):
  two-/three-line operand arrangements stay next to offset math and source/schema/
  builtin callable assertions. Their compact spelling makes ranges inspectable.
- LS `signature.rs`: 217-byte tuple/unit contract probe and 219-byte Any-return
  rejection; LSP `tests/signature/dynamic.rs`: distinct 218-byte Any rejection.
  These short function/call contrasts remain inline; leading-newline differences
  are not normalized. Larger positive receiver arrangements were extracted.
- Remaining large JSON strings in Rust are schema metadata, not Vela source.
  They are `unit-data`, not mislabeled `.vela` fixtures. Short repeated calls,
  setup sentinels and expected labels were not deduplicated merely for equality.

## Ownership, Byte And Logical-Identity Proof

1. New consumers are direct test-only `include_str!` expressions. Existing targets
   and helpers remain: LS `--lib`, LSP `--lib`, LSP binary unit tests and real
   `--test stdio_transport`. Fresh database/project/schema setup is not shared
   globally. `assert_navigation`, `databases_for`, `TestServer`, source-return
   fixture helpers and package/layout drivers retain their different contracts.
2. A temporary Rust program evaluated each original raw/cooked literal and compared
   `original.as_bytes()` to the new `include_bytes!`. **All 196 comparisons passed**,
   including Rust line continuations, escaped quotes, indentation, Unicode,
   leading newlines, final whitespace and absent final newlines. The four package
   literals were also independently checked by Rust, not only a Python decoder.
3. Expanding the new includes back to their respective original literals in all
   63 consumers yielded identical code after ignoring only whitespace outside
   literals/comments. No URI, query, name, schema, assertion, event, branch,
   timeout, helper call or test attribute changed. This check includes the
   schema-record fragment concatenation and the package writes.
4. Logical URIs remain `/workspace/...`, file/untitled/package URIs or the original
   temporary roots, never checked-in fixture paths. SourceIds are still obtained
   from the current graph. Exact bytes plus unchanged offset/line/occurrence
   logic preserve byte-column and UTF-16 positions, including all eight
   definition/declaration returned-receiver entry tests.
5. The marked schema-record fragments are still concatenated before optional
   LF-to-CRLF conversion and marker parsing. Both original/new compositions were
   compared for full marked bytes, stripped bytes and all 12 marker endpoints
   (byte, line, UTF-16 column). The existing LF/CRLF rename test also passed.
6. Fixture total: **25,251 bytes**, replacing **50,924 literal bytes** across
   occurrences. Nine files have a final LF; 88 do not. Seven LS signature fixtures
   intentionally end with the original eight indentation spaces, not a final LF.
   Do not trim those files to satisfy a whitespace linter: that would violate the
   extraction contract. Scoped Rust/report whitespace checks exclude this
   deliberate source-data spelling. The initial extraction added no attributes;
   the independent-review correction below now pins authored LF on checkout.
7. SHA-256 over the sorted 97 fixture entries, each `repository path + NUL + bytes`:
   `fa8828f5351e5744cfd400e12f385351c361c4008121b3ab05497a27109e7be1`.
   Per-file values below are both old evaluated-byte and new file hashes.
8. Repository tests already include external shared support; cross-owner source
   includes stay in that same repository route and hashed subtree. No production
   dependency or distribution copy was introduced. Independent `cargo package`
   test execution was not claimed or validated. Cargo's direct include dependency
   tracking is used; there is no glob discovery or custom invalidation mechanism.

Marked composition hashes:

| EOL | Marked SHA-256 (old = new) | Stripped SHA-256 (old = new) |
|---|---|---|
| LF | `2e8843ba214d663baa028945833b9e90742892f6fe041141fedebd0f35bb94c6` | `a5d1b70047f5a63bb85c815fc5d613a752a1e9ce473691cc794a36f842921c80` |
| CRLF | `bd2d6dd7cf6b3f1d5162876e187246b26c57f322b35634a2bccd443d6fea5dad` | `0aa6d39c92a14447198125308d2aff763d94d8ef2c04108a59cdf1455f82f6a3` |

## Actual Validation

Toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`,
`cargo 1.99.1 (5f94df478 2026-08-27)`, macOS. All commands were finite.
`--test-threads=4` was used for owner suites unless noted. No global formatting,
workspace aggregate, benchmark build/measurement, installed-editor/native-input,
Windows or independently packaged-test gate is claimed.

| Phase / command | Actual result / bound |
|---|---|
| Baseline `cargo test -p vela_language_service -p vela_lsp_server --lib --locked -- --test-threads=4` | **Timed out at 240 s** during LS matrix tests, with no reported failure before cutoff; LSP not reached. Not an aggregate pass. |
| Baseline `cargo test -p vela_lsp_server --test stdio_transport --target-dir target/lsp-matrix-stdio --locked` | 10 passed; 180 s bound; process/support execution 0.89 s. |
| Baseline combined `--lib --locked -- --list` | 773 LS + 842 LSP identities, nonempty; 120 s discovery bound. |
| Baseline combined `--lib source_return --locked -- --test-threads=4` | 19 LS + 11 LSP passed; 120 s bound. |
| Baseline `cargo test -p vela_lsp_server --lib --locked -- --test-threads=4` | 842 passed, 523.51 s test execution; 600 s bound. |
| Baseline `target/debug/deps/vela_language_service-1e77f6596dbd96b0 --test-threads=4` | Previously built pre-edit baseline executable: 771 passed, two ignored, 285.74 s; 600 s bound. |
| Focused combined `cargo test -p vela_language_service -p vela_lsp_server --lib --locked -- --test-threads=4 source_return member_completion inlay formatting call_hierarchy provider_tooling tuple_destructuring schema_record_labels` | 124 LS + 108 LSP passed; 180 s bound. These are actual nonempty existing selections, not new tests. |
| Default `cargo test -p vela_language_service --locked -- --test-threads=4` | 771 passed, two existing ignored; zero doctests; 296.10 s library execution; 900 s bound. |
| Default `cargo test -p vela_lsp_server --locked --target-dir target/lsp-matrix-stdio -- --test-threads=4` | 842 library + seven binary + 10 stdio passed; zero doctests; 900 s bound. |
| All features `cargo test -p vela_language_service --all-features --locked -- --test-threads=4` | 771 passed, two existing ignored; zero doctests; 286.11 s library / 286.73 s command; 870 s subprocess / 900 s tool bound. |
| All features `cargo test -p vela_lsp_server --all-features --locked --target-dir target/lsp-matrix-stdio -- --test-threads=4` | 842 library + seven binary + 10 stdio passed; zero doctests; 393.49 s library / 394.80 s command; 870 s subprocess / 900 s tool bound. |
| Final combined library discovery, default and `--all-features` | Exact output equality with pre-edit discovery, 773 + 842 identities in both lanes; 120 s per subprocess. Neither crate declares its own features; these commands do not claim workspace feature-unification equivalence. |
| `cargo clippy -p vela_language_service -p vela_lsp_server --lib --tests --bins --locked -- -D warnings`, repeated with `--all-features` | Both passed; 180 s each. Explicit target selection excludes the out-of-scope bench. |
| `rustfmt --edition 2024 --config skip_children=true --check` on the 63 changed Rust paths | Passed; 60 s bound. Formatting was applied only to those same paths, without module traversal. |
| Scoped `git diff --check` on changed Rust and this report | Passed. Untracked exact-byte source whitespace is intentionally retained as described above. |
| Temporary Rust byte-evaluation/assertion program; include-expansion and SHA audit | All 196 comparisons / 97 fixture hashes / 63 consumer round trips passed. Rust build/run bounded at 60 s each. Initial audit comparison of rustfmt-rendered expanded text reported only wrapping differences in `provider_tooling_tests.rs`; changed the audit to compare non-whitespace code while preserving literal/comment bytes. No test/source fix was needed. |
| `node --test scripts/lsp-matrix/fixtures.test.js scripts/lsp-matrix/provenance.test.js scripts/lsp-matrix/rust-suites.test.js` | 24 passed, none failed/skipped; 120 s bound. Reader/oracle/provenance/target-mapping sanity, not installed-editor proof. |

The two unchanged ignored tests are
`completion::tests::completion_contexts_scale_in_million_line_workspace` and
`incremental::tests::million_line_synthetic_workspace_checkpoint_refreshes_hir_for_body_edit`.
No ignored scale test was relabeled as passing. Full runs include existing
snapshot, cancellation, stale publication, disk/overlay, source/schema recovery,
watcher/configuration, token-delta and fresh/incremental lifecycle suites.

## Impacted Evidence Identities And SF10 Handoff

The existing `scripts/lsp-matrix/provenance.js` function was called before and after
extraction with `target/debug/vela_lsp_server`:

| Identity | Before | After |
|---|---|---|
| `inputsSha256` | `2b7127e7ca4a8026882f94a38ef0b7a209bea2b682343c919a36eee9eaacee86` | `8529d991d135c2c0ce228369e7f7e98e0c3239069eea393451994fc7215a84b8` |
| `serverSha256` | `53139de0f93710dfa867db6a3bf2cbe6e343c5f6c94c6a87ab3f11d2aab64c23` | unchanged |
| platform | `darwin` | `darwin` |

- The input identity changes because sorted paths and raw bytes under the fixture
  subtree are hashed, even though resolved source bytes are identical. All 68 shared
  origins stay inside `tests/lsp_matrix/fixtures/sources/local/`. The unchanged
  dev-profile server fingerprint is not recaptured editor evidence. Every stdio
  invocation, including full owner runs, used `target/lsp-matrix-stdio`.
- `source-identity.js`'s repository tree identity also changes: it includes these
  Rust edits/new fixtures, this report and concurrent parent work, including
  nonignored untracked files. It is not the fixture/input digest above. The parent
  must capture the final tree identity after integration; embedding a final hash
  in this report would itself change that identity.
- Existing matrix JSON, readers, `features/` links, fixture/query/phase IDs,
  `gate-evidence.json`, semantic/execution contracts and the immutable execution
  baseline were not changed. All 1,327 semantic and 1,477 execution obligation IDs
  are retained; no contract hash migration or capture rewrite was performed.
- Historical installed captures keyed by the old input hash are not evidence for
  the new hash. B02–B08 remain reopened; B00/B01/B09 historical status is not
  promoted to validation of these changes. Strict/native Input/Render and Windows
  gaps remain. Fresh matching input/server/platform captures are needed before
  claiming current installed acceptance; Rust and Node passes do not supply them.
- SF10 can migrate its selected JSON sources/readers in **disjoint source
  directories**. This stage provides no `sourceFile` schema, JS/Rust resolver,
  fallback, schema version or reader behavior change. Do not absorb these local
  direct includes into a new loader or merge scenario helper contracts. Retain
  original JSON lifecycle sequences and independently authored oracles there.
- Parent action: integrate/commit the scoped SF09 work, update shared stage status,
  retain the timeout and ignored-test qualifications, and capture final provenance
  after SF10/parallel work. No other shared documentation was edited here.

## Independent-Review Correction — Authored LF On Checkout

Review identified a portability blocker: without attributes, `core.autocrlf=true`
could turn authored LF into CRLF before `include_str!`. The schema-record test's
intentional `replace('\n', "\r\n")` would then produce CRCRLF, changing parser
input and coordinates. Fix the storage boundary, not the reader or test conversion.

The follow-up started at HEAD `dc6483eb9` with existing SF09, completed SF10 and
other parent work already dirty. Only this report and these three new files were
written; no staging/commits, fixture edits, reader normalization or Rust changes:

- `crates/vela_language_service/tests/fixtures/.gitattributes`: `*.vela text eol=lf`
  and `*.toml text eol=lf`, including the two ordinary-trait package manifests.
- `crates/vela_lsp_server/tests/fixtures/.gitattributes`: `*.vela text eol=lf`.
- `tests/lsp_matrix/fixtures/sources/local/.gitattributes`: `*.vela text eol=lf`.

These basename rules apply recursively only beneath their owning directories.
The shared rule is inside `local/`; SF10's parent attributes and sibling source
subtrees are unchanged. All 97 SF09 files were scanned: **none contains CR bytes**,
so there are no intentional CRLF/mixed-EOL exceptions requiring `-text`. Original
missing final newlines and trailing indentation are preserved.

Actual follow-up validation:

- Before the fix, `git check-attr --stdin text eol` reported both attributes
  unspecified for all 97 ledger paths. Afterward, **97/97 have `text: set` and
  `eol: lf`**, both with default configuration and with
  `git -c core.autocrlf=true -c core.eol=crlf check-attr --stdin text eol`.
  Each attribute subprocess had a 10-second bound; no timeout occurred.
- All 97 byte lengths/SHA-256 values and the aggregate fixture digest above are
  unchanged. No source bytes were normalized in the working tree.
- Baseline filtered schema-record test passed. After the fix,
  `cargo test -p vela_language_service --lib --locked -- rename::tests::schema_record_tests::source_backed_schema_record_labels_produce_complete_owned_edits --exact --test-threads=4`
  passed, as did the same command with `--all-features`. Each run executed the
  existing `for crlf in [false, true]` loop, covering **both LF and generated CRLF**;
  one test passed per lane (0.08 s each). The two post-fix commands shared a
  120-second tool bound; neither timed out. No full-owner rerun is claimed here.
- Scoped attribute/report whitespace checks passed. No Rust formatting was needed.
  Attribute inspection under CRLF configuration is not a Windows checkout or
  installed-editor acceptance run.

The provenance function was called immediately before and after adding the rules
against the current tree containing SF10. `inputsSha256` changed from
`a86810aff228c7556a5d88721c929d12e234bb9ef14a90a4404c1645fc3849e4` to
`6d015fdde4633bedcef028a8da34edd70d061d1203e9a2f29513eead3ae09aeb` because the
new shared `local/.gitattributes` is itself inside the hashed fixture subtree.
`serverSha256` remained
`53139de0f93710dfa867db6a3bf2cbe6e343c5f6c94c6a87ab3f11d2aab64c23` on `darwin`.
These are follow-up snapshots, not replacements for the historical pre-SF10
identities above. No editor evidence was promoted; final integrated tree identity
and any fresh captures remain parent-owned.

## Exact Fixture And Consumer Ledger

Paths below use **S** = `tests/lsp_matrix/fixtures/sources/local/`,
**LS** = `crates/vela_language_service/`, **LSP** = `crates/vela_lsp_server/`.
Consumer lines refer to the **pre-extraction `dc6483eb9` source**, not shifted final
line numbers. Every listed consumer was compared with Rust-evaluated original
bytes; equal-looking but nonidentical sources were not normalized. Fixture links
point at the sole new authored origin. Hashes cover raw UTF-8 bytes, not stripped
markers or formatted text.

| Fixture | Bytes | SHA-256 (old = new) | Original consumers |
|---|---:|---|---|
| [LS/tests/fixtures/completion/impl-and-trait-members.vela](../../crates/vela_language_service/tests/fixtures/completion/impl-and-trait-members.vela) | 418 | `e37e1ed298377ac0c2b22dfc4e736e2857bab090ece08a7f5fd6ff86bfb5064a` | `LS/src/completion/member_tests.rs:11` |
| [LS/tests/fixtures/completion/member-index.vela](../../crates/vela_language_service/tests/fixtures/completion/member-index.vela) | 351 | `a4319245ff3605a8efb6d5233a0ec399bdf41beb375ced02f95316821f587d5a` | `LS/src/completion/member_index_tests.rs:14` |
| [LS/tests/fixtures/definition/tuple-destructuring.vela](../../crates/vela_language_service/tests/fixtures/definition/tuple-destructuring.vela) | 260 | `65c206c8877526c4393037a23f2199cf7cf68aca64ceec37c1005ff71efaeffc` | `LS/src/definition/tuple_destructuring_tests.rs:9` |
| [LS/tests/fixtures/expression-facts/tuple-destructuring.vela](../../crates/vela_language_service/tests/fixtures/expression-facts/tuple-destructuring.vela) | 436 | `9586ba2dcf2605d8479cedcab48c74567ec20bd3dd816ac1e90bfa6293376ca5` | `LS/src/expression_facts/tests.rs:52` |
| [LS/tests/fixtures/hover/tuple-destructuring.vela](../../crates/vela_language_service/tests/fixtures/hover/tuple-destructuring.vela) | 324 | `4fa64e020acb1ca8c387a8b9290cb542b270737fbb7ba1ab18e1bdbb75224e4b` | `LS/src/hover/tests.rs:486` |
| [LS/tests/fixtures/inlay/tuple-destructuring.vela](../../crates/vela_language_service/tests/fixtures/inlay/tuple-destructuring.vela) | 205 | `dbaeef2fa90d0e8404668a438f36b1f80235bd5fea7050bee9143ca46c4b3620` | `LS/src/inlay/tests.rs:305` |
| [LS/tests/fixtures/packages/ordinary-trait/api/src/api.vela](../../crates/vela_language_service/tests/fixtures/packages/ordinary-trait/api/src/api.vela) | 48 | `60bcaf02d2c87635044bc545a456b0dde008c706a06a54d7685da536b0a0b62d` | `LS/src/provider_tooling_tests.rs:84` |
| [LS/tests/fixtures/packages/ordinary-trait/api/vela.toml](../../crates/vela_language_service/tests/fixtures/packages/ordinary-trait/api/vela.toml) | 86 | `cf73d4e591f2ff73424b759acd291a05458cc1de56cf4343d5871ec2e1c026d6` | `LS/src/provider_tooling_tests.rs:88` |
| [LS/tests/fixtures/packages/ordinary-trait/plugin/src/plugin.vela](../../crates/vela_language_service/tests/fixtures/packages/ordinary-trait/plugin/src/plugin.vela) | 121 | `c236bd817378844e76f5daa21e3b5ceaf00319a523f0f42b2328331672b97cf8` | `LS/src/provider_tooling_tests.rs:85` |
| [LS/tests/fixtures/packages/ordinary-trait/plugin/vela.toml](../../crates/vela_language_service/tests/fixtures/packages/ordinary-trait/plugin/vela.toml) | 133 | `05333f4104a668579298c63a03661682fcd6af632ab2a4343544b600e29636de` | `LS/src/provider_tooling_tests.rs:93` |
| [LS/tests/fixtures/references/tuple-destructuring.vela](../../crates/vela_language_service/tests/fixtures/references/tuple-destructuring.vela) | 269 | `84d10c01fea6349726269f3638cbb4d88fc9cef0c693f2f4bc7d379efb4ec7b5` | `LS/src/references/tuple_destructuring_tests.rs:10` |
| [LS/tests/fixtures/rename/schema-record-main.marked.vela](../../crates/vela_language_service/tests/fixtures/rename/schema-record-main.marked.vela) | 334 | `2763b43f33aa0bb5aeea36c0cd791f8771410b32743abc6c030754161e0bce36` | `LS/src/rename/tests/schema_record_tests.rs:9` |
| [LS/tests/fixtures/rename/schema-record-unrelated.marked.vela](../../crates/vela_language_service/tests/fixtures/rename/schema-record-unrelated.marked.vela) | 194 | `4bc58a77212dbde898e8c1e132ef71682f26fdfdaead625c6e3ebd832180b353` | `LS/src/rename/tests/schema_record_tests.rs:11` |
| [LS/tests/fixtures/signature/default-trait.vela](../../crates/vela_language_service/tests/fixtures/signature/default-trait.vela) | 289 | `f5ca5054b6d25cd4a2365920e265379accd063a7172a3dd82eab21aed4071c35` | `LS/src/signature.rs:603` |
| [LS/tests/fixtures/signature/function-default-trait.vela](../../crates/vela_language_service/tests/fixtures/signature/function-default-trait.vela) | 358 | `805e99e4fce04b6b301b23378da23b46385be6ebe0de94f31043a7ec5e3b6d02` | `LS/src/signature.rs:638` |
| [LS/tests/fixtures/signature/function-return.vela](../../crates/vela_language_service/tests/fixtures/signature/function-return.vela) | 311 | `e1ef2351a8b9adcb5764a4c23a6ce576c26d1d7359f325a47d8967e2c14874cb` | `LS/src/signature.rs:462` |
| [LS/tests/fixtures/signature/method-return.vela](../../crates/vela_language_service/tests/fixtures/signature/method-return.vela) | 423 | `5a6b60b01fe208b62393eb2ead2854cc44ad33e206a7e8dcd6b83635757171d5` | `LS/src/signature.rs:497` |
| [LS/tests/fixtures/signature/method.vela](../../crates/vela_language_service/tests/fixtures/signature/method.vela) | 242 | `4ae5e1b3612349488087d65bf69ac286326d5455c9e9bc0cd0ec39d275a6a13d` | `LS/src/signature.rs:428` |
| [LS/tests/fixtures/signature/trait-impl.vela](../../crates/vela_language_service/tests/fixtures/signature/trait-impl.vela) | 338 | `c68dc3e7fb67ae7ba3dc534373582e1e6643c51da1b0175fef7f2610aa590093` | `LS/src/signature.rs:568` |
| [LS/tests/fixtures/signature/trait-receiver.vela](../../crates/vela_language_service/tests/fixtures/signature/trait-receiver.vela) | 222 | `1ce03c097a1773451f4bbda87223983722f0840bcaa376961e955fef8330dba9` | `LS/src/signature.rs:535` |
| [LS/tests/fixtures/symbols/nested-type-members.vela](../../crates/vela_language_service/tests/fixtures/symbols/nested-type-members.vela) | 385 | `0cceb033d2adaaefc69a94d2e94935133329a2523cf70426b1de789b6a698a02` | `LS/src/symbols.rs:746` |
| [LS/tests/fixtures/tokens/constructor-member-uses.vela](../../crates/vela_language_service/tests/fixtures/tokens/constructor-member-uses.vela) | 230 | `3b9c043551219fc07f3029e994120ff41871eca6fc9e1fcacffd90841ec499e7` | `LS/src/semantic_tokens/tests.rs:595` |
| [LS/tests/fixtures/tokens/source-enum-uses.vela](../../crates/vela_language_service/tests/fixtures/tokens/source-enum-uses.vela) | 436 | `839ae7d38a3762603a5f0c1ef81244dd89e0b0d07ab1afca294e6d8962bb6559` | `LS/src/semantic_tokens/tests.rs:664` |
| [LS/tests/fixtures/tokens/source-type-hints.vela](../../crates/vela_language_service/tests/fixtures/tokens/source-type-hints.vela) | 254 | `8e1a00043f5e092de34e6ac0547c3a10557f3b35fdc2d64c2832356d5b2a1834` | `LS/src/semantic_tokens/tests.rs:759` |
| [LSP/tests/fixtures/completion/impl-and-trait-members.vela](../../crates/vela_lsp_server/tests/fixtures/completion/impl-and-trait-members.vela) | 394 | `052abc393b470f4bce826381f7993ec533e9647e4dceb1ad0c0fbcf69ab09578` | `LSP/src/tests/completion_member.rs:791` |
| [LSP/tests/fixtures/definition/struct-field.vela](../../crates/vela_lsp_server/tests/fixtures/definition/struct-field.vela) | 209 | `abfce66a9272f51a71ef4bc063a5ec6e4478dc5ae2cf4ad85d2d44e7913aabf9` | `LSP/src/tests/definition.rs:419` |
| [LSP/tests/fixtures/signature/default-trait.vela](../../crates/vela_lsp_server/tests/fixtures/signature/default-trait.vela) | 207 | `dccea72dba7bd6c1b2f1d1f81144bb3648a6a9fb140eda6b54a8a0e81df2b69b` | `LSP/src/tests/signature.rs:400` |
| [LSP/tests/fixtures/signature/function-default-trait.vela](../../crates/vela_lsp_server/tests/fixtures/signature/function-default-trait.vela) | 264 | `bf92f75174cdf729e713cb72e36a38f7ada604cfa18bfc08766bc16b52d11269` | `LSP/src/tests/signature.rs:462` |
| [LSP/tests/fixtures/signature/method-return.vela](../../crates/vela_lsp_server/tests/fixtures/signature/method-return.vela) | 305 | `ee435a557d0c24d55f7f48afc2a2951e0c15dcecb71556375c262d6bd0dc2b69` | `LSP/src/tests/signature.rs:335` |
| [S/call-hierarchy/cross-file-impl-caller.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/cross-file-impl-caller.vela) | 159 | `5f79c66eec8a7839843ffa004d43fe584def4623d137c28cb596e8ec6eea919f` | `LS/src/call_hierarchy/tests.rs:497`; `LSP/src/tests/call_hierarchy/cross_file.rs:24` |
| [S/call-hierarchy/cross-file-impl.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/cross-file-impl.vela) | 216 | `0ef097e52a38d4324fa6a0a75b40b7a741f9434eb464268459ad7865edb69228` | `LS/src/call_hierarchy/tests.rs:507`; `LSP/src/tests/call_hierarchy/cross_file.rs:34` |
| [S/call-hierarchy/cross-file-method-caller.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/cross-file-method-caller.vela) | 159 | `50375679747a302d28d29534e199cb9e79db009a421292716bc0198e22067ea9` | `LS/src/call_hierarchy/tests.rs:331`; `LSP/src/tests/call_hierarchy.rs:317` |
| [S/call-hierarchy/cross-file-method-owner.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/cross-file-method-owner.vela) | 115 | `a58b05102d44ba5467ae4cb4bc421c936cb70f75c53df9b8b8b53401e024d14b` | `LS/src/call_hierarchy/tests.rs:340`; `LSP/src/tests/call_hierarchy.rs:326` |
| [S/call-hierarchy/cross-file-trait-caller.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/cross-file-trait-caller.vela) | 148 | `6bbc38a79228a181eed1b8904ce263896b29ed28f93163f402c96e321a10b694` | `LS/src/call_hierarchy/tests.rs:690`; `LSP/src/tests/call_hierarchy/cross_file.rs:207` |
| [S/call-hierarchy/cross-file-trait-owner.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/cross-file-trait-owner.vela) | 152 | `8270adf531239a91e38689732ab54065b94252888cb7b58586a2fea9642d4f6c` | `LS/src/call_hierarchy/tests.rs:697`; `LSP/src/tests/call_hierarchy/cross_file.rs:214` |
| [S/call-hierarchy/default-and-interface.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/default-and-interface.vela) | 301 | `6065f1b607a1559553183f18608024dca43faf38a866e9f9e02a7ccc02ce66a2` | `LS/src/call_hierarchy/tests.rs:600`; `LSP/src/tests/call_hierarchy.rs:557` |
| [S/call-hierarchy/trait-impl.vela](../../tests/lsp_matrix/fixtures/sources/local/call-hierarchy/trait-impl.vela) | 347 | `decc89298d453d5e186c8280c2110bd9da5f4b63a1188e436ceb135445328e14` | `LS/src/call_hierarchy/tests.rs:417`; `LSP/src/tests/call_hierarchy.rs:448` |
| [S/completion/function-return.vela](../../tests/lsp_matrix/fixtures/sources/local/completion/function-return.vela) | 246 | `f5e11584798dfa265dfe6ef633b97bd07957df8b96a375378852264ec8e2bc93` | `LS/src/completion/member_tests.rs:204`; `LSP/src/tests/completion_member.rs:689` |
| [S/completion/method-return.vela](../../tests/lsp_matrix/fixtures/sources/local/completion/method-return.vela) | 322 | `26ae003bb7610f03d0946c9d787fb739c0c51e8366a45f600cf46e73505c0b3b` | `LS/src/completion/member_tests.rs:243`; `LSP/src/tests/completion_member.rs:723` |
| [S/definition/method-return-type.vela](../../tests/lsp_matrix/fixtures/sources/local/definition/method-return-type.vela) | 181 | `d8ee673f2edfc471008b7df04d43a38f2024ac908d9c6d0c068f738adc26104a` | `LS/src/definition/type_tests.rs:544`; `LSP/src/tests/definition/type_definition.rs:477` |
| [S/folding/items-and-blocks.vela](../../tests/lsp_matrix/fixtures/sources/local/folding/items-and-blocks.vela) | 333 | `a22129af82f4939ba1d128cec2757e12295ac605b0b1dbfd4afea6d66e216fa8` | `LS/src/folding.rs:612`; `LSP/src/tests/folding.rs:15` |
| [S/formatting/container-hints.expected.vela](../../tests/lsp_matrix/fixtures/sources/local/formatting/container-hints.expected.vela) | 346 | `5580e1510c30a96e989afb1e54becb0b284a433a9c464635c8c5239a479500b9` | `LS/src/formatting/tests.rs:572`; `LSP/src/tests/formatting.rs:224` |
| [S/formatting/container-hints.input.vela](../../tests/lsp_matrix/fixtures/sources/local/formatting/container-hints.input.vela) | 302 | `a4ea548bd06dfa555e4d2b414770e8c66cadd6babdc1c0aeb2f74a0f81c5bbc4` | `LS/src/formatting/tests.rs:562`; `LSP/src/tests/formatting.rs:200` |
| [S/hover/cross-file-main.vela](../../tests/lsp_matrix/fixtures/sources/local/hover/cross-file-main.vela) | 236 | `71963d35bc0daa2e71d5c7f121ca8f70c8ec63607c9b7d5f40e1ae6c1e53960a` | `LS/src/hover/cross_file_tests.rs:12`; `LSP/src/tests/hover/cross_file.rs:15` |
| [S/hover/cross-file-rewards.vela](../../tests/lsp_matrix/fixtures/sources/local/hover/cross-file-rewards.vela) | 251 | `88eacf208c03553252f1d2f675550337c8048d223bb16ee05bb725341d0c4175` | `LS/src/hover/cross_file_tests.rs:21`; `LSP/src/tests/hover/cross_file.rs:24` |
| [S/inlay/any-lambda-and-control.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/any-lambda-and-control.vela) | 274 | `b8daa58c5004cdf572ab1605d4c7172c1835347387cf1c9ed96754a76da6275a` | `LS/src/inlay/suppression_tests.rs:552`; `LSP/src/tests/inlay_suppression.rs:87` |
| [S/inlay/any-parameters.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/any-parameters.vela) | 247 | `71b69cd71b16cd3dc8196a64957e2f5786a961ba575f8480ef203f0611afbac1` | `LS/src/inlay/suppression_tests.rs:259`; `LSP/src/tests/inlay.rs:504` |
| [S/inlay/any-return-negative-and-control.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/any-return-negative-and-control.vela) | 373 | `69af7bf25df13ef0e93d46ca8e51d91e66869f9624cccb94a2d324cd2a797001` | `LS/src/inlay/suppression_tests.rs:323`; `LSP/src/tests/inlay_suppression.rs:924` |
| [S/inlay/function-return-any-parameter.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/function-return-any-parameter.vela) | 271 | `d1d8632efbe33bf8ee64512d4afa3f00081c502d0bbc6a0b00b7ffd23ee46918` | `LS/src/inlay/suppression_tests.rs:287`; `LSP/src/tests/inlay.rs:551` |
| [S/inlay/function-return-parameters.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/function-return-parameters.vela) | 229 | `af97aaa00cdbbfc6cceeb9408456eb7fb69d1a859f3c54f71eaef2d5a7019e8f` | `LS/src/inlay/tests.rs:210`; `LSP/src/tests/signature.rs:273` |
| [S/inlay/function-return-trait-any-parameter.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/function-return-trait-any-parameter.vela) | 314 | `cd4f24ff35e4ff331db2a577555966b2386a56ac05fb49b327207efe86ef9916` | `LS/src/inlay/suppression_tests.rs:395`; `LSP/src/tests/inlay_suppression.rs:850` |
| [S/inlay/lambda-facts.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/lambda-facts.vela) | 326 | `3d5280a6c1f3c3dc0229f89c7068108117da694f7df78c42d521bd53c0e51b95` | `LS/src/inlay/tests.rs:360`; `LSP/src/tests/inlay.rs:197` |
| [S/inlay/method-parameters.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/method-parameters.vela) | 172 | `9fe88466813b12dcba3d5c64e6947f13a262c73cf857838c627cfe8dedcbafd2` | `LS/src/inlay/tests.rs:159`; `LSP/src/tests/inlay.rs:102`; `LSP/src/tests/signature.rs:212` |
| [S/inlay/method-return-any-parameter.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/method-return-any-parameter.vela) | 349 | `fdc9c943670c963585e262d16cb7490a9a472460ae393f0e8342a5ea701c6a3e` | `LS/src/inlay/suppression_tests.rs:355`; `LSP/src/tests/inlay_suppression.rs:990` |
| [S/inlay/method-return-trait-any-parameter.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/method-return-trait-any-parameter.vela) | 392 | `5f3e7d6b7502d2996889b69bc8ac453d22ad50e4defeb2e2a816adcafe6c141e` | `LS/src/inlay/suppression_tests.rs:433`; `LSP/src/tests/inlay_suppression.rs:1067` |
| [S/inlay/variant-any-and-control.vela](../../tests/lsp_matrix/fixtures/sources/local/inlay/variant-any-and-control.vela) | 167 | `5b943087d2f3496da0ea51ff9740cade0e404979a766bdb4c23e4045289e6fd7` | `LS/src/inlay/suppression_tests.rs:473`; `LSP/src/tests/inlay.rs:725` |
| [S/reference-receivers/function-default-trait.vela](../../tests/lsp_matrix/fixtures/sources/local/reference-receivers/function-default-trait.vela) | 321 | `d90e67548219805a94830a423242fae2aed784a30387212d81b520a220d69d61` | `LS/src/references/source_return_tests.rs:65`; `LS/src/references/source_return_tests.rs:313`; `LSP/src/tests/references/returned_receivers.rs:724`; `LSP/src/tests/references/returned_receivers.rs:888` |
| [S/reference-receivers/function-method.vela](../../tests/lsp_matrix/fixtures/sources/local/reference-receivers/function-method.vela) | 285 | `e20cd93e9f26990041e82167edd71445050644b76f7423d1f78e30dbe19280ab` | `LS/src/references/source_return_tests.rs:9`; `LS/src/references/source_return_tests.rs:261`; `LSP/src/tests/references/returned_receivers.rs:646`; `LSP/src/tests/references/returned_receivers.rs:810` |
| [S/reference-receivers/method-default-trait.vela](../../tests/lsp_matrix/fixtures/sources/local/reference-receivers/method-default-trait.vela) | 418 | `8db1aacb140990fe422a6447358645cb71bbd64ceb3fbc410dab9fbdc9e16d83` | `LS/src/call_hierarchy/tests/returned_receivers.rs:113`; `LS/src/references/source_return_tests.rs:191`; `LS/src/references/source_return_tests.rs:429`; `LSP/src/tests/call_hierarchy/source_method_return_receivers.rs:164`; `LSP/src/tests/references/source_method_return_receivers.rs:206` |
| [S/reference-receivers/method-method.vela](../../tests/lsp_matrix/fixtures/sources/local/reference-receivers/method-method.vela) | 376 | `594d0808b70f2bd266957f5542e188ddac5130e3b534edfd303e28d8fea4e45f` | `LS/src/call_hierarchy/tests/returned_receivers.rs:6`; `LS/src/references/source_return_tests.rs:129`; `LS/src/references/source_return_tests.rs:372`; `LSP/src/tests/call_hierarchy/source_method_return_receivers.rs:139`; `LSP/src/tests/references/source_method_return_receivers.rs:181` |
| [S/references/enum-constructors-and-patterns.vela](../../tests/lsp_matrix/fixtures/sources/local/references/enum-constructors-and-patterns.vela) | 316 | `05c032841e55566ac0cd7d85422a36ff45cbee41e3337c8a83270d9ab6fcb5aa` | `LS/src/references/tests.rs:349`; `LSP/src/tests/references.rs:313` |
| [S/references/field-read-write.vela](../../tests/lsp_matrix/fixtures/sources/local/references/field-read-write.vela) | 163 | `33a1c109eb38b0546f271110f77c6b7f1631d0fcd5002ce2a709cb6aa3dc49e5` | `LS/src/references/tests.rs:297`; `LSP/src/tests/references.rs:137` |
| [S/references/imported-record-members.vela](../../tests/lsp_matrix/fixtures/sources/local/references/imported-record-members.vela) | 184 | `a2dbec7b98fcb26c0797aad8b74f2936569ceb981d01936f29d056eeb537d9a7` | `LS/src/references/field_tests.rs:134`; `LS/src/references/highlight_tests.rs:433`; `LSP/src/tests/references/cross_file.rs:24`; `LSP/src/tests/references/highlight.rs:576` |
| [S/references/imported-type-alias.vela](../../tests/lsp_matrix/fixtures/sources/local/references/imported-type-alias.vela) | 158 | `492e9b7c18673d6a80112f9b440e075334f7c4509e4d785bbf7a3eee4c861eb4` | `LS/src/references/highlight_tests.rs:323`; `LS/src/references/value_tests.rs:176`; `LSP/src/tests/references/highlight.rs:430`; `LSP/src/tests/references/values.rs:232` |
| [S/references/imported-value-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/references/imported-value-uses.vela) | 147 | `c63136f92305f05a3dc8f33eed3b7ee24b51d43f80ed52780551a33894f275f1` | `LS/src/references/highlight_tests.rs:193`; `LS/src/references/value_tests.rs:11`; `LSP/src/tests/references/highlight.rs:240`; `LSP/src/tests/references/values.rs:17` |
| [S/references/imported-variant-field-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/references/imported-variant-field-uses.vela) | 295 | `0b2ebb93f00e11169201e2fa4ef219bd4da111afab0bdc6257ae57bc83dcc43d` | `LS/src/references/variant_field_tests.rs:128`; `LSP/src/tests/references/cross_file.rs:777` |
| [S/references/imported-variant-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/references/imported-variant-uses.vela) | 284 | `34947a1311477561a62b1540850a3740966cb87bb5e70fae056f71006ba5f92a` | `LS/src/references/variant_field_tests.rs:60`; `LSP/src/tests/references/cross_file.rs:686` |
| [S/references/local-read-write-calls.vela](../../tests/lsp_matrix/fixtures/sources/local/references/local-read-write-calls.vela) | 168 | `6bce42b983ac9de7c3dd44cbf44da73937643bbc60dc8ebf98cbdbb5df1a903f` | `LS/src/references/tests.rs:590`; `LSP/src/tests/references.rs:662` |
| [S/references/record-explicit-field.vela](../../tests/lsp_matrix/fixtures/sources/local/references/record-explicit-field.vela) | 179 | `8feec3ffce14a1ec396d8e8a0d7fee72371d9336521e3a379972c0649193ca87` | `LS/src/references/field_tests.rs:9`; `LSP/src/tests/references.rs:197` |
| [S/references/record-shorthand-field.vela](../../tests/lsp_matrix/fixtures/sources/local/references/record-shorthand-field.vela) | 171 | `83a63905c48be36f74b9785e0c70aa21971f92e408a511d3a62480ce844184c0` | `LS/src/references/field_tests.rs:60`; `LSP/src/tests/references.rs:255` |
| [S/references/schema-explicit-field.vela](../../tests/lsp_matrix/fixtures/sources/local/references/schema-explicit-field.vela) | 160 | `77d4c96b21c33eb2e0748fba3cd12a1ad49a5007cab8dd297dc507f0efbdb035` | `LS/src/references/schema_field_tests.rs:10`; `LSP/src/tests/references/schema.rs:247` |
| [S/references/schema-shorthand-field.vela](../../tests/lsp_matrix/fixtures/sources/local/references/schema-shorthand-field.vela) | 153 | `7d6c8a3fd76771c820c1ef8905c09bd461e52a84378adb362d5853ba64b6b487` | `LS/src/references/schema_field_tests.rs:100`; `LSP/src/tests/references/schema_fields.rs:11` |
| [S/references/schema-unit-variants.vela](../../tests/lsp_matrix/fixtures/sources/local/references/schema-unit-variants.vela) | 196 | `a506766c16642880e19e59d9adf120e87bdb509a5390ca75134e15031fee9e7f` | `LS/src/references/schema_tests.rs:723`; `LS/src/rename/tests.rs:655`; `LSP/src/tests/references/schema.rs:569`; `LSP/src/tests/rename_schema.rs:260` |
| [S/references/schema-variant-field-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/references/schema-variant-field-uses.vela) | 266 | `b30f3405ceaeb9dfea8ba54d9dfd28fde0fdeeef72469a118d9c6a0d65138100` | `LS/src/references/schema_field_tests.rs:267`; `LSP/src/tests/references/schema_variant_fields.rs:106` |
| [S/references/schema-variant-highlights.vela](../../tests/lsp_matrix/fixtures/sources/local/references/schema-variant-highlights.vela) | 189 | `7e1ce3c99be95768cbd6d1538979168790f28fb0b503fc42bf9bd69f87af9d80` | `LS/src/references/highlight_tests.rs:10`; `LSP/src/tests/references/schema.rs:723` |
| [S/references/script-method-calls.vela](../../tests/lsp_matrix/fixtures/sources/local/references/script-method-calls.vela) | 218 | `b44f655ccbf671eff39f70fd31010417320760a40a779b9a5d3d7970c0067da5` | `LS/src/call_hierarchy/tests.rs:248`; `LS/src/references/tests.rs:408`; `LS/src/references/tests.rs:659`; `LSP/src/tests/call_hierarchy.rs:196`; `LSP/src/tests/references.rs:437`; `LSP/src/tests/references.rs:747` |
| [S/references/trait-impl-highlights.vela](../../tests/lsp_matrix/fixtures/sources/local/references/trait-impl-highlights.vela) | 187 | `5cb3b6c92992770203166681f5a2469c23fb7b58f8581b083582098486745c51` | `LS/src/references/tests.rs:703`; `LSP/src/tests/references.rs:803` |
| [S/references/trait-impl-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/references/trait-impl-uses.vela) | 317 | `0bb90ae6468035261a202e2bf12dae627196c83c2879928b5d05948148169d0d` | `LS/src/references/tests.rs:457`; `LSP/src/tests/references.rs:494` |
| [S/references/variant-field-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/references/variant-field-uses.vela) | 327 | `068784907f059eccf361f5101b70ef5b4f5f7b9a5a437ee69b11971be5f1568a` | `LS/src/references/variant_field_tests.rs:9`; `LSP/src/tests/references.rs:375` |
| [S/rename/function-default-trait.vela](../../tests/lsp_matrix/fixtures/sources/local/rename/function-default-trait.vela) | 316 | `dc1fc544f27ff36e4f9823926492970923ad554dd2f0663d5a40517488a728ad` | `LS/src/rename/source_return_tests.rs:29`; `LSP/src/tests/rename_source_return.rs:74` |
| [S/rename/private-enum-variant.vela](../../tests/lsp_matrix/fixtures/sources/local/rename/private-enum-variant.vela) | 304 | `96163b4413793b4402ef9a6a112ded02e4e661bfb60139b985d44181a5812556` | `LS/src/rename/tests.rs:433`; `LSP/src/tests/rename.rs:452` |
| [S/rename/private-method.vela](../../tests/lsp_matrix/fixtures/sources/local/rename/private-method.vela) | 259 | `8b033c5627f238a7889baeecfc7fac33e8ea9e889d230dd71ecb3b93cd29ea7a` | `LS/src/rename/tests.rs:358`; `LSP/src/tests/rename.rs:393` |
| [S/rename/trait-impl-collision.vela](../../tests/lsp_matrix/fixtures/sources/local/rename/trait-impl-collision.vela) | 241 | `fd9f285d98bd09ea2cfd5669aa27e8cf65b836e7cab46b6b89563b48567a362e` | `LS/src/rename/tests.rs:404`; `LSP/src/tests/rename_collisions.rs:136` |
| [S/returned-receivers/function-default-trait.vela](../../tests/lsp_matrix/fixtures/sources/local/returned-receivers/function-default-trait.vela) | 295 | `0e8331ed61a85a14d9705717002e550ad7bfaaae187d687057b98afdfe53308b` | `LS/src/definition/source_return_tests.rs:125`; `LS/src/hover/tests.rs:959`; `LSP/src/tests/definition.rs:493`; `LSP/src/tests/hover.rs:501` |
| [S/returned-receivers/function-method.vela](../../tests/lsp_matrix/fixtures/sources/local/returned-receivers/function-method.vela) | 239 | `401d6d19bcc3fc77060809d087cdcb34b3a7f7b1b6291499bff1cec35f9163f8` | `LS/src/definition/source_return_tests.rs:62`; `LS/src/hover/tests.rs:766`; `LSP/src/tests/definition/source_return_receivers.rs:43`; `LSP/src/tests/hover/source_return_receivers.rs:9` |
| [S/returned-receivers/method-default-trait.vela](../../tests/lsp_matrix/fixtures/sources/local/returned-receivers/method-default-trait.vela) | 376 | `2b8b37be8dac30aa9526689a94641cb31fce0ea6e69c5d74636055d50b1b0f7a` | `LS/src/definition/source_return_tests.rs:155`; `LS/src/hover/tests.rs:1004`; `LSP/src/tests/definition/source_return_receivers.rs:98`; `LSP/src/tests/hover/source_return_receivers.rs:62` |
| [S/returned-receivers/method-method.vela](../../tests/lsp_matrix/fixtures/sources/local/returned-receivers/method-method.vela) | 320 | `f3d11876349ce1473bf1496820b0c410613439e7263b44b5473fff3e2cf08bbd` | `LS/src/definition/source_return_tests.rs:90`; `LS/src/hover/tests.rs:805`; `LSP/src/tests/definition/source_return_receivers.rs:68`; `LSP/src/tests/hover/source_return_receivers.rs:33` |
| [S/tokens/any-return.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/any-return.vela) | 236 | `74a882c01c8cd17b193c1e60ebe7800dd413905fd0838fd1f4a11e292b72398e` | `LS/src/semantic_tokens/source_call_tests.rs:47`; `LSP/src/tests/semantic_tokens_source.rs:110` |
| [S/tokens/function-default-trait.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/function-default-trait.vela) | 263 | `8d05c9e465b76b515452168185b54ec93e576c5ddc9aa0ab78b4589598149a70` | `LS/src/semantic_tokens/source_call_tests.rs:181`; `LSP/src/tests/semantic_tokens_source.rs:470` |
| [S/tokens/function-return.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/function-return.vela) | 224 | `e88986789797ca03fcc83227e1d8ba4ef6327d8239c99a51bb59721ab4ee06f4` | `LS/src/semantic_tokens/source_call_tests.rs:9`; `LSP/src/tests/semantic_tokens_source.rs:28` |
| [S/tokens/imported-enum-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/imported-enum-uses.vela) | 247 | `41d7f21f82f9981a091dbb90678f40f1d1aa1c32ab49bf92620aaf8867a2f95d` | `LS/src/semantic_tokens/variant_use_tests.rs:10`; `LSP/src/tests/semantic_tokens_source.rs:313` |
| [S/tokens/imported-return-owner.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/imported-return-owner.vela) | 170 | `f57fe85485574f0ee137838941f3e436ffbe4d3977dd46451aa88b9e985c5e1d` | `LS/src/semantic_tokens/source_call_tests.rs:102`; `LSP/src/tests/semantic_tokens_source.rs:202` |
| [S/tokens/method-default-trait.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/method-default-trait.vela) | 339 | `7a734e6016db803bb2fc09a67d692222957afaa3e58bbe2f845b47a8e4e5c17b` | `LS/src/semantic_tokens/source_call_tests.rs:220`; `LSP/src/tests/semantic_tokens_source.rs:551` |
| [S/tokens/method-return.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/method-return.vela) | 300 | `923eaa08280e645c5a46c2291ab7fd7b54c2d49c3b9850fdbc6b3702665b131d` | `LS/src/semantic_tokens/source_call_tests.rs:140`; `LSP/src/tests/semantic_tokens_source.rs:386` |
| [S/tokens/schema-enum-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/schema-enum-uses.vela) | 220 | `fa9b48876fa2fef1742e3486c1e834933787926903650538c59bf63cb9e2079c` | `LS/src/semantic_tokens/tests.rs:709`; `LSP/src/tests/semantic_tokens_schema.rs:561` |
| [S/tokens/script-members.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/script-members.vela) | 255 | `d8c3fbe1c9262dde2154bd10ba50086351f7f86d87c86f4b30d2151ece5a69e5` | `LS/src/semantic_tokens/tests.rs:468`; `LSP/src/tests/semantic_tokens.rs:487` |
| [S/tokens/typed-member-uses.vela](../../tests/lsp_matrix/fixtures/sources/local/tokens/typed-member-uses.vela) | 195 | `b1adb6136d9875f0706ce359eccc365ab8f0df5b172e7c9dfd63f85f1aab50e9` | `LS/src/semantic_tokens/tests.rs:554`; `LSP/src/tests/semantic_tokens.rs:613` |
