# SF03 — Semantic Graphs, Package Inputs And UI-S

## Scope And Result

Implementation evidence for SF03 in [the organization plan](../test-organization-plan.md)
and [runtime census](../test-input-census-runtime.md). Changes are restricted to
`crates/vela_hir`, `crates/vela_analysis`, `crates/vela_package`, and this document.
No shared-root fixture, other owner, Cargo dependency/target declaration, shared
status document, staging or commit was changed by SF03. Concurrent changes from
other stages are not included in this evidence.

- **75 existing input files extracted**, supported by **79 original-literal byte
  proof records** below. Records are source-file/literal selections, not a claim
  that every repeated occurrence has a separate row: a selected cooked literal
  can replace several identical occurrences in one file.
- **11 UI-S source/golden cases**: five HIR and six analysis, including four
  positive controls. Eight reuse existing source documents; three positive
  documents are newly authored. Eleven `.expected` files are new.
- New fixture payload totals: HIR **29**, analysis **48**, package **12**;
  **89** total, plus three owner-local `.gitattributes` files. These comprise the
  75 extracted files, three new positive sources and eleven goldens.
- **16 tests added**, none removed: HIR seven (five cases plus two inventory
  tests), analysis eight (six plus two), package one fresh-copy test.
- Post-change owner suites: **329 passed** in each of default and all-feature
  configurations; no failures/ignored tests. Scoped Clippy and formatting pass.
- All **14 deliberate negative mutations** fail at the intended assertions and
  are restored. Isolated Cargo-file-list copies pass the package unit and new
  semantic UI routes. Actual Cargo archive creation remains blocked by the
  pre-existing versionless workspace path dependencies; this is not reported as
  successful publishing verification.

No production behavior, source identity, expected fact, dependency, or compiler
phase was changed. There is no every-string extraction rule, framework, fixture
schema, build script, glob registration or bless mode.

## Targets, Environments And Preserved Proof

| Owner / target | Boundary and preserved observations |
|---|---|
| HIR `--lib` | Existing `source`/`package_source`, `ModuleGraph`, parsed ingestion and script-method helpers remain distinct. IDs, package/module identity, insertion order, binding maps, source ranges, captures, defaults, metadata and equality assertions stay in their original tests. |
| HIR `--test integration` | Existing conformance/service_impl/task_spawn modules retained; `semantic_ui` added to the existing aggregator (`autotests = false`). No Engine/bytecode execution substituted for HIR resolution. |
| Analysis `--lib` | Graph/body/FunctionId and bare/schema-specific helpers retained. Call targets, expression selection, inferred types, local flow, receiver ownership, callback fixed points, placement arrays and detailed typed diagnostics remain in Rust. |
| Analysis `--test integration` | Existing completion/conformance modules retained; new `semantic_ui` module added. Existing root completion normalization and shared conformance graph are untouched. |
| Package `--lib` | `parse_manifest` still consumes TOML; graph loaders still consume real temporary directory trees. ManifestFileId/spans/capabilities, deterministic discovery, duplicate roots, package identity, path boundaries, host-schema rejection and cleanup assertions retained. |
| Package `--test architecture` | Still audits repository Rust/Cargo source; not converted to Vela fixtures or a partial representative corpus. |

In particular, `facts/implicit_receiver_tests.rs::fixture` still requires exactly
one `hir::unknown_schema` diagnostic for `host::Row` before schema-backed analysis.
No generic helper imposed clean HIR on that intentionally unresolved environment.
Existing helpers always create fresh graph/schema state; none was moved across a
syntax/semantic/compile/runtime boundary.

### HIR UI-S

`crates/vela_hir/tests/semantic_ui.rs`, using existing `rstest`, runs these exact
filters under `--test integration`:

| Suffix after `semantic_ui::semantic_ui::` | Explicit setup / required result |
|---|---|
| `case_1_unresolved_name` | SourceId 1, anonymous package, `game::player`; one `hir::unresolved_name`, candidate `player` and both labels. |
| `case_2_unresolved_import` | Dependency first: SourceId 1 `game::reward`, public `grant`; SourceId 2 `game::main`; one `hir::unresolved_import`, `grant` suggestion. |
| `case_3_private_import` | SourceId 1 `game::reward`, private `secret`; SourceId 2 `game::main`; one `hir::private_import`, including the dependency declaration label. |
| `case_4_valid_import` | Same public dependency graph; no diagnostics, exact import declaration identity and binding reference to SourceId 1, package/module identity checks. |
| `case_5_valid_name` | SourceId 1 `game::player`; no diagnostics, actual parameter/local binding resolution. |

The route is `build_module_source_set`; failure must be
`HirSourceBuildErrorKind::Semantic`, never just any error. Complete `Diagnostic`
values are compared (including severity/code/message/primary span/labels and
empty candidate/repair fields), and all spans must address valid nonempty UTF-8
ranges in the known logical sources. Rendering uses `game/player.vela`,
`game/reward.vela`, `game/main.vela`, not physical fixture paths.

### Analysis UI-S

`crates/vela_analysis/tests/semantic_ui.rs` constructs a fresh graph with explicit
anonymous package and logical module `game`, resolves imports, and requires HIR
success **before** executable analysis. It selects `main` and its actual root
body. Existing unit-test IDs are retained:

| Suffix after `semantic_ui::semantic_ui::` | SourceId / FunctionId | Setup / required result |
|---|---|---|
| `case_1_identity` | 101 / 10101 | Bare generation; two invalid identity diagnostics, while script-reference/dynamic comparisons remain controls in the same source. |
| `case_2_call_placement` | 106 / 10601 | Bare generation; ordered unknown-name, positional-after-named, extra, duplicate, missing-argument diagnostics (five). |
| `case_3_constructors` | 202 / 20201 | Bare generation; eight ordered duplicate/unknown/missing-field or variant diagnostics. |
| `case_4_registered_calls` | 108 / 10801 | Explicit RegistryFacts for `game::add`: required `lhs`, defaulted `rhs`; one missing-required-argument diagnostic. |
| `case_5_valid_source` | 301 / 30101 | Bare generation; strict placement, source order amount/base, slots `[Some(1), Some(0)]`, zero diagnostics. |
| `case_6_valid_registry` | 302 / 30201 | Same explicit registry signature; external-named placement, source order rhs/lhs, slots `[Some(1), Some(0)]`, zero diagnostics. |

Checks include diagnostic count/order, exact codes, Error severity, primary source
slices, label counts, known SourceIds, valid nonempty UTF-8 spans, and empty
candidate/repair fields. The independently retained original unit assertions
still check detailed placement/fact contracts. Full goldens additionally freeze
messages, all rendered labels, coordinates, excerpts and caret positions using
logical `game.vela`.

Both UI owners compare the entire renderer line stream joined with `\n`, without
trimming, path rewriting or newline normalization. Error goldens intentionally
have no trailing newline; successful controls have **empty output goldens**, not
empty source inputs. Golden text was reviewed against source spans and typed
contracts; there is no runtime output-generation/update path.

### Inventory And Freshness

Each owner has `semantic_ui::semantic_ui_inventory` and
`semantic_ui::semantic_ui_inventory_rejects_missing_orphan_empty_and_unregistered_files`.
An owner-local inventory reads the corpus directory and requires regular files
and the exact source/expected filename set for the explicit named rstest cases.
Names are taken from that runner's `#[case::...]` registrations, rather than a
second hand-maintained filename catalog. Empty registration/corpus, missing,
orphan and unregistered pairs fail. Dependencies live in `fixtures/modules`, not
among standalone UI cases. Source cases separately require nonempty text.

There is no glob expansion to invalidate. `include_str!` supplies Cargo input
tracking for registered files; runtime inventory detects added/removed disk
entries without a clean build. Actual missing/orphan/empty/unregistered directory
mutations were tested against the compiled inventory binaries, avoiding a mere
missing-include compile error masquerading as inventory proof.

Package trees use `src/graph.rs::tests::copy_tree`, copying regular files/directories
into existing unique temporary roots and rejecting symlinks. Each loader test
gets its own writable tree. `package_fixture_copies_are_fresh_and_do_not_modify_the_origin`
creates two copies, mutates one, checks the second and checked-in origin retain
original bytes, then removes both. Unicode root naming, duplicate root order,
relative dependency paths and dependency-host rejection remain intact.

## Complete Census Review And Inline Dispositions

Paths below are relative to the named crate unless stated otherwise. Review
covered the census families, helpers and finite `format!` consumers, not just
raw-string search hits. Original tests were read before extracting their inputs.
Residual source scans were used as a cross-check, not as an extraction threshold.

| Owner / census family | Concrete disposition and rationale |
|---|---|
| Package `src/manifest.rs` | **File-backed** complete manifest (`manifests/complete.toml`). Malformed tables, required-field, unknown-key and capability-only fragments **retain-inline** beside their field/span errors. |
| Package `src/graph.rs` | **File-backed** independent-root/shared-dependency and relative-path-dependency trees, including all tiny module files. **Retain-inline** overlap/absent root/ignored-file probes, path escape, nested roots, duplicate IDs, dependency cycle and host-error mutations: their topology or before/after mutation is the assertion. No extraction of outside-root relationships. |
| Package `src/identity.rs`, `tests/architecture.rs` | **Unit-data** identity/version strings; Rust/Cargo needles **not-test-input**. Architecture tests remain unchanged. |
| HIR `module_graph/tests/bindings.rs` | **File-backed** method/default binding, if/block captures, multiline condition and lexical scopes; unresolved-name input shared with UI. **Retain-inline** short shadow/capture/rebinding probes. `tuple_pattern_bindings_use_binding_token_spans` and `binding_tracks_for_pattern_locals` stay beside binding-token selectors rather than split one pattern example from its ranges. |
| HIR `bindings/executable_facts.rs` | **File-backed** source-path facts; repeated imported-value and public-reward inputs shared owner-locally. `map_entries_retain_ordered_hir_owned_logical_keys` **retain-inline**: exact literal/qualified/interpolated key spellings, escapes and order are the local contract. Short float/tuple/array/interpolation examples stay local. |
| HIR `bindings/import_resolution.rs` | **File-backed** executable operands and full constructor main/schema pair; reuse imported-value/public-reward. **Retain-inline** compact alias/shadow/pattern resolution, including `function_bindings_resolve_match_pattern_import_aliases`: its small match and import are read with the exact binding checks. |
| HIR `module_graph/tests/metadata.rs` | **File-backed** large hint, attribute, schema-default and trait-impl documents. **Retain-inline** `builtin_operator_trait_prerequisites_accept_complete_chain` and `...accept_derived_prerequisites`: adjacent short explicit/derived operator-prerequisite controls; `builtin_operator_derive_accepts_nested_script_field_traits`: compact nested prerequisite. `rejects_side_effecting_const_initializers` keeps safe/bad expression rows beside diagnostic checks. Unit/tuple hints and inherent-method metadata are short targeted examples, not an extracted catalog. |
| HIR `module_graph/tests/resolution.rs` | Existing unresolved/private import and valid import sources shared with UI, public/private dependencies file-backed. **Retain-inline** package identity/insertion ordering, declaration IDs, duplicate declarations/aliases, inaccessible-owner and required-method probes. `indexes_top_level_declarations_with_stable_ids` is a compact declaration-kind list; `duplicate_trait_and_impl_methods_report_both_spans` deliberately keeps duplicate definitions beside both-span expectations. |
| HIR `module_graph/tests/ingestion.rs` | **File-backed** parsed-source equivalence program; both fresh construction routes and exact whole-graph/ID equality preserved. Two tiny qualified-name documents **retain-inline** to expose identity rather than physical file organization. |
| HIR `src/source_ingestion.rs`, `module_graph/tests/signature_defaults.rs` | **Retain-inline** mode/source ordering, syntax-versus-semantic rejection, reserved package, required/default signature and recovered-body examples. Small wrappers and spans are clearer together; no helper phase changes. |
| HIR `src/script_methods/tests.rs`, `src/attributes.rs` | **File-backed** multi-owner trait/default expansion. **Retain-inline** tiny method catalog and `builtin_trait_methods_remain_catalogued_without_a_source_trait_shape`: source-root/module-root and builtin trait distinctions are explicit. Attribute display/primitive fields are **unit-data**. The census's 121-byte cross-owner method catalog remains intentionally inline, not a new global tiny-source pool. |
| HIR `tests/{conformance,service_impl,task_spawn}.rs` | Existing conformance **shared-input** unchanged. **File-backed** ordinary Inventory trait positive program. Removed attributes/path/async cases **generated/retain-inline**: bounded rejection tables and their valid name controls remain local; no source target moved. |
| Analysis `src/facts.rs`, `facts/body_binding_tests.rs` | **File-backed** complete HIR body, source/lambda, try-payload and body-binding documents. **Retain-inline** `analysis_facts_collect_function_signature_and_local_hints`, `...include_global_type_hints`, `...include_resolved_expression_facts`: one short fact-collection example each, kept with typed expected declarations/locals. |
| Analysis `facts/implicit_receiver_tests.rs` | **File-backed** game/shapes pair; explicit one-error HIR prerequisite, registry environment, receiver names and exact span selectors unchanged. |
| Analysis `executable/{tests,callback_tests}.rs` | **File-backed** concrete executable receivers, record iterators, map arity/find/logical flow, nested Option/Result/registered callbacks. **Retain-inline** `executable_totalization_preserves_fixed_point_callback_inference`, `collection_fallbacks_preserve_nested_receiver_targets`, `stdlib_callback_slots_keep_named_order_and_ignore_unrelated_lambdas`: each is one focused inference/placement flow with adjacent expression queries, not another broad callback demonstration. |
| Analysis `semantic_facts/tests/{constructor_targets,pattern_fields}.rs` | **File-backed** complete multi-module constructor and private-pattern inputs, larger trait/pattern program. Typed source/registry owner and method/field target checks retained; smaller one-expression negatives remain inline. |
| Analysis `semantic_facts/local_flow_tests.rs` | **File-backed** multi-statement write/join and erased-collection programs. Other concise local flow probes remain beside expected per-use facts. |
| Analysis `semantic_facts/tests/{enum_aliases,external_calls}.rs` | **File-backed** fixed enum API and defs dependency documents. **Generated** bounded import/base/parameter/callee wrappers remain `format!` inputs, including ambiguous, private, absent and shadowing cases; extracting each expansion would obscure the matrix. |
| Analysis `semantic_facts/tests/{host_variant_fields,registry_constructors}.rs` | **Generated/retain-inline** registry/source owner, visibility, variant field and hint matrices. Shared exact token API extracted once. `registry_record_constructor_results_keep_exact_owner_and_refresh_fields` remains inline: concise constructor expressions accompany registry refresh/owner assertions and decoy schemas. |
| Analysis `semantic_facts/tests/{local_exits,loop_flow,value_flow}.rs`, `local_flow/loops/tests.rs` | **Generated** finite statement/expression/loop matrices: `local_exit_facts_keep_only_reachable_successors`, `loop_uses_include_later_iterations_but_exclude_terminal_exits`, value-flow `analyze(expression)` wrapper, `exhausted_loop_budget_erases_carried_refinements_and_keeps_local_resets` with budgets `[0, 1]`. Keep terminated paths and budget-dependent refinements explicit, not many opaque expansion files. |
| Analysis `semantic_facts/tests/operator_targets.rs`, `semantic_facts/tests.rs` | **File-backed** closed-receiver-universe program. **Retain-inline** `unknown_and_any_operands_remain_dynamic_while_typed_operands_are_proven` and `runtime_record_members_are_dynamic_only_when_the_owner_schema_is_absent`: small comparisons intentionally read with expected target/schema availability. Removed borrowed-hint wrappers remain bounded **generated** rejection rows. |
| Analysis `validation/{tests,call_tests,constructor_tests}.rs` | **File-backed** multi-call/comparison/array ordering/loop/receiver/constructor programs and imported schema pair. Four existing sources also feed UI-S. **Retain-inline** single-call, parameter default and dynamic/unsupported tests; `shared_hir_body_constructor_facts_remain_executable_qualified` is a tiny shared-body ownership probe, not an additional large constructor fixture. No slot/source ordering assertions removed. |
| Analysis `validation/task_tests.rs`, `contracts/tests.rs` | **Retain-inline/generated** removed-task fragments and short source controls. Contract compatibility matrix is **unit-data**; do not replace typed contracts with executable examples. |
| Analysis `src/hints.rs`, `hints/{registry_tests,scope_tests}.rs` | **Retain-inline/generated** short schema/alias annotations. `registry_tests::check(imports, hint, expected)` intentionally generates the same structural hint in arguments/returns/trait/field/constant/state positions; static expansion files would lose the relation. `schema_hints_map_to_qualified_record_enum_and_trait_facts` and `imported_schema_alias_hints_map_to_qualified_facts` remain concise controls. Exact token API is the shared-input exception. |
| Analysis `registry/{registry_tests,abi_tests}.rs` | Typed compile-view/registry/ABI facts **unit-data**; removal-audit Rust includes **not-test-input**. `compile_view_facts_feed_analysis_targets_without_reflection_descriptors` **retain-inline**: short registry-backed main with its typed registry environment, not a standalone source tree. |
| Analysis `stdlib/function_tests.rs`, `stdlib/tests/{collection_facts,option_result_functions,option_result_methods,reflection_facts,numeric_conversions}.rs`, `stdlib/parameter_metadata.rs` | **Unit-data**, correcting the census's tentative file-backed suggestion: reviewed consumers directly call fact constructors with `TypeFact`, member names, signatures and typed callback metadata; these files do not feed complete programs into a parser/graph. `parse_i64` and similar strings are method identifiers, not source parsing. No invented stdlib source fixtures. |
| Analysis `literals/tests.rs`, `src/type_fact.rs`, `src/logical_records.rs` | **Unit-data** spelling/radix/range/context and structural fact tables. Short graph wrappers **retain-inline**; `numeric_literal_use_classifies_frozen_dynamic_operator_shapes` keeps signed/grouped operand spelling with its exact classification. No one-file-per-literal conversion. |
| Analysis `completion/tests.rs`, `hover/tests.rs`, `tests/{completion_fixtures,conformance}.rs` | **File-backed** substantial local-binding and player/reward query documents, unchanged marker/offset and result ordering. Short query fragments remain inline. Hover registry is **unit-data**, not a program. Existing completion source/expected and shared conformance origins unchanged; their normalization is not copied into UI-S. |
| Zero-source `vela_def/src/{lib,script,stable}.rs` | **Reviewed, no change needed**: canonical symbol/package/hash strings and explicit IDs feed identity helpers, not source compilation. Preserve stable hashes, package distinctions, rename and order independence. |
| Zero-source `vela_registry/src/{defs,lib}.rs` | **Reviewed, no change needed**: `TypeHintDef::parse` consumes hint fragments (including bounded removed-hint `format!` rows); remaining tests construct typed definitions/registries. No full-program consumer and no fixture quota. |

### Meaningful Duplicates

- HIR `modules/imported_value.vela`: identical 53-byte source reused by the
  imported binding and later import-refresh tests. Their ordering/state assertions
  are independent and unchanged.
- HIR `modules/public_reward.vela`: identical 28-byte dependency shared by
  resolution, alias/binding refresh and UI public-import controls. Private reward
  remains a separate 25-byte document; it is not normalized into the public case.
- Analysis `semantic_facts/token_api.vela`: identical **56-byte** document shared
  by hint scope and registry-constructor ownership tests, not their environments.
- The 82-byte constructor dependency exists independently in HIR and analysis.
  This tiny cross-owner coincidence accompanies different main graphs and phase
  assertions; keep it owner-local rather than introduce a cross-package include
  or unauthorized shared-root fixture. It is not a distribution-copy claim.
- Three 19-byte `shared.vela` files in the independent-roots package tree are
  deliberate distinct package-role files; same bytes do not erase package identity.
- Existing root conformance inputs remain authoritative; no shadow copies added.

## Validation, Counts And Limits

Commands ran from the repository root unless noted; finite timeouts throughout.
The initial owner baseline ran **before extraction**:

```sh
cargo test -p vela_hir -p vela_analysis -p vela_package --locked --no-fail-fast
cargo test -p vela_hir -p vela_analysis -p vela_package --all-features --locked --no-fail-fast
```

Both passed. These whole-owner baseline runs include the focused families later
changed. Early tool output retained only tails, so exact baseline wall times are
not available; no timeout was reported (tool limits 120–180 seconds). Baseline
counts reconstructed from unchanged test identities and the additive diff are
**313**: analysis lib 174/integration 2; HIR lib 110/integration 9; package lib
14/architecture 4; zero doctests. This reconstruction is distinguished from the
fully captured post-change counts below. A default owner run also passed after
initial extraction, before the UI additions.

Focused UI command `cargo test -p vela_hir -p vela_analysis --test integration
semantic_ui --locked` passed **8 analysis + 7 HIR**, with 2/9 existing tests
filtered. `cargo test -p vela_package --lib graph::tests --locked` passed **10**,
5 filtered. These commands used 120–180 second tool bounds and did not time out.
Post-mutation focused and full logs below were measured with a 150-second bound
per command; the entire sequential validation invocation also had a 180-second
tool bound.

| Target | Post default | Post all features |
|---|---:|---:|
| Analysis lib | 174 | 174 |
| Analysis integration | 10 | 10 |
| HIR lib | 110 | 110 |
| HIR integration | 16 | 16 |
| Package lib | 15 | 15 |
| Package architecture | 4 | 4 |
| Doctests, all three | 0 | 0 |
| **Total passed** | **329** | **329** |

`--all-features` selects the affected HIR/package `serde` lanes; analysis declares
no own features. Scoped default/all-feature Clippy cover all targets. Formatting
used only `cargo fmt -p vela_hir -p vela_analysis -p vela_package`, then its check
form; **no global formatting**. Discovery lists match these counts and preserve
all old Rust test function identities; only the 16 named additions appear.

### Captured Post-change Commands

| Command | Exit | Wall time / bound |
|---|---:|---:|
| `cargo test -p vela_hir --lib module_graph::tests --locked` | 0 | 4.18s / 150s |
| `cargo test -p vela_analysis --lib --locked` | 0 | 5.04s / 150s |
| `cargo test -p vela_package --lib --locked` | 0 | 2.08s / 150s |
| `cargo test -p vela_hir -p vela_analysis -p vela_package --locked --no-fail-fast` | 0 | 6.58s / 150s |
| `cargo test -p vela_hir -p vela_analysis -p vela_package --all-features --locked --no-fail-fast` | 0 | 17.50s / 150s |
| `cargo clippy -p vela_hir -p vela_analysis -p vela_package --all-targets --locked -- -D warnings` | 0 | 3.05s / 150s |
| `cargo clippy -p vela_hir -p vela_analysis -p vela_package --all-targets --all-features --locked -- -D warnings` | 0 | 3.02s / 150s |
| `cargo fmt -p vela_hir -p vela_analysis -p vela_package -- --check` | 0 | 0.36s / 150s |
| `cargo test -p vela_hir -p vela_analysis -p vela_package --all-features --locked -- --list` | 0 | 0.94s / 150s |


### Deliberate Failure Proof

For each owner, the disk inventory was separately mutated by removing its error
case `.expected`, adding `orphan.expected`, removing every corpus file, and
adding an unregistered source/expected pair. Each existing compiled inventory
binary ran exactly one test and exited 101 at `complete registered source/expected
pairs`. These are runtime checks, not compilation failures or zero-test filters.

For each owner, separate Cargo-rebuilt exact-case runs then:

1. Replaced the first expected message phrase with `WRONG EXPECTATION`.
2. Appended `9` to the first expected coordinate (`game/player.vela:3:12` or
   `game.vela:9:5`), leaving actual diagnostics unchanged.
3. Added a real diagnostic: HIR `return plaeyr + absent;`; analysis an additional
   `2 === 2;` after `1 === 1;`.

The first two fail full-output equality; the third fails HIR typed diagnostic
vector equality / analysis ordered diagnostic count before golden comparison.
Each mutation used `try/finally` restoration, verifying the entire directory's
filename-to-SHA-256 map against its original bytes. Owner suites subsequently
passed. The exact commands/results follow; binary hashes are invocation-specific.

| Mutation | Command | Result | Wall time |
|---|---|---|---:|
| vela_hir-missing | `target/debug/deps/integration-852a8801ae8e2d40 --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.00s |
| vela_hir-orphan | `target/debug/deps/integration-852a8801ae8e2d40 --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.00s |
| vela_hir-empty | `target/debug/deps/integration-852a8801ae8e2d40 --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.00s |
| vela_hir-unregistered | `target/debug/deps/integration-852a8801ae8e2d40 --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.00s |
| vela_hir-wrong-message | `cargo test -p vela_hir --test integration semantic_ui::semantic_ui::case_1_unresolved_name --locked -- --exact` | 101, intended assertion | 1.85s |
| vela_hir-wrong-span | `cargo test -p vela_hir --test integration semantic_ui::semantic_ui::case_1_unresolved_name --locked -- --exact` | 101, intended assertion | 1.78s |
| vela_hir-extra-diagnostic | `cargo test -p vela_hir --test integration semantic_ui::semantic_ui::case_1_unresolved_name --locked -- --exact` | 101, intended assertion | 1.78s |
| vela_analysis-missing | `target/debug/deps/integration-712eeb1227a1a7ff --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.00s |
| vela_analysis-orphan | `target/debug/deps/integration-712eeb1227a1a7ff --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.01s |
| vela_analysis-empty | `target/debug/deps/integration-712eeb1227a1a7ff --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.00s |
| vela_analysis-unregistered | `target/debug/deps/integration-712eeb1227a1a7ff --exact semantic_ui::semantic_ui_inventory` | 101, intended assertion | 0.00s |
| vela_analysis-wrong-message | `cargo test -p vela_analysis --test integration semantic_ui::semantic_ui::case_1_identity --locked -- --exact` | 101, intended assertion | 1.78s |
| vela_analysis-wrong-span | `cargo test -p vela_analysis --test integration semantic_ui::semantic_ui::case_1_identity --locked -- --exact` | 101, intended assertion | 1.79s |
| vela_analysis-extra-diagnostic | `cargo test -p vela_analysis --test integration semantic_ui::semantic_ui::case_1_identity --locked -- --exact` | 101, intended assertion | 1.84s |

Every mutation restored in finally; complete filename/byte SHA-256 maps matched. Each child timeout 90s. No timeout occurred.


### Cargo Inputs And Isolated Copies

For each owner, `cargo package -p OWNER --list --allow-dirty --offline` passed
(30-second bounds). All 30 HIR, 49 analysis and 13 package fixture/attribute files
are included. No temporary proof files were included in these final file lists.
The metadata warning about missing description/documentation/homepage/repository
was informational.

The actual archive command (60-second tool bound):

```sh
cargo package -p vela_hir -p vela_analysis -p vela_package --allow-dirty --no-verify --offline
```

exited **101**, before packaging, because `vela_package`'s `vela_common` dependency
has no version requirement. Existing workspace dependencies are path-only; SF03
has no authority to change this publishing policy. No successful `.crate` archive
or isolated published full-suite build is claimed.

As a narrower reachability check, each owner's real Cargo file list was copied
byte-for-byte into its own temporary directory under
`crates/vela_package/tests/sf03_work/isolated/OWNER`. Only the copied manifest gained
a standalone workspace section using the existing workspace package/dependency/
lint/profile definitions, with dependency paths resolved back to the checkout.
HIR/analysis copied manifests additionally registered `tests/semantic_ui.rs` as
`sf03_semantic_ui`; **their test source was not changed**. Package ran its original
`--lib` route. This proves the new fixtures/UI inventories and fresh-copy package
route are reachable from Cargo-listed inputs, not independent publication of the
whole dependency graph. Results: **15 package + 7 HIR + 8 analysis passed**.

No shared-root fixture was copied. Existing HIR/analysis conformance includes
(and analysis completion inputs) still require repository-root test data; package
architecture tests intentionally require the repository Rust tree. Those full
integration/source-audit routes were validated in the checkout, not falsely
reported as standalone package tests. The isolated probe does not close those
pre-existing publishing limitations.

| Owner | Listed fixture/attribute files | Copied Cargo-list files | Isolated command | Result |
|---|---:|---:|---|---|
| vela_package | 13 | 19 | `cargo test --manifest-path crates/vela_package/tests/sf03_work/isolated/vela_package/Cargo.toml --target-dir target --offline --lib` | 0, 7.26s / 120s |
| vela_hir | 30 | 72 | `cargo test --manifest-path crates/vela_package/tests/sf03_work/isolated/vela_hir/Cargo.toml --target-dir target --offline --test sf03_semantic_ui` | 0, 11.17s / 120s |
| vela_analysis | 49 | 139 | `cargo test --manifest-path crates/vela_package/tests/sf03_work/isolated/vela_analysis/Cargo.toml --target-dir target --offline --test sf03_semantic_ui` | 0, 11.04s / 120s |


### Byte, Consumer And Attribute Audit

Temporary explicit-selection scripts stored the original Rust literal tokens,
then compiled assertions of the form
`assert_eq!((ORIGINAL_LITERAL).as_bytes(), include_bytes!(DESTINATION))` with
`rustc --edition=2024`. This checks Rust-evaluated cooked escapes as well as raw
bytes, not merely source spelling. Proof units `bytes.rs`, `package_bytes.rs`,
`ui_bytes.rs`, `final_bytes.rs` all compiled and ran successfully, and were
**recompiled after mutation restoration and formatting** (30-second rustc / 10-second
execution bounds). Independently, final file lengths and SHA-256 were checked
against all 79 records below. All passed. Leading/EOF newlines, indentation,
escapes and no-EOF-newline files were preserved, with no source normalization.

All 89 payload files have consumers: static includes for non-tree files, explicit
recursive copy of the two package trees. No new unused fixture was found.
`git check-attr text eol -- ...` confirmed `text: set`, `eol: lf` for representative
HIR/analysis `.vela`/`.expected` and package `.vela`/`.toml` files; owner-local
`.gitattributes` covers each family. Scope-only `git diff --check` passed.

The scripts, compiled byte-proof binaries, mutation logs and isolated copies were
one-off audit tools, not permanent test infrastructure. They were removed before
handoff; this evidence and the actual tests/fixtures remain. After removal, both
full owner commands (default and all features), scoped formatting check and scoped
`git diff --check` passed again in one 180-second-bounded invocation. A final
10-second-bounded audit reread this document and verified all 79 length/SHA-256
records against the actual fixtures; no temporary directory remains.

### Development Failures, Not Acceptance Claims

- Initial extraction script selected a Vela `fn` as a Rust function boundary;
  corrected to the next Rust test attribute before continuing.
- An attempted raw-literal edit auto-indented source and failed byte equality.
  Only that SF03-owned bindings edit was restored from the clean initial HEAD,
  then reapplied with exact bytes; final Rust-evaluated proofs pass.
- Empty golden creation required explicit zero-byte writes. No fake nonempty
  success output was inserted.
- Initial new UI compilation used renderer imports at the wrong module and an
  incorrect `ModuleSource` field name; corrected to `diagnostic_render` and `path`.
- Initial empty error goldens deliberately failed during construction. Reviewed
  output/typed contracts supplied full goldens; missing record-field diagnostics
  have one label, not two. These construction failures are **not** the deliberate
  mutation proof above. No production diagnostics were changed to match a golden.
- Cargo archive failure is recorded above, not repaired outside ownership.
- The final document-audit regex initially omitted the eleven `graph.rs` package
  rows because their origin column includes explanatory text. Parsing the table
  columns instead verified all 79 records; no fixture/evidence bytes changed.
- No SF03 validation timeout occurred; early compile-lock waits from concurrent
  work were bounded. No aggregate workspace acceptance claim is made.

## Parent Handoff

No staging/commit performed. Parent owns shared milestone/status updates and the
stage commit. Suggested shared validation entries are the actual working
`--test integration semantic_ui` filters for HIR/analysis, the full owner
commands above, and package `--lib graph::tests`. Record the corrected stdlib
fact-family unit-data disposition and publishing limitation when reconciling the
shared census/acceptance record. SF04 still owns compiler/literal-range UI-S;
SF03 does not claim that separate lane or other stages complete.

## Exact Evaluated-Byte Records

Fixture paths are relative to the original owner's `tests/fixtures`; `graph.rs`
package-setup rows belong to `vela_package`. Duplicate hash rows intentionally
show independent original consumers and the one shared destination. These are
relocation evidence, not golden generation rules.

| Original owner / function | Fixture (relative to owner tests/fixtures) | Bytes | SHA-256 |
|---|---|---:|---|
| `crates/vela_hir/src/module_graph/tests/bindings.rs::body_binding_query_follows_methods_lambdas_and_parameter_defaults` | `bindings/method_default_bindings.vela` | 256 | `75eaf0e087bef7ff4137c76802b000675caac471a30baebf429794b96d1edcc9` |
| `crates/vela_hir/src/module_graph/tests/bindings.rs::binding_resolves_if_expression_lambda_captures_at_vm_spans` | `bindings/if_capture.vela` | 228 | `94ec8e183d60bbcce93617e4841e7dea8b6791d1bb458229e1faa09d4770f3e6` |
| `crates/vela_hir/src/module_graph/tests/bindings.rs::binding_resolves_block_expression_outer_capture_at_vm_span` | `bindings/block_capture.vela` | 353 | `358ff2028a9000dc12c5313f5c42846f5f830884061860208fcf71f6e591f6d3` |
| `crates/vela_hir/src/module_graph/tests/bindings.rs::binding_resolves_calls_inside_multiline_if_condition` | `bindings/multiline_condition.vela` | 332 | `b485b8a7cf1819b7d5fbe369616fe0bb2fd75378399510d4fabe87f7e3db4ce7` |
| `crates/vela_hir/src/module_graph/tests/bindings.rs::body_hir_tracks_lexical_scopes_for_blocks_loops_and_match_arms` | `bindings/lexical_scopes.vela` | 258 | `1467752a6fa45bfd9293ad333a199b20e22e048a8985ad01386f1ab01ddf75ca` |
| `crates/vela_hir/src/module_graph/tests/bindings/executable_facts.rs::function_bodies_record_source_path_facts` | `bindings/source_paths.vela` | 315 | `ef68345dba703231bf97e95016af2c941768bbcdea55b3283f6140fda9414d5c` |
| `crates/vela_hir/src/module_graph/tests/bindings/import_resolution.rs::body_hir_owns_executable_operands_containers_and_control_flow` | `bindings/executable_operands.vela` | 358 | `040138dad5635691473b3d31cd1f9981fa2dd6728cde0a002923d4bd87df36d2` |
| `crates/vela_hir/src/module_graph/tests/bindings/import_resolution.rs::constructor_bindings_resolve_after_later_qualified_sources_are_added` | `modules/constructor_main.vela` | 427 | `4ce9c0a1b28a14f8072907f06d6ba104cf8a02b8f97bc81085c2da2d43191d72` |
| `crates/vela_hir/src/module_graph/tests/bindings/import_resolution.rs::constructor_bindings_resolve_after_later_qualified_sources_are_added` | `modules/constructor_schema.vela` | 82 | `be79447965ab285e5bd28e1581d138486a6854fd96e94514a4ca016ba225d636` |
| `crates/vela_hir/src/module_graph/tests/bindings/executable_facts.rs::function_bindings_resolve_imported_names` | `modules/imported_value.vela` | 53 | `653ffdd09744bb1c909260018af9c270cf7b0e8a441e7f25fc5e2a2b5f0f1b4e` |
| `crates/vela_hir/src/module_graph/tests/bindings/import_resolution.rs::resolved_imports_refresh_existing_binding_maps` | `modules/imported_value.vela` | 53 | `653ffdd09744bb1c909260018af9c270cf7b0e8a441e7f25fc5e2a2b5f0f1b4e` |
| `crates/vela_hir/src/module_graph/tests/metadata.rs::lowers_type_hint_metadata_for_signatures_structs_and_locals` | `metadata/type_hints.vela` | 297 | `dbad069ecf429b383167038772f08f655cf64d67da9d8fc5cc79b36ff32139e3` |
| `crates/vela_hir/src/module_graph/tests/metadata.rs::lowers_attribute_metadata_for_declarations_and_members` | `metadata/attributes.vela` | 430 | `8a0535c5b4072b6bc48d160ff51d0b62a5abd09a36e5e1a6a4c50769ab061c84` |
| `crates/vela_hir/src/module_graph/tests/metadata.rs::lowers_schema_field_default_metadata` | `metadata/field_defaults.vela` | 179 | `ce6e0d555e22ac9d537be5941540b63546ede6b331a69a9d303afccc4cb180be` |
| `crates/vela_hir/src/module_graph/tests/metadata.rs::lowers_impl_metadata_and_method_bindings` | `metadata/trait_impl.vela` | 331 | `9e1a8a174f73e05f3e7d7cbedce97fe2ffb621ea160925ce2430a5d7b8e4c5cc` |
| `crates/vela_hir/src/module_graph/tests/ingestion.rs::parsed_source_ingestion_preserves_the_complete_hir_graph_and_ids` | `ingestion/parsed_equivalence.vela` | 165 | `a5f72d164b3b62c7914b92142ef37369ecb7f1a9d67889897dc9ee4c3188b4c7` |
| `crates/vela_hir/src/script_methods/tests.rs::trait_defaults_expand_per_owner_share_body_and_yield_to_explicit_methods` | `methods/trait_defaults.vela` | 305 | `eec71592c194f3e8acb3dd48d5c77c8ea04168327b08cd26dad41ad8430b9dac` |
| `crates/vela_hir/tests/service_impl.rs::ordinary_traits_methods_and_base_services_names_remain_available` | `methods/ordinary_inventory.vela` | 368 | `172134e6a7b2c4accdfd6d1bb9d292f7966e225e979159e46982b9a5febe5423` |
| `crates/vela_hir/src/module_graph/tests/bindings.rs::binding_unresolved_names_report_candidate_hints` | `ui/semantic/unresolved_name.vela` | 40 | `0eb5323631448387a511b44d8a52e323317f6664c412864cfcc70716e27ea03b` |
| `crates/vela_hir/src/module_graph/tests/resolution.rs::exact literal` | `ui/semantic/unresolved_import.vela` | 30 | `09e641c4f21b8eba185b2a44ebc612df621832639e663e940d3fb4b9cac6117d` |
| `crates/vela_hir/src/module_graph/tests/resolution.rs::exact literal` | `ui/semantic/private_import.vela` | 24 | `58ef37c1a2b17eb8dc8c75738e76af0d243867462807b39db98654729b0812f2` |
| `crates/vela_analysis/src/executable/callback_tests.rs::callback_params_are_qualified_by_concrete_executable` | `callbacks/concrete_receivers.vela` | 222 | `c070c017fdf0daf64f28c09ef62cfb8a76f4704974bb9a2d703b1855bf18692b` |
| `crates/vela_analysis/src/executable/callback_tests.rs::source_record_and_iterator_callbacks_converge_to_stable_member_facts` | `callbacks/record_iterators.vela` | 413 | `51a32e7f3979b9018d17ecdd16db6150315c65884295855b522789d741ecee59` |
| `crates/vela_analysis/src/executable/callback_tests.rs::map_callbacks_use_value_or_key_value_facts_from_lambda_arity` | `callbacks/map_arity.vela` | 337 | `a6028ef00f555cb706e165b290913834161f3761e600ce7119cd107649f4ee1c` |
| `crates/vela_analysis/src/executable/callback_tests.rs::map_find_function_fallbacks_preserve_specialized_logical_members` | `callbacks/map_find_fallbacks.vela` | 524 | `e1ea704375ab411fd4bf25a3f5c78f97e0e2da9398b9d92440fcb9e1f5f23e4c` |
| `crates/vela_analysis/src/executable/callback_tests.rs::logical_map_entries_survive_option_iterator_array_local_and_callback_flow` | `callbacks/logical_map_flow.vela` | 420 | `8ab828b014789d49f9e5a3f3945cdc89e565a252009e710d99c1a0edf1d8c092` |
| `crates/vela_analysis/src/executable/callback_tests.rs::option_result_and_typed_registry_callbacks_seed_nested_params` | `callbacks/option_result_registry.vela` | 693 | `8bf9509900c44f28e32f7e64e43039c66f511d4caf7c2aba357a83a98a78ff3e` |
| `crates/vela_analysis/src/executable/tests.rs::shared_trait_default_facts_are_qualified_by_concrete_executable` | `facts/executable_receivers.vela` | 447 | `93a556772cdeaf93818e8c8bd40cef432986bfc04e25eb92c0198fd6ec11b381` |
| `crates/vela_analysis/src/facts/body_binding_tests.rs::analysis_ingests_impl_trait_lambda_and_default_binding_generations` | `facts/body_bindings.vela` | 307 | `8cad9d140ddff9dc19d1bf072dff2ba05d425b28f1fafed5aa14f6da0aa5c0bd` |
| `crates/vela_analysis/src/facts/implicit_receiver_tests.rs::fixture` | `facts/implicit_game.vela` | 565 | `8da4d7ad4f28524f348c3c06069b57f5ede34aa4b923b68fae123e1c79bd5321` |
| `crates/vela_analysis/src/facts/implicit_receiver_tests.rs::exact literal` | `facts/implicit_shapes.vela` | 29 | `6641288812b5c15447efb3f2385dbbc77da2b3734a3c801e4b3c225904c646ec` |
| `crates/vela_analysis/src/facts.rs::analysis_facts_unwrap_try_payloads_and_preserve_dynamic_facts` | `facts/try_payloads.vela` | 395 | `ae2740a888ebe1321bb3724f3c0f4d627f4e5611a3659333734ac91a603310d5` |
| `crates/vela_analysis/src/facts.rs::analysis_facts_evaluate_complete_hir_body_and_resolve_targets` | `facts/complete_body.vela` | 552 | `351c48b33c55dce7907aeee48a10f6cef8f5f59ed052e7607ec7352610306224` |
| `crates/vela_analysis/src/facts.rs::semantic_facts_resolve_source_fields_and_direct_lambdas_by_hir_identity` | `facts/source_lambdas.vela` | 290 | `895a49f4c7d9f8fd702a2583ede4335c6ac1eb710fef1fd48ca53d671ebc0ace` |
| `crates/vela_analysis/src/semantic_facts/tests/constructor_targets.rs::executable_constructor_targets_follow_imported_and_qualified_hir_resolutions` | `semantic_facts/constructor_main.vela` | 677 | `27134977a381864078c57efca2b5e49470e7e72ff4e45771ac9b472fc22bd6af` |
| `crates/vela_analysis/src/semantic_facts/tests/constructor_targets.rs::executable_constructor_targets_follow_imported_and_qualified_hir_resolutions` | `semantic_facts/constructor_schema.vela` | 82 | `be79447965ab285e5bd28e1581d138486a6854fd96e94514a4ca016ba225d636` |
| `crates/vela_analysis/src/semantic_facts/tests/pattern_fields.rs::executable_variant_pattern_fields_preserve_script_method_identity` | `semantic_facts/pattern_methods.vela` | 736 | `56a0d34da09f7566366eb68c810d656743f8fe8e84eefe1e5aff909d0ac15da5` |
| `crates/vela_analysis/src/semantic_facts/tests/pattern_fields.rs::exact literal` | `semantic_facts/private_pattern_api.vela` | 99 | `ea1065f0e2fe792b0cd0c31fb3086d7727b42e80b2882fe3e8212b75c87c5d62` |
| `crates/vela_analysis/src/semantic_facts/tests/pattern_fields.rs::exact literal` | `semantic_facts/private_pattern_main.vela` | 246 | `16866e96e1cc25cb963b8bb5e1a7ed6a26e26de9801e65bc4f934c2f1858f04c` |
| `crates/vela_analysis/src/hints/scope_tests.rs::exact literal` | `semantic_facts/token_api.vela` | 56 | `e04c1e12f2627497f797d07c703713855b85ed2c1f1f4c077e6350ef7cfaeca3` |
| `crates/vela_analysis/src/semantic_facts/tests/registry_constructors.rs::exact literal` | `semantic_facts/token_api.vela` | 56 | `e04c1e12f2627497f797d07c703713855b85ed2c1f1f4c077e6350ef7cfaeca3` |
| `crates/vela_analysis/src/semantic_facts/local_flow_tests.rs::local_use_facts_track_straight_line_writes_and_invalidate_control_flow_joins` | `semantic_facts/local_writes.vela` | 631 | `4b03386fe0d88b38e73e4556e0dcaa81f2a448b89a2b3e0373c2489eaa1714a3` |
| `crates/vela_analysis/src/semantic_facts/local_flow_tests.rs::erased_collection_annotations_retain_initializer_and_assignment_shapes` | `semantic_facts/erased_collections.vela` | 520 | `c2f6ece166a4a9b99543f2fb9e51edbe0c6f76b347d14391c27be3736bd92460` |
| `crates/vela_analysis/src/validation/tests.rs::identity_validation_distinguishes_static_values_from_dynamic_references` | `ui/semantic/identity.vela` | 253 | `06b3b0f287120fd6115a22125cd6e539e90ad12691dd1341029e3ef4e35fce52` |
| `crates/vela_analysis/src/validation/tests.rs::comparison_validation_uses_authoritative_derives_and_impl_metadata` | `validation/comparison_traits.vela` | 577 | `b69f5a8504266e07fa288e958c7ac26b8fe9cd9d4519e1692345780bb83d86a7` |
| `crates/vela_analysis/src/validation/tests.rs::array_ordering_validation_covers_proven_elements_and_callback_keys` | `validation/array_ordering.vela` | 453 | `c8424a9a88c51bce6b82a711ecf8e213572ca9b3f46584c2504c8db7bf5f9c0a` |
| `crates/vela_analysis/src/validation/tests.rs::loop_control_validation_stays_within_each_hir_body` | `validation/loop_control.vela` | 220 | `f90b041af72bab691863ffc5061ac493ca63aaaefc741d1d0e57087fde7ae3dc` |
| `crates/vela_analysis/src/validation/tests.rs::shared_body_operator_validation_is_qualified_by_function_id` | `validation/receiver_identity.vela` | 209 | `2b5404af0c44a799d65f0479eaca03a560317a52f288bee902cf927e497b57c2` |
| `crates/vela_analysis/src/validation/call_tests.rs::call_placement_diagnostics_freeze_codes_spans_and_labels` | `ui/semantic/call_placement.vela` | 170 | `bfc5a82f3732fcba2cc04228574717a905473297ca67dc06d8520b0df180b623` |
| `crates/vela_analysis/src/validation/call_tests.rs::external_calls_only_place_named_arguments_and_keep_parameter_spans_absent` | `ui/semantic/registered_calls.vela` | 113 | `314ead2dc2e15989e31cc6c2e1a6ab12ae99223ab86b4303b027befd779c94ed` |
| `crates/vela_analysis/src/validation/call_tests.rs::shared_body_placements_keep_source_order_separate_from_receiver_specific_slots` | `validation/receiver_slots.vela` | 328 | `dd0709de38aed567711ec0a089fff26f64991a45b76322b7bfed67f4c2b067ce` |
| `crates/vela_analysis/src/validation/call_tests.rs::trait_call_validation_preserves_parameter_slots` | `validation/trait_calls.vela` | 242 | `d0836e662d7eabbf00edea705389db90dd9769b643d4b4fee2f2c56228282bfe` |
| `crates/vela_analysis/src/validation/constructor_tests.rs::source_constructor_placements_separate_evaluation_order_from_schema_slots` | `validation/constructor_slots.vela` | 377 | `49a0d99109a759f9a042ca9003882103b1d67cf5847bcc2154f5f4fc3cd739e8` |
| `crates/vela_analysis/src/validation/constructor_tests.rs::constructor_diagnostics_precede_generic_tuple_call_diagnostics` | `ui/semantic/constructors.vela` | 219 | `980264f19a3c5668d087d550f627eb9918d0577c50983912a395b4d501e62cee` |
| `crates/vela_analysis/src/validation/constructor_tests.rs::imported_and_qualified_constructor_paths_share_declaration_identities` | `validation/imported_constructors.vela` | 235 | `7fc4fcdfc0292fa712fdac97139b8c4fc1c5ba99f2c91046acb019a0bc0ba6a8` |
| `crates/vela_analysis/src/validation/constructor_tests.rs::exact literal` | `validation/constructor_schema.vela` | 70 | `af238f09091d217360ec5971f5e3028556308626e221a64a6ecddad9a0c06023` |
| `crates/vela_analysis/src/completion/tests.rs::local_completions_include_function_scope_bindings` | `queries/local_bindings.vela` | 339 | `6d1585b6dd14440722d99cc5e931243c243241e9027b15c8827adfa4fe9ed4e5` |
| `crates/vela_analysis/src/completion/tests.rs::declaration_completions_include_script_declarations` | `queries/player.vela` | 554 | `7f2f1b32c8037ec1131c1320b5c8acac4bc42a60849166226f855e4af40cd125` |
| `crates/vela_analysis/src/completion/tests.rs::declaration_completions_include_script_declarations` | `queries/reward.vela` | 109 | `230489cc282887c7e8de1d9202f75cede27966891d0b40fc533dc109d1acf8d3` |
| `crates/vela_package/src/manifest.rs::manifest_parses_workspace_package_sources_dependencies_and_capabilities` | `manifests/complete.toml` | 272 | `e9262cb55de1b4dc487d936743c5b6eedee8b060979d7f4d1ee8fb50e8b06162` |
| `graph.rs` package setup | `packages/independent_roots/first/vela.toml` | 127 | `cce300e9a24283018c6984c8de80bb19fa892881a974c5eb5cc77d39651a2e89` |
| `graph.rs` package setup | `packages/independent_roots/first/src/shared.vela` | 19 | `a8d09540e3da13aa2607ee4cd7981b4ce4d5a3a9b76877ddad4be504527bd779` |
| `graph.rs` package setup | `packages/independent_roots/second/vela.toml` | 130 | `0f1fd195087fff3776b23f50329e1311e77b2fe3248fa1f1ad936ec4f4eb2980` |
| `graph.rs` package setup | `packages/independent_roots/second/src/shared.vela` | 19 | `a8d09540e3da13aa2607ee4cd7981b4ce4d5a3a9b76877ddad4be504527bd779` |
| `graph.rs` package setup | `packages/independent_roots/common/vela.toml` | 61 | `9d54002d083f4e6a3a4be003631c4f5363f06812ac924d17d61864f4cc1a1b60` |
| `graph.rs` package setup | `packages/independent_roots/common/src/shared.vela` | 19 | `a8d09540e3da13aa2607ee4cd7981b4ce4d5a3a9b76877ddad4be504527bd779` |
| `graph.rs` package setup | `packages/path_dependency/vela.toml` | 92 | `82557d4f040fbd5371b58296bbe7c067ffb79e363654cc6a802978e7db4820f1` |
| `graph.rs` package setup | `packages/path_dependency/src/z.vela` | 10 | `ea12dd7bcd6bec92ab2798d0965a5993d5fa37129a88db080efe89702ce83756` |
| `graph.rs` package setup | `packages/path_dependency/src/a.vela` | 10 | `509a0a5b5ce4e59f5039e30a39324342d7a161296bb8eba761983faaeebf6efd` |
| `graph.rs` package setup | `packages/path_dependency/util/vela.toml` | 60 | `863b4a20fc6c85aead5cd94d70556c1496faab33e3a2cb5a3d879cbe76202473` |
| `graph.rs` package setup | `packages/path_dependency/util/src/lib.vela` | 28 | `7294f73a1b03bed0a62f05956dfa0baa218e5192288ad6aa94aba98e6637d50f` |
| `crates/vela_hir/src/module_graph/tests/resolution.rs::resolves_imports_across_modules` | `ui/semantic/valid_import.vela` | 55 | `556c3741ca4b37649160927c379044317959c08a41c8eded41e4dd48b9b865d6` |
| `crates/vela_hir/src/module_graph/tests/resolution.rs::exact literal` | `modules/public_reward.vela` | 28 | `24694d566ee2519da45cc53188c5536f61f92fc4fc936b88ca903f9a8e77fd07` |
| `crates/vela_hir/src/module_graph/tests/bindings/executable_facts.rs::exact literal` | `modules/public_reward.vela` | 28 | `24694d566ee2519da45cc53188c5536f61f92fc4fc936b88ca903f9a8e77fd07` |
| `crates/vela_hir/src/module_graph/tests/bindings/import_resolution.rs::exact literal` | `modules/public_reward.vela` | 28 | `24694d566ee2519da45cc53188c5536f61f92fc4fc936b88ca903f9a8e77fd07` |
| `crates/vela_hir/src/module_graph/tests/resolution.rs::exact literal` | `modules/private_reward.vela` | 25 | `ce830d310364d8292f2b98fe6dc0a72551e57e761587a50cf400299d998a0add` |
| `crates/vela_analysis/src/semantic_facts/tests/external_calls.rs::exact literal` | `semantic_facts/external_defs.vela` | 141 | `8e621bb95d94ba3dea2b828fb61daf3d80ddca157878ac8d89bba7d166fd3ed1` |
| `crates/vela_analysis/src/semantic_facts/tests/enum_aliases.rs::exact literal` | `semantic_facts/enum_api.vela` | 138 | `21cd16e26850f4fad05ec974e3fb67e4391c1df7cbbf78545022f8a8c389ef9f` |
| `crates/vela_analysis/src/semantic_facts/tests.rs::missing_method_targets_require_a_closed_receiver_universe` | `semantic_facts/closed_receivers.vela` | 306 | `6e0f00e379e2c5f11b9f97b3178cc37982f5c2b9b2365711b454c7f3985a28c7` |


### Newly Authored Controls And Output Goldens

| File | Bytes | SHA-256 |
|---|---:|---|
| `crates/vela_analysis/tests/fixtures/ui/semantic/call_placement.expected` | 1935 | `8bb07bd34db768efb5d93b18f62c360c60318bb8def455ba9f78fef76dcf35ea` |
| `crates/vela_analysis/tests/fixtures/ui/semantic/constructors.expected` | 3439 | `9fcc8b55d734fe1387660d35fac6d2ec08836c20a4de1b12d9a2a4ced456153c` |
| `crates/vela_analysis/tests/fixtures/ui/semantic/identity.expected` | 1012 | `3a3f615e21512e7e981263412e4f636b400b3dc99afdc83c07f924b4d374ce74` |
| `crates/vela_analysis/tests/fixtures/ui/semantic/registered_calls.expected` | 323 | `62f26a4390dc14613aa3586860f1e78d2a679b3ea0e315c18e99df5071387a75` |
| `crates/vela_analysis/tests/fixtures/ui/semantic/valid_registry.expected` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `crates/vela_analysis/tests/fixtures/ui/semantic/valid_registry.vela` | 42 | `679321681edaf46620b6494d26aa8db908d37cf2867d05d6d2f25cfb974ea603` |
| `crates/vela_analysis/tests/fixtures/ui/semantic/valid_source.expected` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `crates/vela_analysis/tests/fixtures/ui/semantic/valid_source.vela` | 83 | `4cbdcbff23c0ed7540c49faee67732675b806860146ebdb1bad52e4d85d3a8df` |
| `crates/vela_hir/tests/fixtures/ui/semantic/private_import.expected` | 506 | `54f6e7c642ddc5ade329761ec8777a064c0c54ab5e1849ae911c4b6abafd1866` |
| `crates/vela_hir/tests/fixtures/ui/semantic/unresolved_import.expected` | 378 | `5c746331c44af8778d0dfdca852849a17758e38e6d59681e39a870e8817b619d` |
| `crates/vela_hir/tests/fixtures/ui/semantic/unresolved_name.expected` | 381 | `9dafd7907a9e3a68ff0628e701365320c062f8509b9238b70167f5951e356f24` |
| `crates/vela_hir/tests/fixtures/ui/semantic/valid_import.expected` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `crates/vela_hir/tests/fixtures/ui/semantic/valid_name.expected` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `crates/vela_hir/tests/fixtures/ui/semantic/valid_name.vela` | 34 | `19f34647241de03bdcb406745f15a60bd6a33ebbb79d537e7c0dfff75f8c23e8` |
