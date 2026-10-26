# SF04 — MIR/compiler input organization and compiler UI-S

Implementation evidence for the scoped SF04 handoff; no commits made. This is not
workspace/SF12 acceptance or SF06 execution-equivalence evidence. Only
`crates/vela_mir`, `crates/vela_bytecode`, this report, and the authorized root
`tests/fixtures/conformance/collections/set_combinations.vela` were written.
Concurrent changes in other owners were left alone. The initial git diff was empty.

## Result and boundaries

- Relocated **48 source inputs and 12 existing MIR expectations**, **37,852 bytes**
  total. Added **eight independently authored compiler diagnostic goldens**.
  There are 67 owner-local source/expectation files plus the one root shared input.
- Every relocation was checked against the original evaluated Rust literal by a
  temporary Rust program using `assert_eq!(original.as_bytes(), include_bytes!(...))`
  **before replacing the literal**. Three passes checked 41, 14 and 5 files.
  Raw leading/trailing newlines, indentation, cooked escapes and missing final
  newlines were retained. No source formatting or golden regeneration occurred.
  Temporary relocation/check programs were removed after validation.
- Existing MIR/bytecode typed assertions, helper setup, logical SourceIds,
  function selection, module insertion order, target descriptors, defaults,
  register/slot/MethodId checks, guards, effects, source coverage, safepoints and
  codec mutation/rejection tests remain. No test was removed or merged.
- Existing library and `instruction_layout` target ownership is unchanged.
  Bytecode still consumes built HIR through `compiler/test_support.rs`; no syntax
  dependency, new support crate, framework, schema, bless path, product API or
  artifact-format change was added. Codec preflight is `artifact-codec` gated.
- Scoped fixture `.gitattributes` in both owners enforce LF for `.vela` and
  `.expected`; no repository-wide renormalization. The root fixture's attributes
  require the parent/shared-owner follow-up because only that exact root source
  path was writable here.

## Full census family review

Paths below are relative to each crate's `src/` unless otherwise stated. These
are suitability decisions, not a zero-inline target. The proposed census moves
were checked against the actual sources: many supposedly complex scenarios use
small one-line inputs and substantial *Rust target setup*, not large Vela inputs.

| SF00 family | Disposition and retained proof |
|---|---|
| MIR `tests/{builder,control_flow_builder,loops_builder,aggregates_builder,assignments_builder,tuple_assignments_builder,patterns_builder,try_builder}.rs` | **File-backed:** all 11 full `program.dump()` pairs in this family, plus guarded variant and default/root/lambda try documents. **Retain-inline:** short aggregate probes, branches, loop mode variations, one-line aliasing RHS writes, nested field/tuple chains and `value??`/effectful-operand comparisons. Their varying operator and exact evaluation-order assertion remain clearer together. The 193/259-byte scalar/dynamic operator sources are paired with their long exact dumps. Descriptors, literal contexts, target insertion and verifier setup remain Rust. |
| MIR `builder/tests/{calls,closures_defaults,constructors,declarations,guards,reflection}.rs`; `verifier/tests/builder_sweep.rs` | **File-backed:** dynamic-method source/function-dump pair, closure topology/default source, central guard placement and imported declaration document. **Retain-inline:** named-call permutation probes, constructor operand/slot/default probes, terminating argument/field cases, reflection arity/placement/effects and short method-default prologues. Their short inputs are not extracted simply because the descriptor setup is long. **Generated/retain-inline:** finite verifier sweep variants with their own configured targets. Helpers still close their own snapshots; no substitution of the production compiler. |
| MIR `tests/{aggregate_patterns,call_guards,control,dynamic_targets,evaluation_order,host_builder,identity,lambda_targets,model,reservations,signature_defaults,snapshot_validation,targets}.rs`; `input/try_targets.rs`; `verifier/tests.rs`, `verifier/tests/{contracts,liveness,try_regions}.rs`; `effects.rs` | **Unit-data:** hand-built operations, ownership, reservations, malformed edges, origin/guard/effect/liveness and target corruption. **Retain-inline:** small HIR/signature controls. **Generated:** finite builder/verifier variants. Invalid MIR cannot be represented by valid source fixtures. Production messages are **not-test-input**. No edits selected. |
| Bytecode `compiler/tests/{expressions,closures_and_bindings,script_methods,state,module_resolution,type_contract_constructors,contract_delegation,value_method_shapes}.rs`, `expressions/assignments_and_access.rs` | **File-backed:** nested captures, seven multi-declaration method-dispatch programs, typed record writeback, computed slices and Option/Result helper chains. **Shared-input:** set combinations and existing root conformance includes. **Retain-inline:** concise expression/branch/match probes, state initializer safe/unsafe contrasts, short module vectors with visible logical identities, contextual/guarded pairs and tuple/record constructor contracts. Similar method programs are not normalized or artificially combined; their capture/pattern/default/typed-let ownership differs. Larger remaining single-assertion expression probes stay local where the terminating expression/pattern is the point. |
| Bytecode `compiler/tests/literals_and_calls.rs`, `literals_and_calls/collection_methods.rs`, `loops_and_errors.rs`, `receiver_lowering.rs` | **File-backed:** reflection metadata chain, Option/Result callbacks and scalar loop. **Retain-inline/unit-data:** scalar tags, radix/escape/range spellings, signed minima, custom receiver registry probes, dynamic named fallback, small callback rows, multi-exit range-plan rejection and enum loop patterns. Registered and neutral compilation remain distinct. Source text in the scalar loop was not changed to match a benchmark variant. |
| Bytecode `compiler/tests/{diagnostics,diagnostic_contracts,call_diagnostics,literal_validation,phase0_frozen_contracts,removed_type_hints,service_impl}.rs` | **File-backed/UI-S:** eight source/golden pairs described below. **Retain-inline/generated:** compact invalid call, duplicate field/member/ID, hint, alias and range matrices, removed-feature rejection and ordinary-method positive controls. Multi-error declaration inventories stay next to their ordered code expectations; this pass does not replace them with one golden or flatten them into `is_err()`. Low-level diagnostic construction is **unit-data**. Syntax/semantic/compiler failure phases remain distinct. |
| Bytecode `compiler/semantic_input/tests/{external_descriptors,field_assignment_contracts,lambda_targets,literal_ownership,logical_records,roots_schema,script_methods,target_placements,task_targets}.rs`; `semantic_input/try_targets.rs` | **File-backed:** selected-root nested lambdas, schema/parameter/default/local guard boundaries, shared-trait-body try ownership, typed and erased container mutations. **Retain-inline:** concise external descriptor, field contract, literal ownership, logical-record, script method, required-signature default and removed-task controls. **Unit-data:** descriptor corruption and manually closed targets. `prepare_source*`, registry views, root selection and exact target owners are unchanged. Reflection logical-record probes remain close to the field-name assertions rather than sharing the larger compiler reflection program. |
| Bytecode `compiler/tests.rs`, `compiler/tests/{requests,native_effects,block_tail_semantics}.rs`; `compiler/mir_backend/selection/tests.rs`; `linker/tests.rs`, `artifact/tests.rs`, `binding_schema/tests.rs`, `portable.rs` | **File-backed:** lexical debug regions, cache-site/native metadata seed and public binding-schema document. **Shared-input:** existing block-tail source. **Retain-inline:** minimal source seeds for source-set selection, linked generation sealing, native ceilings, selected physical coverage and portable corruption. **Generated:** dynamic method/body wrappers. **Unit-data:** artifact/cache/schema/old-version mutations. The selection loop with a captured callback is intentionally not replaced by the different scalar-loop source. Codec assertions and unsupported package-metadata behavior remain unchanged. |
| Bytecode `{lib,linked,program_image,scalar_plan,cache_site,cache_policy,script_methods}.rs`, `package_metadata/tests.rs`, `verification/{tests,linked_tests}.rs`, `compiler/{constant_encoding,error}.rs`, `semantic_input/placements/helpers.rs`, `tests/instruction_layout.rs` | **Unit-data:** bytecode construction, layout sizes, handles, counts, source coverage, snapshots, invalid verification states and typed errors. **Retain-inline:** minimal compiler seeds. No fixture can replace deliberately malformed instructions. Package metadata was not made portable. No changes selected. |

### Cross-owner duplicate review

- **Selected:** `set_combinations.vela`, compiler
  `compiler_lowers_value_method_ids_after_set_combination_chains` and VM
  `runs_compiled_set_combination_methods`: exact **931-byte** equality checked.
  SHA-256 `dc37196944bf1f1115e73e468ce724638b32dd900d0a08bb81c4c3e922ced967`.
  Compiler now includes the authorized root source; no VM writes were made.
  The coordinated VM consumer update is recorded in [SF05](test-input-sf05.md),
  retaining standard-native setup and the independent `OwnedValue::String("daily,quest")`
  assertion. It belongs to the following stage commit, not this compiler change.
- **Not promoted to new root inputs:** scalar loop (265 bytes) and computed slice
  (395 bytes). Compiler-local extraction is useful; benchmark writes and other
  root paths are outside this assignment. SF11 may deliberately share these exact
  origins later after checking benchmark packaging. No benchmark duplicate is
  claimed removed by SF04.
- **Retain-inline:** 153-byte enum descriptor and 147-byte qualified record
  candidates. They are short, their graph/descriptor environments differ, and the
  extra shared files would obscure the assertions without a worthwhile gain.
  The tiny method catalog, tuple-match and match-guard repetitions likewise stay
  local. Existing root conformance inputs are reused without cosmetic relocation.

## Selected moves

All paths below are under `tests/fixtures/` of the stated owner. A paired row gives
source bytes / unchanged expectation bytes. Original assertion/test names remain
in their original modules; includes are direct, with no source factory.

### MIR: `builder/`

| Stem | Bytes | Consumer |
|---|---:|---|
| `parameter_storage` | 49 / 1329 | `tests/builder.rs` |
| `nested_blocks` | 78 / 1416 | `tests/builder.rs` |
| `heap_literal` | 54 / 1183 | `tests/builder.rs` |
| `scalar_operators` | 193 / 3851 | `tests/builder.rs` |
| `dynamic_operators` | 259 / 4017 | `tests/builder.rs` |
| `compound_assignment` | 42 / 1446 | `tests/assignments_builder.rs` |
| `if_join` | 169 / 1933 | `tests/control_flow_builder.rs` |
| `range_loop` | 53 / 1827 | `tests/loops_builder.rs` |
| `match_order` | 59 / 2041 | `tests/patterns_builder.rs` |
| `option_try` | 52 / 3304 | `tests/try_builder.rs` |
| `tuple_writeback` | 60 / 1967 | `tests/tuple_assignments_builder.rs` |
| `dynamic_method_order` | 89 / 1309 | `builder/tests/calls.rs`; existing function-only dump |
| `guarded_variant.vela` | 186 | `tests/patterns_builder.rs` |
| `try_body_owners.vela` | 116 | `tests/try_builder.rs` |
| `closure_tree.vela` | 165 | `builder/tests/closures_defaults.rs` |
| `central_guards.vela` | 255 | `builder/tests/guards.rs` |
| `imported_declarations.vela` | 303 | `builder/tests/declarations.rs`; small dependency remains inline |

Short sources in paired rows are extracted to keep the independently reviewed
source/dump pair together, not because short programs generally need files.

### Bytecode

| Directory/stems (`.vela`) | Bytes, in listed order | Consumers |
|---|---|---|
| `compiler/computed_slice`, `option_result_helpers` | 395, 891 | `tests/value_method_shapes.rs` |
| `compiler/reflection_collections`, `option_result_callbacks` | 1130, 516 | `tests/literals_and_calls/collection_methods.rs` |
| `compiler/scalar_loop`, `nested_captures` | 265, 189 | `tests/loops_and_errors.rs`, `closures_and_bindings.rs` |
| `compiler/named_default_method`, `self_method`, `captured_method`, `binding_pattern_method`, `record_variant_method`, `tuple_variant_method`, `typed_let_method` | 376, 295, 312, 311, 424, 407, 318 | `tests/script_methods.rs` |
| `compiler/typed_record_write` | 255 | `tests/expressions/assignments_and_access.rs` |
| `compiler/debug_regions`, `cache_metadata` | 415, 293 | `compiler/tests.rs` |
| `compiler/public_binding_schema` | 295 | `binding_schema/tests.rs` |
| `semantic_input/typed_containers`, `erased_containers`, `shared_trait_try` | 242, 620, 229 | `semantic_input/tests/target_placements.rs` |
| `semantic_input/selected_lambdas`, `guard_boundaries` | 204, 177 | `semantic_input/tests/lambda_targets.rs`, `roots_schema.rs` |
| `ui/semantic/type_contract`, `literal_range`, `negated_const_range`, `unknown_named_argument`, `positional_after_named`, `too_many_arguments`, `duplicate_argument`, `missing_argument` | 73, 27, 45, 102, 89, 62, 82, 77 | `tests/diagnostic_contracts.rs` |

The root set source is the remaining relocation. Eight new `.expected` files
beside the UI sources are added coverage, not relocated or generated output.

## Compiler UI-S contract

`compiler::tests::diagnostic_contracts` retains its 14 test identities. Five of
those tests now consume eight full-output pairs (the existing call-placement
contract intentionally retains its four ordered cases in one test):

- `type_contract_diagnostic_contract_pins_expected_and_actual_types` — SourceId 705.
- `literal_diagnostic_contract_pins_contextual_conversion_failure` — 706.
- `literal_diagnostic_contract_pins_negated_const_origin` — 710.
- `named_argument_diagnostic_contract_pins_parameter_candidates` — 707.
- `call_placement_diagnostic_contracts_pin_messages_and_source_locations` — 711–714.

Semantic errors still go through `only_semantic_diagnostic` and its exact count
and failure-phase assertion. The negative const remains a direct compiler
`InvalidIntLiteral { literal, error }`, now also explicitly checked before
projection. The existing syntax helper and other typed diagnostic contracts are
not changed. All eight retain severity, exact code/message, primary span,
ordered labels and empty candidates/repairs checks. `source_span` still demands
one occurrence. Rendering uses explicit logical `<stem>.vela` names and
`vela_common::diagnostic_render`, not physical fixture paths or Engine.

Goldens were authored independently from the pinned messages/spans and reviewed
source lines; the tests never generate expected output. Comparison is exact LF
including the final newline, without trimming or path normalization. Before
rendering, every primary/label span must belong to the exact SourceId, be nonempty,
be in range and lie on UTF-8 boundaries.

`compiler::tests::ui_semantic` adds 13 discovered tests: eight named valid
counterparts, one inventory and four negative controls. Inventory checks both
extensions for all eight pairs, nonempty UTF-8 files, exact registration in the
existing typed test module, and exact directory membership. No glob discovery or
build script is involved; named includes remain explicit. The negative controls
reject an extra diagnostic, an out-of-bounds span, a wrong in-bounds span and a
wrong expected message. Missing-output and unregistered-input failures were also
exercised on the real corpus using the already-built test binary, then restored.
No missing/extra fixture or temporary mutation remains.

## SF06 portable execution handoff

`compiler::tests::portable_fixtures::extracted_fixture_is_portable_supported`
has four named `artifact-codec` cases. They compile, encode, decode, compare the
complete portable artifact, re-encode deterministically, link without host
registrations and verify. They **do not execute** and do not establish source vs
portable execution equivalence. SF06 can reuse these authored origins:

| Bytecode `tests/fixtures/compiler/` source | Suggested independent `main()` result for SF06 |
|---|---|
| `nested_captures.vela` | I64 17 (10 + 4 + 3) |
| `scalar_loop.vela` | I64 11940 (finite loop; continue before break) |
| `typed_record_write.vela` | I64 5 (count 2 + 3) |
| `named_default_method.vela` | I64 21 (level 7 + amount 5 × default multiplier 2 + offset 4) |

These expected values are source-derived handoff oracles, **not observed Engine
results**. Source/decoded routes, typed outcomes, budgets and independent Runtime
ownership belong to SF06. Package metadata remains unsupported by format 8;
reflection/cache seeds with custom registrations are not proposed as bare
portable-execution cases.

## Validation and honest failures

All terminal/test subprocesses had finite bounds (10–240 seconds for owner gates;
30 seconds for individual mutation probes). No timeout occurred in this task.
No workspace formatting, workspace test suite or VM execution was run here.

| Command/check | Result |
|---|---|
| Initial `git diff --stat` / `git status --short` | Empty before editing; later peer edits preserved. |
| Baseline `cargo test -p vela_mir -p vela_bytecode --locked` (180 s) | Passed before edits. MIR 213 + 3 doc tests and bytecode doc test visible in bounded output; complete baseline aggregate count was not retained, so no invented baseline total. |
| Three temporary Rust original-literal/file byte checks | 41 + 14 + 5 = 60 exact comparisons passed; total 37,852 relocated bytes. |
| First two bytecode focused invocations (180 s bound) | Both failed to compile: renderer/source were incorrectly imported from the `vela_common` root. Corrected to `vela_common::diagnostic_render`; no dependency or production export change. |
| `cargo test -p vela_bytecode --lib compiler::tests::diagnostic_contracts --locked` | 14 passed after the import correction, including all eight new output comparisons. |
| `cargo test -p vela_bytecode --lib compiler::tests::ui_semantic --locked` | 13 passed, including four deliberate `should_panic` controls. |
| Intermediate `cargo test -p vela_mir --locked` | 213 unit + 3 doc tests passed. |
| Final `cargo test -p vela_mir -p vela_bytecode --locked` (240 s combined lane bound) | **650 passed**: bytecode 431, layout 2, MIR 213, bytecode docs 1, MIR docs 3; zero failures/ignored. |
| Final `cargo test -p vela_mir -p vela_bytecode --all-features --locked` (same bounded invocation) | **666 passed**: bytecode 447, layout 2, MIR 213, bytecode docs 1, MIR docs 3; zero failures/ignored. Covers codec and MIR serde/test-support. |
| `cargo test -p vela_bytecode --lib compiler::tests::portable_fixtures --all-features --locked` | 4 passed; compilation/codec/link verification only. |
| `--list` for bytecode `compiler::tests::{ui_semantic,diagnostic_contracts}` with all features | Nonempty selections: 13 and 14 identities respectively; existing diagnostic identities unchanged. No `#[files]` cache-invalidation claim. |
| Real corpus mutation probes, exact inventory test, existing default binary | Missing `.expected`: exit 101 as expected; extra `unregistered.vela`: exit 101 as expected. Restored inventory: 1 passed, exit 0. |
| First shared-payload byte-audit script | Failed because the VM file contains more than one `let unioned` raw literal. Scoped to `runs_compiled_set_combination_methods`; equality and 931-byte hash then passed. This was an audit selector error, not a test/product failure. |
| Scoped `cargo fmt -p vela_mir -p vela_bytecode` and `-- --check` | Passed; no workspace formatting. |
| `cargo clippy -p vela_mir -p vela_bytecode --all-targets --locked -- -D warnings` and same with `--all-features` (240 s bound) | Both passed. |
| Static direct-include audit | All 67 owner `.vela`/`.expected` files have real source consumers. Twelve distinct existing/new root conformance inputs are referenced across these owners. |
| Scoped `git diff --check` | Passed for owner changes. |

### Final audit and limits

- Rechecked all **60 relocated files** against their recorded original lengths and
  SHA-256 values after formatting and test runs: unchanged.
- `cargo package -p vela_mir --list --allow-dirty` and the corresponding bytecode
  command passed (60-second per-process bounds). MIR listed 123 package paths,
  including all 30 fixture files (29 source/dump files plus attributes); bytecode
  listed 138 paths, including all 39 fixture files (38 sources/goldens plus
  attributes). Both emitted only the existing missing package-description/docs/
  homepage/repository warning. No missing owner-local fixture was found.
- `git check-attr text eol` confirmed `text: set` / `eol: lf` for owner-local source
  and expectation representatives. Parent added a narrowly scoped LF rule for
  root `set_combinations.vela` before this stage commit.
- Root conformance includes are **repository-only tests**, as with the 11 existing
  root conformance inputs already used by bytecode. The new root fixture is not
  part of an individual Cargo package. No production include was introduced.
  Package-list inclusion is not a claim that a separately published crate's test
  suite, or an isolated packaged workspace, was built. No isolated-package build
  was run; parent/SF12 owns that broader distribution acceptance if required.
- Reran default and all-feature scoped Clippy, scoped format check and scoped diff
  whitespace check after the last extraction; all passed. The temporary audit
  scripts, binaries and logs were removed before the package-list checks.
- Counts are actual owner invocations, not feature-unified workspace partitions.
  Peer crates were changing concurrently; no stale installed-editor, browser,
  VM-execution or workspace acceptance claim is inferred from these results.

Parent owns the VM shared-consumer edit, root attributes, shared
validation/progress/acceptance documentation and commits. SF06 owns the four
source/decoded execution comparisons.
