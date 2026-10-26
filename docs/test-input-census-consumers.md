# SF00 Consumer Input Census

Status: census and implementation handoff only. No source extraction, new tests,
builds, test execution or commits were performed for this review. The parent
owns status updates and commits. This document does not close SF07, SF08 or SF11.

Authority: [input organization plan](test-organization-plan.md),
[architecture](architecture.md), [goal](goal.md), and
[validation commands](validation.md). This is test organization for the existing
synchronous language, not authorization for production features or new APIs.

## Scope And Method

Assigned owners: `crates/vela_cli`, `crates/vela_playground_wasm`,
`crates/vela_macros`, `crates/vela_bindgen`, `crates/vela_bindgen_compile_test`,
`examples`, `site`, `scripts` outside `scripts/lsp-matrix`, editor
syntax/query/package consumers, benchmarks and fuzz. This includes SF11 census
work; the initial exclusion of editors/benchmarks/fuzz was incorrect.

To avoid duplicate source decisions, the [runtime census](test-input-census-runtime.md)
owns Engine/VM benchmark source families, including external-comparison extraction,
and the [tooling census](test-input-census-tooling.md) owns LS benchmarks, matrix
inputs and VS Code test/provider inputs. Those notes were read before completing
this census. Here SF11 covers the remaining editor grammar/query/package scripts,
site scripts, fuzz targets/corpus and performance wrappers, plus the independent
external-comparison execution obligation. Census coverage is not implementation
or executable acceptance.

Inspected tracked paths, Cargo target declarations, source-consuming helpers,
existing test bodies, `include_str!`, filesystem writes, fixed replacements,
TypeScript template literals and site scripts. Read the current plan and relevant
progress/removal guidance; inspected the existing parent documentation diff before
writing. Existing unrelated changes are not part of this deliverable.

Paths below are repository-relative. **Proposed** paths do not exist yet. Storage
dispositions describe a decision for later implementation, not a migration that
has happened. Test assertions, options, logical names and ordered events remain
with their current Rust/JS owners. No executable catalog, general loader or new
fixture metadata schema is proposed.

`git ls-files scripts` and directory inspection found only `scripts/lsp-matrix`:
there are **no tracked root-script inputs in the assigned non-matrix subset**.
`site/scripts` is separately reviewed below; this finding does not exempt matrix
seeds or editor scripts from their other owner's census.

## Target Boundaries And Helper Contracts

| Owner / actual target and files | Setup, consumer and proof boundary |
|---|---|
| CLI integration target `run_scripts`, `crates/vela_cli/tests/run_scripts.rs` | `run_cli` and `run_cli_args` spawn `CARGO_BIN_EXE_vela_cli`; `write_script` materializes exact source bytes under `ScriptFixture`. These prove real argument parsing, file ingestion, dispatch and output, not an Engine substitute. Helpers currently use unbounded `Command::output()`. |
| CLI binary unit tests, `crates/vela_cli/src/main.rs` → `metadata.rs` | `schema_json` projects an explicitly built Engine's tooling schema. `cli_schema_json_projects_registered_value_metadata_without_host_surface` compares registered facts and removed fields. It does **not** execute the `--print-schema` dispatch branch. |
| WASM `--lib`, `crates/vela_playground_wasm/src/lib.rs` | Tests call public `compile_script` and `run_script`. Each constructs a fresh Engine through `playground_engine`; clock/random and permissions are deterministic. Compile constructs/drops a Runtime; run additionally calls the named entry with finite instruction/memory/depth limits, converts the result to owned JSON and projects diagnostics. Retain both entry routes. Native execution is not browser/JS-glue proof. |
| Macro target `integration`, `crates/vela_macros/tests/integration.rs` → `tests/value.rs` | `autotests = false` in `crates/vela_macros/Cargo.toml`; `value.rs` is a module, **not** a standalone Cargo test target. Each execution case builds its own registry/Engine/Runtime with the relevant derived types. External-crate derive visibility, schema, codecs, exact owned tags and runtime behavior are the boundary. |
| Macro target `interop_exports`, `crates/vela_macros/tests/interop_exports.rs` | Local `runtime(source)` registers `Record`, `ExternalQuality` and the named exports into a fresh Engine, fails explicitly at compilation/initialization, then returns a fresh Runtime. Local `call` uses existing unbounded call options and materializes `OwnedValue`, propagating VM errors. Module registration has deliberately separate `VelaBindings` setup. Do not merge these setups or cache a global Runtime. |
| Macro `--lib` and target `ui`, `crates/vela_macros/tests/ui.rs` | Unit tests inspect Rust token expansion/classification. `trybuild::TestCases` independently runs pass and compile-fail paths. Keep the proc-macro external-consumer boundary and stderr snapshots; these are not the language UI-P/UI-S/UI-C deliverables. |
| Bindgen `--lib`, `crates/vela_bindgen/src/rust.rs::tests` | `schema(source)` builds a fresh Engine, compiles and clones the compiler-owned binding schema. Only this test helper reads Vela; production `RustBindingsBuilder` consumes schemas. Tests distinguish successful compilation from generator rejection, validate generated Rust using `syn`, and retain deterministic checksum/content assertions. |
| Bindgen compile-test build + `--lib`, `crates/vela_bindgen_compile_test/build.rs`, `src/lib.rs` | Build script compiles `script.vela` with two registered test natives and controlled random, writes `OUT_DIR/vela_bindings.rs`; the library includes generated Rust. Runtime tests install real effect-denying callbacks and Random permission. Build-time stubs are not interchangeable with runtime implementations. Keep this separate generated-public-API consumer crate (`publish = false`). |
| Example integration targets `runnable_examples`, `removed_entrypoints`, plus 14 auto-discovered bins, `examples/Cargo.toml` | Separate manifest/lockfile and serde-enabled Engine dependency. `tests/runnable_examples.rs::run_bin` launches each `CARGO_BIN_EXE_*`, asserts success and returns UTF-8 stdout; it currently has no process deadline/output cap. `examples/src/lib.rs::expect_error` makes expected-error bins exit successfully only after the expected rejection. |
| Example library helpers, `examples/src/lib.rs`, `diagnostics.rs`, `hot_reload_demo.rs` | `example_dir` resolves runtime directory inputs from `CARGO_MANIFEST_DIR/src/bin`. Renderers preserve caller-supplied labels. `hot_reload_demo::run` creates a fresh Engine/runtime, retains the old version, stages/activates, executes the exact old linked artifact and the active runtime separately. Do not replace this with only two result assertions. |
| Site, `site/package.json` and `site/src/components/Playground.astro` | `PLAYGROUND_EXAMPLES` becomes `data-examples`, then `loadExample` sets both source and entry. `execute` calls generated WASM `compile_script`/`run_script`. Current automated commands are docs, Shiki syntax and Astro check/build; none drives browser Compile/Run. |

## SF07 — Rust Boundary Tools

### Macro Families

All paths in this table are under `crates/vela_macros/`.

| Actual input location / family | Disposition and selected target | Assertions and environment to retain |
|---|---|---|
| `tests/value.rs::byte_vector_fields_use_the_runtime_bytes_contract`, `make_bytes` source | `retain-inline`: the short constructor makes `b""` versus array-of-bytes behavior visible beside Rust `Vec<u8>`/nested-vector expectations. | Registered `ByteRecord`, `Bytes`/`Array<Bytes>` hints, codec decode and empty byte values. |
| `tests/value.rs::derived_value_round_trips_through_real_vela_execution`, `increase(ItemGrant)` | `retain-inline`: a single local record transformation; no demonstrated duplicate origin. | Field rename `item_name`, exact count 7, string preservation and `OwnedValue::Record`. |
| `tests/value.rs::derived_enum_round_trips_through_real_vela_match`, `increase(GrantDecision)` | Selected `file-backed` candidate: proposed `tests/fixtures/values/enum_round_trip.vela`. The multi-arm constructor/match is a coherent editable program, while the two Rust calls remain one scenario. | Named-field rename `total`, result count 8/item `gem`, `OwnedValue::Enum`, and the subsequent Pending unit-variant round trip. Preserve evaluated leading/trailing newline bytes. |
| `tests/value.rs::derived_enum_returned_by_rust_native_keeps_match_identity`, `count_decision` | `retain-inline`: short native-result/match program whose native registration is the essential context. | `current_decision` registration, nominal enum matching, exact i64 11. Not a duplicate of the previous enum test. |
| `tests/interop_exports.rs::module_registration_installs_through_application_bindings` | `retain-inline`: one-line `rules::clamp(-7)` program. | Separate module bundle/VelaBindings registration and exact 0. |
| `tests/interop_exports.rs::decoded_shared_values_and_strings_live_only_for_the_invocation` | `retain-inline`: adjacent `inspect`/`transform` functions make the owned copy versus invocation-local decoding contrast clear. | `3:token!`, transformed count 9, unchanged original Rust record. Never turn this into retained caller borrowing. |
| `tests/interop_exports.rs::external_value_enum_keeps_owned_codecs_registration_and_match_identity` | `retain-inline`: compact enum match local to the `ExternalQuality` setup. | Both Rust conversion routes, nominal identity and exact `Rare` string. |
| `tests/interop_exports.rs::exported_value_errors_and_panics_are_converted_and_runtime_remains_usable` | `retain-inline`: four short entry functions map directly to an ordered failure/panic/reuse scenario. | Authored TypeMismatch, both callable names in RustCallablePanicked, normal call after **each** panic. Do not split into isolated fresh-runtime rows. |
| Source-free schema/registration/codec tests in both files | `unit-data`: Rust structs, nested collections, metadata and typed values, not missed Vela inputs. | Complete nested registration closure, field/variant identities, effect and boundary contracts. |
| `src/export/signature/tests.rs`, `src/export/mod.rs`, `src/export_module.rs`, `src/external_value_enum.rs`, `src/type_identity.rs`, `src/value.rs` test modules | `unit-data`: `quote!`/`parse_quote!` Rust signatures, attribute tables, expected expansion needles and identifiers. Fixed token substitution is not a Vela program generator. Production expansion templates are `not-test-input`. | Async/borrow/wrapper rejection, invocation-local argument classification, effect accumulation, private-helper visibility, identity aliases and unsupported Value shapes. No `.vela` conversion. |
| `tests/ui/interop/pass/*.rs`, `tests/ui/interop/fail/*.{rs,stderr}`, `tests/ui/removed/*.{rs,stderr}` via `tests/ui.rs` | `specialized-corpus`: retain the existing trybuild origin and paired stderr. | `ordinary_exports.rs` positive compilation; rejected signatures, removed macros and borrow APIs. Retain actual glob discovery, compiler diagnostics and public dependency boundary. |

There is no justified shared Vela fixture between these differently registered
macro cases just because several contain an enum match or a function named
`increase`. The selected enum extraction is local, not a global fixture.

### Bindgen And Generated Consumer

| Actual source / helper / target | Disposition | Required proof / package obligation |
|---|---|---|
| `crates/vela_bindgen/src/rust.rs::generation_is_deterministic_and_syntactically_valid` through `schema` | `retain-inline`: two exported functions plus a doc attribute/default argument are easy to relate to the assertions. | Generate twice; equal code/schema checksum; parse Rust; retain docs, named module override and absence of async wrappers. |
| Same file, `generation_uses_schema_only_and_rejects_unsupported_iterator_boundaries` | `retain-inline`: one-line parameter and two-declaration nested model are a small, meaningful case table. | Compilation succeeds first; generator rejects with `bindgen::unsupported_iterator_boundary` **and source span**. |
| Same file, `generation_emits_owned_models_and_typed_method_receivers` | `retain-inline`: four concise declarations expose Point/Choice/method relationships beside generated-code checks. | Rust syntax, typed receiver and exact record/variant field-count guards. Do not substitute compile-test's larger, differently registered schema. |
| Same file, keyword cases and production Rust text emission | `unit-data` for keyword/identifier expectations; `not-test-input` for generator output templates. | Escaped Rust identifiers still parse; do not treat every raw Rust string as Vela. |
| `crates/vela_bindgen_compile_test/script.vela` used by `build.rs` and `src/lib.rs` | Existing `file-backed` + `shared-input`; keep this single origin. It already covers typed functions, containers, Point/Choice/methods and permission-denial entries. | Build dependencies and runtime registration remain distinct. `build.rs` already emits `rerun-if-changed=build.rs` and `rerun-if-changed=script.vela`; keep source inside this package. |
| `src/lib.rs::generated_binding_re_resolves_after_compatible_body_reload` derived version | Retain the explicit bounded single replacement over the file-backed origin. This is an intentional local delta, **not** a scale generator and not another authored full program. No v2 file selected. | Preserve `matches("return left + right;").count() == 1`, replacement scope, stage then activate and generated-call 42 → 43. A future fixture edit must not silently update multiple functions. |
| `src/lib.rs::generated_binding_executes_without_runtime_names_or_manual_values` and generated wrappers | Rust `unit-data` and generated Rust build artifact, not source extraction candidates. | Add 42; label `value`; conversions `(4, 4, 7, Some(9), Ok(10))`; Point `(2,3)`, sum 5 and Choice value 7; both precise permission-denied checks. |

**SF07 executable work remains:** perform selected extraction with byte equality,
run macro integration/interop/UI plus bindgen and generated-consumer tests, and
check fixture packaging/build invalidation. Do not weaken generated-call,
visibility, native-effect or trybuild proof to make sources uniform.

## SF08 — CLI And WASM Public Routes

### CLI Input And UI-C Selection

All real-process source literals currently live in
`crates/vela_cli/tests/run_scripts.rs`; there is no existing CLI UI golden corpus.

| Current family / logical basename | Current independent expectation | Disposition / later storage |
|---|---|---|
| `cli_runs_script::script_main`, `basic.vela` | Exact stdout `Scalar(I64(5))\n`, successful status. | Selected `file-backed` UI-C source + stdout/stderr pair, proposed `crates/vela_cli/tests/fixtures/ui/basic.{vela,stdout,stderr}`. |
| `cli_runs_script::standard_time_and_random_helpers`, `stdlib.vela` | Exact stdout `Scalar(I64(13))\n` with CLI's clock/random settings. | Selected same UI-C layout, stem `stdlib`; retain deterministic public setup, not WASM's different clock/random configuration. |
| `cli_reports_script_diagnostics::compile_diagnostic`, `invalid.vela` | Unresolved-name code/message, source text and `invalid.vela:3:12`. | Selected same layout, stem `invalid`. Preserve the literal's initial newline/indentation so coordinates do not move. |
| `cli_reports_script_diagnostics::runtime_diagnostic`, `runtime_error.vela` | Division-by-zero message, helper/main stack source and coordinates `3:12`/`7:12`. | Selected same layout, stem `runtime_error`. Full stderr must preserve both frames, not just the primary diagnostic. |
| `cli_reports_script_diagnostics::removed_async_source`, `async_default.vela` | Rejection containing an error and async, forbids obsolete rerun guidance. | Selected same layout, stem `async_default`. Keep forbidden-guidance assertion alongside complete output/status. |
| `cli_rejects_removed_async_flag`, `async_explicit.vela` | Clap rejects `--async` before compiling its two-function source. | Existing setup is a real source-bearing location, not missed by the census. For byte-preserving relocation use proposed `ui/async_explicit.vela`; pair outputs by a named `removed_async_flag` case with typed args/status. Do not claim this source is parsed. A future separately reviewed source-less argument case needs no dummy `.vela`. |
| `cli_renders_clap_help` | Help description/usage (`vela_cli` or `vela_cli.exe`), no `--async`. | `retain-inline` args/needles (`unit-data`), no source file. Preserve the explicit platform executable-name distinction. |
| `crates/vela_cli/src/metadata.rs` test | Format 4 and registered field/type/binding facts, forbidden removed metadata. | `unit-data`: no Vela input. Keep as unit proof when adding process-level schema coverage. |

UI-C is intentional coverage strengthening, not merely moving strings. Add full
stdout, full stderr and **exact exit code** for selected cases (success 0,
`main`'s execution/compile failure 1, Clap flag failure 2 to verify on execution).
Keep expected statuses/args in Rust; no command DSL. Explicitly represent empty
streams. Add nonempty registration/inventory checks for source and output pairs,
including the deliberate flag-case/source basename difference. Wrong status,
wrong output and missing/orphan output files must demonstrably fail. Golden
messages must be independently reviewed, not regenerated during comparison.

### Subprocess And Temporary Ownership Work

SF08 owns the following executable follow-up in the existing CLI test boundary:

- `run_cli` and `run_cli_args` currently block in `Command::output()`. Replace
  with an owner-local helper using a finite deadline, concurrent capture of both
  streams with per-stream caps, and kill/reap on timeout or excess output. Preserve
  raw output and status. Test bounded failure paths without adding CLI commands.
- `unique_test_dir` creates only a PID/time/sequence name; `write_script` calls
  `create_dir_all`, not exclusive atomic creation. `ScriptFixture::Drop` cleans up
  after construction, but a write failure before construction has no such guard.
  Require atomic ownership, collision handling and cleanup through setup failure
  and unwinding; never remove a directory the case did not acquire.
- Set cwd explicitly. Normalize at most the known temporary-root prefix where
  needed for a golden; do not strip arbitrary paths, whitespace or stack lines.
- Add real `--print-schema <SCRIPT>` dispatch proof. `src/main.rs::run_script`
  constructs the Engine, prints schema and returns **before** `compile_file`.
  Use a script that would fail or write if executed and assert no execution side
  effect; the positional script is still required. This is not JSON diagnostics.
- Cover missing source, controlled relative script path/cwd, and filesystem IO
  rooted at the script's parent (the actual `with_fs_io(fs_root)` route). Include
  an unchanged outside sentinel. Check the existing confinement contract rather
  than inventing new restrictions/options.
- `examples/tests/runnable_examples.rs::run_bin` has the same unbounded capture
  issue; coordinate finite example process capture with its separate owner-local
  setup. Do not create a support crate or move example tests into CLI.

### WASM Source Families

All current test sources are in `crates/vela_playground_wasm/src/lib.rs::tests`.

| Test/input family | Disposition and reason | Route / independent proof |
|---|---|---|
| `run_script_reports_runtime_error` | `retain-inline`: short division-by-zero example. | Run route; false `ok`, diagnostic `vm::division_by_zero`. |
| `compile_script_accepts_dynamic_record_field_access` | `retain-inline`: one record and one field-read function, no entry required. | Compile route, dynamic parameter field access; do not turn this into run-main coverage. |
| `compile_script_rejects_removed_extern_state`, `removed_service_call_reports_generic_source_diagnostic` | `retain-inline`: short rejection sources tightly coupled to diagnostic assertions. | `E_PARSE` versus `hir::removed_service_call` and exact Service rejection message remain distinct. |
| `compile_and_run_reject_removed_async_and_task_sources` | `retain-inline`: four concise async/await/spawn/continuation cases, explicit Compile/Run route axis. | Both routes, null value, matching code/error severity/object span; not just `is_err`. |
| `run_script_returns_json_value::json_array` | `retain-inline`: compact heterogeneous array beside `json!(["vela", 42])`. | JSON representation through run route. |
| `run_script_returns_json_value::level_reward` | Selected `file-backed`: proposed `crates/vela_playground_wasm/tests/fixtures/json/level_reward.vela`. | Existing independently authored JSON 40. This source uses `int`, leading/trailing whitespace and different formatting from the site source. It is **not byte-identical**, so relocation must not silently replace it with the site example. |

Fresh Engines and distinct compile/run behavior are retained. The added actual-site
lane below complements these tests; a similar title or equal result is not evidence
that the advertised bytes were exercised.

## SF08 — Examples

`examples/tests/runnable_examples.rs` already launches all **14** current bins:
eight `example_exact_output` rows and six `example_contains_output` rows. Preserve
these process observations and the bins' internal checks. No example target
consolidation is selected.

| Bin / actual source origin | Route, expectation family and disposition |
|---|---|
| `native_function`: `examples/src/bin/native_function/main.vela` | Adjacent `main.rs` includes/compiles source with native registration; exact result 44. Existing `file-backed`, retain. |
| `serde_value`: `examples/src/bin/serde_value/main.vela` | Adjacent `main.rs` includes source; owned serialization/result and unchanged original amount, exact process output. Existing `file-backed`, retain; keep examples' serde feature. |
| `script_state`: `examples/src/bin/script_state/main.vela` | Included source; runtime state and Rust updates, multiple exact observations in stdout. Existing `file-backed`, retain. |
| `container_type_hints`: `examples/src/bin/container_type_hints/main.vela` | Included source; exact i64 17. Existing `file-backed`, retain. |
| `io_stdlib`: `examples/src/bin/io_stdlib/main.vela` | Included source; `main.rs` creates `input.txt`, checks written `output.txt`, stdout contains input and result length 13. Vela is `file-backed`; text `hello from fs` is IO `unit-data`. |
| `modules`: `examples/src/bin/modules/game/main.vela`, `config.vela`, `reward.vela` | `examples/src/bin/modules/main.rs` calls `compile_dir(example_dir("modules"))`, entry `game::main::main`, exact 16. Existing source tree, not three independent test cases. Keep directory/module identities and runtime file reachability. |
| `hot_reload_function_swap`: `examples/src/bin/hot_reload_function_swap/hot_reload_function_swap_v1.vela`, `hot_reload_function_swap_v2.vela` | `hot_reload_demo::run`, old-before 20, old-after 20, new-after 30 plus accepted report/version fields. Existing `file-backed` scenario. |
| `hot_reload_function_swap_invalid`: `examples/src/bin/hot_reload_function_swap_invalid/hot_reload_function_swap_v1.vela`, `hot_reload_function_swap_invalid.vela` | Same helper, expected rejection `v0 unchanged` via `expect_error`. Preserve the incompatible version and rejection path. v1 selected for sharing as below. |
| `package_dependency_demo`: `examples/src/bin/package_dependency_demo/main.rs::write_fixture` and `run_demo` | Inline two manifests/two initial modules plus replacement dependency source; selected source-tree extraction below. `PackageCompileRequest::for_root`, actual workspace load/reload and `main::main` call prove 42 → 43. Bin unit test `example_ordinary_package_dependency_compiles_runs_and_reloads` and process exact-output row both consume `run_demo`. |
| `bad_schema_duplicate_field`: `examples/src/bin/bad_schema_duplicate_field/bad_schema_duplicate_field.vela` | Included with explicit source label and duplicate-field rejection. Existing `file-backed`, retain. |
| `generic_type_hint_denied`: `examples/src/bin/generic_type_hint_denied/generic_type_hint_denied.vela` | Included with explicit source label; only builtin parameterized hints allowed. Existing `file-backed`, retain. |
| `random_permission_denied`: `examples/src/bin/random_permission_denied/random_permission_denied.vela` | Included with explicit source label; native random permission denial. Existing `file-backed`, retain. |
| `reflect_schema_mutation_denied`: `examples/src/bin/reflect_schema_mutation_denied/reflect_schema_mutation_denied.vela` | Included with explicit source label; invalid reflection target. Existing `file-backed`, retain; no schema-mutation feature expansion. |
| `reflect_unknown_field_denied`: `examples/src/bin/reflect_unknown_field_denied/reflect_unknown_field_denied.vela` | Included with explicit source label; unknown field `leve`. Existing `file-backed`, retain. |
| `examples/tests/removed_entrypoints.rs` | Five small rejection sources plus ordinary-trait/default positive control: `retain-inline`. Fresh Engine per case; keep trait control distinct from removed attributes and direct/imported dispatch rejection. No extraction quota. |

Selected duplicate group: the two `hot_reload_function_swap_v1.vela` files are
byte-identical, **71 bytes**, SHA-256
`ed51facccb6ea0c2caee82a17351a3ed44ce8377d9f77c66470f18df7bb1c2f2`.
Use the existing successful-bin path as the single authored origin; update the
invalid bin's include to that sibling source when authorized. Both bins belong
to the same examples package, so no distribution copy or new shared directory is
needed. Retain each caller's logical labels and independent success/rejection
expectations. The tiny existing v1/v2 files need not be moved merely for symmetry.

Selected package tree: move the fixed inputs from `write_fixture` into proposed
`examples/src/bin/package_dependency_demo/fixtures/app/vela.toml`,
`fixtures/app/src/main.vela`, `fixtures/app/lib/vela.toml`, and
`fixtures/app/lib/src/api.vela`; put the update at proposed
`fixtures/updates/api.v2.vela` under that same bin directory. Materialize fresh
writable copies, then write the update to logical `app/lib/src/api.vela` **after**
initial execution. Keep dependency `lib = { path = "lib" }`, package IDs
`dev.vela.example.app`/`dev.vela.example.lib`, source roots, filename and newline
bytes. This is a fixed package graph, not a generator. Public demo inputs remain
adjacent to the demo, not under repository-only root tests.

Temporary ownership follow-up: `package_dependency_demo::fixture_root` is only
PID-based and cleanup occurs only at the successful end of `run_demo`; its unit
case and process route need isolated ownership and failure cleanup.
`io_stdlib::TempDir` has PID/time/counter naming and Drop cleanup but uses
`create_dir_all` rather than exclusive acquisition. Keep this distinct from the
CLI helper; no global temp root or mutable Engine sharing is justified.

## SF08/SF11 — Exact Site Inputs And Syntax Consumers

### Advertised Playground Programs

The actual source is **`site/src/data/playground-examples.ts::PLAYGROUND_EXAMPLES`**,
not any Rust test with the same display name. It is consumed by
`site/src/components/Playground.astro` on both documentation playground pages.
The four plain template literals contain no interpolation or escape conversion;
all start at their first declaration and end at `}` **without a final newline**.
The lengths below were checked from the current literal bytes. Results are
independently derived acceptance expectations, **not execution results from this
census**.

| Exact TS row / source lines | UTF-8 bytes | Entry | Independent native compile/run obligation |
|---|---:|---|---|
| `Level reward`, lines 11–31, `struct Reward` with `i64` fields, `score_reward` and constructor in `main` | 343 | `main` | Compile success; run JSON number **40** (`12 * 3 + 4`). |
| `Collections`, lines 36–45, map gold/gem/xp and duplicate `daily` set input | 282 | `main` | Compile success; run JSON number **37** (`10 + 25 + 2`). |
| `Methods`, lines 50–67, `DamageResult`, inherent `score`, constructor in `main` | 289 | `main` | Compile success; run JSON number **50** (`42 + 8`). |
| `Runtime error`, lines 72–75, local `before = 10`, `before / 0` | 57 | `main` | Compile success; run false `ok`, null value and `vm::division_by_zero` error with a valid source range for these exact bytes. Do not treat it as a compile-error example. |

Selected `shared-input` extraction: proposed single authored origins
`site/src/data/playground/level_reward.vela`, `collections.vela`, `methods.vela`,
`runtime_error.vela`. Retain the TS title/entry records; import raw source for the
site instead of keeping a second literal. Native tests must load these exact
origins and exercise the existing WASM compile/run exports with independent Rust
expected JSON/errors. Keep both the source selection and its entry mapping tied
to the advertised table; merely adding four detached test files is insufficient.

**Open package/reader decision:** site-owned paths outside a Cargo crate cannot
become unconditional external `include_str!` dependencies of a packaged WASM
crate. Before implementing this shared lane, the SF08/SF11 owners must settle a
package-safe repository test location or co-location and verify both consumers.
No existing site-to-Cargo distribution-copy/equality mechanism was found, so this
census does **not** approve a second editable copy or claim a distribution-copy
is already supported. Keep the site as the selected authored origin unless that
ownership decision is explicitly revised. This coordination item blocks shared
lane completion, not the other owner-local extractions.

Current native reward and division-by-zero tests have different bytes and
behavioral emphases. Keep them independent as described above; do not silently
rewrite `int` to `i64`, remove indentation, or claim native tests already cover
all four site examples.

### Highlighting And Documentation

| Source / helper / actual command | Disposition and proof |
|---|---|
| `site/src/syntax/fixtures/complete.vela` + `complete.expected-scopes.json`; `site/scripts/check-vela-highlighting.mjs`; `npm --prefix site run test:syntax` | Existing `file-backed` source with specialized token/scope oracle; keep this origin and independent requiredScopes/requiredTokens. The script loads `site/src/syntax/vela.tmLanguage.json` through Shiki and checks tokens/scopes. This is syntax coverage, not semantic compilation/execution. |
| Same script's literal `state count: i64 = 0\nextern state legacy: Any\nasync fn old() { value.await }` | `retain-inline`: short negative-keyword contrast beside assertions that extern/async/await are not keyword scopes and ordinary state still is. Do not merge into the valid corpus or extract every word probe. |
| `site/scripts/check-reduced-surface.test.mjs` | Regexes, stale-claim strings and short removed-code examples are `unit-data` for an audit, not executed Vela. Tests read bilingual markdown and `site/astro.config.mjs`; preserve independent negative/positive pattern expectations and stable removal-note URLs. |
| `site/scripts/check-doc-placeholders.mjs`; `site/src/content/docs/**/*.md`, `*.mdx` | Markdown is `not-test-input` for language execution; it **is** input to text audits. `collectMarkdownFiles`/`markdownFiles` recurse and read current docs, not synthesize script programs. Ordinary prose fences do not become runnable cases automatically. |
| `site/src/components/Playground.astro`, `site/src/syntax/vela.tmLanguage.json`, site config | Production component/grammar/config text is `not-test-input` except where explicitly read by package/document checks. Do not extract UI strings or grammar regexes as Vela fixtures. |

Packaging route: `.github/workflows/pages.yml` builds the wasm32 release artifact,
uses matching wasm-bindgen CLI to emit `site/public/pkg`, builds Astro and uploads
`site/dist`. `Playground.astro` resolves the generated JS using `BASE_URL`.
Selected raw source imports must survive the real site build and emitted example
payload with exact bytes/entry names; native unit tests alone do not prove that.
Actual browser Compile/Run, displayed diagnostics/results, JS glue and asset/base
paths remain a **separately scoped open browser-smoke gap**, not an SF census or
source-organization completion claim.

## SF11 — Editor, Benchmark And Fuzz Census

These owners are in scope. Source decisions already recorded by the runtime and
tooling censuses are referenced rather than duplicated; remaining consumer paths,
helper contracts, dispositions and executable follow-up are recorded here. No
implementation is included.

### Grammar And Query Execution

- `editors/tree-sitter-vela/test/corpus/state.txt` is a `specialized-corpus`:
  ordinary initialized state, removed extern-state recovery and malformed-state
  recovery each retain their authored source/tree pair. Do not create parallel
  `.vela` copies of its negative cases.
- Keep `site/src/syntax/fixtures/complete.vela` as the existing complete query
  input. It has declarations, methods, strings/comments and nested braces; it
  is not promised to be a runnable semantic program. A concrete positive parse
  selection also includes `examples/src/bin/native_function/main.vela` and
  `examples/src/bin/container_type_hints/main.vela`. Check retained syntax, not
  a broad glob containing intentional rejection programs.
- `editors/tree-sitter-vela/package.json` currently has a broad `parse:fixtures`
  script using `../../../...` paths. From that package directory the repository
  sources are at `../../...`; do not cite the existing npm script as a verified
  current positive-only gate. Owner correction/validation remains pending.
- Account for **four behavioral queries plus two syntax query kinds**, all under
  `editors/zed/languages/vela/`, rather than describing an arbitrary first four
  as the behavioral set. All six are shipped and checked for presence by
  `editors/zed/scripts/validate-package.js`; all six need actual capture/range
  exercise. This clarifies the plan's four-query wording without editing it:

| Kind / actual query file | Existing capture contract / disposition |
|---|---|
| Behavioral: `brackets.scm` | Parentheses/braces/brackets, `@open` and `@close`; retain specialized query source and check paired ranges. |
| Behavioral: `indents.scm` | Block/record/map/match/array/argument/parameter regions, `@indent` and `@end`; retain query source and check containing region/end. |
| Behavioral: `outline.scm` | Function/type/impl/const/state `@item` and `@name`; retain query source and inspect declaration/name ranges. |
| Behavioral: `textobjects.scm` | Function/class `@*.inside` and `@*.around`, comment `@comment.around`; retain query source and check inner/outer selection boundaries. |
| Syntax highlighting: `highlights.scm` | Declaration/member/type/literal/operator captures and fallback precedence; retain query source, independent consistency mappings and actual token ranges. |
| Syntax-region overrides: `overrides.scm` | `@string` and `@comment.inclusive` regions, relevant to `config.toml`'s `not_in = ["string", "comment"]`; retain this distinct query kind and verify region boundaries, not only color captures. |

These are production query files (`not-test-input` as Vela source), exercised by
specialized query validation; do not duplicate their patterns into `.vela` files.

Concrete commands for the editor owner, **not run here**, with cwd
`editors/tree-sitter-vela`:

```sh
npx --yes tree-sitter-cli@0.25.10 test
npx --yes tree-sitter-cli@0.25.10 parse ../../site/src/syntax/fixtures/complete.vela ../../examples/src/bin/native_function/main.vela ../../examples/src/bin/container_type_hints/main.vela
npx --yes tree-sitter-cli@0.25.10 query ../zed/languages/vela/highlights.scm ../../site/src/syntax/fixtures/complete.vela
npx --yes tree-sitter-cli@0.25.10 query ../zed/languages/vela/brackets.scm ../../site/src/syntax/fixtures/complete.vela
npx --yes tree-sitter-cli@0.25.10 query ../zed/languages/vela/indents.scm ../../site/src/syntax/fixtures/complete.vela
npx --yes tree-sitter-cli@0.25.10 query ../zed/languages/vela/outline.scm ../../site/src/syntax/fixtures/complete.vela
npx --yes tree-sitter-cli@0.25.10 query ../zed/languages/vela/overrides.scm ../../site/src/syntax/fixtures/complete.vela
npx --yes tree-sitter-cli@0.25.10 query ../zed/languages/vela/textobjects.scm ../../site/src/syntax/fixtures/complete.vela
```

A successful query invocation alone is insufficient. Check nonempty expected
captures and source ranges: highlights on named declarations/members/literals;
bracket `open`/`close` pairs; indent regions and their `end`; outline `item`/`name`
for Player/Stats/Reward/Tickable/award/current_player; override string/comment
ranges; textobject function/class around/inside and comments. Keep authored
expectations independent of query output, and reject unexpected parse recovery
in the positive selection. `node editors/zed/scripts/validate-package.js` checks
package text, fixture presence and capture spellings, not these range results.
No current execution success is claimed by listing these commands.

### Editor Grammar And Package Consumers

| Actual input/helper/target | Disposition, ownership and retained proof |
|---|---|
| `editors/tree-sitter-vela/grammar.js`, `tree-sitter.json`, `package.json`; generated `src/grammar.json`, `src/node-types.json`, `src/parser.c`, `src/tree_sitter/{alloc,array,parser}.h` | Grammar/config are production `not-test-input`, generated parser artifacts remain generated outputs, not independent source fixtures. The actual corpus runner is `tree-sitter test` over `test/corpus/state.txt` (one positive and two recovery cases); retained-valid parse and six-query commands are above. Keep the canonical grammar at this directory and verify regeneration leaves the expected tracked outputs consistent. |
| `tests/fixtures/lsp_highlighting/showcase.vela` and `consistency.json` read by both `editors/zed/scripts/validate-package.js` and `editors/vscode/scripts/validate-package.js` | Existing `file-backed` + `shared-input`, retain canonical paths. Validators check showcase markers and authored semantic-token/TextMate/Zed mappings. The showcase intentionally contains unresolved calls and schema-dependent names; do not convert it to a standalone executable success fixture or infer supported host APIs from token text. Site `complete.vela` has a different purpose/oracle and stays independent. |
| `editors/zed/scripts/check-grammar-checkout.test.js::repository`, `check-grammar-checkout.js::checkGrammarCheckout` | `retain-inline`/`unit-data`: `.gitignore` contents and `grammar.js` sentinels (`user edits must survive`, `unstaged local work`, `tracked duplicate`) test filesystem/Git ownership, not JS grammar or Vela parsing. Fresh `mkdtempSync` root per case, `t.after` cleanup, local Git repos/indexes. Keep absent-cache acceptance, ignored staged/unstaged work preservation, and tracked-duplicate rejection without deletion. `execFileSync` Git calls have no explicit timeout; any bounded-helper follow-up stays owner-local. |
| `editors/zed/scripts/validate-package.js` → `extension.toml`, `languages/vela/config.toml`, all six `.scm`, `src/lib.rs`, `Cargo.toml`, shared highlighting files and grammar source/generated parser | Package/launcher audit inputs, not a new Vela corpus. Preserve thin-launcher forbidden-protocol checks, local grammar repository/path/rev checks, capture precedence and shared consistency mappings. The current manifest/validator expects this checkout's absolute repository root; isolated-tree validation must account for that existing constraint rather than copying a machine path into fixtures. No package pass was run here. |
| `editors/zed/src/lib.rs::tests`, separate `editors/zed/Cargo.toml` | `unit-data`: configured path, arguments, sorted environment, default `--stdio`, missing/blank path. `command_from_binary_settings` only builds a command; these unit tests do not launch an LSP process. Keep the isolated `vela_zed_extension` cdylib crate and its launcher boundary. |
| `editors/vscode/scripts/validate-package.js` → `package.json`, `language-configuration.json`, `syntaxes/vela.tmLanguage.json`, `extension.js`, `README.md`, shared highlighting files | `unit-data`/audit inputs. Preserve trust policy, settings, activation, semantic-token types/modifiers/scopes and thin-launcher checks; no `.vela` extraction of expected scope/setting strings. VS Code test/provider source inputs remain in the tooling census. |
| `editors/vscode/scripts/check-removed-consumers.test.js` | Additional direct matrix reader outside `test/`: local `load` parses JSON, several assertions concatenate `spec.files`, and selection sources pass through `parseMarkers`. SF10 reader migration must include these consumers if those fields change; do not interpret a future source reference as text. Its own removed-spelling regexes, source needles and `state count: i64 = 0` probe are `retain-inline`/`unit-data`. Preserve grammar/generated-node/TextMate/Zed state checks, obsolete metadata rejection and example presence/absence audit. Matrix source ownership remains with tooling. |
| `editors/vscode/scripts/package-vsix.js` via `npm --prefix editors/vscode run package` and `.vscodeignore` | Production packaging, `not-test-input`: `run` launches npm/cargo/vsce, `bundleServer` copies the selected built binary, `syncLicense` copies LICENSE. This is **not** a source-fixture copy pipeline. `.vscodeignore` excludes `scripts/**` and `test/**` while production grammar remains packaged; shared repository audit inputs need not be shipped as runtime dependencies. `spawnSync` checks errors/status but has no timeout. No packaging command is run in this census. |

The ignored `editors/zed/grammars/vela` checkout is a build/cache output, not a
second authored grammar or a test `distribution-copy`. Never reset/delete user
work there to satisfy a validator. For later package validation retain the
canonical grammar/config/generated parser and all six query files, and distinguish
checkout-based audits from installed package behavior. Grammar regeneration is
`npx --yes tree-sitter-cli@0.25.10 generate` in `editors/tree-sitter-vela`, followed
by a repository-root `git diff --exit-code -- editors/tree-sitter-vela/src` when
unchanged generated output is expected; it was not run here.

The three site scripts are fully inventoried in the site table above, not omitted
as generic scripts. `check-reduced-surface.test.mjs` audits bilingual prose/fences;
`check-doc-placeholders.mjs` audits prose; `check-vela-highlighting.mjs` tokenizes
the existing complete source and retains its short inline boundary probe.
The two production TextMate grammars at `site/src/syntax/vela.tmLanguage.json`
and `editors/vscode/syntaxes/vela.tmLanguage.json` are **not byte-identical**
(10,759 and 8,226 bytes respectively, checked read-only). Retain independent
consumer-specific grammar contracts; no blind deduplication or distribution-copy
classification is selected. Their joint removal audit is not whole-grammar
byte-equality or whole-token-output proof.

### Benchmark Coverage Reconciliation And Remaining Wrappers

Tracked bench paths and all root-crate Cargo manifests identify only Engine, VM
and language-service benchmark owners. No additional Criterion/divan/`#[bench]`
source family was found in `crates`. Do not invent a fourth benchmark owner:

| Already-owned targets / source families | Census of record |
|---|---|
| Engine `hot_reload`, `generation_memory`, `actor_memory`, `actor_concurrency`, `interop` | Runtime census, including static source candidates and genuine size/generation factories. |
| VM `baseline`, `external_compare`, `verified_mir_compile`, `scalar_block_allocations`, plus integration `baseline_cache_delta`/`external_compare_contract` and target `baseline_workload_contract` | Runtime census, including nested workload source modules, cross-language duplicate groups, compile consumer and raw-plan allocation proof. `baseline_cache_delta` is an integration module, not another declared bench target. |
| LS `incremental_latency` | Tooling census, including `module_text`/`module_document` generators and justified inline `SUPPORT_TEXT`. |

The real remaining benchmark-consumer locations are the five `tools/perf` scripts:

| Actual helper / input route | Disposition and executable limitation |
|---|---|
| `tools/perf/capture_command.py::{output_text,main}` | `not-test-input` command/metadata strings; no Vela source. Runs an explicit command from repo root, combines stdout/stderr, records provenance/elapsed/status, copies successful captures to a requested baseline. `Popen` stream accumulation/wait and metadata subprocesses have no finite timeout/output cap. Preserve child status; capture is not a semantic oracle. |
| `tools/perf/capture_external_compare.py::{output_text,main}` | `not-test-input`: constructs `cargo bench -p vela_vm --bench external_compare -- ...`, stores output/provenance and optionally copies a successful capture. No duplicate authored workload. Same unbounded subprocess/accumulation concern; a bench that only prints `status=error` can still exit zero, so fix selected-runtime failure at the owning bench boundary rather than trusting capture success. |
| `tools/perf/ab_compare.py::{run,best_of,main}` | Runtime args/row regex are `unit-data`; captured timing/checksum rows are measurement data, not Vela fixtures. Interleaves binaries, rejects child failure/empty rows, compares checksums for common rows and estimates noise. Missing candidate rows are skipped and no independent semantic expected values are supplied; it cannot close SF11 preflight. `subprocess.run(capture_output=True)` lacks timeout/output cap. |
| `tools/perf/compare_keyvalue_bench.py::{parse_rows,row_key,main}` | Existing key=value capture format retained; no source extraction. Keys are runtime/mode/bench, with metric/regression/checksum comparisons over the intersection; empty selection exits 2 and mismatches/regressions exit 1 (unless checksum override requested). This does not prove complete workload discovery or independent semantic correctness. |
| `tools/perf/profile_external_compare.sh` | Shell args and profiler/output plumbing are `not-test-input`. Builds then selects the newest executable, starts it, samples for 10 seconds, and uses `wait ... || true`; profiler success is not benchmark success or exact binary provenance. Keep profiling separate from correctness gates; no Vela source or new extraction origin here. |

No wrapper self-test files were found in the tracked perf-tool inventory. This is
not permission to create a test framework or alter performance policy in SF00.
SF11 should preserve row IDs, provenance, checksum checks and timing windows while
ensuring any wrapper used as an executable acceptance route has finite process/
output bounds and cannot promote a failed selected workload to success. Existing
measurement captures/baselines are `not-test-input` for source organization; do
not regenerate them because source paths move. Source loading stays outside timed
regions. The runtime census already selects per-language external-comparison
fixtures; the following section adds the independent-expectation/process contract,
not a competing extraction plan.

### External Comparison: Preserve Proof, Add Independent Expectations

Actual shared source definitions are
`crates/vela_vm/benches/external_compare/workloads.rs` and its
`workloads/core.rs`, `workloads/value_keyed.rs`, `workloads/extended.rs` modules.
`Workload` stores fixed Vela/Lua/Rhai/Node/Python sources; `all_workloads` chains
22 workload records. These are static source programs, **not generated scale
inputs** merely because a wrapper supplies an iteration count.

Actual consumers/targets:

- Bench `crates/vela_vm/benches/external_compare.rs`, with runtime implementations
  `external_compare/vela.rs`, `embedded.rs` and `process.rs`.
- `crates/vela_vm/tests/integration.rs` includes `external_compare_contract.rs`.
  The contract file is a module in target **`integration`**, not a standalone
  `--test external_compare_contract` target. It imports the workload/config/version
  modules using explicit `#[path]` declarations.
- Existing tests check unique IDs, all five nonempty sources, Vela compile/link,
  Lua/Rhai execution, two-iteration Vela/Lua/Rhai checksum equality, and selected
  opcode/verified-MIR inventories with fixed checksums. This is substantial proof;
  do not describe benchmark correctness coverage as absent.

**Missing obligation:** cross-runtime agreement can agree on a wrong result, and
Node/Python do not participate in the embedded equality test. SF11 needs an
untimed low-iteration semantic preflight with independently authored expected
results for every selected workload/runtime. Keep the full 22-ID inventory;
selected structural inventory checksums are not a universal expected-result table.
Expected values must not be calculated by running the Vela implementation during
comparison or blessed from another runtime. Preserve existing equality and
structural assertions as complementary proof.

`process.rs::ProcessRuntime::{locate,version,command_usable,run_process}` currently
uses unbounded `Command::output()` for discovery/version/workload execution.
`run_process` rejects nonzero status and malformed checksum output, but
`external_compare.rs::main` prints `status=error` and continues to `Ok(())` for
process-runtime failures. SF11 must distinguish unavailable optional runtimes
from a selected available runtime's compile/execute/parse/wrong-result failure;
the latter must fail the run. Coordinate finite process/output bounds with this
owner; do not pull its process implementation into CLI tests.

If static workload files are extracted by the benchmark owner, keep one origin
per language/workload, use the language's real extension, update both consumers
and package reachability, and load before timed loops. Preserve workload/runtime
IDs, existing hot-loop timing windows and cache-mode distinctions. Preflight is
correctness work, not a performance measurement or production-feature expansion.
Engine/VM source extraction and duplicate origins remain as selected in the
runtime census; LS source choices remain in tooling. The wrapper and fuzz
families here complete the remaining consumer inventory rather than excluding
them from SF00.

### Fuzz Targets, Static Seed And Mutation Corpus

`fuzz/Cargo.toml` is a separate `vela-fuzz` workspace (`publish = false`) with two
explicit libFuzzer bins, `parser` and `portable_plan`, each `test = false`,
`doc = false`, `bench = false`. Ordinary workspace tests do not run them. Retain
its lockfile/path dependencies, bytecode `artifact-codec`/`test-support` and Engine
`artifact-codec` features; do not move fuzzing under a normal unit-test runner.

| Actual family / helper / consumer | Disposition / selected origin | Boundary and assertions to preserve |
|---|---|---|
| `fuzz/fuzz_targets/parser.rs`, `fuzz_target!(source: &str)` → `parse_source_with_id(SourceId::new(1), source)` | `generated` fuzzer input; no authored static program, no tracked parser corpus in this checkout. Do not invent a seed-copy requirement. | Exercises parser completion/no panic over UTF-8 strings and drops the parsed result. It does not execute scripts, validate semantic success or cover invalid UTF-8 decoding through this `&str` boundary. Bound future smoke duration with libFuzzer flags. |
| `fuzz/fuzz_targets/portable_plan.rs::base_artifact` | Selected `file-backed`: move the fixed 2-function branch/range-loop source to proposed **`fuzz/fixtures/portable_plan/base.vela`**, included by this helper. This is authored static source, not a generator just because it produces a binary artifact. Keep its exact evaluated bytes (currently a single cooked line, no final newline). | Fresh Engine for one-time compile/encode; `OnceLock<Vec<u8>>` caches only immutable encoded bytes, not a mutable Engine/Runtime. Preserve portable v8, scalar block/range-loop and coverage-manifest availability. Do not replace with a similar Engine/VM workload whose selected plans may differ. |
| Same target's raw `input: &[u8]` → `PortableProgramArtifact::decode(input)` | `generated` arbitrary byte input; current corpus remains `specialized-corpus`. | Keep the unconditional direct decoder route before dispatching mutation selectors, including empty/unknown-prefix input. No script execution occurs. |
| `fuzz_value`, `scalar_code`, `scalar_range`, `invalid_plan_id`, `invalid_source_point_id`, `encode_unchecked` and selector match | Genuine `generated` binary mutation, retain Rust construction rather than extracting each generated artifact. | Per invocation clone/decode the immutable base, mutate handles/register/coverage/exit/source fields, recalculate length/checksum and re-decode. Keep little-endian fixed-int bincode and trailing-byte rejection; source changes must still reach the expected scalar structures. Decoder results are discarded: this is no-panic coverage, **not** an assertion that every mutated payload rejects. |
| `fuzz/corpus/portable_plan/{coverage_manifest,exit_target,operand_range,payload_limit,plan_handle,source_point}` plus 29 hash-named files | Existing `specialized-corpus`: 35 tracked byte/control seeds, 239 bytes total at census, lengths 1–52 bytes. Keep the existing corpus path and binary bytes; no conversion to `.vela`, JSON, strings or output goldens. | Named prefixes `c/e/o/p/h/s` target coverage count, done target, cursor register, oversized header, plan handle and source point. Payload-limit uses `MAX_PAYLOAD_BYTES + 1` (64 MiB + 1). The other payload bytes feed native-width little-endian `fuzz_value`, **not decimal text parsing**, even when a named seed looks textual. |

The six named seeds are concrete byte payloads: `coverage_manifest` is `63 30 0a`,
`exit_target` is `65 34 32 39 34 39 36 37 32 39 35 0a`, `operand_range` is
`6f 36 35 35 33 35 0a`, `payload_limit` is `70 0a`, `plan_handle` is
`68 34 32 39 34 39 36 37 32 39 35 0a`, and `source_point` is
`73 36 35 35 33 35 0a`. Preserve EOF/byte order rather than normalizing these
small files as text. Hash-named seeds also pass through the same raw-decode and
selector routes; their names are not an expectation schema.

SF11 executable follow-up: extract only the static base, compare evaluated bytes,
include it within the isolated fuzz source tree, build both bins with their
actual codec/test-support features, then run finite parser/portable-plan smoke
commands. Preserve corpus files and libFuzzer mutation coverage; no clean-corpus
rewrite or snapshot regeneration. `.github/workflows/ci.yml` already checks fuzz
bins and runs both targets with `-max_total_time=300`; that is wiring, not a
current result. Focused 30-second commands are listed below. No fuzz campaign or
coverage claim is made by this census.

## Package, Byte And Discovery Obligations

1. **Owner-local Cargo sources:** proposed macro/WASM/CLI fixtures must be included
   with the owning package's tests. Check package file lists and an isolated
   packaged-tree test/build route; a checkout-only passing include is insufficient.
   Keep external-consumer test targets and dev-only dependencies intact. Do not
   move public compile-test build inputs into repository-only root fixtures.
2. **Generated bindings:** `script.vela` is required at build time and test time.
   Keep its existing `rerun-if-changed`; if a new input is eventually chosen, wire
   it explicitly. `OUT_DIR/vela_bindings.rs` remains generated output, never a
   second authored fixture. `publish = false` does not remove isolated source-tree
   reachability obligations.
3. **Examples:** keep adjacent included sources, the shared reload v1 and proposed
   package tree within `examples`. The modules example reads from disk at runtime,
   unlike embedded `include_str!` consumers: test it from the isolated example tree
   with its manifest-relative layout, not just a copied executable. Keep path
   dependencies/lockfile/serde settings in the isolated validation topology.
4. **Site sharing:** resolve the explicitly open cross-owner native-test/package
   placement before adding external Cargo includes. No approved distribution copy
   exists yet; if independent distribution requires one, first name an existing
   copy/equality route and one authored origin. No symlinks or duplicate fallback
   literals. Keep emitted site sources and entry records linked.
5. **Exact bytes:** compare evaluated Rust/TS source bytes before deleting literals.
   Preserve leading raw-string newlines, indentation, cooked escapes and each
   site's missing final newline; do not run source formatting as extraction.
   Preserve CLI logical basenames/coordinates and example diagnostic labels.
   Any scoped Git-attribute protection belongs to the coordinating owner, not this
   doc-only task; do not silently renormalize unrelated sources.
6. **Inventory:** named `include_str!` cases still need explicit registration;
   validate nonempty selection and UI-C pairs. Existing trybuild globs keep their
   specialized behavior. For any newly chosen glob, prove add/delete discovery
   without a clean build before inventing build invalidation. No target-count or
   zero-inline quota.
7. **Generators:** distinguish fuzzer-produced parser input and portable binary
   mutations from the fixed portable base; the latter is selected for extraction.
   Runtime/tooling notes retain their genuine benchmark scale generators. Package
   `fs::write`, fixed TS literals and the single compile-test replacement must not
   hide static input under a blanket “generated” exemption. Rust token matrices
   remain unit data; generated binding/parser code remains build output.
8. **Editor/fuzz distribution:** preserve the isolated Zed crate, canonical grammar
   and all six queries, VSIX production grammar/config inclusion, and the fuzz
   crate's proposed local seed include plus existing byte corpus. Test-only shared
   highlighting sources stay repository audit inputs, not unconditional installed
   extension dependencies. Preserve ignored Zed checkout work and binary corpus
   bytes; there is no approved new distribution-copy or grammar deduplication.

## Handoff Validation And Remaining Decisions

These are existing focused commands for later implementation, **not results of
this census**, from repository-root cwd unless noted. Use finite external command
bounds and reconcile actual discovered identities; no builds are required for
this documentation task.

```sh
cargo test -p vela_macros --test integration
cargo test -p vela_macros --test interop_exports
cargo test -p vela_macros --test ui
cargo test -p vela_macros --lib
cargo test -p vela_bindgen --lib
cargo test -p vela_bindgen_compile_test --lib
cargo test -p vela_cli --test run_scripts
cargo test -p vela_cli
cargo test -p vela_playground_wasm --lib
cargo check -p vela_playground_wasm --target wasm32-unknown-unknown --release
cargo test --manifest-path examples/Cargo.toml --test runnable_examples
cargo test --manifest-path examples/Cargo.toml --test removed_entrypoints
cargo test --manifest-path examples/Cargo.toml --bin package_dependency_demo
cargo test -p vela_vm --test integration external_compare_contract::
cargo bench --workspace --no-run
cargo check --manifest-path fuzz/Cargo.toml --bins --all-features --locked
cargo fuzz run parser -- -max_total_time=30
cargo fuzz run portable_plan -- -max_total_time=30
cargo test --manifest-path editors/zed/Cargo.toml --lib
node --test editors/zed/scripts/check-grammar-checkout.test.js
node editors/zed/scripts/validate-package.js
node editors/vscode/scripts/validate-package.js
node --test editors/vscode/scripts/check-removed-consumers.test.js
npm --prefix site run test:docs
npm --prefix site run test:syntax
npm --prefix site run build
```

Package inspection at implementation should include `cargo package -p <owner>
--list` for affected Cargo owners and `cargo package --manifest-path
examples/Cargo.toml --list`, followed by isolated validation of the actual required
files. These are command templates, not claims that path-only/unpublished dependency
packaging succeeds. Report dependency/toolchain/environment blockers separately;
do not mark package proof complete from a manifest read. Apply default/affected
feature and broader gates from `validation.md` at the owning stage.

Parent coordination items:

- SF07: selected enum fixture move, preserved inline/token/trybuild dispositions,
  generated-consumer build/package proof.
- SF08: required UI-C goldens/status/inventory and bounded process/temp ownership;
  schema/path/filesystem dispatch coverage; selected native WASM reward and example
  package-tree moves; shared example v1 with both independent routes retained.
- SF08/SF11: settle package-safe actual-site source sharing and execute all four
  advertised programs with independent expectations. Browser smoke stays open.
- SF11 editor owner: validate four behavioral queries (brackets, indents, outline,
  textobjects) plus highlighting and syntax-region overrides, explicit positive
  parse paths, package consumers and capture/range checks. Coordinate the additional
  VS Code removal-audit reader with SF10. No editor status/evidence update is
  authorized by this census.
- SF11 benchmark owner: runtime/tooling census source decisions stand; add independent
  semantic preflight, selected process-runtime failure handling and bounded
  subprocess wrappers without losing checksum/inventory tests or timed boundaries.
- SF11 fuzz: extract the fixed portable base inside the fuzz owner; retain arbitrary
  parser input, binary mutation logic, all 35 byte/control corpus seeds and both
  existing specialized targets. Build/smoke remains implementation work.

Validation performed for the census itself: existing tests/helpers inspected,
tracked-path/Cargo benchmark inventory and runtime/tooling notes reviewed, reload-v1
byte equality, site literal lengths/EOF-newline properties, distinct TextMate grammar
bytes and tracked fuzz corpus counts/bytes checked read-only. No test/build, runtime,
package, grammar-query or browser pass is claimed. Only this census document is
authorized for modification.
