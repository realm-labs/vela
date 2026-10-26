# Test Organization Execution Record

This records the owner-scoped TO00–TO09 migration from `bb11d53eb`, not a
claim that every test should become parameterized. The
[plan](../test-organization-plan.md) owns scope; [validation](../validation.md)
owns gates. Rust commands use installed stable 1.99.1. `rstest` is dev-only,
version 0.28.0 with default features disabled. No custom framework was added.

## TO00 — Baseline And Pilot

Two byte-preserved record-writeback sources replace four copies. Compiler tests
`compiler_lowers_{nested,indexed}_record_field_writes` map to
`compiler_lowers_record_writeback::{case_1_nested,case_2_indexed}`; VM tests
`runs_compiled_{nested,indexed}_record_field_write_source` map to
`runs_compiled_record_writeback::{case_1_nested,case_2_indexed}`.
Slot-write lower bounds 3/1, indexed SetIndex, absence of dynamic field accesses
and exact I64 results 11/14 remain separate owner assertions.

All four focused cases passed again with all features, locked/offline. Actual UI
discovery lists two runners. The fast alias now skips the current
`removed_macros_and_borrow_apis_are_unavailable`, not its historical name.
Workspace formatting and diff whitespace checks passed.

## TO01 — Foundations And Front-End

Seven owners audited; six gained actual rstest consumers. Seventeen former tests
expose 94 named cases; the seven-owner total grows from 468 to 545 passes.

- Common: canonical primitive names, noncanonical rejections, numeric mappings,
  nonnumeric exclusions and exact scalar tags/type names/display strings.
- Def: canonical DefPaths and independently authored FNV/BLAKE3 expected hashes.
  Typed identities and explicit-ID/separation scenarios remain ordinary tests.
- Registry: seven removed collection names retain bare and parameterized-name
  rejection. Collision/registration/ownership assertions remain explicit.
- Syntax: four declaration/container-formatting tests become
  `formatting_formats_declarations_and_type_hints::case_1_item_declarations`,
  `case_2_state_identifiers`, `case_3_builtin_container_type_arguments` and
  `case_4_nested_result_container_type_arguments`. Exact source/output bytes,
  SourceId and empty diagnostics are unchanged; no normalization was introduced.
- HIR: all six insertion permutations and two signature-recovery/default-owner
  input families become independently selected cases. All resolution, span and
  metadata-index assertions remain.
- Analysis: integer suffix/radix, signed minima, overflow direction and float
  width/error rows preserve exact scalar values, spellings and error details.
- Package: no change; filesystem/identity sequences, distinct manifest assertions
  and whole-repository architecture audits are not redundant homogeneous rows.

C05's narrow `tests/fixtures/conformance/source_graph.rs` builds a fresh graph
for HIR and analysis. SourceIds 1/2, anonymous package, module names, insertion
order and import resolution are unchanged. Shared source files and both owners'
assertions/test identities are unchanged; the dependency is not a standalone case.

Before: 468 tests passed. After: 545 passed in both default and all-feature modes.
Both-mode all-target Clippy passed for all seven owners. Discovery verified all
94 cases and six representative exact selections each ran one test. Changed-file
format and whitespace checks passed. Validation used a source snapshot with its
own lockfile during parallel editing; edited files matched the snapshot bytes.
Final combined-tree gates are recorded separately in TO09.

Most family names remain searchable prefixes; split families are explicitly named
`primitive_tags_reject_noncanonical_names`, `nonnumeric_primitives_have_no_numeric_tag`,
`resolves_binary_and_hex_integer_spellings`, `accepts_signed_minima`,
`classifies_both_integer_overflow_directions`, `validates_float_widths` and
`rejects_float_overflow`. No active catalog/filter referenced the retired names.
Historical archive references remain historical, not compatibility wrappers.

## TO02 — MIR, Compiler And Shared Inputs

Nine C03/C04 fixtures replace 18 exact source copies across compiler/VM/Engine:
assignment operators, index reads, block tails, break/continue, script self writes,
script named-argument order and three receiver-specific method-call forms.
All 18 original raw literals matched the extracted bytes, including newlines.
Consumer names and registry differences remain; lowering/operand facts and runtime
results/trace `[2, 1]` are still independent observations.

Six original family names remain selectable prefixes with named cases:
`mir_verifier_accepts_full_builder_control_flow_and_loop_fixtures` (control flow,
range loop), `borrowed_host_execution_is_rejected_before_mir` (four rejections),
`compiler_rejects_f32_and_f64_literal_overflow` (two widths),
`loop_placement_failures_are_analysis_diagnostics_across_lambda_boundaries`
(four placements), `set_from_array_accepts_one_positional_or_named_source_operand`
(two calling forms), and `negated_float_const_and_schema_default_keep_literal_origins`
(two owners). Tuple verification moves intact to
`mir_verifier_accepts_trapping_tuple_destructuring_with_arity_guards`.
Raw malformed MIR/artifacts, linker/format rejection, roots/budgets and physical
selection retain their explicit tests. Similar but nonidentical sources stay separate.

Baseline: 208 MIR / 424 bytecode unit passes, plus integration/docs; 35 VM
execution-core and 24 Engine compiler-option passes. After: 213 MIR / 430 bytecode
all-feature unit passes (418 bytecode default), 2 integration and 4 doc passes;
75 paired VM and 24 Engine tests pass in each mode. Four-owner all-feature Clippy,
case discovery, two exact selections, changed-file format and whitespace checks
pass. Owner snapshots isolate lock updates; final root gates are in TO09.

## TO03 — VM, Reflection And Standard Values

Four numeric conversion families retain their function-name prefixes, exposing
5 widening, 2 mixed-expression, 6 successful narrowing and 6 out-of-range cases.
Three bytes failures map to `bytes_reads_reject_out_of_bounds` with
`case_1_negative_index`, `case_2_index_at_length`, `case_3_short_endian_buffer`.
All 34 numeric/bytes/char source literals remain byte-identical. One existing-test
support helper constructs fresh standard registries, natives and linked programs;
no mutable execution state is shared across cases.

`primitive_negative_fixtures_fail_in_expected_phase` becomes a module containing
2 `compile_time` and 5 `runtime` cases. The existing fast-lane skip still matches
all seven; compiler codes and runtime error payloads retain separate assertions.
Reflection's absent-hint family exposes absent/empty/invalid-tuple cases. Two
stdlib ID tests become five typed function/method/type/variant/field lookup
families, retaining all positive, missing and removed-name assertions.

GC/root/budget/cache/re-entry/permission and invocation-local borrowed-native
scenarios remain explicit. Host's owned-carrier test, eight removal doctests and
workspace unsafe audit remain unchanged. Stdlib/runtime manifest-wide coverage
and uniqueness loops remain exhaustive rather than fixed representative lists.
C10's compiled/linked convenience APIs stay distinct public-path tests; no
redundancy was established from similar results. C12 needs no filename-only rename.

Baseline numeric/bytes/char: 8/10/1 passed. After: 23/10/1; conformance 11,
reflection metadata 4 and stdlib ID cases 17 passed. All five owners' default
library tests: 914 passed. All-feature tests/docs in the owner snapshot: 980
passed with its incomplete-root unsafe audit skipped; the real-root unsafe audit
passed separately. Discovery, exact selections, conformance skip, scoped Clippy
and formatting passed. Final root coverage runs the unsafe audit without exclusion.

## TO04 — Engine, Reload And Bindings

Three string tests become `executes_string_literals::{case_1_multiline,
case_2_interpolated,case_3_escaped_braces}` with original bytes, fresh Engine/VM
and exact strings. Scalar conversion retains its prefix with 11 typed cases;
decoding/rejection assertions move intact to
`script_arg_conversions_decode_and_reject_scalar_tags`. Four invalid Option
inputs move from the positive scenario to `script_arg_conversions_reject_invalid_optional_values`.
Byte buffers, tuples, Result/maps/sets and runtime call tests stay explicit.

Bindgen's keyword family exposes five exact escaped identifiers and syn parsing;
its schema-only iterator rejection family exposes parameter and nested-model
cases, retaining diagnostic code and source presence. Source replacement in the
generated-binding reload test now asserts exactly one match before replacement.
Hot-reload ABI, old code/state/native owners, rejected activation, runtime affinity,
re-entry, cache fault and authorization scenarios retain existing fresh helpers
and all observations. Hot-reload and compile-test need no rstest dependency.

Four-owner baseline: 608 passes; after: 627 default and 633 all-feature passes,
including integration and doctests. Both-mode Clippy, case discovery, nonempty
exact selection, changed-file format and whitespace checks passed in the isolated
source snapshot. No active external references required migration.

## TO05 — Macros And Rust Boundaries

Signature borrowed-root/return/wrapper rejection families expose 9/8/12 cases;
inputs and phase distinctions are unchanged. Rejection checks now assert exact
diagnostic strings rather than substrings or only `is_err()`. Three export
rejections map to `export_rejects_invalid_contracts` (wrapper, effect removal,
task-spawn effect). Owned value emission remains a record/enum pair with all
positive/negative token checks; host-projection attributes expose five cases.
Four value-shape rejections become `rejects_unsupported_value_shapes` (skipped
fields, generic structs, skipped variants, tuple variants). Type identity keeps
four exact rejection cases and an explicit successful module/name test.

Trybuild runners/globs, all reviewed stderr files, macro execution and removal
coverage remain unchanged. Historical module names that own rejection coverage
stay named; a cosmetic rename would only churn references (C11).
Baseline: 21 unit + 8 value integration + 6 export execution + 2 UI runners.
After, both feature modes: 56 unit + 8 + 6 + 2 pass. All 27 UI fixtures pass
(one compile-pass, 22 interop compile-fail, four removal compile-fail). Discovery
lists 56 unit tests; exact selection, scoped Clippy and formatting pass. The macro
doctest harness has zero tests; broader removal doctests are covered in TO09.

## TO06 — Language Service And Protocol

Thirteen pure cursor classifications become `cursor_context_classifies_kind`
cases, keeping SourceId, source bytes and expected kinds. Two line-index tests
become four `utf16_offsets_resolve_exact_bytes` and two
`utf16_offsets_reject_unaddressable_columns` cases. Completion projection becomes
four `completion_edit_projection_converts_unicode` and five
`completion_edit_projection_rejects_invalid_byte_ranges` cases. Byte/UTF-16 and
LF/CRLF expectations remain independently authored, not computed by production.

The 16 retired identities were unlinked after catalog/gate/review/filter audits;
no evidence migration or support-code changes were needed. Linked cursor tests,
ordered overlay/lifecycle/cancellation, frozen snapshots, cache equivalence and
stdio framing/deadline/cleanup scenarios remain unchanged.

Before/after passes: cursor 36/36, line index 5/9, projection 24/32; unchanged
service lifecycle 18, shared markers 2, protocol lifecycle 69, cancellation 16 and
stdio 10. All 28 new cases pass individual exact selection. Affected all-feature
families, scoped Clippy and formatting pass. The source-snapshot aggregate hit
240 seconds, so only bounded owner passes were claimed here; complete final
identity coverage is reconciled in TO09 without an aggregate-success claim.

## TO07 — CLI, Browser Boundary And Examples

The two example registration macros are removed after accounting for every
consumer: `example_exact_output` has eight cases and `example_contains_output`
has six. All 13 existing executable/output pairs and comparison policies remain;
the package-demo adds real subprocess coverage for the fourteenth retained bin.
The five-source removal loop becomes named rejection cases with fresh Engines.
Examples remain a separate workspace with an explicit dev dependency; its lock
adds ten packages and upgrades/removes none.

CLI success maps to `cli_runs_script::{case_1_script_main,
case_2_standard_time_and_random_helpers}`; diagnostic cases use
`cli_reports_script_diagnostics` for removed async source, compile error and
runtime error. Status, streams, filenames, coordinates and stack assertions remain;
removed flags and help stay separate tests.
WASM's two JSON successes share `run_script_returns_json_value`; the array check
is strengthened to complete equality. Four removed-source cases retain both
compile/run routes, independently discovered as eight cases. Production code,
error codes, null results, severity and span-object assertions are unchanged.

Before/after combined CLI/WASM/example gates: 31/43 passes per feature mode.
After: CLI 8, host WASM 14, examples 21 (14 real bins, six removal/trait tests and
one package lifecycle test). All-feature Clippy, wasm32 release check, default
CLI/WASM Clippy, discovery, eight exact selections and scoped formatting pass.
Cross-compilation and host tests are not browser execution evidence.

## TO08 — Non-Rust Consumers And Fixture Housekeeping

The existing `FixtureWorkspace` now owns `appendTo`, consolidating repeated
provider, native-input, completion and signature fixture copying. `materialize` still
requires a new root; append requires an existing isolated directory. Preflight
checks symlinks and destination collisions, then probes the complete planned tree
on the destination filesystem to detect native case/Unicode/file-directory aliases.
The probe is always removed; writes are exclusive. The documented contract assumes
an exclusively owned stable root, not transactional recovery from arbitrary I/O
failure or concurrent mutation. Regression tests reproduced and fixed trailing-slash
symlink bypass and case-alias partial writes before acceptance.

Marker stripping reuses `parseMarkers`. Both symbol providers reuse the existing
metadata-only schema helper; the exact duplicate `symbol-schema.js` is deleted.
Source-backed schemas, intentional manifest/schema updates and transformed outline
text remain separate. Forty-two files across 16 fixture families and both symbol
schema outputs matched their previous bytes. Session isolation, private profiles,
short socket paths, cleanup and existing oracle/provenance checks remain intact.

Node matrix/editor/Zed suites: 321 baseline, 335 final passes, zero skips.
Focused independent review passed 32 checks and three native Unicode-alias probes.
Site docs (7), syntax (36 scope + 36 token checks), Tree-sitter corpus (3), both
package validators, syntax and whitespace checks pass. Site/Zed/grammar/fuzz and
benchmarks need no structural conversion: their specialized runners, independent
goldens, malformed seeds and measurement boundaries are retained. No demonstrated
orphan warranted deletion. Matrix catalog/baseline/captures remain unchanged:
1,327 semantic, 1,477 execution obligations and 27 declared installed checks.

### Fresh Installed-Editor Proof And Limits

The intermediate aggregate `npm --prefix editors/vscode test` timed out at 300
seconds. Its 25/27 partial proof became stale when the helper fix changed inputs;
it was not reused. Four fresh final-input partitions all exited zero:

| Partition | Passes | Elapsed | Process bound |
|---|---:|---:|---:|
| Base | 24 | 113.059s | 300s |
| Workspace symbols | 1 | 22.255s | 220s |
| Selection syntax | 1 | 480.063s | 640s |
| Selection lifecycle | 1 | 402.948s | 640s |

Selection used the existing 600-second internal limits and isolated suite paths,
not a longer retry of the aggregate. Existing `mergeEditorResults` verified
identical provenance, passing statuses and exactly one of each of 27 declared
checks. VS Code 1.137.0 on darwin/arm64; final fingerprints:

```text
inputsSha256: 2b7127e7ca4a8026882f94a38ef0b7a209bea2b682343c919a36eee9eaacee86
serverSha256: 1320b927ccda25692a96788aa7cd95bdf040bf4b35d803e55cf1422e0c92570e
```

Raw results, traces, invocation harness and validated merge remain locally in
`editors/vscode/test-results/run-T08f/`. The ignored invocation harness only calls
existing suites/materializers/merge validation; it is not a repository test runner.
No aggregate exit-zero, Windows, strict-matrix or physical Input/Render claim follows.
`npm --prefix editors/vscode run test:input` failed before interaction because
`observer.vsix` was missing during installation; logs remain in
`editors/vscode/test-results/input-0KGG45/`. This unrelated packaging blocker was
not patched or converted into passing evidence.

## TO09 — Combined-Tree Acceptance And Audit

All 23 root crate owners and non-Rust owners have an explicit disposition above.
Eighteen root crates use rstest only for tests; package, host, hot_reload,
stdlib_runtime and bindgen_compile_test deliberately do not. Both locks retain
all original package versions/sources/checksums: root adds nine external packages,
examples adds ten (its prior graph differs). No runtime API/format or production
behavior changed. Independent Git-tree review confirmed each TO00–TO08 commit's
manifest/lock edges, source/helper imports and assertion-preserving staging.

### Rust Gates

Run on the actual combined repository with stable 1.99.1, locked/offline:

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` and separate examples fmt | Pass. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Pass. |
| `cargo doc --workspace --all-features --no-deps` | Pass, no warnings. |
| `cargo test --workspace --all-features --no-fail-fast -- --test-threads=4` | Timed out at 240s during language-service; no aggregate pass. |
| Corresponding default workspace test command | Timed out at 240s; no aggregate pass. |
| Both feature modes excluding language-service | Timed out at 240s during LSP; no aggregate pass. |
| Per-owner/module/exact-test partitions, plus workspace doctests | Complete discovered-identity coverage below; no failed/missing tests. |
| Examples all-feature tests / all-target Clippy | Pass: 21 tests, including all 14 bins. |
| `cargo check -p vela_playground_wasm --target wasm32-unknown-unknown --release` | Pass. |
| `cargo bench --workspace --no-run` | Pass. |
| `cargo check --manifest-path fuzz/Cargo.toml --bins` | Pass. |

Counts are deduplicated by discovered owner/target/test identity, not sums of
repeated runs. All language-service/LSP module selections were checked against
actual `--list` output before execution. Timed groups were not retried with
larger budgets; only still-missing identities ran individually under finite bounds.

| Mode | Non-LS unit/integration | LS passes | Doctests | Total passes | Ignored | Missing |
|---|---:|---:|---:|---:|---:|---:|
| All features | 3,707 | 771 | 54 | 4,532 | 2 | 0 |
| Default | 3,692 | 771 | 54 | 4,517 | 2 | 0 |

Per-owner feature resolution may differ from aggregate workspace unification;
these are coverage results, not an exit-zero aggregate gate. CI retains the full
commands. Actual-root unsafe audits, all 27 trybuild fixtures in both modes,
portable/artifact tests, conformance and removal doctests ran without weakening
coverage. Native-target metadata confirms artifact-codec and all seven exposed
serde features; unfiltered offline metadata attempted uncached `js-sys 0.3.99`
and failed, so no all-platform metadata success is claimed.

Fifteen LS partitions cover 773 discovered identities: two ignored, 770 passes
in groups and one remaining reference-matrix case passed individually in 25.45s.
Thirty-one LSP partitions cover 842 library identities; 84 identities missing after
three group timeouts all passed individually (longest 95.42s). The reference
matrix is finite and intentionally exhaustive; it was not simplified for speed.
Default/all-feature standalone LS and LSP discovery used the same respective
executable hashes. The two existing ignored million-line checkpoints remain:

- `completion::tests::completion_contexts_scale_in_million_line_workspace`
- `incremental::tests::million_line_synthetic_workspace_checkpoint_refreshes_hir_for_body_edit`

Raw commands, lists, identity reconciliation, timings, timeout logs and unchanged
lock hashes remain under `target/to09-*`, especially
`to09-partitions-final-summary.log` and `to09-partitions-reconciled-{default,all-features}.log`.
These local artifacts are not a new committed test catalog or runner.

### Repository Gates And Limits

Final combined matrix/editor/Zed Node suites pass 335 tests. Site docs pass seven
checks, syntax passes 36 scope and 36 token checks, and Astro builds 145 pages.
VS Code/Zed package validators pass. Tree-sitter's unchanged corpus passed three
cases in TO08; grammar artifacts and fingerprints were not regenerated. Fresh
installed-editor software proof and the native-input packaging blocker are
recorded in TO08. Builds are not browser, performance, fuzz-campaign, native-input
or independent Windows proof. No evidence obligations/captures were relabeled.

Final fast-alias discovery lists 4,505 tests; the two current UI runners, unsafe
audit and all seven phase-separated negative conformance cases are correctly
excluded from that fast lane, not full gates. All 61 local Markdown links/anchors
across seven changed documents pass. Source extraction/lock audits, formatting
and whitespace checks also pass. Further wholesale parameterization is not
required: reviewed scenario/audit/no-change outcomes are part of completion.
Aggregate CI and strict/native evidence remain separate gates.
