# SF07 — Rust Boundary Test Inputs

Status: selected extraction and owner validation complete; no commit made. The
parent owns staging, the stage commit and shared status documents. This is not
workspace-wide SF12 acceptance or publishable `.crate` verification.

Authority: [plan](../test-organization-plan.md),
[SF00 consumer census](../test-input-census-consumers.md#sf07--rust-boundary-tools).
Baseline revision: `dc6483eb9609d4b0e2b0b1285d11cb73fcffa340`. Initial diff was
clean; peers subsequently edited other owners. Their changes were not modified.
Validation used macOS/aarch64, `rustc 1.99.1 (58cf8f9c3 2026-10-13)` and
`cargo 1.99.1 (5f94df478 2026-08-27)`.

## Concrete Changes And Preserved Boundaries

- `crates/vela_macros/tests/value.rs` now directly includes
  `tests/fixtures/values/enum_round_trip.vela` in
  `derived_enum_round_trips_through_real_vela_match`. Only the source expression
  changed. The fresh Engine, derived registration, codec, calls and assertions
  are unchanged: renamed `total`, Granted count 8/item `gem`, `OwnedValue::Enum`,
  then Pending through the **same** Runtime.
- The fixture is one authored origin, local to the macro package. No loader,
  registry, glob, fallback literal, dependency or new test target was introduced.
  `integration.rs` still owns `mod value`; `autotests = false` is unchanged.
- `crates/vela_macros/tests/fixtures/.gitattributes` scopes `*.vela text eol=lf`
  to this fixture tree. `git check-attr text eol` confirmed `set` / `lf`.
- Bindgen and the generated-consumer crate require **no tracked changes**.
  Their existing source placement and guarded reload already meet the census.
  `OUT_DIR/vela_bindings.rs` remains generated Rust, not an authored source copy.

No production code, manifest, shared documentation, trybuild source/snapshot,
call option, runtime setup, test identity or assertion changed. No global
formatting was run. Existing unbounded call options in these short macro tests
were retained; finite external command deadlines bounded validation. This is
not a new VM-budget claim.

## Whole-Owner Family Review

Paths below are relative to the named crate. Every SF07 census row was reviewed;
no source files were invented for data-only or no-change families.

### `vela_macros`

| Family | Decision and retained proof |
|---|---|
| `tests/value.rs::byte_vector_fields_use_the_runtime_bytes_contract` | Retain inline `make_bytes`: short constructor directly explains `Bytes` / `Array<Bytes>`, empty bytes and nested-vector codec decoding. |
| `derived_value_round_trips_through_real_vela_execution` | Retain inline record `increase`: local field rename `item_name`, count 7, preserved string and exact `OwnedValue::Record` remain adjacent. |
| `derived_enum_round_trips_through_real_vela_match` | Extract the coherent multi-arm program; both calls and independent Rust expectations remain together as described above. |
| `derived_enum_returned_by_rust_native_keeps_match_identity` | Retain inline `count_decision`: native registration is essential local context, nominal matching returns exact i64 11. Not a duplicate of the extracted program. |
| Source-free value tests | Keep Rust structs/enums, schema and codec data: explicit field/variant names, empty record, typed binding identity and complete nested registration closure. |
| `tests/interop_exports.rs::module_registration_installs_through_application_bindings` | Retain one-line `rules::clamp(-7)` and exact 0; separate `VelaBindings` setup is not merged with the other export helper. |
| `decoded_shared_values_and_strings_live_only_for_the_invocation` | Retain adjacent short inspect/transform functions, `3:token!`, transformed count 9 and unchanged Rust original. Invocation-local decoding is not caller borrowing. |
| `external_value_enum_keeps_owned_codecs_registration_and_match_identity` | Retain compact match and its native registration; both Rust conversions and exact `Rare` remain. |
| `exported_value_errors_and_panics_are_converted_and_runtime_remains_usable` | Retain four short entries beside ordered TypeMismatch/panic/reuse assertions. Both panic callable names and a successful normal call after each panic remain on one Runtime. |
| Source-free interop tests | Keep normalized boundary modes, effects, registration bundle count/order, public/private Rust calls and hidden context as unit data. |
| `src/export/signature/tests.rs` | Keep Rust `quote!` signature matrices: async, collection/reference returns, receivers/wrappers, invocation-local arguments and additive effects. These are not Vela programs. |
| `src/export/mod.rs` | Keep token expansion tests for owned/shared backing, panic conversion, absence of host fallback, wrapper rejection and effect restrictions. |
| `src/export_module.rs` | Keep Rust module expansion/public-export/private-helper visibility test. |
| `src/external_value_enum.rs` | Keep Rust token/expansion test for external traits, owned codecs and absence of removed host members. |
| `src/type_identity.rs` | Keep identifier/attribute cases for stable aliases, module requirements and conflicting names. |
| `src/value.rs` | Keep Rust record/enum expansion and rejection matrices for host projections, skipped/generic fields and unsupported variants. Production expansion templates are not test input. |
| `tests/ui.rs` and `tests/ui/**` | Preserve specialized trybuild globs: 1 pass, 22 interop compile-fail, 4 removed-API compile-fail files. All 26 fail sources have exactly paired `.stderr` files; no snapshot regeneration. This is macro UI, not language UI-P/UI-S/UI-C. |

The `interop_exports` helper still constructs a fresh registry/Engine/Runtime;
its module-registration route remains distinct. No mutable setup is cached or
shared across cases. Similar enum matches and names do not justify merging
differently registered programs.

### `vela_bindgen` And `vela_bindgen_compile_test`

| Family | Decision and retained proof |
|---|---|
| `vela_bindgen/src/rust.rs::generation_is_deterministic_and_syntactically_valid` | Retain short inline doc/default-argument/two-function schema. Fresh Engine compilation precedes generation; two outputs/checksums agree, Rust parses, docs/module override remain and async wrappers are absent. |
| `generation_uses_schema_only_and_rejects_unsupported_iterator_boundaries` | Retain small parameter/nested-model table. Source compiles first; generator rejects with `bindgen::unsupported_iterator_boundary` and source location. |
| `generation_emits_owned_models_and_typed_method_receivers` | Retain four concise Point/Choice/method declarations beside Rust parse, typed receiver and exact field-count checks. Not the larger consumer schema. |
| Keyword cases and Rust output templates | Five keyword cases remain Rust unit data; production emission strings are not Vela test inputs. |
| `vela_bindgen_compile_test/script.vela` | Keep existing single file-backed origin, included by `build.rs` and runtime tests. Functions, containers, models/methods and permission-denial entries remain intact and package-local. |
| `generated_binding_re_resolves_after_compatible_body_reload` | Keep `matches("return left + right;").count() == 1` immediately before replacement, then stage, activate and generated-call 42 → 43. No second full program or v2 fixture. This is an explicit bounded delta, not a scale generator. |
| `generated_binding_executes_without_runtime_names_or_manual_values` | Keep generated add 42, label `value`, conversions `(4, 4, 7, Some(9), Ok(10))`, Point `(2, 3)`, sum 5, Choice value 7 and both precise permission-denied checks. Rust values/wrappers are not source extraction candidates. |
| Build/runtime registration | Build-time native stubs and controlled random remain separate from runtime effect-denying callbacks plus Random permission. The public generated-consumer crate remains separate, `publish = false`. |

## Exact Byte Evidence

Before removing the raw Rust literal, a Python byte comparison extracted its
contents between `r#"` and `"#` and asserted equality with the new file. This
literal has no cooked escapes. A later comparison against the baseline Git blob
and isolated source-tree copy also passed. No trimming/reindentation/source
formatting occurred; the 301-byte source starts with LF and ends with `}\n`.

| Input | Bytes | SHA-256 |
|---|---:|---|
| Original enum literal = new fixture = isolated fixture | 301 | `2a487f19093aac3427a0df9f2f4fddb3f326a8c4a5764622a609763347981824` |
| Unchanged generated-consumer `script.vela` = isolated copy | 1148 | `340f2173751f46d84249d5f95bc4631fb8c1af389bb9e7a5e42594011c49d946` |
| Existing single-replacement reload result (not a file) | 1152 | `48f302ce7ee2bbc8544374a1ca69419745a8eba1f3e0ed17f8a0f39041c26c53` |

The original generated script contains exactly one replacement needle. Rust
assertions remain independent of script results. Explicit test registration
consumes the new fixture; there is no new glob requiring add/delete discovery.

## Executed Tests And Checks

All commands below were actually executed. Unless marked isolated, cwd was the
repository root. Each full owner test lane passed **83 Rust tests**, zero failed,
ignored or filtered: bindgen lib **9**, generated-consumer lib **2**, macros lib
**56**, macro integration **8**, interop **6**, UI **2**. The two UI runners also
executed **27 trybuild cases** (1 pass + 26 expected compile failures); these are
not 27 additional libtest identities. No identity was added, renamed or removed.

| Invocation | Bound | Result |
|---|---:|---|
| `cargo test -p vela_macros -p vela_bindgen -p vela_bindgen_compile_test --all-targets --locked -- --test-threads=4` before edits | 240 s | 83 passed + all 27 trybuild cases. |
| Same command after extraction | 240 s | 83 passed + all 27 trybuild cases. |
| Same owner selection with `--all-targets --all-features --locked -- --test-threads=4` | 180 s | 83 passed + all 27 trybuild cases. |
| Same owner selection with `--all-targets --all-features --locked -- --list` | 180 s shared with default Clippy | Nonempty discovery: 9/2/56/8/6/2 = 83 identities, zero benchmarks. |
| `cargo fmt -p vela_macros -p vela_bindgen -p vela_bindgen_compile_test -- --check` | 30 s | Passed; scoped check only. |
| `cargo clippy -p vela_macros -p vela_bindgen -p vela_bindgen_compile_test --all-targets --all-features --locked -- -D warnings` | 180 s | Passed. |
| Same Clippy command without `--all-features` | 180 s shared with discovery | Passed. |
| Isolated `cargo test -p vela_macros -p vela_bindgen -p vela_bindgen_compile_test --all-targets --all-features --offline -- --test-threads=4` | 300 s | Fresh source-tree build, 83 passed + all 27 trybuild cases. |
| Isolated same owner selection with `--all-targets --offline --locked -- --test-threads=4` | 180 s | 83 passed + all 27 trybuild cases. |
| Isolated `cargo test --offline --locked -vv -p vela_macros --test integration value::derived_enum_round_trips_through_real_vela_match -- --exact` after touching only its fixture | 120 s subprocess | 1 passed, 7 filtered; Cargo reported that exact included fixture changed and rebuilt the target without cleaning. |
| Isolated `cargo test --offline --locked -vv -p vela_bindgen_compile_test --lib` after touching only `script.vela`, repeated after warming that exact target | 120 s per subprocess | Both invocations passed 2 tests; warmed probe reported `Dirty vela_bindgen_compile_test` specifically because `script.vela` changed. |

`cargo metadata --no-deps --format-version 1 --offline --locked` confirmed all
three owners declare **no features** (`{}`). Default/all-features owner lanes
therefore do not represent an invented feature matrix; no additional affected
owner feature exists. These scoped commands are not full workspace gates.

### Package Inclusion And Isolated Build

Executed `cargo package -p <owner> --list --allow-dirty --offline` for all three
owners before extraction and again during source-tree assembly (30-second bound
per assembly command). Post-extraction lists contain **77** macro entries,
**6** bindgen entries and **7** generated-consumer entries. Assertions checked
the macro fixture/aggregator/module/UI runner, bindgen source and consumer
`script.vela`/`build.rs`/library are included. Metadata warnings about missing
package description/documentation/homepage/repository were nonfatal.

Actual `cargo package -p <owner> --allow-dirty --no-verify --offline` was also
attempted for each owner (30 seconds each). **All three failed, exit 101**:
version requirements are missing for existing path dependencies, respectively
`vela_common`, `vela_bytecode`, `vela_engine`. `--no-verify` does not bypass
manifest packaging checks. No versions, registry dependencies, manifest changes
or production dependencies were added to manufacture a publishable archive.

Fallback source-package validation used a temporary tree at
`crates/vela_bindgen_compile_test/target/sf07-source-package`. Procedure:

1. Query workspace metadata and recursively collect local dependency declarations
   from the three owners (20 packages, including dev/build edges).
2. For each package, run `cargo package --list --allow-dirty --offline` and copy
   only listed existing files into matching `crates/<name>` directories. The
   three owners contributed 74/3/4 real files respectively; Cargo's synthetic
   `.cargo_vcs_info.json`, `Cargo.lock`, `Cargo.toml.orig` entries are not source
   files and were not fabricated.
3. Copy the root workspace manifest/lockfile. Restrict **only the temporary**
   workspace member list to the closure; retain original manifests, dependency
   paths, profiles and lints. No symlinks or checkout-path substitutions.
4. Build/test from that cwd with its own initially empty target directory using
   the isolated commands above. The fixture and build-time script remain exact
   bytes. `--no-deps` metadata verified all 20 local manifests and 103 local
   dependency edges resolve inside the temporary tree.
5. Touch each selected input without changing bytes and rerun its consuming test
   target. Cargo explicitly detected the included fixture and build script input;
   generated output existed, and build output retained both
   `cargo::rerun-if-changed=build.rs` and `cargo::rerun-if-changed=script.vela`.

This proves owner source inclusion, isolated local-path build/test reachability,
and incremental invalidation—not publication or a normalized `.crate` install.
The temporary tree and build artifacts were removed after validation; they are
not deliverable fixtures or files to stage.

### Failures, Recovery And Limits

- The first isolated all-feature attempt included `--locked` (240-second bound)
  and exited 101 before tests because the reduced workspace required a lockfile
  update. The 300-second offline retry changed **only the temporary lockfile**.
  Its 131 package name/version/source/checksum tuples were verified to be a
  subset of the original 183; no new package version was selected. The later
  isolated default test run passed with `--locked`.
- A Python audit initially failed because system Python 3.9 lacks `tomllib`.
  The lock identity check was redone using the explicit lockfile string fields;
  no Python package was installed.
- Full offline `cargo metadata` of the temporary workspace exited 101 attempting
  to fetch uncached `js-sys v0.3.99`. The local-path audit used `--no-deps`
  successfully instead. Native tests built/passed offline; no cross-target or
  exhaustive registry-metadata success is claimed.
- The first combined invalidation wrapper exited 1 after both Cargo invocations
  passed: it expected a `Dirty` line for a generated-consumer target that was
  instead newly compiled under that narrower dependency selection. Repeating the
  same target after touching `script.vela` passed both tests and the exact
  dirty-input assertion. No product test or production change was needed.
- **No command timed out.** Cargo occasionally waited for peer build/cache locks;
  the fresh isolated UI runner exceeded its informational 60-second notice but
  completed within the finite outer deadline. All actual product-test invocations
  that reached tests passed. Packaging failures remain unresolved as above.

Final scoped whitespace/format checks and byte checks apply to this handoff.
No commit, shared status/design update, global format, broader workspace suite,
site/editor/browser gate or runtime-feature change is claimed.
