# Checkpoint D consumer boundary

Base: checkpoint C `f55b2d770`. Removal-only consumer changes; no commits,
replacement opaque objects, conversion APIs, re-entry model, or native captures.
This note supersedes affected pre-D success claims, not unrelated accepted proof.

## Contract and fixture disposition

- Tooling schema is **4**. Schema 3 and removed Host/view facts,
  scoped-resource returns, index capabilities, receiver/storage capabilities,
  collection-view and constructor fields reject rather than default. Null and
  empty removed fields also reject. There is no opaque schema replacement.
- Existing registered Value schemas retain identity/ABI, fields, methods/traits,
  synchronous value signatures, docs and source locations. The CLI test uses an
  actual existing `Value` derive and compares the exported registry facts.
- Retained value-tooling fixtures use record metadata, not renamed Host handles.
  Schema/source records with colliding names retain separate navigation,
  references and rename ownership. Ordinary source coverage remains exact.
- Borrowed query successes are retired: 45 `hover-s3` rows, six
  `completion-callable-hints` rows, four `completion-stdlib-arguments` rows, and
  the dependent `signature-s3` group rows now prove syntax rejection. They do not
  certify supported view signatures, parameters or hover facts. Selection and
  outline fixtures may retain removed spellings as explicit syntax recovery.
- Extern-state bindings have no state facts. Ordinary VM-owned `state`, imported
  state references, source declarations, collections, reflection and script
  reload remain. Mixed state fixtures now use ordinary state initializers;
  independent extern/release rejection tests explicitly cover the removed path.
- `host` semantic-token provenance and release-insertion actions are removed.
  Schema-native values use `schema` provenance. Removed extern declarations and
  their uses have lexical/unresolved tokens, not ordinary state metadata.
- Borrowed/transparently mutating and dependent construction/method examples,
  their test registrations and the dependent build script are removed. Existing
  native/value, serde, module/package, reload, script-state examples and
  independent safety rejection tests remain.
- Portable fuzz payload mirrors compiler format **8**, synchronous fields only;
  there is no legacy decoder.

## Validation actually run

All Rust commands used `rustup run 1.98.0`. These are bounded native test runs,
not installed-editor interaction evidence or complete repository acceptance.

| Gate | Result |
| --- | --- |
| Language-service all-features tests | **769 passed, 2 failed, 2 ignored**; the two failures consume excluded parent highlighting goldens listed below. |
| LSP-server all-features tests | **830 unit + 7 binary + 10 stdio tests passed**, zero failed. Serial run completed in a 420-second command bound; library tests took 230.84 seconds. |
| CLI and playground all-features tests | **15 passed**: CLI metadata 1, CLI scripts 7, playground 7. |
| Examples all-features tests | **15 passed**: source rejection 2, runnable examples 13. |
| Tooling/CLI/playground Clippy, all targets/features, `-D warnings` | Passed. |
| Examples Clippy, all targets/features, `-D warnings` | Passed. |
| Formatting: affected Rust packages, examples and fuzz | Passed. |
| VS Code package validator | Passed. |
| Removed-consumer Node guard | **8 passed**, zero failed. |
| Root LSP-matrix Node self-tests | **292 passed, 7 failed**, across the five excluded parent scripts below. |
| Site syntax, placeholder and build gates | Passed: **36 scopes / 36 token checks**, Astro **0 errors / 0 warnings / 0 hints**, **145 pages** built. |
| Fuzz `cargo check --manifest-path fuzz/Cargo.toml --bins` | Passed. |
| Portable fuzz smoke, `-runs=16 -max_len=32` | **16 runs completed**. Non-sanitized Cargo executable warned about missing sanitizer callbacks/coverage; this is not a coverage-guided fuzz campaign. |

Earlier broad runs timed out at 120/180/240 seconds. A concurrent LSP run timed
out at **420 seconds** after **827 passes and one then-unfixed token expectation
failure**, with calls/schema-lifecycle token matrices unfinished. It was not a
full pass. The later completed serial LSP run above supersedes that incomplete
run. No installed VS Code/native interaction suite or new native capture was run.

## Parent-owned follow-up, not edited here

Language-service failures:

- `tests/fixtures/lsp_highlighting/consistency.json` still declares `host`
  provenance; `semantic_tokens_highlighting_consistency_table_uses_known_taxonomy`
  rejects it against the reduced legend.
- `tests/fixtures/lsp_highlighting/showcase.vela` still uses `extern state`;
  `semantic_tokens_highlighting_showcase_pins_current_collapses` expects the
  former state token at line 29. Ordinary state should remain independently
  covered when the parent refreshes this golden.

Root Node golden failures (seven tests, five files):

- `scripts/lsp-matrix/invalid-config-schema-contracts.test.js:50`
- `scripts/lsp-matrix/workspace-picker-contracts.test.js:16`
- `scripts/lsp-matrix/workspace-symbol-lifecycle.test.js:9`
- `scripts/lsp-matrix/workspace-symbol-ownership.test.js:9` and `:31`
- `scripts/lsp-matrix/workspace-symbol-types.test.js:8` and `:28`

These still pin old schema diagnostics, Host/Class partitions or source/schema
ordering. Update the parent goldens to the retained Value/Struct contract; do not
restore Host metadata to satisfy them. Parent also owns root Cargo/lockfile and
status/design documentation reconciliation (tooling schema 4 / portable format
8 and the actual remaining repository gates).

Existing native captures/hashes are historical for changed schema, token and
extern-state fixtures. Their old hashes have not been rewritten to manufacture
fresh acceptance. Unaffected evidence remains intact; changed routes require a
separate authorized capture/audit before they can claim current native proof.
