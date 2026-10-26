# Repository Test Organization Plan

Status: TO00–TO09 execution complete, one commit per stage, including reviewed
no-change dispositions. [Execution evidence](archive/test-organization-acceptance.md)
records bounded test coverage and validation limitations: aggregate Rust/editor
commands timed out; native Input/Render and Windows acceptance remain incomplete.
This is not a claim that every test was converted or every possible duplicate removed.

## Objective And Authority

Consolidate equivalent test inputs, compatible preparation and homogeneous
assertions using **existing `rstest`**, not a custom framework. Organize current
tests by their semantic owner and execution boundary; remove demonstrated
redundancy and stale scaffolding without weakening retained contracts.

[goal.md](goal.md) and [architecture.md](architecture.md) remain authoritative.
The completed runtime-removal pass is not reopened. Do not add language/runtime
features, replacement opaque objects, conversion APIs or a new re-entry model.
[validation.md](validation.md) owns commands; this document owns migration scope
and organization, not a replacement validation policy.

Success is understandable ownership, canonical input data and independently
selectable cases—not fewer tests, fewer assertions or an unmeasured speed claim.

## Current Inventory And Evidence

Snapshot: `bb11d53eb` plus the uncommitted `rstest` pilot. Inventory combines Cargo
metadata, test wiring, shared fixture consumers and representative bodies. It is
an entrypoint/family audit, **not a proof that every test has already been
classified or that all duplicate tests have been found**.

- Root workspace: **23 crates**. Metadata was inspected with
  `cargo metadata --locked --offline --no-deps --format-version 1`.
- `rstest = { version = "0.28.0", default-features = false }` is workspace-managed;
  only `vela_bytecode` and `vela_vm` currently opt in as dev dependencies.
- The pilot shares `tests/fixtures/conformance/record_writes/{nested,indexed}.vela`
  across compiler and VM named cases. The four original input copies were
  byte-identical to the extracted files; four independently scheduled cases and
  scoped Clippy/format checks passed in the preceding pilot, not this planning run.
- The previous removal acceptance recorded **4,333 passes and two ignored tests**.
  That is historical evidence, not a newly executed baseline, a migration quota,
  or a requirement to preserve exactly that generated test count.
- Existing shared root corpora are `tests/fixtures/{conformance,diagnostics,
  completion,lsp_highlighting}` and `tests/lsp_matrix/fixtures`.
- Examples, fuzzing and the Zed extension have separate Cargo manifests. They are
  not additional root-workspace test owners. There are 14 retained example bins.

### Root Rust Workspace: All Owners

Paths in this table are relative to `crates/<crate>/`. “Library” includes the
proc-macro library harness where applicable. Explicit integration aggregators
must stay aggregators: their module files are not independent Cargo targets.

| Crate | Current test entrypoints and input forms | Planned disposition |
|---|---|---|
| `vela_common` | Library tests in `primitive`, `capability`, `interop_type`, `diagnostic_render`, and `lib.rs`; Rust tables and constructed diagnostics. | Named scalar/tag/capability rows; retain exact renderer output and const-context proof. |
| `vela_def` | Library `stable`/`script` identity and hash tests; fixed Rust vectors. | Named canonical path/hash rows, preserving independently authored expected IDs and typed identities. |
| `vela_syntax` | Library parser/CST/AST/recovery/format tests; `tests/diagnostic_fixtures.rs`. | Named pure cases and owner-local file pairs; keep lossless bytes, typed recovery and exact output. |
| `vela_hir` | Library attributes/source-ingestion/module-graph/script-method tests; `tests/integration.rs` wires conformance and removal modules. | Parameterize independent resolution permutations; share conformance source descriptions, not owner-sensitive results. |
| `vela_analysis` | Library facts/hints/literals/validation/registry/flow tests; `tests/integration.rs` wires completion and conformance fixtures. | Named pure type/literal rows; preserve fixed-point, executable-owner, neighbor-fact and diagnostic assertions. |
| `vela_registry` | Library registration/compile-view/type-hint tests. | Separate typed registration/rejection families; preserve collision kinds and complete projections. |
| `vela_package` | Library identity/manifest/filesystem graph tests; `tests/architecture.rs`. | Parameterize pure identity/manifest inputs; keep filesystem/cycle/authorization and repository audits explicit. |
| `vela_mir` | Library builder/verifier/input/effect tests and full MIR dumps; constructed malformed graphs. | Named homogeneous build/verify cases; retain typed structural, liveness, root and budget proof. |
| `vela_bytecode` | Library compiler/semantic-input/linker/verifier/portable/schema/selection tests; `tests/instruction_layout.rs`. | Extend the pilot, share exact sources, group homogeneous cases; keep raw instruction/artifact mutation and layout assertions. |
| `vela_vm` | Library execution/heap/collection/native/reflection tests; `integration`, `conformance`, `baseline_workload_contract` targets. | Named ordinary executions, numeric tags and failure rows; preserve GC/budget/session/cache/root proof and raw instruction inputs. |
| `vela_engine` | Library source/package/runtime/registration/binding tests; `tests/integration.rs` wires args/reload/reflection/portable/removal tests. | Named ordinary execution/conversion cases and fresh preparation; retain installed/native/package/re-entry/state scenarios. |
| `vela_hot_reload` | Library function/method/schema/state/package/module/trait ABI and policy/report tests. | Named narrow ABI families; preserve staging/owner/state/report sequencing and exact error payloads. |
| `vela_host` | Owned structural-carrier library test; `tests/unsafe_boundaries.rs`. | Likely retain unchanged: one coherent roundtrip and a workspace unsafe-source audit are not redundant source suites. |
| `vela_reflect` | Library registry/metadata/member/policy/script-type tests; `tests/value_access.rs`. | Named same-shape queries; share only equivalent constructors and retain permissions and before/after value observations. |
| `vela_stdlib` | Library ID/manifest/reflection-native tests. | Named typed lookup subsets; keep whole-manifest uniqueness/completeness and registration/policy checks. |
| `vela_stdlib_runtime` | Library binding-vector/implementation/ID checks against the stdlib manifest. | Likely retain manifest-wide loops: a fixed representative case list would lose automatic coverage of additions. |
| `vela_macros` | Proc-macro library tests; `integration`, `interop_exports`, `ui` targets; Rust tokens and trybuild fixtures. | `rstest` for ordinary token/classification cases; **keep trybuild** pass/fail discovery and `.stderr` comparisons. |
| `vela_bindgen` | Library source-to-schema/generated-Rust tests in `src/rust.rs`. | Named keywords/unsupported-boundary cases; retain deterministic generation and structural/source-bearing diagnostics. |
| `vela_bindgen_compile_test` | `build.rs` plus shared `script.vela`, generated `OUT_DIR` bindings and library execution/reload tests. | Fresh setup helper if useful; keep build-time stubs, real runtime implementations and generated public calls distinct. |
| `vela_language_service` | Feature-local library tests; shared JSON/Rust support; direct database/query observations. | Named independent input/line-ending/capability rows; preserve incremental/frozen/fresh and overlay lifecycle scenarios. |
| `vela_lsp_server` | Library protocol and binary tests; `tests/stdio_transport.rs`; real request/notification/process execution. | Named independent wire/process inputs; keep lifecycle, cancellation, publication and termination assertions. |
| `vela_cli` | Library metadata tests; real-binary `tests/run_scripts.rs`. | Named process-output/source-rejection cases; preserve flags, exit status, streams and source locations. |
| `vela_playground_wasm` | Host-side compile/run/JSON tests in `src/lib.rs`; wasm32 release compilation. | Named rejection/output cases, preserving compile-versus-run routes; do not equate host tests or cross-compilation with browser execution. |

### Repository-Level And Non-Rust Owners

| Area | Existing entrypoints/corpora | Disposition |
|---|---|---|
| `tests/fixtures` | Shared `.vela`/`.expected` diagnostics/completion, multi-module conformance, highlighting data. | Canonical shared data only when multiple owners really consume it; preserve logical source identities and expectations. |
| `tests/lsp_matrix` | JSON corpora, Rust `support`, catalog, obligations, immutable baseline, migration journal and captures. | Reuse existing machinery; preserve IDs, contract hashes, pending proof and evidence levels. No second registry. |
| `scripts/lsp-matrix` | Node tests, fixture/oracle/trace/provenance helpers, Rust suite discovery and matrix runner. | Keep Node ownership. Parameterized Rust names require exact linkage review, not a new runner. |
| `examples` | Separate Cargo package; 14 real bins, `runnable_examples.rs`, `removed_entrypoints.rs`, bin-local tests. | Replace existing homogeneous registration macros with `rstest`; keep real processes and package/reload scenarios. |
| `editors/vscode` | Node helpers, `@vscode/test-electron`, installed VSIX suites, separate native-input runner. | Reuse existing fixture materialization and short isolated-session paths; retain input/render/trust/process/provenance checks. |
| `editors/tree-sitter-vela` | Generator, generated parser, corpus and query checks. | Keep Tree-sitter corpus semantics and runner; not a Rust parametrization target. |
| `editors/zed` | Isolated Rust extension and Node package/cache/query validation. | Keep extension/package ownership; package validation is not installed Zed behavior proof. |
| `site` | Node docs/syntax checks, independent Shiki scope goldens, Astro build and playground data. | Reuse data where semantics agree; keep existing JS/TS tooling. Site build is not DOM/browser runtime proof. |
| `fuzz` | Isolated libFuzzer parser/portable-plan targets and raw malformed seeds. | Preserve fuzz infrastructure; deterministic regression cases supplement, not replace, campaigns. |
| Benchmarks | Engine actor/memory/generation/reload/interop, VM baseline/comparison/scalar/MIR, language-service latency targets. | Keep build contracts separate from timed measurements; do not move performance probes into ordinary cases. |
| `.cargo` / `.github` | `test-fast`, Rust/example CI, scheduled fuzz, VS Code OS/version jobs, pages/WASM and release jobs. | Reconcile filters, target/feature/OS coverage and commands after renames. Preserve existing final gates. |

## Concrete Cleanup Backlog

These findings distinguish demonstrated input/setup repetition from possible
redundant proof. No candidate authorizes deletion merely by appearing here.

| ID | Existing evidence | Action and retained proof |
|---|---|---|
| C01 | `.cargo/config.toml` skips `removed_service_macros_are_unavailable`, but `crates/vela_macros/tests/ui.rs` now declares `removed_macros_and_borrow_apis_are_unavailable`. | Confirm actual discovered names and repair the fast-lane filter. Do not accidentally omit UI from full validation. |
| C02 | `examples/tests/runnable_examples.rs` defines `success_example!` and `output_contains_example!`. | Replace both registration macros with two ordinary named rstest families. Keep exact-output versus substring contracts distinct. |
| C03 | Compiler/VM assignment, index-read, block-tail, break/continue and script-method-writeback programs are repeated literals. | Extract each exact shared input; keep opcode/operand checks in bytecode and results/effects in VM. |
| C04 | Compiler/Engine named String/Array method programs are repeated, but compiler uses a minimal registry and Engine installs standard natives. | Share text, **not** the registry or execution boundary. Preserve named-argument order and type facts. |
| C05 | HIR and analysis conformance tests both construct `conformance::core` / `conformance::reward` with SourceIds 1/2. | Share a source-set description or narrow test-only graph helper; keep facts and execution setup separate. `reward_module.vela` is not a standalone executable case. |
| C06 | VM bytes/char/conversion source runners repeat standard compilation/link/native setup; existing test support already handles related paths. | Compare settings and reuse an existing fresh runner where equivalent; retain exact tags, error payloads and heap/budget assertions. |
| C07 | Engine string tests, VM numeric conversions/bytes bounds, pure identity tables, macro classification and bindgen keyword tests have homogeneous orchestration. | Named cases with explicit expected values, errors and options; no universal callback-based assertion registry. |
| C08 | LSP Rust and JS consumers already share Unicode/schema/selection corpora and independent marker goldens. | Consolidate only repeated transformations/materialization. Keep byte versus UTF-16 coordinate and service/protocol/editor proof separate. |
| C09 | Provider and native-input editor runners repeat marker stripping and file-copy setup. | Reuse existing helpers or a narrow collision-checked append operation; do not weaken new-root materialization or session isolation. |
| C10 | VM `program_execution.rs` has source/result-similar compiled and linked convenience-call tests. | Review the distinct public API paths and ownership/error observations before proposing removal. Identical return values are not enough. |
| C11 | Names such as `service_impl.rs`, `task_targets.rs`, `service_path_tests.rs` and Provider-era tooling modules now own rejection/ordinary-trait proof. | Consider cohesive renaming and exact reference updates. Never delete removal rejection by historical filename or generic word. |
| C12 | `standard_borrowed_native_dispatch.rs` concerns invocation-local access to VM-owned values. | Keep its safety proof; do not confuse it with removed caller-borrowed Rust injection. Rename only if it improves clarity. |
| C13 | Diagnostic helpers currently normalize CRLF and trim trailing output; recovery/format tests assert authored bytes. | Freeze each owner's existing comparison policy first. Do not spread normalization to hide range/order/whitespace regressions. Any policy strengthening is a reviewed assertion change. |

### Representative Sources For C03/C04/C07

- Bytecode: `src/compiler/tests/expressions/assignments_and_access.rs`,
  `block_tail_semantics.rs`, `loops_and_errors.rs`, `script_methods.rs`,
  `literals_and_calls.rs` and `value_method_shapes.rs`.
- VM: `src/tests/execution_core.rs`, `block_tail_semantics.rs`, `control_flow.rs`,
  `script_methods.rs`, `evaluation_order.rs`, `numeric_conversions.rs`, `bytes.rs`
  and `tests/conformance.rs`.
- Engine: `src/tests/strings.rs`, `src/tests/native/compiler_options.rs` and
  `src/runtime/inline_cache_method_tests.rs`.
- Macro signatures: `src/export/signature/tests.rs`; bindgen: `src/rust.rs`.

The named-argument family must keep compiler slot projection and runtime trace
`[2, 1]` as separate observations. The method-with/without-registry family must
continue exercising both environments. These are input-sharing opportunities,
not certified duplicate tests.

## Organization Rules

### What Counts As The Same Input

Compare source/token/raw-input bytes **and** logical filenames/SourceIds,
package/module graph, registry/schema/native installation, compiler options,
capabilities/reflection policy, budgets/GC settings, initial state, ordered events,
load route, line ending and platform/process requirements.

- Same text with different registries or lifecycle states remains distinct cases.
- Same owner/environment/assertion shape can use one rstest function with named
  cases and multiple meaningful observations per case.
- Same bytes at different semantic layers share a fixture, not an oracle.
- Reuse immutable prepared products within a compatible case only where creation
  identity is not being tested. No global preparation cache or shared mutable
  Runtime/heap/budget/registry/cache across independently isolated cases.
- Fresh-versus-incremental, source-versus-decoded-artifact and old-versus-active
  generations are required execution dimensions, not incidental duplicated work.

### Use Existing Forms, Not A Framework

1. **`#[rstest]` + `#[case::name]`:** typed Rust inputs, expected values/errors,
   explicit options and homogeneous assertion logic.
2. **`#[rstest]` + `#[files]`:** curated source/expected directories with one
   execution contract per family. A matched source is one independently named
   test; pair its expected file by stem, not a second independent glob.
3. **Existing goldens:** keep `.expected`, `.stderr`, JSON fixture oracles and
   typed expectations already owned by the relevant subsystem. Do not invent
   source-comment directives, a general expected-result JSON language or a new
   bless/update mode. Normal tests only read reviewed expectations.
4. **Ordinary `#[test]` scenarios:** stateful ownership, reload, re-entry, caches,
   races, forged artifacts and whole-manifest/source audits. Parameterizing an
   independent outer setting must not flatten their internal sequences.
5. **Existing specialized runners:** trybuild, Node, test-electron/native input,
   Tree-sitter, libFuzzer and benchmark harnesses stay with their current owners.

Add `rstest.workspace = true` only as a dev dependency in participating root
crates. Keep default features disabled. Do not build wrapper macros, an
assertion-callback registry, custom discovery/expectation DSL, shared support
crate, production serialization API or a new integration binary just to adopt it.
A narrow ordinary helper or `rstest` fixture needs real consumers/responsibility
and must construct fresh state; do not add forwarding layers for a single caller.

The separate examples package cannot inherit root workspace dependencies without
changing workspace ownership. If converted, declare the same dev-only version
there and update its lockfile without joining workspaces or upgrading unrelated
packages. Fuzz/Zed do not need rstest merely because they are in this repository.

### Fixture And Naming Policy

- Keep owner-local data under `crates/<owner>/tests/fixtures/<family>/`; existing
  genuinely multi-owner sources may stay under `tests/fixtures`. Do not relocate
  all data solely to make the tree visually uniform.
- Shared input is canonical; owner-specific expected values, opcodes, diagnostics
  and state observations remain explicit near their assertions.
- Preserve bytes on initial extraction and demonstrate equivalence. Formatting
  ordinary fixtures can follow separately after reviewing source spans, dumps,
  logical filenames, expected coordinates and replacement-match counts. No loader
  normalization or blanket golden regeneration.
- Do not glob mixed parser/runtime diagnostic directories or dependency modules
  as if each file were a standalone test. Check missing/orphan pairs and empty
  selections through small owner-local inventory assertions, not a new harness.
- A raw MIR/bytecode/artifact/native Rust input stays that kind of input; source
  compilation does not replace a forged handle/register/offset/layout boundary.
- Record old test identity to actual new discovered case names in the migration
  review/commit notes, not a second global catalog. `#[case::name]` can generate
  `case_1_name`; changing order can change exact names. Verify `--list`, filters,
  explicit integration wiring, `.cargo` skips and linked evidence.

## Execution Stages

Each stage consists of small, coherent, independently verifiable changes, not
one crate-wide search-and-replace. Add a dev dependency with its first actual
conversion. Update related test wiring, names, filters, docs and evidence in the
same owning change; commit only when authorized by the execution request.

### TO00 — Baseline, Ownership And Pilot Acceptance

Scope: this inventory, Cargo/test target wiring, the existing pilot, `.cargo`
filters and links into the plan. The inventory is recorded; executable baseline
capture and repository migration remain pending.

- Select bounded initial families; record owner/input/route/assertions and existing
  helper consumers. Mark each family `parameterize`, `share-input`, `share-setup`,
  `retain-scenario`, `retain-audit` or `duplicate-review` in ordinary review notes.
- Discover exact test names and feature gates; resolve C01. Preserve the pilot's
  four independently runnable cases and unchanged input bytes.
- Do not treat root `Cargo metadata` as an executed coverage baseline or claim
  all tests were classified from this document's representative sample.

Exit: supported target map, nonempty selected before/after case lists, reviewed
old/new identity mapping and no custom framework. Pilot status is explicit.

### TO01 — Foundations And Front-End

Owners: common, def, registry, package, syntax, HIR and analysis.

- Start with typed scalar/hash/name/literal/manifest tables and independent graph
  insertion-order permutations; retain complete expected types and errors.
- Extract owner-local formatter/diagnostic pairs only where one contract fits;
  preserve existing exact bytes/recovery/typed diagnostic observations.
- Share conformance source descriptions (C05), not an SDK Engine or runtime with
  parser/HIR tests. Keep package filesystem/identity scenarios explicit.

Exit: each former row remains independently selectable; Unicode/CRLF, ordering,
source/executable identity and positive/rejection boundaries remain covered.

### TO02 — MIR, Compiler And Artifact Owners

Owners: MIR/bytecode, with paired VM consumers only when extracting shared input.

- Extend the pilot to C03/C04; keep lowering, operand/effect and runtime oracles
  with their owners. Use existing compiler/test support first.
- Split homogeneous verifier build-success/source-rejection loops into named
  cases; keep special tuple-arity, source-span and error-phase assertions.
- Keep raw malformed MIR, linker/bytecode verification, checksum-valid invalid
  artifacts, physical selection and format rejection as explicit boundary tests.

Exit: source extraction equivalence plus existing MIR/root/budget/selection,
link/codec and rejected-input proof. No production dependency or format change.

### TO03 — VM, Reflection And Standard Values

Owners: VM, reflect, stdlib, stdlib_runtime and host.

- Named ordinary executions, numeric conversion/tag/bit-pattern cases and bytes
  bounds; separate compile rejection from runtime traps in conformance.
- Consolidate equivalent fresh standard runners after checking existing helpers.
- Parameterize typed metadata/ID lookup subsets, not one erased ID/expectation
  table. Keep manifest-wide completeness/uniqueness and unsafe audits.
- Retain GC roots, memory accounting, budgets/call depth, permissions, exact
  owner/cache/dispatch and synchronous callback sequencing as explicit scenarios.

Exit: original scalar tags/float bits/error payloads and cleanup observations
remain; host/stdlib_runtime may have a documented no-change outcome.

### TO04 — Engine, Reload And Generated Bindings

Owners: Engine, hot_reload, bindgen and bindgen_compile_test.

- Start with Engine strings/ordinary conversions and bindgen keywords/narrow
  diagnostic families. Share exact compiler/Engine inputs without merging setup.
- Reuse fresh constructors for equivalent cache/Engine/registration preparation;
  retain each injected cache corruption or binding policy observation.
- Reload before/after pairs can share files. Assert every source replacement's
  intended match count; do not let formatting silently turn reload into a no-op.
- Preserve exact old closure code/state/native owner, active-image rejection,
  runtime affinity, native host effects, package declarations/capability expansion,
  binding ceilings and error unwinding. Build-time stubs remain distinct.

Exit: existing old-generation, state, re-entry and authorization regressions pass
without global caches or flattening lifecycle tests into result-only rows.

### TO05 — Macros And Rust Boundary Tests

Owner: macros plus affected generated-binding consumers.

- Use named ordinary TokenStream/classification cases with exact diagnostics.
- Keep trybuild's existing pass/fail globs and reviewed `.stderr` fixtures; do
  not wrap each compile-fail file in a new rstest or custom registration layer.
- Preserve removed-feature compile-fail/doctest coverage and owned-value/native
  macro execution. Review historical module/runner names with exact references.

Exit: ordinary macro cases and the existing UI/doctest lanes remain meaningful;
fast-lane skips and complete gates use actual current names.

### TO06 — Language Service And LSP Protocol

Owners: language_service, lsp_server and their shared matrix Rust support.

- Convert independent fixture/line-ending/cursor/capability rows, not ordered
  edit/save/close/schema/cancellation sequences into a Cartesian product.
- Reuse existing pure marker/schema/event transformations; keep independent
  byte/UTF-16 goldens, source IDs and database versus wire adapters.
- Preserve repeated queries and old snapshots when they prove cache invalidation,
  immutability or fresh equivalence. Keep real stdio frames/deadlines/cleanup.
- Review exact catalog/gate/rust-suite identities before renaming a linked test;
  land mapping changes with test changes, using existing migration machinery.

Exit: nonempty discovery and exact results in service, protocol and stdio owners;
unchanged contract or explicit reviewed migration; no accidental proof promotion.

### TO07 — CLI, Browser Boundary And Examples

Owners: CLI, playground_wasm and the separate examples package.

- Replace C02 macros with rstest named exact-output and contains-output families.
  Keep direct example binaries and package/reload/state scenarios.
- Parameterize real CLI success/rejection cases, preserving complete exit and
  stream assertions. Removed source and removed CLI flags are different cases.
- Parameterize WASM compile/run/JSON expectations without collapsing the entry
  routes; retain wasm32 release checking. Browser UI loading remains separate
  evidence, not a capability claimed by host-side Rust tests.

Exit: actual processes still run, every retained example target is represented,
feature/manifest/lock boundaries remain intact and no browser proof is invented.

### TO08 — Non-Rust Consumers And Fixture Housekeeping

Owners: scripts, editors, site, matrix, benchmark and fuzz data.

- Keep current Node/Tree-sitter/editor/fuzz/benchmark frameworks, not rstest ports.
- Reuse collision-checked materialization while preserving existing new-root
  guarantees, logical names, isolated workspace-symbol/selection sessions, private
  profiles and byte-based macOS socket limits.
- Remove proven unused helpers/orphan data; keep source-backed and metadata-only
  schema setup distinct. Review stale filenames/comments by current contract,
  not blanket replacement of words such as `host`, `provider` or `task`.
- Preserve package/grammar/highlighting/parity tests and independent scope goldens.
  Keep malformed fuzz seeds and actual benchmark measurement methodology.

Exit: affected native/provider/command/input/render boundaries and provenance
checks still exist; no stale capture is relabeled current because helpers moved.

### TO09 — Repository Acceptance And Documentation

Scope: all changed owners, Cargo manifests/locks, CI/aliases, fixture inventory
and current testing guidance. This is not a second full implementation pass.

- Every audited family has a justified disposition, including intentional
  no-change outcomes. Remove obsolete macro/helper wrappers only after consumers
  and assertions are accounted for.
- Review old/new independently selectable cases and exact linked identities;
  no lost expectation, empty glob, stale skip filter or dangling integration mod.
- Final default/all-feature/serde/artifact-codec, example, WASM and platform jobs
  retain their meaning. Dependency updates are dev-only and unrelated locked
  package versions remain unchanged where possible.
- Update the durable testing strategy/validation entrypoints, archive phase
  ledgers if needed, and record software results separately from strict/native
  evidence gaps. Do not mark this plan complete from code formatting alone.

Exit: final gates below pass or explicit genuinely external/evidence limitations
remain recorded without claiming their acceptance.

## Required Before/After Review For Every Family

A short review/commit note must answer:

1. Which existing semantic contract and owner does each old test establish?
2. Are inputs byte-identical, or do installation/options/IDs/events differ?
3. What setup is actually duplicated, and what must remain fresh/independent?
4. Where is every old assertion, error phase, side effect and cleanup check now?
5. What actual generated names replace the old names, and which filters/catalogs
   consume them? Does a selected test command execute a nonzero expected set?
6. For a proposed deletion, which retained test establishes the **same complete
   contract and public path**? If none is demonstrated, retain it.
7. Which targeted commands ran and what do their results prove—not prove?

Do not create a permanent parallel case database to collect these answers.
Existing matrix catalogs remain authoritative where they already own evidence.

## Evidence Preservation

The existing LSP inventory retains **1,327 semantic / 1,477 execution IDs** and
an immutable execution baseline; 158 migrated proof records remain pending.
B02–B08 are reopened, with B00/B01/B09 historical acceptance preserved and B02
as the current native checkpoint (1,423 execution obligations remain).

- Test renaming/link changes must use existing reviewed old/new identity/hash
  records where required; never delete pending obligations to improve counts.
- Passing a renamed historical test does not certify changed contracts.
- The previous 27 installed checks passed only in matching-provenance partitions;
  aggregate npm timed out. New helper/fixture inputs need fresh provenance before
  reusing that proof. Existing merge validation must remain strict.
- Service, protocol, installed-provider, command, physical Input/Render and OS
  coverage are independent. This refactor is not permission to declare strict
  or Windows/native acceptance complete.

## Validation And Completion

Ordinary iterations run nonempty selected owner families, generated-case listing,
formatting and affected Clippy. Full gates remain required at repository closure
or in the owning CI lane; phases are not permission to skip them.

Pilot naming/selection examples (commands for future execution):

```sh
cargo test -p vela_bytecode --lib compiler_lowers_record_writeback --all-features --locked -- --list
cargo test -p vela_vm --lib runs_compiled_record_writeback --all-features --locked
cargo test -p vela_bytecode --lib compiler::tests::expressions::assignments_and_access::compiler_lowers_record_writeback::case_1_nested --all-features --locked -- --exact
```

Use actual discovered names for later families, rather than copying a filter
that quietly matches zero tests. Selected modes must cover the ordinary default
lane and relevant feature lanes, not solely all-feature unification.

Final Rust gates, from [validation.md](validation.md):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked --no-fail-fast
cargo clippy --manifest-path examples/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo test --manifest-path examples/Cargo.toml --all-features --locked --no-fail-fast
cargo doc --workspace --all-features --locked --no-deps
```

Include affected default-feature, example-bin, wasm32, benchmark/fuzz-build,
site/grammar/package, Node matrix, protocol/process and installed-editor gates
from the same source of truth. Preserve finite timeouts and raw failed runs;
partitions need the existing exact-provenance checks, not relaxed validation.
A site build, fuzz compilation or benchmark build is not corresponding behavioral
or performance acceptance.

Measure performance only if claimed: record matching toolchain/features, selected
inputs, preparation/compilation counts, execution counts and repeated timing
samples separately from the organization change. No promised speedup or test-
count target is part of this plan.

## Execution Outcome

The inventory and stage checklists above preserve the pre-migration baseline and
review criteria. All stages have owner-scoped implementation or explicit no-change
outcomes in the [execution record](archive/test-organization-acceptance.md).

| Backlog | Outcome |
|---|---|
| C01 | Current macro UI skip repaired and discovery checked; full UI gates retained. |
| C02 | Both example registration macros replaced; all 14 actual bins covered. |
| C03–C04 | Eleven shared exact-byte fixtures including the pilot; lowering and execution assertions remain independent. |
| C05 | One narrow fresh HIR/analysis source-graph helper; no shared result oracle. |
| C06–C07 | Existing fresh setup consolidated and homogeneous typed cases named across owners. |
| C08–C09 | Existing marker/schema/fixture materialization reused, with collision/symlink regression checks and fresh installed provenance. |
| C10 | Compiled/linked convenience APIs retained: equal results do not establish duplicate public-path coverage. |
| C11–C12 | Historical filenames retained where they still own rejection or invocation-local safety proof; no cosmetic rename required. |
| C13 | Existing diagnostic normalization and exact formatter/recovery bytes preserved; no policy spreading or golden regeneration. |

The completed bounded pass does not impose a conversion quota or a new discovery
framework. Future test additions should follow the ownership/naming rules here;
aggregate CI gates and separate native-input evidence remain required, not replaced
by this refactor's partition records.
