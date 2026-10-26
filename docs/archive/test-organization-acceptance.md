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
