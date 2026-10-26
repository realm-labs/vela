# Test Input Organization And Selective Fixture Extraction

Status: SF00–SF11 implementation and SF12 residual audit completed, one commit per
stage. Validation limits remain open: no aggregate Rust exit-zero run and installed
editor 26/27, not full acceptance. See
[the execution record](archive/test-input-organization-acceptance.md).
This replaces the previous TO00–TO09 completion criteria.

## Objective And Authority

**Improve test readability and consolidate meaningful repeated inputs; extract
source files where doing so helps, not mechanically for every string.** Review
the whole repository, but keep concise local inputs inline when that makes the
input/assertion relationship clearer. Reused or substantial programs, source trees
and source/golden families are strong extraction candidates. There is no target
of zero inline scripts, a fixed line-count threshold or maximum fixture count.
Use existing `rstest`, ordinary Rust tests and specialized runners—not a custom
framework, registration macro, test catalog or expectation DSL.

[goal.md](goal.md) and [architecture.md](architecture.md) remain authoritative.
Runtime simplification is complete and is not reopened. This work changes test
organization, not language semantics, runtime APIs or executable artifact formats.
[validation.md](validation.md) remains the gate source of truth.

### Correction To The Previous Pass

Commits `0f1f66d9d` through `044e5afc7` delivered useful parameterization, eleven
shared source fixtures and helper cleanup. Preserve them. The
[TO execution record](archive/test-organization-acceptance.md) records actual
changes and tests, **not completion of the broader input-organization review**.
Revisit source-storage choices instead of treating rstest adoption as completion;
reviewed inline retention is also a valid outcome.

In particular, `crates/vela_engine/src/tests/strings.rs` has three named cases
but still embeds all three programs. Review each for useful extraction rather than
assuming either that rstest finishes the work or that all three need files.
VM numeric/bytes cases, diagnostic rows, lifecycle versions and tooling JSON need
the same suitability review. Neither source length nor test category alone decides
storage; context, reuse, byte sensitivity and navigation cost matter.

### Reference Patterns Actually Reviewed

The read-only reference audit checked executable fixtures, paired parser/UI
expectations, formatter cases, compiler inputs, reload versions and Cargo fixture
invalidation—not just dependency declarations. The useful patterns are:

| Pattern observed | Vela adaptation |
|---|---|
| Feature-local source directories discovered by `#[files]` | One independently selectable file per homogeneous case, using the current owner runner. |
| Named cases carrying `include_str!` inputs and paired outputs/options | Keep typed Rust expectations and options; source and long textual goldens live in files. |
| Source plus diagnostic sidecar and exact-pair inventory checks | Reuse Vela's existing `.expected`, typed diagnostics and reviewed JSON oracles; no new universal schema. |
| Reload scenarios loading explicit version files | Use version files for substantial scenarios; retain short adjacent version pairs when they make the change clearer. Preserve every intermediate assertion. |
| Directory change tracking for macro-discovered inputs | Verify add/delete discovery without a clean build; add only minimal Cargo invalidation if required. |

The reference is not uniformly file-first: some inline cases and source generation
remain there too. Choose the representation that fits each Vela test; do not copy another
runtime's features, boolean-result convention, test framework or validation policy.
Keep repository artifacts domain-neutral and free of sibling-project paths.

## Additional Structural Comparison Findings

The follow-up audit compared actual owner modules, integration targets, process
helpers, formatter/runtime routes, repository consumers and CI—not just fixture
layouts. The following obligations supplement selective extraction. Distinguish
**preserving existing proof** from **adding missing coverage** in stage reviews;
source relocation must not silently change assertions or product behavior.

| Area | Existing baseline / remaining obligation | Stage |
|---|---|---|
| Semantic ownership and harness placement | Shared block-tail/conformance inputs already have distinct compiler/VM assertions; Engine and VM already aggregate integration modules. Record what each family proves (resolution, lowering/verifier, execution, public API), each target's boundary, and helper ownership/consumers. Do not run every input through every layer or move all integration tests under `src/`. | SF00, owning stages |
| Formatter corpus reuse | Syntax, language-service and LSP tests already check selected idempotence cases. Add a small explicit retained-valid source selection with successful reparsing, second-format equality and selected executable before/after result equivalence. Keep malformed-source behavior, byte-sensitive cases and layout goldens separate. | SF02 |
| Source/portable execution equivalence | Engine has a decoded-program execution smoke; bytecode has round-trip, structural corruption and old-format rejection tests. Add a small portable-supported set comparing source execution with encode/decode/link execution in a fresh receiving Engine, with equivalent arguments/settings and exact results or errors/effects. Codec round-trip alone is not execution equivalence. | SF04–SF06 |
| Runtime modes and lifecycle endpoints | Existing scalar-plan tests pin exact budget failure points/overflow spans; re-entry tests cover roots, failure unwind and reuse. Preserve relevant mode selections, cleanup and effect ordering. Reload proof includes final-old-owner reclamation, no generation self-retention and isolated state for Runtimes sharing an image—not just old/new return values. | SF05–SF06 |
| Actual CLI process boundaries | Existing process helpers use unbounded `Command::output()`. Add bounded process/output capture and isolated temporary ownership; extend actual dispatch proof to supported schema, path and filesystem routes, alongside full UI-C goldens. | SF08 |
| Advertised executable inputs | All 14 example bins already execute. The four site playground programs are not directly tied to their own execution expectations by native WASM tests. Connect their actual source/entry names to existing compile/run routes, with independently authored results; do not assume similar-looking test programs are identical. | SF08, SF11 |
| Grammar/query consumers | Tree-sitter corpus, highlighting checks and historical query execution already exist. Name the current retained-valid parse selection and execution of the six shipped Zed query files on an existing complete fixture, checking captures/ranges. Package-text checks alone do not prove query behavior. | SF11 |
| Benchmark correctness preflight | Existing benchmark compilation/checksum plumbing is not a complete semantic oracle. Add untimed, low-iteration expected-result checks for the external-comparison workloads; selected-runtime execution failures must fail distinctly from unavailable optional runtimes. Preserve workload IDs and timing windows; no performance claim. | SF11 |

Implementation constraints for these obligations:

- Use ordinary review notes for ownership, not an executable catalog. A helper's
  contract includes error phase, setup freshness and instrumentation; VM execution
  helpers must not swallow compiler diagnostics. Do not cache a mutable global
  Engine or consolidate deliberately different helpers just to reduce duplication.
- Keep actual external-consumer, macro visibility, process and allocator contracts
  at their current boundaries. Any justified target move must preserve feature
  gates, ignored status, filters and assertions, and review newly concurrent global
  state/temp paths. There is no target-count or test-runtime reduction quota.
- Select relevant runtime modes explicitly, not a Cartesian product of all inputs,
  GC settings, cache/profile settings and physical plans. Cleanup assertions must
  respect legitimately retained results/state rather than require universal zero
  allocations. Portable coverage does not add package metadata, retained MIR or
  process-local generation encoding to the current format.
- CLI helpers stay owner-local: finite deadlines and both-stream output caps,
  kill/reap on failure, and atomic temporary-directory creation with collision and
  unwind-cleanup tests. Keep exact status/goldens. Exercise `--print-schema` without
  script execution, missing source files, controlled relative cwd and filesystem
  roots at the script directory, including an unchanged outside sentinel. Do not
  invent additional filesystem restrictions or CLI commands.
- Reuse the actual site programs without a second authored copy or a new fixture
  schema. Keep independently expected results and existing compile/run contracts.
  Ordinary prose fences are not automatically executable test cases.

### Separate Coverage Gap: Real Browser Execution

The site now has native actual-source and emitted-payload checks, but no actual
browser execution gate. Native WASM tests and wasm32 compilation do not exercise generated JS glue, asset
base paths or Playground UI behavior. Editor-owned `playwright-core` is present,
but is not a site browser suite. Track a separately scoped finite smoke for actual
Compile/Run of the four playground examples, displayed results/diagnostics and
page/asset failures. Reuse an existing browser tool where appropriate; do not
implement a browser framework, new workers/session APIs or new product features.
This is **not an SF source-organization completion requirement** and must remain
explicitly open rather than being marked passed by site builds or editor tests.

Do not import reference-only source-free SDK features, async/JIT/object APIs,
a script-language test command, syntax-aware structural linter or its CI-only
validation policy. Test proposals in reference docs are not implemented features.
Vela's current default/affected-feature/codec/serde/platform gates and production
dependency boundaries remain authoritative.

## Current Inventory And Remaining Scope

Audit baseline: `044e5afc7`; working tree was clean before this planning change.
There are 23 root-workspace crates, 18 with dev-only `rstest`, plus separate
examples/fuzz/editor/site owners. Root conformance currently contains 22 `.vela`
files, eleven introduced by TO00/TO02. Existing file-backed inputs are preferred
canonical origins; evidence-backed deduplication may change their location.

The table's **signal counts are triage only**, not counts of programs/tests or
completed migrations. They count tracked Rust files, including benches and mixed
production/test files, containing this exact regex:

```text
r#+"|"(pub )?fn |"let |"state |"struct |"enum |"(pub )?use |"(pub )?mod[ ]
```

Method: enumerate `git ls-files`, select `crates/**/*.rs`, apply `rg -l` to those
paths and group by crate. There are 420 matching files. Raw Rust strings/audit
needles cause false positives; const/trait-only documents, expressions and assembled
inputs can be missed. **A zero regex count does not close an owner.** SF00 follows
source arguments and helper consumers as well as lexical searches.

The following tables define review coverage, **not a mandatory extraction list**.
Apply the suitability rules below to each family; no-change is allowed when
supported by its readability/semantics rather than a blanket owner exemption.

| Owner | Signal files | Input families to review / likely disposition |
|---|---:|---|
| `vela_common` | 1 | Diagnostic renderer source documents and suitable output goldens; retain Rust diagnostics/spans. |
| `vela_def` | 0 | Identity/hash unit data stays Rust; verify no hidden source consumers. |
| `vela_registry` | 0 | Type-hint fragments and registry metadata stay typed unit data. |
| `vela_package` | 3 | Graph source trees/manifests, including one-line dependency modules; exclude Rust audit needles. |
| `vela_syntax` | 24 | Parser, AST/CST, recovery, removal rejection, formatting input/output; not just existing diagnostic fixtures. |
| `vela_hir` | 11 | Source ingestion, resolution, defaults/recovery, imports and rejection modules; preserve logical identities. |
| `vela_analysis` | 29 | Facts, flow, hints, ownership and finite formatted programs; literal spelling tables remain unit data. |
| `vela_mir` | 28 | Builder/verifier source programs and helper-built variants; raw MIR stays constructed Rust. |
| `vela_bytecode` | 40 | All compiler source families, semantic inputs and diagnostics; raw instruction/codec corruption stays Rust. |
| `vela_vm` | 57 | Execution, conversions, bytes, collections, reflection, cache/GC/budget sources and static bench workloads. |
| `vela_engine` | 63 | Native/state/re-entry, strings, package trees, reload versions, test source factories and bench sources. |
| `vela_hot_reload` | 11 | Function/state/module/trait ABI generations; retain policy/report/activation sequences. |
| `vela_host` | 0 | Owned-carrier tests and unsafe Rust audit remain; no invented Vela fixture requirement. |
| `vela_reflect` | 3 | Script metadata and member-reordering versions; Rust values and source audits remain Rust. |
| `vela_stdlib` | 0 | Manifest/ID/registration audits remain exhaustive typed Rust checks. |
| `vela_stdlib_runtime` | 0 | Manifest-to-runtime coverage stays exhaustive, not representative file cases. |
| `vela_macros` | 2 | Vela execution sources in integration tests; Rust token tests and trybuild `.rs/.stderr` remain. |
| `vela_bindgen` | 1 | Source-to-schema/generation and rejection inputs; keyword strings remain unit data. |
| `vela_bindgen_compile_test` | 0 | Existing `script.vela` retained; derived static reload version still needs review despite zero signal. |
| `vela_language_service` | 63 | All feature documents, incremental/overlay/snapshot versions and static bench support modules. |
| `vela_lsp_server` | 81 | Protocol, global-state, watcher/schema/lifecycle and stdio documents, including short malformed updates. |
| `vela_cli` | 2 | Real-process success/rejection inputs, including removed-flag setup source; preserve filenames/coordinates. |
| `vela_playground_wasm` | 1 | All compile/run/JSON input documents; preserve distinct entry routes. |

Repository-level scope is equally binding:

| Owner | Remaining work |
|---|---|
| `tests/fixtures` | Reuse existing source/golden families and shared graphs. Do not relocate for visual uniformity alone. |
| `tests/lsp_matrix` | Review JSON `files`, actions, phases and source-bearing oracles. Keep concise self-contained JSON; any selected extraction must coordinate Rust/JS readers. |
| `scripts` / VS Code | Inline fixture/protocol/editor seed documents; preserve existing Node/provider/input runners and independent oracles. |
| Examples | Adjacent source files already common; package-demo trees/updates and removal-test programs remain inline. |
| Site | Four playground programs and complete highlighting probes; independent token/scope goldens stay authoritative. |
| Benchmarks | Static Engine/VM/LS workloads, including Rust modules named `workload_sources`; genuine size generators remain explicit. |
| Fuzz | Static portable-plan base program; retain malformed binary corpora, mutation logic and libFuzzer. |
| Tree-sitter / Zed | Existing specialized corpus/package/query inputs remain; do not duplicate corpus source into parallel `.vela` files. |
| CI / Cargo / docs | Discovery, skips, target/feature wiring, byte preservation and source packaging; ordinary documentation snippets are not automatically test cases. |

The matrix alone has 218 tracked JSON files. A separate decoded-string scan found
source-like declarations in 208; these are workload indicators, not exact document
counts. Review nested lifecycle sources as well as `files`; neither extracting
only easy fields nor extracting every JSON string is the goal. Existing raw-byte
provenance covers the fixture subtree.

## Target Layout And Responsibilities

The following layout is proposed, not a claim these new paths already exist:

```text
crates/<owner>/tests/fixtures/
  <feature>/<case>.vela
  formatting/<case>.input.vela
  formatting/<case>.expected.vela
  diagnostics/<case>.vela
  diagnostics/<case>.expected
  reload/<scenario>/v1.vela
  reload/<scenario>/v2.vela
  reload/<scenario>/rejected.vela
  packages/<scenario>/<package>/vela.toml
  packages/<scenario>/<package>/src/...
crates/<owner>/benches/fixtures/<workload>.vela

tests/fixtures/conformance/<feature>/<case>.vela
  # Genuinely cross-owner inputs; avoid cosmetic relocation.
tests/lsp_matrix/fixtures/<family>.json
  # Logical paths, ordered events, query data and independent oracles.
tests/lsp_matrix/fixtures/sources/<family>/<phase>/<document>.vela
  # Marked source documents, inside the existing provenance subtree.
```

Examples normally keep public scripts adjacent to their binaries; shared example
inputs may have one example-local location. Example-only rejection inputs belong
under `examples/tests/fixtures`. Site-only and fuzz-only inputs stay with those
owners. Use the real source extension for external-language benchmark workloads,
not `.vela` for everything.

SF00 chooses one authored origin for each group selected for sharing.
Required package/build and provenance reachability take precedence over cosmetic
directory uniformity. Reuse or relocate an existing fixture when it removes actual
duplication; update all consumers and hashing/packaging rules together. The two
example reload binaries currently have byte-identical v1 scripts and need this
review too. Repository-only test sharing is different from a distributed input.
If independent distribution requires a physical copy, it must be a reviewed
`distribution-copy` of one authored origin with an existing copy/byte-verification
path, not another editable source. No symlink workaround or new packaging framework.

- **Source files** own inputs selected for extraction: substantial/reused programs,
  useful dependency/version trees and long source/golden pairs. Suitable concise
  documents can remain next to their assertions in Rust/JS or existing JSON.
- **Rust/JS tests** own registrations/options, logical names/SourceIds/URIs,
  entry names/arguments, typed expected values/errors, orchestration and assertions.
- **Existing goldens** own long formatted/rendered output. Do not replace typed
  scalar tags, opcode checks, spans or error payloads with stringified snapshots.
- **Existing specialized formats** remain authoritative for Rust compile-fail,
  Tree-sitter source/tree corpora and malformed binary inputs. No parallel copies.

Physical fixture paths must never silently replace logical compiler filenames,
package/module identity or LSP URIs. Writable package/editor tests materialize fresh
copies; no test modifies checked-in fixtures.

## Source Classification And Deduplication Rules

Review suitability before changing storage:

- Prefer extraction for substantial multi-function programs, meaningful repeated
  inputs, package/module trees, sizeable reload versions and long source/golden
  pairs. Files should make source easier to read, edit or reuse.
- Prefer inline for a short source closely coupled to one assertion, a small table
  of syntax errors, a tiny before/after comparison or explicit byte/escape/EOL
  cases where an external file hides the behavior under test. This can include a
  complete one-line function, not only tokens.
- Do not add dozens of tiny files or a reader/schema migration merely to meet an
  extraction quota. Keep useful existing JSON corpora self-contained when that
  representation is clearer. There is no automatic rule based on line count.

Each source-bearing family receives a disposition in ordinary review notes; use
location-specific rationale where needed, not a new case catalog or a mandatory
comment on every short literal:

| Disposition | Rule |
|---|---|
| `file-backed` | Chosen when a separate source file improves readability, editing, reuse or source-tree/golden organization. |
| `retain-inline` | Short/local documents or existing self-contained JSON whose meaning is clearer beside assertions/events; record the family-level reason. |
| `shared-input` | Same evaluated bytes can have one file with several consumers; registries/options/assertions may still differ. |
| `unit-data` | Identifiers, literal spellings, isolated lexer tokens, Rust token streams, expected labels, diagnostic needles and marker-parser micro-inputs can remain inline. |
| `generated` | Genuine bounded scale/combinatorial/mutation input whose construction is part of the test; name the generator and why files would lose coverage. |
| `specialized-corpus` | Existing authoritative source/tree, trybuild or binary format; already file-backed without conversion. |
| `distribution-copy` | Only for independently packaged routes that cannot share a path; name the single authored origin, packaging requirement and copy/equality verification. Never a second canonical source. |
| `not-test-input` | Production text, documentation or audit needle; explicitly exclude rather than counting it as migrated. |

A small fixed `format!` table can remain when it clearly exposes the varying
expression and avoids duplicated wrappers; it need not become one file per row.
Do not use string factories to hide large programs that would be clearer in files.
Partial editor documents and complete functions both use the same suitability test.

**Lifecycle orchestration and source storage are separate choices.** Review before,
after, incompatible and removed-state versions together. Prefer version files for
substantial scenarios; a tiny adjacent `return 1`/`return 2` pair can be clearer
inline. Intentional replacements retain exact match-count/change-scope checks.
Do not flatten old/new-generation proof into result-only rows, and do not retain
large duplicated scripts merely because their tests are stateful.

Source equality alone does not justify a global tiny fixture or test deletion:
coincidental `fn main() { return 1; }` inputs in unrelated scenarios may stay local.
Share meaningful repeated inputs when it improves ownership and maintenance.
Combine orchestration only
when source bytes, logical identities, graph, registration, load route, options,
permissions, budgets, initial state and ordered events are equivalent. Retain
source-vs-artifact, old-vs-active, registry-vs-no-registry and service-vs-protocol
observations. No global mutable Runtime/registry/cache or hidden setup cache.

Use the existing compile/run helpers first. Within one compatible case, compile
once and retain all meaningful observations where compilation count/identity is
not itself tested. Do not merge different programs into a larger artificial
program or replace exact Rust assertions with a universal `return true` contract.
Remove a duplicate helper/test only after mapping every assertion and public path.

## Using Rstest Without A Framework

1. **`#[case::name]`:** use typed inline data or `include_str!` according to the
   suitability decision, especially when expected values/errors/options vary.
   Preserve current named identities where possible. Moving a large literal into
   `const &str` alone is not extraction.
2. **`#[files]`:** for one homogeneous contract over a curated directory. Pair
   expected files by stem; do not create a Cartesian product of source/output globs.
   Keep dependency modules and reload versions outside standalone-case globs.
3. **Ordinary `#[test]`:** keep lifecycle, raw artifact, race and exhaustive audit
   scenarios; independently choose inline or file-backed input for readability.
4. **Small owner-local inventory assertions:** reject missing/orphan expected
   pairs, empty discovery and accidental extra inputs. These inspect filenames;
   they are not a runner, registry, callback table or expectation language.

Adding/deleting a globbed fixture must change actual `--list` output without a
clean build. Verify this in SF01 using a temporary test input that is then removed.
If the current macro/Cargo combination does not invalidate discovery, add a minimal
owner build-script `rerun-if-changed` for that directory, integrating with any
existing build script. Do not generate tests or build a custom discovery system.
Existing named include cases require explicit case registration by design.

No new support crate, wrapper macro, generalized result JSON, source-comment
expectation directives, bless mode or snapshot dependency. Keep rstest dev-only,
with default features disabled; no dependency adoption quota or unrelated upgrades.

## Required Language UI Test Deliverable

This is an explicit deliverable of the redesign, not an optional extraction
candidate. Here **UI means user-visible language diagnostics and CLI output**,
not graphical editor interaction. Rust macro trybuild tests remain a separate
boundary suite and do not satisfy this deliverable. Suitable golden families
are required; that does not require converting every diagnostic unit test.

### Existing Baseline And Gap

- `crates/vela_syntax/tests/diagnostic_fixtures.rs` already compares one parser
  source/rendered-output pair from `tests/fixtures/diagnostics`.
- VM diagnostic fixtures cover division-by-zero stack rendering and reflection
  lookup candidates; hot-reload tests have an ABI-report golden. Preserve these
  specialized environment/sequence assertions instead of putting them into one
  generic parser runner.
- HIR/analysis/compiler already have detailed typed diagnostic assertions, such as
  `crates/vela_bytecode/src/compiler/tests/diagnostic_contracts.rs`. They are not
  yet organized into a discoverable source/rendered-expectation UI corpus.
- `crates/vela_cli/tests/run_scripts.rs` runs actual processes. Success output is
  compared exactly; diagnostic cases mostly check failure plus stderr fragments.
  They do not yet pin complete stdout/stderr and the exact exit code.

The new work builds on these tests; it is not a claim that diagnosis or error
coverage was previously absent.

### Three Required Layers

| Deliverable / owning stage | Required corpus and contract |
|---|---|
| UI-P — Parser, SF02 | A nonempty source/golden family for existing syntax errors, recovery and removed-syntax diagnostics. Preserve lossless/recovery assertions and validate diagnostic count/order/code/severity/spans/labels as applicable before comparing complete rendered output. Include valid-input control coverage; successful parsing cannot accidentally satisfy an error golden. |
| UI-S — Semantics, SF03/SF04 | Source/golden families for existing name/import/visibility, call/type-contract and literal-range errors at their actual HIR/analysis/compiler boundary. Keep positive controls, logical source/module identities and explicit bare/registered or multi-module setup where needed. Do not flatten all failures into `is_err()` or route every test through Engine. |
| UI-C — CLI, SF08 | Real-process source/output cases for success, compile error, runtime error with stack and removed-source/flag rejection. Compare full stdout, full stderr and exact exit code, retaining existing source locations and forbidden obsolete guidance. Keep argument/flag errors distinct from errors obtained by compiling a script. |

Use existing supported behavior only. Do not add a lint command, warning policy,
JSON diagnostic output mode or CLI options to imitate a reference repository.
`--print-schema` is not a JSON-diagnostics lane. If a currently supported case is
platform-dependent, make the expectation difference explicit rather than weakening
all comparisons.

### File And Runner Organization

Proposed locations for new owner-local cases:

```text
crates/vela_syntax/tests/fixtures/ui/parser/<case>.vela
crates/vela_syntax/tests/fixtures/ui/parser/<case>.expected
crates/vela_hir/tests/fixtures/ui/semantic/<case>.vela
crates/vela_hir/tests/fixtures/ui/semantic/<case>.expected
crates/vela_analysis/tests/fixtures/ui/semantic/<case>.vela
crates/vela_analysis/tests/fixtures/ui/semantic/<case>.expected
crates/vela_bytecode/tests/fixtures/ui/semantic/<case>.vela
crates/vela_bytecode/tests/fixtures/ui/semantic/<case>.expected
crates/vela_cli/tests/fixtures/ui/<case>.vela
crates/vela_cli/tests/fixtures/ui/<case>.stdout
crates/vela_cli/tests/fixtures/ui/<case>.stderr
```

These are proposed paths, not newly created files. Reuse existing shared inputs
where appropriate; do not copy the existing root diagnostic corpus just to make
names uniform. A multi-module case can be a small source tree with one expectation
for the complete ordered diagnostic set, not a glob that treats dependencies as
separate cases. Source-less CLI argument errors need not have dummy scripts.

- Reuse owner test targets/module wiring and current compile/render/process
  helpers. Use `rstest #[files]` for a truly homogeneous corpus; named cases with
  `include_str!` for varying environments, arguments and expected status codes.
  Store exact CLI exit codes and arguments as typed Rust case data, not a new
  metadata/command DSL. Empty stdout/stderr expectations must be explicit.
- Add narrow owner-local pair/inventory tests: missing/orphan expectations, empty
  corpus, unexpected inputs and undiscovered cases must fail. Verify automatic
  add/delete discovery without a clean build for globbed families. For explicitly
  named cases, verify registration/inventory consistency and nonempty selection.
- Keep Vela's existing `.expected` format and typed assertions; no generic
  `.diagnostics.json` schema, custom harness, snapshot dependency or bless mode.
- Goldens are reviewed files, never generated from the implementation during
  comparison. A malformed span must fail a typed range check, not disappear during
  rendering and match an anonymous diagnostic. Preserve related labels and notes.
- Make subprocess output deterministic through controlled cwd, logical names and
  actual supported settings. If needed, replace only the known temporary-root
  prefix with a fixed token using an owner-local helper. Do not strip arbitrary
  paths, lines, whitespace, ordering or messages to obtain matches.
- Keep existing diagnostic CRLF/trailing-newline normalization local and explicit.
  New golden lanes state their exact comparison policy. Any change from substring
  checks to full output/status checks is intentional UI coverage strengthening,
  reviewed separately from byte-preserving source extraction.

### UI Acceptance

UI-P, UI-S and UI-C each require discovered, independently runnable passing cases
and their inventory checks; macro trybuild or editor-provider passes cannot replace
any of them. Demonstrate that a wrong expected message/span, an extra diagnostic,
a missing output file and a wrong CLI status fail the relevant check, then restore
reviewed fixtures. Retain focused typed unit tests and VM/reload report tests that
establish additional contracts. Record actual test names/targets and commands in
[validation.md](validation.md) when implementation lands, not placeholder commands
for suites that do not yet exist.

## Exact Bytes, Goldens And Package Inputs

Initial extraction must preserve **evaluated bytes**, not the appearance of the
Rust/JSON literal: raw-string leading/trailing newlines, cooked escapes, Rust line
continuations, JSON escapes, indentation, NULs, Unicode, tabs and missing EOF newline.
Check old/new length and bytes before deleting the literal; a UTF-8 byte hash may
be recorded in review, but hash equality is not a replacement for assertion review.

- No automatic trimming, reindentation, formatting or newline insertion during
  extraction. No blanket golden regeneration or normalization in new readers.
- File readers use strict UTF-8 with matching Rust/JS rejection of invalid bytes,
  not replacement characters from permissive Node reads. An authored BOM remains
  source content rather than being silently stripped. Existing reader parity tests
  must cover invalid UTF-8, BOM, NUL, mixed EOL and missing final newline; malformed
  byte inputs belong in decoder fixtures, not a lossy string representation.
- Existing diagnostic helpers' CRLF/`trim_end()` policies stay local; formatter,
  lossless parser, source slices and coordinate tests retain exact comparison.
- Protect ordinary LF fixtures with scoped Git attributes. Use byte-preserving
  exceptions for authored CRLF/mixed-EOL files; check with `git check-attr` and
  byte reads. There is no current tracked `.gitattributes`; do not renormalize the
  whole repository. Editor formatting must not silently rewrite sensitive files.
- Existing intentional LF-to-CRLF variants may remain generated from one canonical
  LF input. Do not double-convert already-CRLF data or remove independent EOL proof.
- Recheck `.find`/`.rfind`, match counts, span slicing, caret constants and rendered
  filename/line/column assertions. Storage-path changes must not update oracles
  merely to hide a changed logical name.
- Static manifests/package trees can be fixtures too. Preserve relative dependency
  paths, insertion order, platform path tests and isolated mutable copies.
- Source needed at Cargo package/build time must be included in that owner's
  package and tracked by its build dependencies. Verify package contents and the
  affected build/run route from an isolated packaged tree, not just the checkout.
  Cross-owner test sharing must not accidentally make published production code
  depend on repository test data.

## Shared Tooling JSON: Coordinated Migration, Not Shadow Files

Current `tests/lsp_matrix/support/mod.rs` uses `Spec.files: BTreeMap<String, String>`
and `Action.source: Option<String>`; JS `FixtureWorkspace` also interprets these
values as source text. Putting a filename into a string field would compile the
filename, not load it. The current corpus README explicitly relies on JSON escaping
for Unicode/EOL. This requires an explicit test-data change, not textual replacement.

First decide whether extraction has concrete value for a family. Short sources
whose relationship to actions/oracles is clearer in JSON may remain there; adding
a loader/schema is not justified just to eliminate inline text. If no worthwhile
family is found, SF10 records a no-change outcome without introducing new readers.

For families selected for extraction, the proposed representation is an explicit
`{"sourceFile":"sources/<family>/<phase>/<document>.vela"}` reference, relative
to its owning fixture JSON. Resolve only declared source fields through existing
Rust/JS fixture readers, before marker parsing/materialization. No implicit path
strings, arbitrary recursive object interpretation or generalized includes.

- JSON continues to own logical paths, graph topology, action ordering, phase/query
  identities and independent coordinate/behavior oracles.
- Review initial files, change/write payloads, phase files, `diskSource`/`openSource`,
  recovery/repaired and post-edit expected documents. Extract only suitable inputs;
  partial edit text, small source/event sequences and parser micro-inputs may remain.
- If extraction is justified, SF10 specifies the accepted fields/version transition
  and ports a selected family across all consumers. A document has one canonical
  representation: inline text or a file reference, never a duplicate fallback.
  Inline and referenced forms can coexist for different documents by design;
  retained inline source is not obsolete compatibility to remove.
- Missing files, wrong types/unknown reference fields, traversal, absolute/drive
  paths, backslashes, symlink escape and conflicting source forms fail identically
  in Rust and JS. Fixture storage containment is distinct from logical-path checks.
- Compare decoded marked bytes, stripped text, marker positions and full expected
  responses before/after. Goldens remain authored independently of production APIs.
- Keep sources inside the existing recursively hashed fixture subtree. Review all
  provenance consumers, including direct `include_str!`/JSON readers and specialized
  oracle modules; updating only the shared `Spec` parser is insufficient.

Path-bearing hashes change even for byte-identical extraction. Raw-byte provenance,
normalized contract digests and executable/server identity are different evidence.
Preserve 1,327 semantic / 1,477 execution IDs, immutable baselines and pending proof.
Keep B02–B08 reopening and native-input gaps; use existing reviewed migrations where
contracts change. Recapture affected proof under the new input fingerprints—never
rewrite old captures or promote provider proof to physical Input/Render acceptance.

## Execution Stages

These are new stages, not renamed claims that TO00–TO09 already completed them.
The SF00 census covers [runtime/compiler](test-input-census-runtime.md),
[tooling/matrix](test-input-census-tooling.md) and
[public/repository consumers](test-input-census-consumers.md). Each stage reviews its whole assigned scope and closes the selected
changes, not merely a sample of easy cases; it does not externalize every input.
Execution preserves the requested stage commit boundary; shared fixtures have one
writer. The execution record, not this work/exit checklist, owns actual gate status.

| Stage | Work and concrete starting points | Required exit |
|---|---|---|
| SF00 — Full census | All source entrypoints/helpers across the owner tables; `#[case]`, ordinary tests, constants, format/replace/concat/include, filesystem writes, JSON/JS/TS, build/bench/fuzz. | Every source-bearing location has owner, route, consumers and a disposition; each duplicate group selected for sharing has a canonical origin and packaging/provenance obligations; reviewed independent copies may remain independent. Record family proof responsibilities, target boundaries and helper contracts/freshness. Unresolved rows remain open. Use ordinary review notes, not a new executable catalog. |
| SF01 — Representative decisions | Review all three Engine string cases, a formatter input/output family and a retained-closure scenario. Demonstrate both useful extraction and justified inline retention; use glob discovery only for a suitable family. | Complete each selected change; prove bytes/assertions and applicable discovery. A short inline case and a substantial file-backed case both show the intended policy. These samples do not finish an owner. |
| SF02 — Source syntax, rendering and UI-P | Review common renderer and syntax input families; implement the required parser UI corpus and inventory checks. | UI-P passes with complete output and retained typed/recovery assertions; selected formatter corpus reparses, is idempotent and preserves executable results. Suitable extraction and intentional inline retention accounted for. |
| SF03 — Semantic graphs and UI-S | Review HIR/analysis/package source families; implement semantic UI cases for resolution/import/visibility at their actual owners. | Semantic UI corpus/inventory and positive controls pass; SourceId/package/path/order and typed facts remain unchanged. Short local unit cases can stay inline. |
| SF04 — MIR, compilation and UI-S completion | Review compiler inputs and fixed variants; complete compiler-boundary UI cases for call/type-contract and literal-range errors. Coordinate shared consumers. | All UI-S owner lanes pass without flattening errors; raw MIR/bytecode/codec proof and lowering/runtime assertions retained. Select portable-supported equivalence cases with SF06 without expanding the format. |
| SF05 — VM and value reflection | Review VM suites and script reflection documents, including numeric/bytes cases; audit host/stdlib/runtime unit-data dispositions. | Large/shared inputs extracted where useful; short cases may remain beside typed expectations. Exact tags/bits/errors, selected mode comparisons, budget charges, effects and cleanup sequences retained. |
| SF06 — Engine and reload | Review Engine/hot_reload scenarios, source factories and rejected versions; extract substantial/shared generations and package trees. | Selected scenarios improved without fragmenting tiny version pairs; old executable/state/native owners and rejected-activation identity proof unchanged. Preserve final-owner reclamation, self-retention rejection and shared-image state isolation; add the selected source/decoded execution comparisons. |
| SF07 — Rust boundary tools | Macro integration Vela inputs, bindgen schema inputs and compile-test reload version; preserve Rust token/unit/trybuild data. | UI stderr/native/value/generated-call proof passes; build script/source-package dependencies include fixtures. |
| SF08 — Public routes and UI-C | Implement real CLI UI source/output families with exact exit codes and pair checks; organize suitable WASM/example inputs. | UI-C full stdout/stderr/status checks pass with bounded processes/output and isolated temp ownership; actual schema/path/filesystem routes covered. All 14 example bins, source spans and distinct compile/run routes remain covered; coordinate actual site inputs with SF11. |
| SF09 — Tooling-local documents | Review language-service/LSP feature and lifecycle sources, including malformed updates. Extract substantial/shared documents; retain concise local edits when clearer. | No flattened sequences or changed logical URIs; each selected move preserves bytes and linked identity/evidence mapping. |
| SF10 — Shared matrix corpus | Review source-bearing JSON families for actual readability/reuse gains before changing readers. Migrate only justified selections with all their consumers; keep useful self-contained JSON. | Either a justified no-change result or validated reader/byte/marker/provenance migration. No shadow sources; inline source is supported where suitable. |
| SF11 — Other repository inputs | Script/editor seeds, site programs/probes, static benchmark and fuzz base sources; audit existing grammar/Zed corpora. | No missed source modules/templates; actual site inputs execute with independent results, current grammar/query checks run, and external-comparison semantic preflight fails wrong results. No file IO in timed loops; package/build paths and specialized runners still work. Real-browser smoke remains separately scoped. |
| SF12 — Residual and acceptance audit | Reconcile reviewed families, selected extraction/sharing work, inline-retention decisions, references, test identities, filters/CI and docs. | Selected work and all three required language UI layers complete; remaining inputs have sensible reviewed storage. No shadow copies, unused fixtures, missing pairs or lost assertions. No zero-inline target; gates/evidence reported separately. |

Stages SF02–SF09 may be developed in parallel only with disjoint writes; shared
source changes and matrix readers require explicit coordination. Do not batch
unrelated assertion strengthening or language fixes into source relocation.

## Completion And Verification

Each owner/family review answers:

1. Which source families were reviewed, including helpers and lifecycle versions?
   Why does each selected extraction/sharing change improve readability or reuse,
   and which concise/local/byte-sensitive inputs are better retained inline?
2. Which identical source copies were removed? Which environments or public paths
   must remain separate? What setup was consolidated without sharing mutable state?
3. Are decoded input bytes and independent expected outputs unchanged? Does every
   old assertion, failure phase, cleanup check and logical identity still exist?
4. Which actual discovered test identities changed? Update real filters/catalogs,
   preserve integration aggregators and require nonempty exact selection.
5. Do files have real consumers, pairs match, and newly added/deleted cases invalidate
   discovery? Does packaging include all inputs required by each retained route,
   with one authored origin and verified distribution copies only where necessary?
6. Do UI-P/UI-S/UI-C each have an actual source/output corpus, correct owner route,
   inventory checks and full comparisons, without weakening existing typed tests?
7. Are the additional structural comparison obligations closed at their owning
   stages, distinguishing preserved proof from intentionally added coverage?
8. Which commands passed, timed out or could not run? Which evidence became stale?
   Keep the separately scoped browser gap visible; builds are not browser proof.

**Completion means reviewed, useful organization—not zero inline source.** Finish
identified worthwhile extraction/sharing work and preserve deliberate inline tests.
Do not use a blanket owner exemption to ignore large repeated programs, or force
small clear tests into files to improve a grep score. There is no fixture-count or
extraction-percentage target and no promised rstest speedup; expanded named-case
counts are not a regression by themselves.

For each implementation stage: run current focused baseline tests, preserve byte
and assertion mappings, then owner tests/default+affected feature lanes, discovery,
formatting and scoped Clippy. Include reader/invalid-reference/empty-pair tests as
needed. Follow the full Rust/example/WASM/site/Node/editor/bench/fuzz gates in
[validation.md](validation.md) at closure or the owning CI lane; do not import a
different repository's validation policy. Bounds must be finite and timeouts remain
recorded. Partition results require exact discovery/feature/binary reconciliation
and never become an aggregate exit-zero claim.

Installed editor results require fresh identical input/server/platform provenance;
physical Input/Render and OS obligations remain distinct. The existing missing
observer-extension prerequisite is tracked in [blocked.md](blocked.md), not solved
by fixture migration. A source reorganization can be implemented while an external
gate is pending, but neither the gate nor new evidence may be marked accepted.

The census is review evidence, not implementation acceptance. Follow the execution
record for completed changes and actual commands; historical TO results do not
validate SF work. Execute the suitability decisions rather than another rstest-only
pass or mechanical extraction sweep.
