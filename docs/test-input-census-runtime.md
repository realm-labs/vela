# SF00: Runtime And Compiler Test-Input Census

## Scope And Evidence

This is an SF00 review, not an extraction implementation or an acceptance report.
It covers `vela_common`, `vela_def`, `vela_registry`, `vela_package`,
`vela_syntax`, `vela_hir`, `vela_analysis`, `vela_mir`, `vela_bytecode`, `vela_vm`,
`vela_engine`, `vela_hot_reload`, `vela_host`, `vela_reflect`, `vela_stdlib`, and
`vela_stdlib_runtime`, including their benchmark inputs. Only this document is
written by this task; test/code changes and commits belong to subsequent work.

Authority: [test-organization-plan.md](test-organization-plan.md),
[goal.md](goal.md), [architecture.md](architecture.md),
[runtime-simplification-plan.md](runtime-simplification-plan.md), and
[progress.md](progress.md). The supplied task authorizes this census within the
staged SF00–SF12 execution; it does not authorize implementing later stages here.
The inspected HEAD was `044e5afc7`. Existing planning-document edits were present
and are not part of this census change. The `docs` parent was verified before
creating this file.

### Method And Honest Counts

- Enumerated tracked Rust paths and inspected each owner's Cargo target wiring,
  test/module locations, source-bearing families, and compile/parse/graph helpers.
  Followed `include_str!`, `#[path]`, `ModuleSource`, source-ingestion, compile/link,
  formatting/rendering, filesystem-write, and benchmark consumers. Also searched
  `format!`, `replace`, `concat!`, `push_str`, constants and helper return values.
- Read representative actual assertions and lifecycle bodies, not just names.
  Particularly inspected Engine strings, formatter pairs, retained closures,
  state reclamation, re-entry GC/unwind, VM budgets/scalar plans, compiler
  diagnostics, package materialization, and zero-signal owners' typed tests.
- The table below reproduces the plan's exact lexical signal against the tracked
  files during inspection. **819 Rust files, 270 signal files, 16 owners**. A
  signal file is not a source document, test, extraction candidate or reviewed
  assertion count. Mixed production/test files and Rust audit needles inflate it;
  cooked literals, helper-built inputs and type fragments can escape it.
- A separate lexical raw-string payload comparison identified concrete repeated
  byte sequences used below. This was not Rust macro expansion or evaluation of
  cooked/format strings. It also found MIR expected dumps and Rhai sources, which
  must not be mislabeled Vela programs. Listed duplicate sizes are raw UTF-8
  payload bytes, including whitespace; equality is not proof of equal setup.
- There were 22 root conformance `.vela` files, no owner-local `.vela` files under
  these 16 crate trees, and no owner `build.rs` in the inspected tree. Root shared
  fixtures therefore remain important; a zero owner-local count is not zero
  file-backed coverage.
- No Cargo test, test discovery, build, benchmark, formatter or Clippy run was
  performed. Baseline evidence here means inspected test bodies, not a fresh
  pass. No exact total of evaluated source documents or expanded tests is claimed.

| Owner | Tracked Rust files | Lexical signal files | Owning stage |
|---|---:|---:|---|
| `vela_common` | 5 | 1 | SF02 |
| `vela_def` | 3 | 0 | SF03 audit |
| `vela_registry` | 4 | 0 | SF03 audit |
| `vela_package` | 5 | 3 | SF03 |
| `vela_syntax` | 49 | 24 | SF01/SF02 |
| `vela_hir` | 40 | 11 | SF03 |
| `vela_analysis` | 88 | 29 | SF03 |
| `vela_mir` | 89 | 28 | SF04 |
| `vela_bytecode` | 93 | 40 | SF04 |
| `vela_vm` | 227 | 57 | SF05; benchmarks SF11 |
| `vela_engine` | 138 | 63 | SF01/SF06; benchmarks SF11 |
| `vela_hot_reload` | 28 | 11 | SF06 |
| `vela_host` | 5 | 0 | SF05 audit |
| `vela_reflect` | 38 | 3 | SF05 |
| `vela_stdlib` | 6 | 0 | SF05 audit |
| `vela_stdlib_runtime` | 1 | 0 | SF05 audit |

### Reading The Dispositions

`file-backed` below means selected for later extraction unless explicitly marked
existing. `shared-input` selects one authored origin but **not** one combined
runner. `retain-inline`, `unit-data`, `generated`, `specialized-corpus` and
`not-test-input` have the meanings in the plan. These are family decisions, not
blanket crate exemptions. Proposed paths do not yet exist. Within each owner
section, paths are relative to `crates/<owner>/` unless a repository-root path is
spelled out. Braces and wildcards describe explicit file groups, not discovery
rules to implement.

For all selected moves: preserve evaluated bytes first, including leading/EOF
newlines, cooked escapes and indentation. Keep logical names, SourceIds,
PackageIds, module ordering, entry names and span selectors in Rust. Do not use
fixture filenames as compiler identities. No new framework, catalog, shared
mutable Engine, universal boolean oracle, or production dependency on fixtures.
Keep malformed input and dependency/version files outside standalone-case globs.

## Targets And Helper Boundaries

| Owner/target | Existing boundary and consumers | Helper contract to preserve |
|---|---|---|
| Common/def/registry `--lib` | Renderer, identity and definition-unit tests in `src` | Construct fresh diagnostic/registry data. No Engine/VM route is needed to test IDs or hint fragments. |
| Package `--lib`; `--test architecture` | Manifest/graph IO versus repository source-architecture audit | `src/graph.rs::tests::{fixture,write}` create writable temporary trees; paths/order/errors are observations. `tests/architecture.rs` reads Rust/Cargo source, not Vela fixtures. |
| Syntax `--lib`; `--test diagnostic_fixtures` | AST/CST/token/formatter internals versus public parse/render fixture | `parse_source_with_id`, AST casts and `syntax_tokens` retain diagnostics and lossless text. Diagnostic helper alone normalizes CRLF and trims output ends; do not spread this policy to lossless/formatter cases. |
| HIR `--lib`; `--test integration` | Graph internals versus `conformance`, `service_impl`, `task_spawn` modules | `src/module_graph/tests.rs::{source,package_source}` constructs explicit identities. `build_single_source` differs from `build_module_source_set`; syntax errors aggregate before semantics. |
| Analysis `--lib`; `--test integration` | Fact/validation internals versus completion/conformance modules | Local `graph`/generation helpers select a body, registry facts and FunctionId, often with exact-expression span selectors. Fresh graph/schema per invocation; deliberately unresolved schema cases must not gain a blanket empty-diagnostics assertion. |
| MIR `--lib` | Builder, target-snapshot and verifier internals | `src/tests/builder.rs::try_build_with_parameters_and_contexts` owns graph, literal contexts, FunctionId 700, snapshot, debug-local/liveness settings. Other builder helpers configure different targets/guards/receivers/default roots; they are not interchangeable compiler wrappers. Preserve `test-support`/`serde` gates. |
| Bytecode `--lib`; `--test instruction_layout` | Compiler/semantic-input/linker/codec internals versus public instruction layout | `src/compiler/test_support.rs` keeps syntax, semantic and compiler failure phases in `TestCompileError`; selects functions from the same built source set. `semantic_input/tests.rs::prepare_source*` owns target preparation rather than execution. Codec stays under `artifact-codec`; instruction-size tests remain Rust. |
| VM `--lib`; `--test integration`; `--test conformance`; `--test baseline_workload_contract` | VM internals; diagnostic/external-comparison/cache-delta modules; conformance; bench inventory | `src/test_support.rs` is included by unit/integration/bench consumers. It builds fresh HIR and preserves frontend diagnostics in `InvalidHirGraph`, then compiles. It must never turn a compiler failure into a successful VM error case. `src/tests.rs` has separate owned-result, caller-supplied heap/budget, host, cache and profiler routes. |
| Engine `--lib`; `--test integration` | Private registration/cache/state/source-loader tests versus public args/reload/prelude/removal/reflection/portable modules | Keep source/file/directory/changed-file/package routes separate. Native test helpers link against that Engine and install its VM natives. Do not substitute the bare VM compiler or bypass registry checks. Preserve `serde`, `artifact-codec`, `schema-artifact` boundaries. |
| Hot reload `--lib` | ABI comparison, ProgramVersion and staged reports, not production source compilation | `src/tests/test_compile_support.rs` builds fresh sources/registry (including its `Player` definition), compiles and links, then calls linked-artifact APIs. Frontend failures panic in this helper; the returned reload error is ABI/policy failure, unlike Engine source errors. |
| Host `--lib`; `--test unsafe_boundaries` | Owned carrier round-trip versus workspace Rust AST audit | Audit traverses source trees with `syn`; do not replace it with representative Vela programs. |
| Reflect `--lib`; `--test value_access` | HIR-to-metadata and typed reflection/policy versus public owned-copy and removal surface | Fresh `ModuleGraph`/`TypeRegistry` or typed record setup. No VM needed for metadata-only tests; VM/Engine reflection natives keep their own execution proof. |
| Stdlib and stdlib_runtime `--lib` | Definitions/IDs/registration and manifest-to-runtime mapping | Iterate authoritative manifests; retain exhaustive length/uniqueness/mapping checks rather than replace them with a few executable examples. |

HIR, analysis, VM and Engine have explicit integration aggregators with
`autotests = false`. Preserve the current module filters; a module filename is
not necessarily a `--test` target. No target relocation is selected by this
census. Benchmarks retain their `harness = false` boundaries and allocator/process
ownership. Later glob adoption must demonstrate add/delete invalidation without a
clean build; absence of an existing build script is not proof invalidation works.

## SF01 Decisions

| Current location/case | Decision and proposed origin | Proof that stays at the current boundary |
|---|---|---|
| `vela_engine/src/tests/strings.rs::executes_string_literals`, all three named rows | `multiline`: **retain-inline**, because the cooked Rust `\n` and triple-quoted Vela string are the point. `escaped_braces`: **retain-inline**, one local interpolation/brace example. `interpolated`: **retain-inline**, the single short function and expected `reward gold: 7` are clearer together; it is not a substantial/reused program merely because it occupies several lines. | Keep all three named rows and exact `OwnedValue::String`; fresh Engine, SourceId 1, compile/link/run `main`. No forced three-file quota. |
| `vela_syntax/src/formatting.rs::formatting_formats_declarations_and_type_hints`, `item_declarations` row | **file-backed** pair: proposed `crates/vela_syntax/tests/fixtures/formatting/item_declarations.input.vela` and `.expected.vela`. The dense five-declaration input and long layout expectation benefit from source editing. Other short hint rows remain inline. | Empty diagnostics plus exact output; preserve the cooked continuation's lack of an initial newline. Keep this named case unless a genuinely homogeneous discovered family is implemented. |
| `vela_engine/tests/hot_reload.rs::retained_closure_pins_old_generation_across_handle_layout_reload` | **file-backed** pair: proposed `crates/vela_engine/tests/fixtures/reload/retained_closure_layout/v1.vela`, `v2.vela`. Multi-function versions change helper loop, handle ordering and closure behavior; version files expose that relationship. | Preserve selected scalar loops in both images, generation inequality, retained/new closure results 17/131 and nested old-call execution-unit failure at limit 1. Do not flatten into two output rows. |
| `vela_engine/tests/hot_reload.rs::retained_old_closure_reads_state_removed_from_active_generation` | **retain-inline** adjacent tiny versions. | Removed-state report, active-state absence and old closure reading 5 remain distinct assertions. Do not merge with private state-reclamation tests. |

This supplies useful extraction and justified inline retention without inventing
large string fixtures. SF01 discovery proof should attach to a suitable formatter
or parser corpus, not to reload versions or three dissimilar string examples.

## Common, Identity And Registry Families

| Owner/current files | Disposition, route and rationale | Required retained observations |
|---|---|---|
| `vela_common/src/diagnostic_render.rs` tests | **retain-inline**: three short source documents and compact renderer expectations; the source offsets and gutter alignments are immediately visible. `renders_missing_sources_as_stable_offsets` has no source document. No extraction quota here. | Exact rendered text, primary/related labels, multiline spans and gutter position. These are manufactured diagnostics, not parser/host-write acceptance tests. |
| `vela_common/src/{lib,capability,interop_type,primitive}.rs` | **unit-data**: IDs, spans, capability bits, primitive metadata and scalar tags. Production messages are **not-test-input**. | Structured diagnostic fields, type/identity conversions and exhaustive primitive/capability properties. |
| `vela_def/src/{lib,script,stable}.rs` | **unit-data**: canonical path/hash examples, explicit IDs and method identity helpers. Followed `script_*_id` helpers: inputs are symbol/package strings, not source documents. | Stable BLAKE3/FNV expectations, distinct packages, root symbols, explicit rename stability and declaration-order independence. |
| `vela_registry/src/defs.rs` hint tests; `src/lib.rs` registry tests | **unit-data**: `TypeHintDef::parse` fragments, including generated `format!("{name}<i64>")` removed-hint rows; typed `Def`/access/signature construction and compile-view lookup. | Unit/tuple/Option/Result shapes, one-element-tuple rejection, removed borrowed-hint rejection, ownership/declaration order/access and duplicate paths. No full-program source consumer was found in these tests. |

## Package Families — SF03

| Current file/group | Disposition and proposed destination | Consumer/helper and proof |
|---|---|---|
| `vela_package/src/manifest.rs::tests` | **file-backed** complete workspace/package/source/dependency/capability/host manifest to proposed `tests/fixtures/manifests/complete.toml`; **retain-inline** malformed table, missing-field, unknown-key and capability-only fragments. | `parse_manifest` consumes TOML, not `.vela`; keep `ManifestFileId`, spans, capability bits and missing-field diagnostics in Rust. |
| `vela_package/src/graph.rs`: independent roots/shared dependency and relative path-dependency cases | **file-backed** source trees to proposed `tests/fixtures/packages/{independent_roots,path_dependency}/`, including manifests and tiny modules. Tiny modules are useful here as a tree, not globally shared functions. | `fixture`/`write` materialize independent writable copies; preserve duplicate root order, Unicode temp naming, package/source/manifest counts, deterministic discovery, relative dependency paths and dependency host-schema rejection. |
| Same file: workspace overlap, path escape, nested roots, duplicate IDs, cycle and dependency-host rejection | **retain-inline** compact topology/error probes; repeated two-member construction remains explicit. Do not externalize the outside-root path relation. | Preserve authorized roots, canonical paths, IO-error owner, module/text order, cycle edge chain, ignored non-source file and cleanup. These are not ordinary executable fixtures. |
| `vela_package/src/identity.rs`; `tests/architecture.rs` | **unit-data** identity/version examples; **not-test-input** Rust/Cargo audit needles, while retaining the audit as a test. | Loader/compiler boundary and removed Provider/export checks remain at the repository-source boundary. |

Engine's package execution trees are separately owned below. Sharing a manifest
because it has the same boilerplate would hide different root requests,
registrations and capabilities; no cross-owner package helper consolidation is
selected.

## Syntax Families — SF02

| Current file/group | Disposition and concrete next step | Proof/helper boundary |
|---|---|---|
| `src/parse/tests.rs`: structured signature/schema/trait/block/postfix/container/match/loop examples; `src/ast/expr_tests.rs`, `items_tests.rs`, `syntax/statement_tests.rs` | **file-backed** sizeable grammar demonstrations under proposed `tests/fixtures/{parser,ast}/`, starting with `ast_expr_variants`, `ast_update_operators`, `ast_path_delimiters`, parser trait/impl methods and match-pattern examples. Keep individual node assertions in their original modules; do not share near-matches without byte comparison. | Lossless source reconstruction, item/node order, token ranges, operator kinds, AST field accessors and span slicing. Parser and AST proof are not equivalent merely because both parse. |
| `src/parse/tests.rs` removed async/await, extern/borrowed/release, legacy declaration, generic-hint/null and malformed-fragment rows; `src/parse/tests/item_boundaries.rs` | **retain-inline** narrow rejection/control rows; **file-backed** a selected UI-P corpus under proposed `tests/fixtures/ui/parser/` for missing name, recovered declaration and removed async syntax. Reuse existing root generic-hint fixture rather than copy it. | UI-P adds independently reviewed complete output, count/order/code/severity/spans/labels and valid controls. Existing typed/recovery/lossless assertions stay; parser success cannot satisfy an error case. |
| `src/formatting.rs` declaration pair and `formatting_formats_container_type_hint_example` | **file-backed** input/expected pairs under proposed `tests/fixtures/formatting/`; **retain-inline** simple arithmetic, use newline, else attachment, shebang and comment/trivia microcases. | Keep exact text and existing second-format equality. The container example calls undefined `score`; it is a formatter case, **not** an executable-equivalence candidate as written. Select a separately valid small program (e.g. existing arithmetic/branch case) for reparse/idempotence and Engine-owned before/after execution. Do not repair the formatter example during relocation. |
| `src/cst_parser/{bracket,paren,pattern,expression,declaration,method,map}_recovery_tests.rs`, `parameter_default_tests.rs`; `src/cst_parser/cst_expr/records.rs` tests | **generated** prefix/context/repaired/LF–CRLF cross-products; **retain-inline** fixed short malformed/default fragments. | Preserve Unicode prefixes, exact recovery positions, retained sibling declarations and source reconstruction. The `.replace('\n', newline)` construction is intentional EOL coverage, not a hidden static fixture. |
| `src/ast/{call_recovery_tests,expr_lambda_tests,expr_path_tests}.rs`; loop iterable table in `syntax/statement_tests.rs`; `src/syntax_validation/recovery/{tests,required_child_tests}.rs` | **generated** bounded operator/context/newline/required-child matrices; **retain-inline** local valid/repaired controls. | Keep ranges and missing-child distinction; do not turn each combination into an opaque file or normalize EOLs. |
| `src/ast/{attributes,patterns,syntax}.rs`, `src/ast/expr/literals.rs`, `src/{token,syntax_kind,syntax_node}.rs` tests | **unit-data** token/literal/attribute/kind fragments; **retain-inline** short parser wrappers. Extract only larger pattern/attribute document examples into the AST family above when their assertions span a whole document. | Literal decoded bytes, escape/radix spelling, syntax-kind classification and AST token ownership. `cst_parser`, `parse`, `syntax_validation` production strings are **not-test-input**. |
| `tests/diagnostic_fixtures.rs` | Existing **file-backed** `tests/fixtures/diagnostics/generic_type_hint.{vela,expected}` at repository root. Extend owner runner for UI-P, no shadow corpus copy. | One diagnostic and complete rendered output currently; local CRLF/`trim_end()` policy is explicit. Add nonempty/pair/orphan checks and discovery proof at implementation, not assumed present. |

## HIR Families — SF03

| Current file/group | Disposition and origin | Observations/consumers |
|---|---|---|
| `src/module_graph/tests/{bindings,metadata}.rs` and `bindings/executable_facts.rs` | **file-backed** substantial multi-declaration bodies/defaults/attributes/closure binding examples under proposed `tests/fixtures/{bindings,metadata}/`. **retain-inline** single resolution/shadow/capture/token cases. | Graph declarations, body ancestry, local/capture identity, path facts, hashes and exact source spans remain typed Rust checks. |
| `src/module_graph/tests/bindings/import_resolution.rs`, `resolution.rs` | **file-backed** multi-module alias/constructor/visibility trees under proposed `tests/fixtures/modules/`; **retain-inline** small insertion-order/package-identity rows. | `source`/`package_source` retain IDs and order; dependencies are not discovered standalone cases. Start UI-S with unresolved import/private import/name resolution and a valid graph control, at HIR rather than Engine. |
| `src/module_graph/tests/ingestion.rs` | **file-backed** `parsed_source_ingestion_preserves_the_complete_hir_graph_and_ids` source under proposed `tests/fixtures/ingestion/parsed_equivalence.vela`; **retain-inline** two tiny qualified-name documents. | Both add-source and add-parsed-source graphs remain fresh; assert graph equality, module/declaration/body IDs, not only diagnostic success. |
| `src/source_ingestion.rs`, `src/module_graph/tests/signature_defaults.rs`, `src/script_methods/tests.rs`, `src/attributes.rs` | **retain-inline** short source-set mode, source ordering, signature/default ownership and root-vs-module cases; **unit-data** attribute displays. Larger multi-method catalogs can use proposed `tests/fixtures/methods/`; do not merge source-root and module-root identities. | Syntax versus semantic error kind, source order, reserved package and required/default body semantics. Source-method catalog has a byte-identical small cross-owner example noted below, but a tiny global fixture is not required. |
| `tests/{conformance,service_impl,task_spawn}.rs`; root `tests/fixtures/conformance/source_graph.rs` | Existing **shared-input** core/reward graph; **generated/retain-inline** removed-attribute/path/async tables; **file-backed** the retained ordinary `Inventory` trait/impl success program in `service_impl.rs`. | Preserve rejection at source ingestion and ordinary-trait positive control. `conformance_graph()` creates fresh graph, IDs 1/2 and `conformance::core`/`conformance::reward`, then resolves imports; reward is a dependency, not an executable test. |

## Analysis Families — SF03

| Current file/group | Disposition and concrete candidates | Required boundary |
|---|---|---|
| `src/facts.rs`, `facts/{body_binding_tests,implicit_receiver_tests}.rs`; `src/executable/{tests,callback_tests}.rs` | **file-backed** complete callback/receiver programs under proposed `tests/fixtures/{facts,callbacks}/`; specifically `implicit_receiver_tests::fixture` game/shapes documents and `option_result_and_typed_registry_callbacks_seed_nested_params`. **retain-inline** tiny binding/flow probes. | Fresh graph plus optional schema, body/FunctionId ownership and ordered lambda facts. The receiver fixture deliberately has exactly one unresolved `host::Row` HIR diagnostic before schema-backed analysis; a generic clean-graph helper would destroy its proof. |
| `src/semantic_facts/tests/{constructor_targets,pattern_fields}.rs`, `local_flow_tests.rs` | **file-backed** larger alias/visibility/trait-pattern and branch-flow documents under proposed `tests/fixtures/semantic_facts/`. | Exact source versus registry owner, expression spans, member targets, narrowing and unavailable-owner negatives. No execution required. |
| `src/semantic_facts/tests/{enum_aliases,external_calls,host_variant_fields,local_exits,loop_flow,operator_targets,registry_constructors,value_flow}.rs`, `semantic_facts/tests.rs`, `local_flow/loops/tests.rs` | **retain-inline/generated** finite imports/callee/hint/exit/expression matrices. Extract sizeable fixed setup documents, not the small varying wrapper; proposed sibling `semantic_facts/` source trees for multi-module setups. | `format!` inputs are real source consumers even without raw literals. Preserve decoy schemas, exact package/module selection, terminated paths, diagnostic ownership and expression lookup uniqueness. |
| `src/validation/{tests,call_tests,constructor_tests,task_tests}.rs`, `src/contracts/tests.rs` | **file-backed** multi-call/constructor/comparison programs under proposed `tests/fixtures/validation/`; **retain-inline** individual invalid calls/removed-task fragments. UI-S candidates: invalid static identity, named argument placement, required constructor fields, with independent output under proposed `tests/fixtures/ui/semantic/`. | Keep placement source-order versus slot-order arrays, static/dynamic/unsupported distinctions, count/code/primary span/related labels and positive controls. Generation helpers select actual body and schema; do not route all failures through bytecode. |
| `src/hints.rs`, `hints/{registry_tests,scope_tests}.rs`, `src/registry/{registry_tests,abi_tests}.rs` | **retain-inline/generated** annotation/import wrappers; **unit-data** typed RegistryFacts and ABI metadata. **not-test-input** Rust `include_str!` removal audits in `abi_tests.rs`. | Bare/registered views, exact type owners and ignored/unavailable decoys remain separate environments. |
| `src/stdlib/function_tests.rs`, `stdlib/tests/{collection_facts,option_result_functions,option_result_methods,reflection_facts}.rs` | **file-backed** multi-operation/callback/metadata bodies under proposed `tests/fixtures/stdlib_facts/`; short one-call rows remain **retain-inline**. | These assert inferred facts/targets, not native execution. Preserve registry-neutral standard facts and callback parameter/result facts. |
| `src/literals/tests.rs`, `src/type_fact.rs`, `src/logical_records.rs`, `stdlib/parameter_metadata.rs`, `stdlib/tests/numeric_conversions.rs` | **unit-data** literal spelling/context/range and typed contract/manifest facts; **retain-inline** short logical-record graph controls. | Exact scalar type/range errors, logical-record identity, named slots and full parameter metadata checks. Do not make one `.vela` per literal spelling. |
| `src/completion/tests.rs`, `src/hover/tests.rs`; `tests/{completion_fixtures,conformance}.rs` | **retain-inline** local query fragments; **file-backed** substantial schema-plus-function query documents under proposed `tests/fixtures/queries/`. Existing completion root source/expected and shared conformance graph remain canonical. | Completion ordering/type details and hover facts are not parser UI goldens. Preserve marker/query offsets, local normalization and graph schema setup. |

## MIR And Bytecode Families — SF04

| Owner/current file group | Disposition and concrete next step | Proof and helper limits |
|---|---|---|
| MIR `src/tests/{builder,control_flow_builder,loops_builder,aggregates_builder,assignments_builder,tuple_assignments_builder,patterns_builder,try_builder}.rs` | **file-backed** substantial nested-pattern, aliasing assignment, tuple writeback and nested-try programs under proposed `tests/fixtures/builder/`. Long exact `program.dump()` strings are **file-backed expected output**, e.g. scalar operators and Option-try dumps, not Vela source. **retain-inline/generated** short branch/loop/operator/expression tables. | Preserve exact dump and typed operation/order/origin/guard/safepoint assertions. Literal-context and target insertion are test setup, not text to infer from the fixture. |
| MIR `src/builder/tests/{calls,closures_defaults,constructors,declarations,guards,reflection}.rs`; `src/verifier/tests/builder_sweep.rs` | **file-backed** multi-function defaults/closures, effectful argument/constructor and reflection placement inputs under proposed `tests/fixtures/{calls,defaults,reflection}/`. Keep tiny invalid-target sources inline. | Helpers manually close snapshots with signature/receiver/constructor/reflection descriptors; verifier sweep uses its own configured build. Preserve evaluation before slot projection, default roots and termination behavior. Do not replace with the full production compiler. |
| MIR `src/tests/{aggregate_patterns,call_guards,control,dynamic_targets,evaluation_order,host_builder,identity,lambda_targets,model,reservations,signature_defaults,snapshot_validation,targets}.rs`, `src/input/try_targets.rs`, `src/verifier/tests.rs` and `{contracts,liveness,try_regions}.rs`, `src/effects.rs` | **unit-data** hand-built MIR, identities, reservations, snapshots, origins, malformed edges and liveness; **retain-inline** small HIR/source controls such as required signature defaults; **generated** builder/verifier finite source variants. | Preserve intentional inconsistent inputs and verifier failure kinds. A source fixture cannot represent malformed MIR. Production diagnostic strings in builder/input/verifier modules are **not-test-input**. |
| Bytecode `src/compiler/tests/{expressions,closures_and_bindings,script_methods,state,module_resolution,type_contract_constructors,contract_delegation,value_method_shapes}.rs`, `expressions/assignments_and_access.rs` | **file-backed** multi-function/shape/control-flow/default programs under proposed `tests/fixtures/compiler/`; **shared-input** the concrete duplicate groups below and existing root conformance files. Short opcode/guard probes stay **retain-inline**. | Preserve register/opcode/slot/MethodId/closure/default/source-span assertions; do not replace them with runtime success. Module vectors retain package identities and source insertion order. |
| Bytecode `src/compiler/tests/literals_and_calls.rs`, `literals_and_calls/collection_methods.rs`, `loops_and_errors.rs`, `receiver_lowering.rs` | **file-backed** sizeable reflection collection chain, callback and scalar-loop programs; **retain-inline/unit-data** scalar/radix/tag/escape/range and small single-call rows. | Registered versus registry-neutral lowering, named evaluation order, exact scalar constants, dynamic fallback and selected-plan rejection remain separate observations. |
| Bytecode `src/compiler/tests/{diagnostics,diagnostic_contracts,call_diagnostics,literal_validation,phase0_frozen_contracts,removed_type_hints,service_impl}.rs` | **file-backed** selected UI-S sources/expected output under proposed `tests/fixtures/ui/semantic/`; **retain-inline/generated** compact invalid call/hint/range/alias matrices and typed low-level diagnostic construction. | `only_syntax_diagnostic`/`only_semantic_diagnostic` assert phase and count. `source_span` requires one occurrence; occurrence-aware labels retain ordering. Preserve severity/code/message/primary/labels and empty repairs/candidates where asserted. Add reviewed goldens and inventory, not a blanket `is_err()` runner. |
| Bytecode `src/compiler/semantic_input/tests/{external_descriptors,field_assignment_contracts,lambda_targets,literal_ownership,logical_records,roots_schema,script_methods,target_placements,task_targets}.rs`, `semantic_input/try_targets.rs` | **file-backed** complex target-placement/default/schema/closure documents under proposed `tests/fixtures/semantic_input/`; **retain-inline** small closed-negative roots/ownership rows; **unit-data** descriptor corruption. | `prepare_source*` preserves built HIR, analysis, registry and exact target owners. Static literal handling is not a runtime conversion case. |
| Bytecode `src/compiler/tests.rs`, `src/compiler/tests/{requests,native_effects,block_tail_semantics}.rs`, `src/compiler/mir_backend/selection/tests.rs`; `src/linker/tests.rs`, `src/artifact/tests.rs`, `src/binding_schema/tests.rs`, `src/portable.rs` | **retain-inline** small source seeds used to mutate generations, coverage, schema or portable payloads; **file-backed** larger cache/metadata seeds; existing block-tail is **shared-input**. **generated** linker method/body wrapper. | Keep same-source-set function selection, generation sealing, native effect ceilings, selected coverage, schema fingerprint and source/linked/portable distinctions. Codec corruption/old versions stay constructed Rust. |
| Bytecode `src/{lib,linked,program_image,scalar_plan,cache_site,cache_policy,script_methods}.rs`, `package_metadata/tests.rs`, `verification/{tests,linked_tests}.rs`, `compiler/{constant_encoding,error}.rs`, `semantic_input/placements/helpers.rs`, `tests/instruction_layout.rs` | **unit-data** bytecode/layout/codec/snapshot mutations and typed errors, plus **retain-inline** any minimal compiler seed. | Missing/out-of-range handles, sizes, counts, exact source coverage and verification errors cannot be replaced by valid source files. `package_metadata` is not portable-encoded by adding fixtures. |

## VM Families — SF05

| Current file/group | Disposition and concrete candidates | Assertions/environment retained |
|---|---|---|
| `src/tests/{execution_core,control_flow,program_execution,records_enums,script_methods,iteration,modules,consts,runtime_semantics}.rs` | **file-backed** multi-function recursion/capture/trait/constructor/default/iterator documents under proposed `tests/fixtures/execution/`; module graphs under `tests/fixtures/modules/`. **shared-input** existing root and duplicate groups below. **retain-inline** single opcode/value/branch or small version controls. | Keep linked-only execution, native registration, managed-heap versus owned export, module identity, exact scalar/collection results and budget/error phase. Deep recursion sources are fixed programs; the runtime depth/size is a test parameter, not a reason to hide source. |
| `src/tests/{bytes,numeric_conversions,char,deferred_literals,scalar_numeric_ops,type_guards}.rs` | **retain-inline** numeric widening/narrowing/range rows and byte index/error/endian microcases beside exact expected tags/bits/payloads. **file-backed** `bytes_iterators_yield_u8_values` and the length/slice/get/hex program under proposed `tests/fixtures/bytes/`; substantial composite contract programs under `tests/fixtures/contracts/`. | U8/U32 versus I64, float bits, Result errors, immutable bytes and typed guard failures stay exact. Do not mark all bytes/numeric tests extracted or exempt because they use rstest. |
| `src/tests/reflection_{metadata,modules,members,permissions,values}.rs` | **file-backed** long metadata/member/module queries under proposed `tests/fixtures/reflection/`; first candidates: 4,078-byte methods/traits/variants query and 2,597-byte module/functions/exports query. **retain-inline** one-call denied/unknown/removed-metadata rows. **shared-input** repeated script trait query in `reflection_values.rs`. | Per-family compile helpers configure registry, reflection policy, native IDs and SourceIds differently. Preserve returned metadata structure, candidates, permissions, source spans and updated-copy semantics, not only a truthy result. |
| `src/{string_methods,map_methods,set_methods,math_stdlib}.rs` tests; `src/array_methods/tests/{aggregation_and_ordering,higher_order_and_mutation,lookup_and_transform}.rs`; `src/stdlib/tests/{core,option_result,option_result_chains}.rs` | **file-backed** full string/collection/callback/Option–Result chain programs under proposed `tests/fixtures/stdlib/`; **retain-inline** short error or one-call tests. Share only byte-identical programs, not similarly named owned/managed cases. | Owner-local compile/link helpers, standard natives, heap/budget route, ordering and exact results remain. For example the two set-combination programs differ: one returns `daily,quest`, the other 34; they must not collapse. |
| `src/tests/{dynamic_method_dispatch,evaluation_order,linked_context_execution,linked_bytecode_profiler,linked_execution_core}.rs` | **file-backed** complete dispatch/effect/closure programs; **retain-inline/generated** two-method adapter and synchronous callback-recursion wrappers. **unit-data** hand-built linked code. | Trace order, cache get/set counts and guards, profiler offsets, closure owner, callback stack and budget depth cleanup. `run_marked` native effects must not disappear into a generic runner. |
| `src/tests/linked_{array,map,set,string,readonly}_standard_method_cache.rs`, `linked_standard_method_cache.rs`, `option_result_standard_method_cache.rs`, `standard_{array,string,map_set,option_result,callback}_id_dispatch.rs`, `standard_borrowed_native_dispatch.rs`, `standard_id_dispatch.rs` | Primarily **unit-data** hand-built instructions, caches and method IDs; retain them in Rust. Selected actual source programs are **retain-inline** if narrowly setting up one cache site, otherwise join `tests/fixtures/dispatch/`. | Preserve cache cold/hot comparisons, heap/value routes, native accounting, allocation failures and permission checks. `standard_borrowed_native_dispatch` is internal invocation value access, not restored script borrowing. |
| `src/tests/linked_standard_method_cache_{fixtures,support}.rs`, `linked_standard_method_cache_fixtures/array.rs`; `src/tests/scalar_blocks.rs` | **unit-data**, not hidden `.vela` fixtures: linked-program/plan builders. | Explicit logical charges, malformed register/value/plan construction and source points must remain constructible. No text conversion of bytecode/physical plans. |
| `src/{heap,heap_execution,heap_graph,heap_values,container_contracts,collection_mutation,equality,frame,owned_contract,owned_value,runtime_type_guards,script_object,small_storage,value,value_key,budget,error,constant_loads,closure_calls,script_builtin_methods,try_propagation}.rs`; their test submodules, `frame/registers.rs`, `owned_export/tests.rs`, `tests/owned_boundary.rs`, `serde_tests.rs`, `option_result_methods/access.rs`, `ordered_keyed.rs` | **unit-data/generated** runtime values, rooted heaps, graphs/cycles, finite growth, memory charges and serde carriers. **retain-inline** minimal source seeds where present. No Vela fixture replacement for these structures. | GC reachability, mutation rollback, identity/key semantics, contract error payload, runtime-affinity, deep export limits and feature/platform gates. Source-like Rust diagnostic strings are not migration inputs. |
| `tests/conformance.rs`, `tests/diagnostic_fixtures.rs`, `src/tests/block_tail_semantics.rs` | Existing **shared-input/file-backed** root conformance and division-by-zero/reflection goldens. | Keep positive/compile-negative/runtime-negative separation, registered standard setup, source maps and complete stack/candidate rendering. These do not satisfy parser/semantic UI delivery on their own. |


## Engine And Hot Reload Families — SF06

| Owner/current file/group | Disposition and concrete next step | Boundary/proof |
|---|---|---|
| Engine `src/tests/reflection/metadata.rs` | **file-backed** first-priority `engine_standard_natives_register_reflection_metadata` program (the large source begins near line 621), proposed `tests/fixtures/reflection/standard_metadata.vela`; also tuple descriptor query. | The large raw payload is 20,677 bytes. Keep registration metadata checks before compilation and the existing runtime oracle; extraction is not permission to rewrite its boolean contract or weaken independent Rust metadata assertions. |
| Engine `src/tests/reflection/{reflect_calls,reflection_natives,permissions}.rs`, `tests/{script_reflection,reflection_configuration}.rs` | **file-backed** substantial script metadata/module trees; **retain-inline** local permission/configuration probes; **unit-data** typed policy setup. | Native host execution context, script module export metadata, absent/present registrations and permissions. Filesystem module tests copy to fresh temp roots; keep public integration route. |
| Engine `src/tests/native/{compiler_options,budgets_and_permissions}.rs`, `tests/{context,typed_native,io,random,type_binding,script_identity}.rs` under `src` | **file-backed** long combined string/array/native/default/random/IO programs under proposed `tests/fixtures/{native,io,random}/`; **shared-input** existing named-argument inputs. **retain-inline** one-call native registration, call limits, simple type/identity/permission controls. | Native signatures, tags, effects, deterministic clock/random services, IO host roots and method-ID lowering stay in Rust. Do not share mutable host state or replace Engine linking with a bare compiler. |
| Engine `src/tests/runtime_args.rs`, `runtime_args/{managed_values,state_reclamation}.rs` | **file-backed** nested state records, recursive container contracts and retained-value programs under proposed `tests/fixtures/state/`; **retain-inline** tiny state versions, runtime-affinity/argument error probes and reclamation examples. | State validation before replacement, Rust updates, retained values through collection, explicit drop/safe-point sequence, serde-gated decode and exact cross-Runtime rejection. |
| Engine `src/runtime/{tests,reentry_tests,control_tests,bytecode_profile_tests,inline_cache_core_tests,inline_cache_native_tests,inline_cache_method_tests,inline_cache_hot_reload_tests}.rs` | **file-backed** re-entry GC programs, multi-function initialization and sizeable cache/reload generations under proposed `tests/fixtures/{reentry,initialization,reload,cache}/`. **shared-input** repeated Counter method program. **retain-inline** cancellation/error smoke and small old/new increments. **generated** profile helper-count scale. | Fresh Runtime state, explicit shared-image tests, rejected-owner pointers, cold/hot instrumentation, error unwind/reuse and initializer transaction limits remain. State/plan sharing tests must not become generic output-only cases. |
| Engine `src/runtime/{image,state}.rs`, `src/source.rs`, `src/engine/metadata.rs` | **unit-data** image/state internals; **retain-inline** tiny compile/file-loader controls; **generated** repeated state/entry construction in `state.rs`. | These mixed files contain tests missed by an inline raw-string-only census. Preserve source/manifest-loader ownership and retained generation bookkeeping. |
| Engine `src/tests/source_reload/{dir_basic,dir_function_abi,dir_schema_trait_abi,changed_file_functions,changed_file_native_method,changed_file_schema_trait,source_file_runtime,source_file_native_method,source_file_schema_trait,source_file_tuple_abi,runtime_safe_points,runtime_rejection_policy,source_diagnostics}.rs` | **file-backed** substantial schema/trait/event/initializer scenarios as proposed `tests/fixtures/reload/<scenario>/{v1,v2,rejected}.vela`, and module trees when a directory route is tested. **retain-inline** tiny body-change/signature/rejection pairs and path/IO probes. See helper-built inputs below before extracting only test-local literals. | Source/file/directory/changed-file are distinct routes. Preserve stage-before-safe-point observations, active return/state before and after rejection, structured reports, source diagnostics versus deferred ABI outcomes, removed module/function/trait/native behavior and compatible state initialization. |
| Engine `src/source/package/tests.rs`, `tests/native_capabilities.rs` beneath it | **file-backed** complete ordinary dependency and native-capability reload package trees under proposed `tests/fixtures/packages/`; **retain-inline** minimal manifest/path/root-request negatives and small value-only closure factory. | Preserve import visibility, dependency body update, changed/impacted packages, rejected ABI/image identity, package budget, host-read/write declarations and actual observed effect changes. `write_package` generates TOML and renames app `api.vela` to `main.vela`; that is semantic topology, not incidental setup. |
| Engine `tests/{hot_reload,reload_base,prelude,portable_artifact,owned_export,args,removed_borrowing,removed_declarations,removed_execution}.rs` | **file-backed** sizeable closure/layout/native generations (SF01 choice above); **retain-inline** short stale/replay/removal/prelude/export/error probes; **unit-data** owned conversion tables. Existing portable smoke stays small inline. | Public API boundaries, source-vs-portable loading, old generation IDs, exact tags, cyclic-export rejection/reuse and no Rust write-back stay independent of private tests. |
| Engine `src/{binding,compiler_registry,native,registration,type_registration}.rs`, `interop/tests.rs`, `standard/{bindings,functions,types}.rs`, `src/tests/validation/{host_method_validation,module_type_validation,native_function_validation}.rs` | **unit-data** descriptor/conversion/ABI/registration checks; **retain-inline** local minimal compile controls. | Do not classify Rust registration strings/type names as full Vela programs or weaken exhaustive standard binding and permission checks. |
| Hot reload `src/tests/{function_abi,function_policy,method_abi,schema_abi,trait_module_abi,state_abi,registry_manifest,package_abi}.rs` | **retain-inline** compact ABI/version pairs and typed policy/signature schemas; **file-backed** substantial stable-ID schema versions and recursive/default-closure initializer families under proposed `tests/fixtures/reload/`. **shared-input** exact stable-rename versions with Engine noted below. | IDs, parameter order/type/defaults, access/effect ceilings, change reports and rejection policy remain typed. `package_abi` constructs metadata, not real filesystem package loading. |
| Hot reload `src/tests/runtime_reports.rs` | Existing **file-backed** root ABI report v1/v2/expected files; **file-backed** substantial method module factory variants below; **retain-inline** small state/image/profile/staging pairs. | Arc ownership, verified MIR/layout analyses, profile/state/image indexes, staged-report timing, old-version lifetime and report details. Do not move these into Engine or generic diagnostic runner. |

### Helper-Built Inputs Requiring Explicit Decisions

| Producer | Consumers and selected disposition |
|---|---|
| Engine `src/tests/source_reload/fixtures.rs::write_reward_modules`, `write_reward_module`, `write_native_reward_module` | Directory and changed-file families plus `function_abi_helpers`/`report_helpers`. **retain-inline** tiny reward/native parameterized wrappers; variation exposes the exact ABI/body change. Keep actual filesystem writes and logical `game` module topology. |
| Same file: `write_schema_reward_modules`, `write_enum_reward_modules`, `write_stable_schema_rename_modules`, `write_trait_impl_modules`, `write_trait_abi_modules` and their singular writers | **shared-input** the identical main module repeated by schema/rename/enum/trait writers: proposed Engine-local `tests/fixtures/reload/modules/main.vela`. **retain-inline** short count-field fragments and return/reward variants. **file-backed** the sizeable fixed stable-rename and trait-implementation versions; enumerate actual call arguments before materializing them, not an unbounded template file. |
| Engine `src/tests/source_reload/host_method_and_source_helpers.rs` | Signature/helper/event writers supply directory/changed-file ABI tests. **retain-inline/generated** concise signature, optional/default parameter and event permutations. They are Vela input factories even though tests may only pass a reward integer. |
| Engine `src/tests/source_reload/function_abi_helpers.rs`, `report_helpers.rs`, `mod.rs` | These consume writers and orchestrate compilation/staging/reports; **retain** owner-local helpers. For example removed-function helper checks old result before staging, before activation and after rejection, report code/target and no destination version. Shared source must not remove directory-vs-changed-file branches or immediate-source-error assertions. |
| Engine `src/source/package/tests.rs::write_package`, local closure `source(value)`; `native_capabilities.rs::SOURCE` and `.replace(...)` | Manifest/source assembly feeds package snapshots and reload, not just source compiler. Complete trees are selected above. Keep the tiny `helper/callback/invoke` value substitution inline. For the native write-to-read replacement, preserve exact match count/change scope when moving `SOURCE`; use explicit versions if safer, without changing effect/old-closure assertions. |
| Hot reload `src/tests/runtime_reports.rs::module_sources`, `script_method_module_sources_with_bonus`, `inherent_script_method_module_sources_with_bonus` | `ModuleSource` factories feed image/metadata/update tests. **retain-inline** tiny reward graph. **file-backed** finite trait/inherent bonus versions under proposed `tests/fixtures/modules/{trait_method,inherent_method}/`; keep SourceIds 1/2, anonymous package and `game::main`/`game::reward` paths in Rust. |
| Analysis `facts/implicit_receiver_tests::fixture`; `executable/callback_tests::graph`; `validation/*::graph` | First owns substantial documents selected for extraction; others build fresh graphs and select main/Probe/body, not static singleton fixtures. Keep schema decoys, diagnostics and literal/expression selectors local. |
| MIR `builder/tests/*::try_build*`/`build_fixture`; VM `src/tests.rs::compile_standard_program_source*`, collection-local compile helpers | Helpers consume source plus descriptors/natives/options. Extraction changes source arguments only; differing target snapshots, standard registration and owned/heap export routes prohibit blind helper deduplication. |
| VM `dynamic_method_dispatch::assert_dynamic_iterator_adapter_cache`, `linked_context_execution` callback loop; bytecode `linker/tests::assert_linked_dynamic_method_source` | **retain-inline** small fixed method/body substitutions. Preserve cache site/hit counts, callback depth cleanup and linker dispatch checks. |
| Syntax recovery and analysis alias/flow matrices | **generated** bounded context/operator/EOL/import alternatives where construction expresses the test dimension; not a loophole for hiding large fixed programs. |

## Concrete Shared-Input Decisions

The first four selections are high-value; remaining small repeats need not all
be extracted. All proposed root conformance additions have the named stage as
single writer, with consumers updated together. No distribution-copy is selected:
these are repository test/bench routes. Before landing root-to-crate includes,
verify affected Cargo/package reachability; do not introduce production includes
of repository-only test data.

| Actual repeated payload / current origins | Canonical origin and writer | Independent consumers/proof |
|---|---|---|
| 931 bytes: bytecode `src/compiler/tests/value_method_shapes.rs::compiler_lowers_value_method_ids_after_set_combination_chains`; VM `src/set_methods.rs::runs_compiled_set_combination_methods` | **shared-input**, proposed `tests/fixtures/conformance/collections/set_combination_chain.vela`, SF04 writer coordinating SF05. | Compiler asserts no unresolved method names and exact standard `sort_by` ID; VM installs standard natives and expects String `daily,quest`. Share bytes, not runner or assertions. |
| 187 bytes at four Engine `src/runtime/inline_cache_method_tests.rs` sites (near lines 23, 74, 130, 195) | **shared-input**, proposed Engine `tests/fixtures/cache/counter_method.vela`, SF06. | Fresh cache/runtime setup for each case; accepted/rejected reload and cache instrumentation are not redundant tests. |
| 496 bytes at VM `src/tests/reflection_values.rs` near lines 167 and 354 | **shared-input**, proposed VM `tests/fixtures/reflection/script_trait.vela`, SF05. | Keep registry/module/value-reflection setup and its separate assertions; bytes alone do not authorize orchestration consolidation. |
| 163-byte v1 and 192-byte v2 stable member-rename scripts: Engine `src/tests/source_reload/source_file_schema_trait.rs` near lines 61/82; hot reload `src/tests/function_policy.rs` near lines 187/210 | **shared-input**, proposed `tests/fixtures/conformance/reload/stable_member_rename/{v1,v2}.vela`, SF06. | Engine source-file staging/safe point versus linked-artifact ABI policy. Preserve explicit member IDs and complete old/new versions; do not rename schema members while extracting. |
| 320-byte enum `Damage` program repeated in VM `src/tests/records_enums.rs` near lines 1114/1142 | **shared-input**, proposed VM `tests/fixtures/records/enum_defaults.vela`, SF05. | Keep separate execution/budget route observations; compare assertions before considering compile-once within a compatible case. |
| 265-byte scalar-loop program: bytecode `src/compiler/tests/loops_and_errors.rs` near line 125; VM `benches/baseline/workloads.rs` near lines 75/95 | **shared-input candidate**, proposed `tests/fixtures/conformance/scalars/typed_loop.vela`, SF04 with SF11 consumer update. | Compiler typed-op/plan proof and differently instrumented benchmark rows retain separate IDs/measurement modes. Packaging/reachability review precedes cross-owner bench include. |
| 395-byte computed-array-slice program: bytecode `src/compiler/tests/value_method_shapes.rs` near line 62; VM `benches/baseline/workload_sources.rs` near line 691 | **shared-input candidate**, proposed `tests/fixtures/conformance/collections/computed_slice.vela`, SF04/SF11. | Lowering IDs versus workload execution/timing. No file IO in timed loops. |
| 153-byte enum-pattern source across bytecode `semantic_input/tests/external_descriptors.rs`, `tests/phase0_frozen_contracts.rs`, VM `tests/records_enums.rs`; 147-byte qualified record-pattern source across bytecode `semantic_input/tests/script_methods.rs`, `tests/phase0_frozen_contracts.rs`, VM `tests/modules.rs` (all beneath `src/compiler` or `src` as applicable) | **shared-input candidates**, proposed `tests/fixtures/conformance/patterns/{enum_descriptor,qualified_record}.vela`, SF04. Coordinate complete module graph, not only matching main text. | Compile target identity, frozen diagnostic/legacy pattern semantics and runtime false-arm/budget proof are different. Recheck dependency documents before selecting sharing. |
| Small repeats: 121-byte HIR/bytecode method catalog; 122-byte VM retained-adder; 124-byte compiler/VM tuple match; 158-byte match guard; 164/184-byte hot-reload initializer versions | **retain-inline** where the local before/after or input/assertion relation remains clearer. Larger surrounding families may extract owner-locally; these are not evidence for a global tiny-program pool. | Identity, phase, nested capture, literal match and initializer dependency observations remain independent. |
| Existing root `tests/fixtures/conformance/{assignments,control_flow,record_writes,named_arguments,script_methods}/` and core/reward/primitive/dynamic-dispatch inputs | Existing **shared-input** canonical origins, no cosmetic move. | Bytecode lowering, VM results/errors and Engine registry lowering remain distinct. HIR/analysis alone share `source_graph.rs`; VM builds its own execution graph. |

No assertion deletion or test merging is selected. Approximate/normalized source
equality is insufficient: near-matching collection programs frequently exercise
different routes or values.

## Host, Reflection And Standard Libraries — SF05

| Owner/current files | Disposition and next step | Existing proof |
|---|---|---|
| Host `src/call_value.rs`, `tests/unsafe_boundaries.rs` | **unit-data** owned record/bytes/map/tuple/scalar carriers; **not-test-input** Rust files/needles consumed by the unsafe audit. | Carrier round-trip preserves U8 tag and data; audit parses actual Rust and confines unsafe to reviewed boundary files. No Vela source input was found; this conclusion follows consumer inspection, not the zero signal. |
| Reflect `src/script_types.rs` | **file-backed** full attributed struct/enum/trait metadata documents under proposed `tests/fixtures/metadata/`; **retain-inline** concise reorder/rename versions when adjacent comparison is clearer. | HIR registration keeps source IDs, docs/attrs, field/variant order/defaults, schema hashes and stable identity. This is metadata, not a new host-object surface. |
| Reflect `src/modules/tests.rs`, `src/modules/records.rs` | **file-backed** substantial module/export documents and module graphs under proposed `tests/fixtures/modules/`; **retain-inline** minimal visibility/path controls; **unit-data** record representations. | Package/module lookup, export ordering, access and metadata fields. Keep actual ModuleSources and registry setup separate. |
| Reflect `src/tests/{calls,fields,implements}.rs`, `src/members/tests/{candidates,metadata,policy}.rs`, `src/{metadata,permissions,types}.rs`, `tests/value_access.rs` | **unit-data** typed records/registries/policies; **not-test-input** source audit needles in `value_access`. No blanket extraction of strings that are names/docs/expected errors. | Independent copied updates leave original value unchanged; denied writes preserve original; candidate lists, permissions and absence of object/adapter escape APIs remain asserted. |
| Stdlib `src/{manifest,ids,reflection_natives}.rs` tests | **unit-data** manifest definitions, stable IDs, primitive mapping, duplicates and effect/policy metadata; production signature/doc strings are **not-test-input**. | Retain manifest lengths, definition lookup, removed names, derived IDs and reflection registration policy. Runtime source execution belongs to existing VM/Engine consumers, not invented fixtures here. |
| Stdlib runtime `src/lib.rs` tests | **unit-data** `stdlib_function_runtime_bindings`/`stdlib_method_runtime_bindings` results. | Function binding count equals `STD_FUNCTIONS`, every function has typed implementation, method count equals `STD_METHODS`, IDs unique. Do not downgrade whole-manifest checks to a representative script. |

## Bench Inputs And Indirect Consumers — SF11 Handoff

These are included in the census even though their extraction belongs to SF11.

- **VM static baseline programs:** `benches/baseline/workload_sources.rs` and
  `workload_sources/{calls,ranges,strings,iterators,sets,typed_containers}.rs`, plus
  source literals in `benches/baseline/workloads.rs`, are **file-backed** candidates
  for `crates/vela_vm/benches/fixtures/baseline/`. They are complete authored
  programs, not generated workloads just because their enclosing module is named
  `workload_sources`. Consumers include the `baseline` bench and
  `tests/baseline_workload_contract.rs`. Keep workload IDs, entry modes,
  host/native setup, warmup, checksums and timed regions in Rust. Rust
  argument/heap-size factories in workload setup stay **generated/unit-data**.
  `benches/scalar_block_allocations.rs` is a separate **unit-data** consumer of
  hand-built linked code/scalar plans, not these source constants: retain its
  global allocator, one-versus-10,001 iteration result and allocation checks.
- **VM external comparison:** `benches/external_compare/workloads/{core,extended,
  value_keyed}.rs` contains Vela, Lua, Rhai, JavaScript and Python source. Select
  **file-backed** per-language files under proposed
  `crates/vela_vm/benches/fixtures/external_compare/`, using real extensions.
  Consumers are `external_compare`, `verified_mir_compile` and the integration
  `external_compare_contract` module; `baseline_cache_delta` consumes benchmark
  support rather than compiling another source family.
- Concrete comparison duplicates include the 515-byte Vela set lookup program
  in `core.rs` near line 557 and `value_keyed.rs` near line 727, and the 463-byte
  **Rhai** map program at `core.rs:496` and `value_keyed.rs:46,147`.
  **shared-input** each within the benchmark owner; retain workload IDs and
  distinct Vela routes even where another language's implementation is identical.
  Do not infer whole-workload equality from one language payload.
- Existing `embedded_workload_checksums_match` runs Vela/Lua/Rhai at two
  iterations and compares their results; Lua/Rhai compile/run and opcode/MIR
  inventory checks also exist. Missing SF11 work is independently expected
  semantic preflight and selected external-process failure handling, **not** an
  absence of any runtime comparison. No benchmark/performance run was made here.
- **Engine static bench input:** `benches/{hot_reload,interop,actor_concurrency}.rs`
  and `benches/actor_memory.rs::memory_source(ArtifactShape::Small)` are
  **file-backed** candidates under proposed Engine `benches/fixtures/`, keeping
  reload accepted/rejected versions together. **generated** large
  `actor_memory::memory_source`, `generation_memory::{top_level_source,
  call_heavy_source,lambda_source}`: function count/generation/lambda shape is
  the measurement dimension. Do not replace scale generation with one static file.
- No owner build script was found. Before future fixture moves, inspect package
  inclusion and run the affected isolated packaged/build route if needed. Existing
  repository-root includes are not proof every proposed bench path ships.

## Existing Lifecycle And Assertion Baselines To Preserve

Inspected code establishes the following obligations; none was run in this task.

| Source/test | Existing observations beyond success/failure |
|---|---|
| Engine `src/tests/native/compiler_options.rs::engine_installs_registered_native_functions_into_vm` | Exact registered owned I64 arguments/results; linked execution returns 5. Retain this small inline baseline. |
| VM `src/tests/execution_core.rs::execution_unit_budget_stops_dispatch_before_next_instruction`, `call_depth_budget_stops_recursive_scripts` | Exact budget kind/limit, consumed units and call depth reset to zero. |
| VM `src/tests/scalar_blocks.rs::linked_scalar_block_executes_checked_operations_and_exact_budget_units`, `linked_scalar_block_stops_at_the_first_trap_source` | Selected versus ordinary execution, exact 4/3 charges, first failure span, arithmetic overflow and matching ordinary error. Hand-built plans are necessary. |
| VM `src/tests/linked_context_execution.rs::synchronous_collection_callbacks_obey_call_depth_budget` | Both direct-array and iterator callbacks inherit depth limit 8, error has source span, depth returns to zero. |
| Engine `src/runtime/reentry_tests.rs` | Nested same-session native/method calls; parent roots survive GC; retained/cloned re-entry result survives incremental GC; inherited depth failure followed by successful reuse returning 9. |
| Engine `tests/hot_reload.rs` retained/active-call tests | Active synchronous call and nested re-entry keep old owner until safe point; old/new results; selected plans; generation inequality; rejected native-closure update leaves exact `Arc` owner unchanged. |
| Engine `src/tests/runtime_args/state_reclamation.rs` | Removed closure-valued state cannot self-pin; retained generation count 2 while old closure lives, then 1 after explicit drop and safe point; removed state ID retained then reclaimed. This proof is already present. |
| Engine `src/runtime/tests.rs` shared-image tests | Shared execution-data/plan owner versus separate heaps, retained-value roots and Runtime IDs; cross-Runtime retained argument rejected; first state increments while second stays unchanged. |
| Engine `src/tests/source_reload/runtime_safe_points.rs`, package/native-capability families | Compatible state, added-state initialization, rejected publication, changed/impacted package sets and native read/write policy are separate obligations; inspect every touched scenario's intermediate checks before extraction. |
| Engine `tests/portable_artifact.rs`; bytecode `src/portable.rs` | Receiving Engine decodes/links/executes without source compilation; current codec round-trips selected coverage/scalar range plans and rejects checksum-valid corruption and obsolete versions before payload decode. Existing execution smoke is not source-vs-decoded equivalence over a selected corpus. |
| Bytecode `src/compiler/tests/diagnostic_contracts.rs` | Explicit phase/count/severity/code/message/span/ordered-label contracts; match uniqueness/occurrence helpers and empty repairs/candidates where required. Goldens must supplement, not replace these checks. |

## Prioritized Next Work And Coverage Limits

1. **SF01:** keep all three Engine string rows inline with the reasons above;
   extract the formatter declaration pair and retained-closure layout versions.
   Demonstrate byte equality and unchanged assertions. Attach add/delete discovery
   proof only to a suitable homogeneous corpus.
2. **SF02:** parser UI-P source/complete-output inventory plus selected AST/parser
   documents and formatter pairs. Preserve losslessness and EOL generators.
   Add selected valid reparse/idempotence proof; execute before/after at an
   existing Engine boundary, not by adding a runtime dependency to syntax.
3. **SF03:** package trees, HIR import/visibility UI-S, analysis callback/implicit
   receiver documents and semantic UI-S. Keep bare/registered and graph-ingestion
   distinctions. Audit def/registry typed inputs without inventing fixture quotas.
4. **SF04:** shared set-combination source first; compiler diagnostic UI-S and
   MIR source/dump pairs next. Agree on selected portable-supported scalar,
   collection/default and error cases with SF06; no package metadata, retained
   MIR or process-local generation encoding additions.
5. **SF05:** VM metadata queries, collection chains, bytes iteration and exact
   duplicate families; preserve raw VM/cache/GC/plan constructions. Reflect
   metadata source extraction is separate from typed host/stdlib audits.
6. **SF06:** Engine's large standard metadata program, reusable Counter source,
   re-entry GC programs, substantial reload/module factories and package versions.
   Preserve reclamation and isolation proof. Extend the existing portable public
   route with a small source-versus-decode/link comparison in fresh receiving
   Engines, equal arguments/settings and exact outputs/errors/effects; do not
   claim round-trip structural tests already establish that equivalence.

Remaining work is explicit:

- This is a family-level census with targeted body inspection, not a Rust-aware
  exhaustive literal evaluator or assertion-by-assertion migration ledger. No
  exact extraction percentage or complete deduplication claim is made. Later
  stages must reconcile every changed literal/helper caller, evaluated byte
  sequence, named test identity, feature and assertion before deleting inputs.
- Concrete large/repeated candidates have decisions; residual short cases have
  family-specific retention reasons. Where a row selects only substantial
  documents in a mixed file, stage review must enumerate that file's remaining
  source arguments rather than treating the file as automatically closed.
- Conditional sharing of module-pattern and benchmark inputs remains open until
  complete environment/dependency and packaging review. This is not permission to
  silently merge source graphs or introduce a distribution copy.
- UI-P/UI-S inventory, adversarial golden/count/span checks and actual discovery
  invalidation have not been implemented or validated here. UI-C and other
  unassigned owners are outside this document's implementation scope.
- Formatter execution equivalence and selected source/portable equivalence remain
  planned coverage strengthening; source extraction alone does not close them.
- Bench and build/package invalidation were inspected structurally, not run.
  There is no fresh whole-workspace acceptance, browser execution, installed-editor
  provenance, physical Input/Render or Windows evidence. Existing separate gaps
  remain separate; they must not be promoted by this census.

Use [validation.md](validation.md) for later focused gates. This task inspected
baseline tests instead of running large tests/builds. Parent-owned status,
decision and validation documents were deliberately not edited; this note does
not declare SF01–SF12 complete or authorize commits.
