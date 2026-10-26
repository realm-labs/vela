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
