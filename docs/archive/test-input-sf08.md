# SF08 — Public Routes And CLI UI Evidence

## Scope And Result

Owner-local SF08 implementation per `docs/test-organization-plan.md` and
`docs/test-input-census-consumers.md`. Writes are limited to `crates/vela_cli`,
`crates/vela_playground_wasm`, `examples`, and this report. No shared status docs,
site/SF11 sources, dependencies, product CLI features, or commits were changed by
this work. Parent owns commits and shared execution/status records. Other owners
were editing shared dependencies concurrently; their work was not reverted.

Implemented: six real-process CLI UI cases with complete stdout/stderr/exact
status, inventory and negative controls, bounded owner-local subprocess helpers,
atomic temporary-directory ownership, schema/path/filesystem dispatch coverage,
selected WASM reward extraction, example package-tree extraction and reload-v1
sharing. All 14 example bins remain exercised.

**Deferred by assignment:** coordinated SF11 actual-site native WASM lane. No
site imports, shadow copies, future SF11 source files, or package-unsafe includes
were added. Actual browser Playground smoke remains separate and unproved.

## CLI Families And Goldens

Sources and twelve output files live in
`crates/vela_cli/tests/fixtures/ui`. `run_scripts.rs` retains the existing two
success and three diagnostic named rows; the old single flag test becomes one
named rstest row. Each row explicitly supplies arguments, environment and typed
`Exit::{Success, ScriptFailure, UsageFailure}` (exact codes 0/1/2). Every child has
an explicit cwd. UI environments are cleared and supply `NO_COLOR=1`, `TERM=dumb`.
Relative logical filenames avoid any temporary-path normalization.

| Discovered identity in `--test run_scripts` | Source / output stem | Independent oracle |
|---|---|---|
| `cli_runs_script::case_1_script_main` | `basic` / `basic` | Code 0; `Scalar(I64(5))\n`; empty stderr. |
| `cli_runs_script::case_2_standard_time_and_random_helpers` | `stdlib` / `stdlib` | Code 0; `Scalar(I64(13))\n`; empty stderr. Existing CLI clock/random setup unchanged. |
| `cli_reports_script_diagnostics::case_1_removed_async_source` | `async_default` / `async_default` | Code 1; full `syntax::removed_async` diagnostic at 2:1; empty stdout; still forbids `rerun with ` plus the backtick-quoted `--async`. |
| `cli_reports_script_diagnostics::case_2_compile_diagnostic` | `invalid` / `invalid` | Code 1; full unresolved-name primary and related label, both 3:12, including `no similar names found`; empty stdout. |
| `cli_reports_script_diagnostics::case_3_runtime_diagnostic` | `runtime_error` / `runtime_error` | Code 1; complete division-by-zero output, primary/runtime labels at 3:12, caller at 7:12, `while executing` helper frame; empty stdout. |
| `cli_rejects_removed_async_flag::case_1_removed_async_flag` | `async_explicit` / `removed_async_flag` | Code 2; complete Clap error/tip/usage/help; empty stdout. Source is preserved but deliberately not parsed. |

Goldens were captured once and reviewed against the old assertions and actual
CLI rendering. Comparison only reads reviewed files; there is no bless mode or
runtime regeneration. It compares bytes, including explicit empty streams,
whitespace, related labels, stack lines and final newlines. The sole platform
branch spells the Clap executable `vela_cli.exe` on Windows; no arbitrary path,
line, ANSI, whitespace or newline stripping is used. Windows was not run here.

Other CLI dispositions:

- `cli_renders_clap_help`: retain inline argument/description/usage unit data;
  still handles both executable spellings and forbids `--async`, now also pins
  code 0 and empty stderr.
- `src/metadata.rs::cli_schema_json_projects_registered_value_metadata_without_host_surface`:
  unchanged unit-data proof of format 4, registered type/field/binding facts and
  forbidden removed metadata. Not a Vela source fixture.
- `cli_print_schema_does_not_compile_or_execute::{case_1_invalid_source,case_2_would_write}`:
  short new sources retained inline. The actual `--print-schema schema.vela`
  process succeeds with a supported parsed `SchemaArtifact` and `fs::write_string`
  metadata, including when compilation would fail; the writing script leaves no
  `executed.txt`. Separate `cli_print_schema_still_requires_positional_script`
  pins code 2 without the positional script. This is not JSON diagnostics.
- `cli_reports_missing_source`: isolated absent relative file, exact code 1,
  empty stdout, source-bearing error, and no created file. OS error wording is
  not promoted to a cross-platform golden.
- `cli_relative_script_uses_script_parent_as_fs_root`: concise IO scenario stays
  inline. From a controlled cwd, `scripts/io.vela` reads the actual script-parent
  input rather than the cwd decoy, writes only the script-parent output, returns
  exact i64 19, observes rejected parent read/write results, and leaves the real
  outside sentinel byte-identical. Exercises existing `with_fs_io`, not a new
  confinement rule or product option.

## Inventory, Negative Controls And Ownership

`tests/support/ui.rs` checks exactly six sources and twelve output files,
including `async_explicit` → `removed_async_flag`. It rejects empty/missing/orphan
inventories and verifies every source has exactly one explicit include in the
case owner. Named registration is intentional: no glob/discovery build script,
registry runner, command DSL, generalized framework or new support crate.

Permanent checks in `support::ui::tests`:

- `ui_inventory_is_nonempty_and_registered`;
- `inventory_rejects_empty_selection`;
- `inventory_rejects_missing_and_orphan_pairs::{case_1_missing_stdout,case_2_missing_stderr,case_3_missing_source,case_4_orphan_output,case_5_unregistered_source}`;
- `comparator_rejects_wrong_status_output_and_extra_diagnostics`: positive control
  and wrong typed 1/2 status, stdout and stderr expectations against actual output.

Live negative experiments also ran exact tests, each returning Cargo **101**:
wrong `basic.stdout` → stdout mismatch; change `invalid.stderr` coordinate 3:12
to 3:13 → stderr mismatch; append an extra diagnostic → stderr mismatch; replace
basic source temporarily with division by zero → expected Success/status mismatch;
remove `basic.stdout` → inventory mismatch. Each mutation was restored in a
`finally` block, then original-source byte equality and passing tests rechecked.

CLI `tests/support/process.rs` and examples `tests/support/process.rs` are
separate owner-local modules. They adapt the existing LSP stdio helper's owned
child, concurrent capped pipe readers and kill/wait guard (LSP read only).
Production process tests allow 30 seconds total including pipe completion and
1 MiB per stream, read at most cap+1, poll caps while the child is alive, and
kill/reap on deadline, excess output or capture errors. Raw success/failure exit
status and output bytes are returned without conversion. A Drop guard covers
unwinding. No shell, unbounded `Command::output`, or CLI test-only command is used.

Both owners discover:
`support::process::tests::bounded_failure_kills_and_reaps::{case_1_deadline,case_2_stdout_cap,case_3_stderr_cap}`,
`captures_both_streams_beyond_pipe_capacity_and_exact_status`, and `process_probe`.
The test executable itself is the child; probes sleep after emitting excessive
output so a passing cap test must terminate/reap a live child. Both-stream proof
captures 128 KiB on each pipe and exact exit 7. Failure probes have a two-second
deadline, verify the specific failure and observable reaped status, and must
return within five seconds. The no-env probe is inert, not ignored coverage.

CLI `support/temp.rs` and examples `src/temp_dir.rs` exclusively acquire roots
using atomic `create_dir`, retry only `AlreadyExists` (128 attempts), and own
cleanup before any setup write. They never claim/remove colliding directories.
Each owns `collision_never_claims_or_removes_foreign_directory`,
`setup_error_and_unwind_remove_only_acquired_directory`, and
`concurrent_owners_have_distinct_roots`. Tests include a foreign sentinel,
exhausted collision selection, retry to a new root, setup IO failure, panic
unwind and simultaneous distinct live owners. Examples share only their own
small temp owner between IO/package demos and process cwd isolation; no Engine
or mutable script state is shared.

## WASM Family Dispositions

All 14 existing identities remain unchanged under `--lib`:

| Family | Disposition / independent retained proof |
|---|---|
| `run_script_returns_json_value::case_2_level_reward` | Extracted to `tests/fixtures/json/level_reward.vela`, exact JSON 40. Original `int` hints, initial newline, indentation and trailing spaces are preserved. |
| `run_script_returns_json_value::case_1_json_array` | Retain compact inline heterogeneous array; exact JSON `["vela", 42]`. |
| `run_script_reports_runtime_error` | Retain short inline division; false `ok`, `vm::division_by_zero`. |
| `compile_script_accepts_dynamic_record_field_access` | Retain inline record/read function; compile route without a main entry. |
| `compile_script_rejects_removed_extern_state` | Retain short inline rejection; `E_PARSE`. |
| `removed_service_call_reports_generic_source_diagnostic` | Retain inline rejection; `hir::removed_service_call` and exact Service message. |
| `compile_and_run_reject_removed_async_and_task_sources` | Retain four inline cases (`async_function`, `await_expression`, `task_spawn`, `task_continuation`) × explicit Compile/Run routes; false `ok`, null value, code/severity/object span checks retained. |

Each invocation still creates a fresh Engine. The 585-byte native reward source
is **not** the site's 343-byte `i64` reward source. Matching JSON 40 does not prove
site byte linkage. Site examples, compile/run entry mapping, package-safe sharing
and actual-site native tests are left to coordinated SF11; browser execution is
not claimed by the native tests or wasm32 build.

## Example Family Dispositions

`runnable_examples` preserves every original named identity and oracle: eight
`example_exact_output` rows and six `example_contains_output` rows. Only bounded
capture and isolated cwd setup changed. Metadata still lists 14 bins.

| Bin / family | Disposition and retained observations |
|---|---|
| `native_function` | Existing adjacent `main.vela`; exact i64 44, native registration. |
| `serde_value` | Existing adjacent `main.vela`; exact actor/applied/score/label/original amount output, serde feature retained. |
| `script_state` | Existing adjacent `main.vela`; exact first/second/name/projected/final_level/final_gold/ticks output. |
| `container_type_hints` | Existing adjacent `main.vela`; exact i64 17. |
| `io_stdlib` | Existing adjacent `main.vela`; IO text remains unit data; exact printed text/length 13/output `done`; atomic temp owner replaces nonexclusive setup. |
| `modules` | Existing `game/{main,config,reward}.vela` tree; runtime `compile_dir`, `game::main::main`, exact i64 16. Isolated source-tree execution also passed. |
| `hot_reload_function_swap` | Existing v1/v2 files, exact accepted report and version fields, old-before 20/old-after 20/new-after 30. Successful bin's v1 remains the authored origin. |
| `hot_reload_function_swap_invalid` | Include the successful sibling's identical v1, delete only redundant copy. Keep logical v1 label and own incompatible update; rejection remains `v0 unchanged`. |
| `package_dependency_demo` | Extract two manifests/two initial modules under adjacent `fixtures/app` and replacement under `fixtures/updates/api.v2.vela`. Fresh writable copies, initial execution before dependency update, ordinary package workspace load/reload and exact 42 → 43 in both unit and process tests. IDs, dependency path `lib`, source roots and logical filenames unchanged. Guard now cleans setup/runtime failures and unwinds, not just success. |
| `bad_schema_duplicate_field` | Existing named file; duplicate `item_id` rejection and source label retained. |
| `generic_type_hint_denied` | Existing named file; exact builtin-only parameterized-hint rejection retained. |
| `random_permission_denied` | Existing named file; `math::random`/`random` capability denial retained. |
| `reflect_schema_mutation_denied` | Existing named file; invalid reflection target, no schema feature added. |
| `reflect_unknown_field_denied` | Existing named file; unknown field `leve` rejection retained. |
| `removed_entrypoints` | Five concise removed Service/Provider/direct/imported-dispatch sources stay inline; ordinary trait/default positive control stays distinct; six identities and fresh Engines unchanged. |

## Exact-Byte Proof

Before replacing literals, evaluated raw/cooked bytes were written and compared
with the original input bytes. Final checks repeated equality against Git's
original Rust literals, not formatted source text: all **12** literal moves plus
the shared v1 matched. No source formatting or trimming was applied. Scoped
fixture `.gitattributes` set LF; `git check-attr text eol` verified all three owner
families. CLI source basenames and diagnostic coordinates are unchanged.

Paths below are relative to the UI directory, WASM JSON directory, or package
fixture directory respectively. SHA-256 values describe exact UTF-8 bytes:

| Input | Bytes | SHA-256 |
|---|---:|---|
| CLI `basic.vela` | 33 | `0bf420f17800dd5ddcf03f0a70d9ec387eed2c42bbbac076b3e0b8f36b003ad9` |
| CLI `stdlib.vela` | 80 | `6b229ebe14a1910ab9a2d7e054b0b86c90421301d1778fe42163f8166179a467` |
| CLI `invalid.vela` | 41 | `31cfd0e357189a24f3ff06fd93b5461154e19b18076c5db17ca167c8a86e347e` |
| CLI `runtime_error.vela` | 72 | `5dd445a0205901a42e226799f5767be529a665bdd64f8131722026032e67fe7a` |
| CLI `async_default.vela` | 36 | `270785ee4c2a6c9576c2a8873268677c17501c81573637a41027ab028625efc0` |
| CLI `async_explicit.vela` | 86 | `721b3f14bda69496c5dccc9888bc3067be5a6d9b7f59e52fbb96ec6c75ad63b8` |
| WASM `level_reward.vela` | 585 | `e481b155b36c0ec63dd1acf3b47ba11251cc5ba03b218edf19276cf36e9d5ed3` |
| Package `app/vela.toml` | 132 | `1119e021186e1ed86c305c5ef79e87457beea202723951264953032206680582` |
| Package `app/src/main.vela` | 56 | `ec57afefb4a694e96da10d03e9fcf191c4e2a873eedf527ba53986e050f2f960` |
| Package `app/lib/vela.toml` | 94 | `9a449fdb706c4c702cd6d7122f134220e536665e625b05817640026e1ff10914` |
| Package `app/lib/src/api.vela` | 31 | `9bcf66598832478792e001c29088e27b90f841b69848cad9dff86f9cebc7100b` |
| Package `updates/api.v2.vela` | 31 | `2172c3f01f3bbbdbe79fd9a2f5d51eadec2a061fca6b184754ba747f57cc9c2f` |
| Shared `hot_reload_function_swap_v1.vela` | 71 | `ed51facccb6ea0c2caee82a17351a3ed44ce8377d9f77c66470f18df7bb1c2f2` |

## Validation And Packaging

All commands had finite external timeouts. Focused baseline commands used 180s;
owner/default/affected lanes 240s; package list/negative checks 120–180s;
isolated builds/tests 600s; formatting/byte audits 10s. No timeout occurred.
No workspace-wide fmt/test/Clippy pass is claimed.

| Command / lane | Result |
|---|---|
| Baseline `cargo test -p vela_cli --test run_scripts` | 7 passed. |
| Baseline `cargo test -p vela_playground_wasm --lib` | 14 passed. |
| Baseline `cargo test --manifest-path examples/Cargo.toml --test runnable_examples --test removed_entrypoints --bin package_dependency_demo` | 14 + 6 + 1 passed. |
| Final focused CLI/WASM commands above | 28 + 14 passed, including negative controls. |
| Final focused examples, adding `--lib` | 19 process/helper + 6 rejection + 1 package + 3 temp tests passed. All 14 bins executed. |
| `cargo test -p vela_cli -p vela_playground_wasm --locked`, repeated with `--all-features` | Each: CLI 1 metadata + 28 integration; WASM 14; no failures/ignored. |
| `cargo test --manifest-path examples/Cargo.toml --locked`, repeated with `--all-features` | Each: 29 tests total across owner targets; all 14 bins, no failures/ignored. Examples' serde dependency feature remains enabled. |
| `cargo clippy -p vela_cli -p vela_playground_wasm --all-targets -- -D warnings`, repeated with `--all-features --locked` | Passed. |
| `cargo clippy --manifest-path examples/Cargo.toml --all-targets -- -D warnings`, repeated with `--all-features --locked` | Passed. |
| `cargo check -p vela_playground_wasm --target wasm32-unknown-unknown --release --locked` | Passed; build proof, not actual browser execution. |
| `-- --list` for CLI `run_scripts`, WASM `--lib`, examples `runnable_examples`; examples Cargo metadata | Nonempty 28 / 14 / 19 identities; 14 bin targets. CLI flag identity intentionally gains `::case_1_removed_async_flag`; all original WASM/example rows retained. |
| Explicit changed-file `rustfmt --edition 2024 --config skip_children=true --check` | Passed, including each support module. An earlier multi-root recursive check flagged temp-helper formatting; corrected owner file and reran the explicit nonrecursive check. No workspace fmt. |
| Scoped `git diff --check`, input byte equality, scoped LF attributes | Passed. |

`cargo package --list --allow-dirty` succeeded for CLI, WASM and examples with
31 / 7 / 50 entries respectively, including fixtures, empty goldens, support
modules, shared reload v1 and runtime modules tree. Actual `cargo package
--allow-dirty --no-verify --offline` failed for each owner because existing
`vela_bytecode` path dependencies lack a version requirement. This is an existing
publication-topology limitation; no dependency versions/manifests were changed,
and no distributable `.crate` archive pass is claimed.

An isolated source topology was materialized under ignored
`examples/target/sf08-package-proof`: affected owners copied **only actual source
files in their Cargo package lists**, with their original manifests, workspace
manifest/lock/config and local dependency source topology. Generated package
metadata/normalized manifests were not invented. Owner bytes were verified and
refreshed after negative mutations were restored. Separate fresh build directories
prevented using a previously built checkout executable. No symlink, site include
or fallback fixture path was used. Both commands passed with `--locked --offline`:

```sh
CARGO_TARGET_DIR=examples/target/sf08-package-proof/build cargo test --manifest-path examples/target/sf08-package-proof/Cargo.toml -p vela_cli -p vela_playground_wasm --locked --offline
CARGO_TARGET_DIR=examples/target/sf08-package-proof/build-examples cargo test --manifest-path examples/target/sf08-package-proof/examples/Cargo.toml --locked --offline
```

The temporary topology was removed at SF12 after the workspace unsafe audit found
its copied Rust sources under the audited examples tree. Future disposable source
proofs belong under root `target/`; the audit itself was not weakened.

Isolated results: CLI 29, WASM 14, examples 29 including all 14 process bins and
runtime-loaded modules from the isolated manifest-relative tree. This proves
package-list source reachability with the repository's path-dependency topology,
not registry publication. Shared docs/status, coordinated actual-site SF11 and
browser smoke remain parent/other-owner follow-ups.
