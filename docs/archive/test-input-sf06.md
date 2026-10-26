# SF06 — Engine and reload input organization

## Result and scope

SF06 is implemented against `dc6483eb9` (SF00–05), without a commit. This stage
writes only Engine/Hot Reload test code and fixtures, excluding benches, plus
this report. No runtime feature, production API, dependency, Cargo target, codec
format, shared root fixture, or shared status/design document was added/changed.
Concurrent SF07/public-route/tooling work appeared during validation and is not
part of this report or this stage's acceptance. Parent owns shared status and
commits.

- **74 literal sites -> 64 authored source files**, with exact evaluated-byte
  checks, not dedenting or Vela reformatting. Includes the **20,677-byte** standard
  reflection program and all four identical **187-byte** Counter consumers.
- **10 evaluated finite helper variants** replace sizeable fixed templates:
  stable rename (2), trait implementation/reward (4), trait/inherent bonus (4).
- **27 package manifest/source/update files** preserve actual filesystem trees,
  initial/rejected/accepted versions, app `main` versus dependency `api`, and
  native effect reduction/expansion. Tests copy trees into fresh roots.
- **101 new fixture files total**: 88 Engine, 13 Hot Reload. All have consumers.
  Fixture count is an accounting result, not an extraction quota.
- **2,399 pre-existing assertion macros remain in order; all 578 existing
  test/rstest functions remain present** across the two owners, excluding
  benches. No tests were merged or renamed. Four default-feature formatter
  cases and six feature-gated portable cases are added to the existing public
  `integration` target; the original portable smoke remains.

## Complete census family dispositions

Paths in this table are relative to `crates/vela_engine/` unless marked Hot
Reload. Review included raw/escaped strings, cases, includes, format/replace
factories and filesystem writes, not only large raw literals. The byte tables
below name every moved literal; retained inputs do not have a zero-inline goal.

| Census family | Final disposition and retained proof |
|---|---|
| `src/tests/reflection/metadata.rs` | **File-backed** standard metadata catalogue and tuple descriptor query. All independent registry/type/method/parameter/docs checks before compilation, the entire existing boolean script contract, and the original runtime oracle remain. **Inline** the two private/native visibility probes (205/233 bytes): each checks one registration's visibility, not a catalogue. Rust descriptors/docs/attributes remain **unit-data**. |
| `src/tests/reflection/{reflect_calls,reflection_natives,permissions}.rs` | **File-backed** reflected native iterator loop. **Inline** the four short callable/typed-argument/call-permission/non-callable probes and two local metadata-list permission probes (279/196 bytes). Their registration/policy contrast is clearer beside the result/error; no common runner or host context is shared. `permissions.rs` is empty at this baseline, not an unreviewed source family. |
| `tests/{script_reflection,reflection_configuration}.rs` | **File-backed** enum metadata program and complete `game/{main,reward}` reflection module documents, still written to a fresh temp root and compiled through `compile_dir`. Logical paths and grant docs/event/default metadata unchanged. **Inline** configuration setter-order/budget/disabled-registration probes. |
| `src/tests/native/compiler_options.rs` | **File-backed** ten combined string transform/option/split/parse, indexed-shape, registry-value, collection, array lookup/transform and standard-native programs. **Shared-input** the existing three root named-argument fixtures, unmodified. **Inline** the 11 local native/signature/predicate/argument-transform/range/Option-Result/named-method controls, including the synchronous native baseline returning I64 5. Method IDs, unresolved-name rejection, exact result/error tags and SourceIds remain in Rust. |
| `src/tests/strings.rs` | **Retain-inline** all three previously reviewed SF01 cases: small string predicate/transformation inputs with immediate scalar/string oracles. No cosmetic move or new string-fixture pool. |
| `src/tests/native/budgets_and_permissions.rs`, `src/tests/context.rs` | **Inline** three budget/depth/pure-native probes and six clock/capability/reflected-clock probes. The longest context probe merely calls two configured clocks; extraction would hide its local registration/permission relation. Execution options, exact limits, host services and permission errors unchanged. |
| `src/tests/{io,random}.rs` | **File-backed** combined stdio/filesystem program, direct/reflected seeded-random programs and substantial random metadata query. **Inline** read-denial, sandbox read/write (two operations), parent escape and one-call random denial. Temp host roots, exact output-file content, seeds, host execution context, bounds and metadata assertions unchanged. |
| `src/tests/{typed_native,type_binding,script_identity}.rs` | **File-backed** full script identity schema (record/enum IDs, payload and entry); logical `game::reward`, anonymous package, SourceId 81 and independent linked/reflected identity comparisons stay local. **Inline/unit-data** typed/native conversion registration, exact scalar/container tags and short native calls. The largest optional-value probe is four closely coupled native calls (199 bytes), not a separate source tree. No Rust value-conversion or ABI table is turned into source data. |
| `src/tests/runtime_args.rs` | **File-backed** trait-based program-image execution and both named/cached method/affinity programs. **Inline/unit-data** tiny positional/named/duplicate/mixed-mode/entry-affinity controls and Send checks. Runtime identity, method binding and argument errors remain explicit. |
| `src/tests/runtime_args/managed_values.rs` | **File-backed** persistent/nested/shared-image records, recursive contracts, serde record, managed/returned/retained record programs. **Inline** single-cell contract mismatches, small numeric/string map-key serde controls and small cross-Runtime record rejection. Keep all pre-replacement validation, rejected-value preservation, Rust updates, serde-only decode/no-write-back, explicit retained clones, collection trigger and exact cross-Runtime rejection. |
| `src/tests/runtime_args/state_reclamation.rs` | **Inline**, unchanged small versions. Full endpoint proof retained: removed closure-valued state cannot self-pin; two generations and removed state survive while an old closure lives; explicit drop plus safe point reduces the generation count to one and reclaims the removed state ID. These are not new tests or missing coverage. |
| `src/runtime/{tests,reentry_tests}.rs` | **File-backed** all four multi-function re-entry programs, aggregate-category initialization and alias/cycle reload initializer. **Inline** compact initializer transaction/depth/budget pairs, shared-image owner controls and tiny reload additions. Keep parent roots, retained/cloned child result, inherited depth failure then successful reuse (9), initializer rollback/alias identity, shared execution-data/plan pointers versus separate heaps/roots/IDs, cross-Runtime rejection and independent state increments. |
| `src/runtime/{control_tests,bytecode_profile_tests}.rs` | **Inline/unit-data** cancellation/deadline/host-call errors, reuse and small profiling loops; the 186-byte two-function scalar profile probe remains next to its exact subpoint/charge oracle. **Generated** bounded 256-helper profile scale. No generated large fixed program is used to evade extraction. |
| `src/runtime/inline_cache_{core,native,method,hot_reload}_tests.rs` | **Shared-input** Counter used at four sites; **file-backed** its accepted update and public/rejected ABI pair. Other compact field/state/native/dynamic/callback/iterator and old/new closure pairs remain **inline**, with typed injected stale entries and cold/hot counters as **unit-data**. Preserve exact rejected execution-data pointer, immutable dispatch, old-generation cache/profile routing and weak-owner final reclamation, not just output equality. |
| `src/runtime/{image,state}.rs`, `src/source.rs`, `src/engine/metadata.rs` | **Inline/unit-data** mixed-file image/state bookkeeping and tiny file-loader/metadata controls; **generated** bounded repeated state/entry scale in `state.rs`. Inspected separately from raw-string test modules. Production error/help text in initialization/source/error/validation is **not-test-input**. |
| `src/tests/source_reload/{dir_basic,dir_function_abi,dir_schema_trait_abi,changed_file_functions,changed_file_native_method,changed_file_schema_trait}.rs` | **File-backed/shared-input** via the reviewed module writers below for substantial stable-schema and trait generations. Remaining **inline/unit-data/generated** tiny body/signature/schema-field/enum-field/trait-method/native-effect/event variants retain their actual directory versus changed-file routes. Deterministic loading, source failures versus deferred ABI rejection, before-stage/before-activation/after-rejection values, reports and removed declarations remain distinct. No test-local-string-only exemption for helper inputs. |
| `src/tests/source_reload/{source_file_runtime,source_file_native_method,source_file_schema_trait,source_file_tuple_abi}.rs` | **Shared-input** exact stable-member rename pair with Hot Reload; canonical bytes live Engine-locally because shared-root writes were excluded. **Inline** compact signature/default/schema/enum/trait/event/tuple/source/IO pairs. In particular 135/166-byte trait presence pairs stay adjacent; they are not the byte-identical public reward-module templates. Stage timing, exact repair/error/report fields, no destination version and callable old code remain unchanged. |
| `src/tests/source_reload/{runtime_safe_points,runtime_rejection_policy,source_diagnostics}.rs` | **File-backed** both substantial state-record generations. **Inline** short scalar/transitive-initializer/public-state/failed-addition/rejection/safe-point pairs and duplicate-ID diagnostics. Preserve no-reinitialization, added state initialization, transactional failure, old state/image and new Runtime isolation, event-end nested calls, and immediate source versus staged policy errors. |
| `src/tests/source_reload/fixtures.rs` | **Shared-input** identical main module at all five schema/rename/enum/trait writers. **File-backed** enumerated stable-rename `(2,false)/(6,true)` and trait `(2,false)/(2,true)/(6,false)/(6,true)` documents. Exhaustive matches reject unreviewed helper combinations. **Inline/generated** tiny reward/native wrappers and count/return/required/defaulted method fragments; variable dimensions are visible in Rust. Existing filesystem writers and `game` topology stay. |
| `src/tests/source_reload/{host_method_and_source_helpers,function_abi_helpers,report_helpers,mod}.rs` | **Inline/generated/unit-data** concise signature/default/helper/event permutations and typed ABI builders; **retain owner-local orchestration** for directory/changed-file branches, removed-function before/stage/reject observations and repair hints. No new runner or global cache. |
| `src/source/package/tests.rs` | **File-backed** ordinary public-function/type/transitive/dependency-reload trees and their rejected/update sources. Private visibility and impacted-package tests share the public tree and overwrite only their distinct dependency before loading. **Inline** minimal root-request/snapshot/manifest/capability negatives and bounded package loop. **Generated** tiny value-only helper/callback/invoke closure substitution remains local. Package ID, alias visibility, changed/impacted sets, budget, prior-root replay and accepted/rejected execution unchanged. |
| `src/source/package/tests/native_capabilities.rs` | **File-backed** complete native write-to-read and read-to-write initial trees plus explicit reduction/rejected expansion documents. Reduction equals exactly one original replacement, no rewritten closure/state body. **Inline/generated** one-call declaration/grant/observed-effect matrix and small `VALUE_SOURCE`; host effect descriptors/counters remain Rust **unit-data**. Preserve reads/writes 3/2 through old closure, state 3, later result 14 with writes still 2, and zero writes on rejected expansion. |
| `tests/hot_reload.rs` | **File-backed** six native-closure/rejected, reflection and trait-dispatch generations. Existing root SF01 retained-closure layout v1/v2 remain canonical and unchanged. **Inline** short stage/re-entry/removed-state versions, including active synchronous old-owner completion. Old scalar plans, generation inequality, budget rejection, exact owner pointer after rejection, native dispatch and old removed state remain intact. |
| `tests/{reload_base,prelude,portable_artifact,owned_export,args,removed_borrowing,removed_declarations,removed_execution}.rs` | **Inline/unit-data** short stale/replay/removal/prelude/export/owned conversion controls; the 4–5-line trait/removed-provider source continuations are intentional local positive/negative pairs. Cyclic export rejection/reuse, exact tags, public owned boundary and removed-entrypoint rejection stay. Original portable smoke unchanged; new selected execution coverage described below. |
| `src/{binding,compiler_registry,native,registration,type_registration}.rs`, `src/interop/tests.rs`, `src/standard/{bindings,functions,types}.rs`, `src/tests/validation/{host_method_validation,module_type_validation,native_function_validation}.rs` | **Unit-data/inline** independent native/ABI/descriptor/conversion validation and tiny compile controls. **File-backed** the multi-entry Option/Result and collection binding programs found in the mixed `standard/bindings.rs` file. Keep exhaustive binding checks, guards, permissions, active-binding re-entry and short primitive/tuple/map functions. No production strings or registered names misclassified as programs. |
| Hot Reload `src/tests/{function_abi,function_policy,method_abi,schema_abi,trait_module_abi,state_abi,registry_manifest,package_abi}.rs` | **Shared-input** stable-member pair from Engine. **File-backed** recursive initializer, nested closure/recursive closure/default closure generations, including the unrelated-helper version; identical nested-closure initial bytes share one file. **Inline/unit-data** compact exact-contract/order/default/effect/access/export/version pairs and 164-byte transitive/unrelated helper comparisons. `package_abi` remains typed metadata construction, not filesystem package loading. |
| Hot Reload `src/tests/runtime_reports.rs` | Existing root ABI v1/v2/report files remain **file-backed**, untouched. **File-backed** both finite trait/inherent bonus variants; Rust retains SourceId 1, anonymous package and `game::main`. Tiny reward graph retains SourceIds 1/2 and both module paths **inline/generated**. Image/profile/state/MIR/layout/Arc/report/staging/lifetime checks stay typed and ordered. No migration into Engine or a diagnostic framework. |

### Lifecycle endpoints and assertion preservation

The extraction did not replace sequences with output-only rows. Named endpoints
validated in both feature lanes include:

- `removed_closure_state_does_not_pin_its_own_generation` and
  `reload_safe_point_reclaims_removed_state_after_final_old_owner_drops`:
  retained counts 2 then 1, removed-state ID lifetime, explicit drop/safe point.
- `retained_old_closure_uses_exact_cache_profile_and_reclaims_execution_data`:
  old cache populated, new cache untouched, old profile advances, weak owner
  remains before the safe point and cannot upgrade afterward.
- `runtime_initializes_vm_state_once_and_shared_programs_remain_isolated`,
  `shared_actor_runtimes_share_execution_data_but_isolate_mutable_owners`,
  `shared_runtime_image_keeps_vm_states_isolated`, and selected-plan scale:
  shared immutable execution data versus separate roots/heaps/IDs/state.
- All four re-entry tests: same-session nested calls, GC roots, retained clone,
  depth unwind and reusable result; all public retained-closure and active-call
  tests retain their source-versus-active-generation distinctions.
- Package reduction/expansion: unchanged declared permissions versus observed
  native effects, old closure's original write, exact effect counters and
  rejected image identity. Compatible/failed initializer transaction scenarios
  retain pre/post state and publication assertions.

A one-shot Rust-token audit compared every old assertion macro in order, retaining
literal contents while ignoring formatting/comments/optional trailing commas,
and checked every old test/rstest function identity. It is complementary to the
literal reverse-substitution proof and passing lifecycle suites, not a new test
framework. Temporary migration/audit scripts are not part of the handoff.

## Added execution equivalence

`tests/formatting_execution.rs` uses the existing dev dependency
`vela_language_service` and public `document_formatting`, validates its whole-file
edit range and idempotence, then independently compiles/runs original and formatted
source in fresh Engines/Runtimes. There is no syntax/runtime dependency inversion
or new formatter API. All four actual SF02 root sources are included, unchanged:

| Source | Independent typed Engine oracle, both routes |
|---|---|
| `assignments/local_operators.vela` | `main()` -> I64 20 |
| `assignments/index_reads.vela` | `main()` -> I64 10 |
| `control_flow/block_tail.vela` | `terminated()` -> Unit; `nonterminated()` -> I64 9; `defaulted()` -> Unit |
| `control_flow/break_continue.vela` | `main()` -> I64 8 |

`tests/portable_artifact.rs`, gated by `artifact-codec`, includes the actual four
SF04 bytecode `tests/fixtures/compiler/` authored origins, without copying them:

| Source | Independent typed Engine oracle, source and decoded routes |
|---|---|
| `nested_captures.vela` | I64 17 |
| `scalar_loop.vela` | I64 11940 |
| `typed_record_write.vela` | I64 5 |
| `named_default_method.vela` | I64 21 |

Each source is compiled independently for the direct and encoding routes. The
latter encodes/decodes a `PortableProgramArtifact`, links into a **fresh receiving
Engine**, and executes without receiving-side source compilation. Both Runtimes
first reject a zero-unit call with exact `BudgetExceeded { ExecutionUnits, limit:
0 }`, then succeed twice with the independent exact I64 oracle. Empty arguments,
100,000 units, 1 MiB memory and depth 32 are identical. This proves execution,
budget rejection and reuse, not just artifact structural equality. Format 8 is
unchanged; no package/MIR/generation format expansion is involved.

Two additional named cases use one small inline `test::record(7)` program with
independently registered source/receiving host-write natives. Each route starts
with zero host calls; grant returns I64 8 and exactly one call; denial returns
exact `PermissionDenied { native: "test::record", capability: "host_write" }`
and zero calls. Native arguments are independently asserted to be exactly I64 7.
These are real effect/error oracles, not source-versus-decoded equality alone,
and do not alter the four selected SF04 inputs.

## Validation and reachability

Commands ran with finite tool bounds; no timeout occurred. These are owner gates,
not whole-workspace/benchmark performance/browser/editor acceptance.

| Command | Bound | Result |
|---|---:|---|
| `cargo test -p vela_engine -p vela_hot_reload --locked -- --test-threads=4` before editing | 240 s | Passed retained baseline. |
| Same default command after edits | 240 s | Passed: Engine lib 438, integration 57, Hot Reload lib 96, Engine docs 26, Hot Reload docs 3 (**620**). |
| `cargo test -p vela_engine -p vela_hot_reload --all-features --locked -- --test-threads=4` | 240 s | Passed: 443 + 64 + 96 + 26 + 3 (**632**). |
| `cargo test -p vela_engine --test integration --all-features --locked formatting_execution -- --test-threads=4` | 240 s combined focused bound | **4/4** named source cases passed; six entry/value oracles per route. |
| Same integration selection `portable_artifact` | same bound | **7/7** passed: original smoke, four SF04 cases, granted/denied native effect cases. |
| `cargo test -p vela_engine --test integration --features artifact-codec --locked portable_artifact -- --test-threads=4` | 120 s | **7/7** passed with only the codec feature selected, independently of serde/schema-artifact. |
| Owner `cargo test ... -- --list`, default and all features | 120 s each; summary replay 200 s total | Nonempty discovery: **620/632** including doc tests. Old identities retained; +4 default/+10 all-feature compared with baseline definitions. |
| `cargo clippy -p vela_engine -p vela_hot_reload --all-targets --locked -- -D warnings` | 240 s | Passed. |
| Same scoped Clippy with `--all-features` | 240 s | Passed. |
| `cargo fmt -p vela_engine -p vela_hot_reload -- --check` | 60 s | Passed after formatting changed Rust files; no workspace fmt. |
| Scoped `git diff --check` | 10 s | Passed. |
| One-shot byte/assertion/consumer audit | 10 s | 74 literal sites rechecked against HEAD; 2,399 old assertion macros and 578 old functions preserved; 101 fixture consumers; every include resolves. Final hash replay verifies all **111** literal/helper/package proof rows against the 101 files. |
| `cargo package -p <owner> --list --allow-dirty --offline --locked` for both owners | 45 s each, 100 s enclosing bound | All **88/13** owner-local fixtures listed; only pre-existing missing package-description/homepage metadata warnings. |

Initial focused compilation caught a test construction error: calling `.clone()`
on `CompiledProgram` selected its dereferenced `UnlinkedProgram` clone, which the
portable API does not accept. Fixed by compiling independent direct/encoding
inputs (no API change); both focused tests and both full lanes subsequently
passed. Initial fmt check found expected new-code formatting differences and was
fixed. The first whole-tree write audit noticed concurrent other-owner edits;
the final scope/consumer audit excludes those changes rather than attributing or
reverting them. No failing test was deleted, assertion weakened, or limit raised.

Fixtures use explicit `include_str!` consumers, or existing filesystem routes
with a small package-local copy helper. This is not a glob-discovered UI corpus:
there are no source/golden pairs or new generic reader to validate. Missing
included files invalidate compilation; package trees are read anew on every run.
The one-shot consumer audit found no orphan fixtures. Relative root/SF04 and
Engine-to-Hot-Reload includes are **repository test sharing**, not a claim that
an isolated published crate tarball can run cross-owner tests. Cargo listing
proves owner-local packaging, and actual default/all-feature builds prove these
repository-relative includes. No production include uses repository test data,
no distribution copy/symlink was introduced, and both existing SF01 root files
remain unchanged. No benchmark writes or runs, new UI lane, workspace gate,
real-browser execution or installed-editor evidence is claimed.

## Scoped LF attributes — independent-review follow-up

Added only these fixture-root metadata files; no repository-wide attribute rule:

- `crates/vela_engine/tests/fixtures/.gitattributes`: `*.vela text eol=lf`
  and `*.toml text eol=lf`.
- `crates/vela_hot_reload/tests/fixtures/.gitattributes`: `*.vela text eol=lf`.

The patterns cover the actual authored extensions (Engine: 77 Vela sources and
11 TOML manifests; Hot Reload: 13 Vela sources), not arbitrary/binary fixture
files. Before and after adding attributes, all **111** recorded length/SHA-256
rows matched the same **101** authored files. None contained CR bytes. No source
or manifest was rewritten, renormalized, trimmed, or given a different final
newline; byte-sensitive authored whitespace and escapes remain intact. The two
attribute files are metadata, not additional authored programs/package inputs.

`git check-attr --stdin text eol`, supplied every authored fixture path, reports
**text: set / eol: lf for all 101 files** (202 attribute results). Explicit
`git check-attr text eol --` checks also confirmed the large reflection program,
`packages/public_function/vela.toml`, and Hot Reload's trait-method v1 source.
The attribute and hash audits each ran within a 10-second bound.

Inspected `copy_package_fixture` and enumerated all six actual named copy roots:
`dependency_reload` (4 files), `native_read_to_write` (2),
`native_write_to_read` (2), `public_function` (4), `public_type` (4), and
`transitive` (6). They remain strictly below `tests/fixtures/packages/`; the new
Engine `.gitattributes` is above, outside every copied tree. All 22 copied files
remain the original `.vela`/`.toml` inputs, with no attribute metadata. The Rust
helper and its call sites are unchanged.

Fresh validation: `cargo test -p vela_engine --lib --locked
source::package::tests -- --test-threads=4` passed **24/24** tests within a
120-second bound, including native capability reloads. Scoped `git diff --check`
passed. No Rust changes required formatting/Clippy or a full-suite rerun for this
metadata-only follow-up. Writes are limited to the two attributes and this
report; no benches or commits.

## Literal byte proof

Base: `dc6483eb9`. The one-shot migration compared each extracted UTF-8 payload
byte-for-byte and reversed every include substitution to recover the complete
original Rust file. Leading/trailing newlines, indentation, escapes, and logical
source IDs are unchanged. This table uses original HEAD line numbers; paths are
repository-relative. SHA-256 hashes describe evaluated source bytes, not Rust
literal spellings. Helper/package materialization proof is recorded separately.

| Original source:line | Authored fixture | Bytes | SHA-256 |
|---|---|---:|---|
| `crates/vela_engine/src/tests/reflection/metadata.rs:203` | `crates/vela_engine/tests/fixtures/reflection/tuple_descriptors.vela` | 1420 | `bda0058938dc768135886e54c098f458f93e89b6974b6a67cafe061a172003db` |
| `crates/vela_engine/src/tests/reflection/metadata.rs:621` | `crates/vela_engine/tests/fixtures/reflection/standard_metadata.vela` | 20677 | `9fa4c8badf2fd80964cde705ba85d8522640fd0de8e8673b7cc816b43d1988a6` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:274` | `crates/vela_engine/tests/fixtures/native/string_transforms.vela` | 248 | `33bce670323d4092d966da9ac573611815cd12b3a6e555f323c167e71232fa72` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:352` | `crates/vela_engine/tests/fixtures/native/string_options.vela` | 223 | `fee9d5ab966bb8ecd24bcfa1856bc515e1b09affcdee31b862e361972dc698df` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:395` | `crates/vela_engine/tests/fixtures/native/string_splits.vela` | 266 | `c11d1422a319775e6ff7b55668bc7efdf0c3c9d2426e4c2376ab008137f42a72` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:438` | `crates/vela_engine/tests/fixtures/native/indexed_collection_shapes.vela` | 504 | `5b1acdcf2a4900abac0fccba77899782f34c296a5ba3920ba13ae9557a9ed6b4` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:471` | `crates/vela_engine/tests/fixtures/native/string_parsing.vela` | 321 | `7c8f6d2af972b76333aab82e4d4b268bf828a5d5013629b5fb4389bca8e686e7` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:579` | `crates/vela_engine/tests/fixtures/native/registry_value_methods.vela` | 415 | `92bba8db901e7456d0827e35c873ec0dbaab819e97c5bfd9a6c770dd708f189e` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:629` | `crates/vela_engine/tests/fixtures/native/collection_methods.vela` | 731 | `55dc1e9d7459d73bafb9ab3e2b0ea3f95108a385220f176f8f38e4a0eddbcd7c` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:703` | `crates/vela_engine/tests/fixtures/native/array_lookup.vela` | 247 | `cf5e2da12045b96f75f071ce9e7bb3b32c361ddf63d86519ecd0a0a06bd45f15` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:742` | `crates/vela_engine/tests/fixtures/native/array_transforms.vela` | 227 | `b4db57a618a1d538f4e52542b27f4be1fa6e812915dac67332ba6f4a31ef342d` |
| `crates/vela_engine/src/tests/native/compiler_options.rs:876` | `crates/vela_engine/tests/fixtures/native/standard_natives.vela` | 467 | `0ffbcbc8f7ef61abbca7c5a5abc8337178eb8d4cfa93edbe0726be4f80e282fa` |
| `crates/vela_engine/src/tests/random.rs:67` | `crates/vela_engine/tests/fixtures/random/seeded_bounds.vela` | 213 | `784e66e6967de1c5a0cc9d49eee11d21d577612b6514931b0a95127a94ee25a9` |
| `crates/vela_engine/src/tests/random.rs:177` | `crates/vela_engine/tests/fixtures/random/standard_metadata.vela` | 752 | `f342732e6a388603ca21d05339abdb8902ee3dd177aee1e7f23d685f052eae8f` |
| `crates/vela_engine/src/tests/random.rs:209` | `crates/vela_engine/tests/fixtures/random/reflected_seeded_bounds.vela` | 283 | `5e3b5bd3fba6cfde1ab937cd3801d3bce0e65002e24f5dd56e44b304b0d26236` |
| `crates/vela_engine/src/tests/io.rs:100` | `crates/vela_engine/tests/fixtures/io/stdio_and_files.vela` | 215 | `2ad8246cbb8d7fd1aaa98d9a523e0f6f7298f0dfa1b15124c7b449dd529b9f6a` |
| `crates/vela_engine/src/tests/runtime_args.rs:34` | `crates/vela_engine/tests/fixtures/state/program_image.vela` | 333 | `b2ddd70d35201aa4115305a30c9b1be7bce8e7a8bef609ab8a25caf590213252` |
| `crates/vela_engine/src/tests/runtime_args.rs:282` | `crates/vela_engine/tests/fixtures/state/cached_methods.vela` | 300 | `a653eee3ca059088eabc10d07a40a1b8b1e2217b0cce4652017fdbc460bf1aa4` |
| `crates/vela_engine/src/tests/runtime_args.rs:355` | `crates/vela_engine/tests/fixtures/state/method_affinity.vela` | 262 | `8dee904f17fbe9bae2e1377b1a72355b4070dfbc6f57a17628bccadbd2c17b38` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:22` | `crates/vela_engine/tests/fixtures/state/persistent_record.vela` | 309 | `64acecedfac3e8ede7ec8feaba8da3cdf2555ad89f5231de1609f6d831f5c0d4` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:129` | `crates/vela_engine/tests/fixtures/state/nested_record.vela` | 646 | `218fe478a2779639dfc5b5b25569502f38809e145b88f1d5a075f3745b1ddd61` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:173` | `crates/vela_engine/tests/fixtures/state/shared_image_records.vela` | 322 | `38e15d70d1f3ded67fe9be52683f3ae9725b331e71ded132e19e648dd1f7779d` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:355` | `crates/vela_engine/tests/fixtures/state/recursive_contracts.vela` | 568 | `a7c13cec7fd8bc13f1cc0b4c8fb245f6eba4b1e08fe78af7ce2f933d1085ed1d` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:522` | `crates/vela_engine/tests/fixtures/state/serde_record.vela` | 252 | `3de547bfa0bce6ea9455491cb3d08d069ed01a5330c6bed8b9d2baf5a665cb8e` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:702` | `crates/vela_engine/tests/fixtures/state/managed_record.vela` | 243 | `6a3c87f080e844a2c0f60c864c73acae03c6d606b1f2591c77e4e7fdc8868c1d` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:744` | `crates/vela_engine/tests/fixtures/state/returned_record.vela` | 195 | `25d05fbd7b360bb99d2dcf433d8fce57002720b3148fbcc6d59238f72509a296` |
| `crates/vela_engine/src/tests/runtime_args/managed_values.rs:796` | `crates/vela_engine/tests/fixtures/state/retained_record.vela` | 251 | `eb24cc591f02c478b3adf1f71f494a92c1dce11dc9c5111e06a056253c5e335d` |
| `crates/vela_engine/src/runtime/reentry_tests.rs:40` | `crates/vela_engine/tests/fixtures/reentry/nested_native.vela` | 258 | `4ef796487b431666070b7a05d7e375ca9581b8244a07481584e11113707c14c2` |
| `crates/vela_engine/src/runtime/reentry_tests.rs:84` | `crates/vela_engine/tests/fixtures/reentry/parent_roots.vela` | 232 | `7e901c1329b5a8263ec7a8b7cce0c6ee1b73818b58d3f4a2a2adf5910466ce8e` |
| `crates/vela_engine/src/runtime/reentry_tests.rs:142` | `crates/vela_engine/tests/fixtures/reentry/retained_result.vela` | 354 | `7e4735f2fca7c51e9c516ace5a179024cf4736d45c4e03e7ed17411dab1b7560` |
| `crates/vela_engine/src/runtime/reentry_tests.rs:193` | `crates/vela_engine/tests/fixtures/reentry/depth_unwind.vela` | 167 | `c9df58d995a88302d62c9d6d7b7b77b347d39d23e117903ec270cef5ac7a2014` |
| `crates/vela_engine/src/runtime/tests.rs:231` | `crates/vela_engine/tests/fixtures/initialization/aggregate_categories.vela` | 411 | `8646578a68b63d99bffa39045a5a54d92c950dd3152455520845e6112726b5bc` |
| `crates/vela_engine/src/runtime/tests.rs:429` | `crates/vela_engine/tests/fixtures/initialization/aliases_and_cycles.vela` | 293 | `7b130980f837cde1111ab6ea144483e5fff0b6f0afcf418b28b9df28a709e02d` |
| `crates/vela_engine/src/runtime/inline_cache_method_tests.rs:23` | `crates/vela_engine/tests/fixtures/cache/counter_method.vela` | 187 | `5c9010c0f459124e0c7c591ee304919a7258c2a810f8fe9dadc845850e2fc092` |
| `crates/vela_engine/src/runtime/inline_cache_method_tests.rs:74` | `crates/vela_engine/tests/fixtures/cache/counter_method.vela` | 187 | `5c9010c0f459124e0c7c591ee304919a7258c2a810f8fe9dadc845850e2fc092` |
| `crates/vela_engine/src/runtime/inline_cache_method_tests.rs:130` | `crates/vela_engine/tests/fixtures/cache/counter_method.vela` | 187 | `5c9010c0f459124e0c7c591ee304919a7258c2a810f8fe9dadc845850e2fc092` |
| `crates/vela_engine/src/runtime/inline_cache_method_tests.rs:195` | `crates/vela_engine/tests/fixtures/cache/counter_method.vela` | 187 | `5c9010c0f459124e0c7c591ee304919a7258c2a810f8fe9dadc845850e2fc092` |
| `crates/vela_engine/src/runtime/inline_cache_method_tests.rs:233` | `crates/vela_engine/tests/fixtures/cache/counter_method_updated.vela` | 187 | `bfa6fe052ca06b3ab10a7c962a7338f97a19ede20192e510c0a91614b1205f90` |
| `crates/vela_engine/src/runtime/inline_cache_method_tests.rs:287` | `crates/vela_engine/tests/fixtures/cache/counter_public.vela` | 198 | `dfa30a69c2c167d95144e2ae365aa435179e432254a40e12854d3a3c15e6328b` |
| `crates/vela_engine/src/runtime/inline_cache_method_tests.rs:326` | `crates/vela_engine/tests/fixtures/cache/counter_rejected.vela` | 173 | `5ad1b6c78ea75fa9c09b4cf7ae0cb2fe186ecb4b58192dda824eb908c2fbd3f9` |
| `crates/vela_engine/tests/hot_reload.rs:299` | `crates/vela_engine/tests/fixtures/reload/native_closure_v1.vela` | 249 | `5eca33105aca184e55e7fbb3e6ed79a25a905f9ed2e9d484057487ad0a78bdaa` |
| `crates/vela_engine/tests/hot_reload.rs:327` | `crates/vela_engine/tests/fixtures/reload/native_closure_rejected.vela` | 252 | `dab28b52cba3121a0278e516a04aca4e82e9f132342297447711a56b65dd0db2` |
| `crates/vela_engine/tests/hot_reload.rs:370` | `crates/vela_engine/tests/fixtures/reload/reflection_v1.vela` | 405 | `29ec291b0b22c7c38ef07ec4b5f043e063ce48289b46d0e2d53f06f5f31e9c6b` |
| `crates/vela_engine/tests/hot_reload.rs:401` | `crates/vela_engine/tests/fixtures/reload/reflection_v2.vela` | 434 | `ba2289a2b17e9b6d6da2c914069182caffc44d78a93b8110f4d8cb4ce5313bf6` |
| `crates/vela_engine/tests/hot_reload.rs:446` | `crates/vela_engine/tests/fixtures/reload/trait_dispatch_v1.vela` | 259 | `5886530de525cbb774c69d51a1e9f7062dd74213064c77f39360355715725911` |
| `crates/vela_engine/tests/hot_reload.rs:477` | `crates/vela_engine/tests/fixtures/reload/trait_dispatch_v2.vela` | 263 | `61c10d7b27df449b6c6649be34b74817aba7e79bf1313c16966ab10f1b22e335` |
| `crates/vela_engine/tests/script_reflection.rs:55` | `crates/vela_engine/tests/fixtures/reflection/script_metadata.vela` | 741 | `f51f7d488b601c186c72dc94af0865c5284dcb99454e27ebf322628cd93c6028` |
| `crates/vela_engine/tests/script_reflection.rs:103` | `crates/vela_engine/tests/fixtures/reflection/modules/game/reward.vela` | 111 | `1c355e3fb793aff0bf9598ce00127744c403ebe6e81273e39867e61ea5c25539` |
| `crates/vela_engine/tests/script_reflection.rs:114` | `crates/vela_engine/tests/fixtures/reflection/modules/game/main.vela` | 790 | `78dcea721545d9a5d8a42c0596fb6b219467326f8471810446ee02089e94814e` |
| `crates/vela_engine/src/tests/source_reload/runtime_safe_points.rs:217` | `crates/vela_engine/tests/fixtures/reload/state_record_v1.vela` | 266 | `f16312829be379ca435108da197d7e2d9d7720435427ad1a4f9c3b9498761d8f` |
| `crates/vela_engine/src/tests/source_reload/runtime_safe_points.rs:247` | `crates/vela_engine/tests/fixtures/reload/state_record_v2.vela` | 274 | `a0916cdce7731e7cdc98329e52e3f73c222ac74cfeae894707e36467bb75b22d` |
| `crates/vela_engine/src/tests/source_reload/fixtures.rs:127` | `crates/vela_engine/tests/fixtures/reload/modules/main.vela` | 60 | `2c126279e5c4feac3646919f5c59018f54c16db2c3979e433a978f7ac8919d37` |
| `crates/vela_engine/src/tests/source_reload/fixtures.rs:192` | `crates/vela_engine/tests/fixtures/reload/modules/main.vela` | 60 | `2c126279e5c4feac3646919f5c59018f54c16db2c3979e433a978f7ac8919d37` |
| `crates/vela_engine/src/tests/source_reload/fixtures.rs:255` | `crates/vela_engine/tests/fixtures/reload/modules/main.vela` | 60 | `2c126279e5c4feac3646919f5c59018f54c16db2c3979e433a978f7ac8919d37` |
| `crates/vela_engine/src/tests/source_reload/fixtures.rs:322` | `crates/vela_engine/tests/fixtures/reload/modules/main.vela` | 60 | `2c126279e5c4feac3646919f5c59018f54c16db2c3979e433a978f7ac8919d37` |
| `crates/vela_engine/src/tests/source_reload/fixtures.rs:373` | `crates/vela_engine/tests/fixtures/reload/modules/main.vela` | 60 | `2c126279e5c4feac3646919f5c59018f54c16db2c3979e433a978f7ac8919d37` |
| `crates/vela_engine/src/tests/source_reload/source_file_schema_trait.rs:61` | `crates/vela_engine/tests/fixtures/reload/stable_member_rename_v1.vela` | 163 | `06e26749c9ef18c0c6a28d77a3e5ebcc0198b04b1bb1c5e8273f420fee66b30d` |
| `crates/vela_engine/src/tests/source_reload/source_file_schema_trait.rs:82` | `crates/vela_engine/tests/fixtures/reload/stable_member_rename_v2.vela` | 192 | `6f92afa97d0b2f06a1cde0dfd5b9d85cd6004b5a992b2ac4ff40658edfaf19b9` |
| `crates/vela_hot_reload/src/tests/function_policy.rs:187` | `crates/vela_engine/tests/fixtures/reload/stable_member_rename_v1.vela` | 163 | `06e26749c9ef18c0c6a28d77a3e5ebcc0198b04b1bb1c5e8273f420fee66b30d` |
| `crates/vela_hot_reload/src/tests/function_policy.rs:210` | `crates/vela_engine/tests/fixtures/reload/stable_member_rename_v2.vela` | 192 | `6f92afa97d0b2f06a1cde0dfd5b9d85cd6004b5a992b2ac4ff40658edfaf19b9` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:188` | `crates/vela_hot_reload/tests/fixtures/reload/recursive_initializer_v1.vela` | 134 | `c2755100ff542d935bf32f10176951ad2cdb4b382b29312cee34677535914bcb` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:199` | `crates/vela_hot_reload/tests/fixtures/reload/recursive_initializer_v2.vela` | 134 | `3b8df27729bd3c13edaf14f4392a8ab64232a896efadc590817f4483256fa810` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:218` | `crates/vela_hot_reload/tests/fixtures/reload/closure_initializer_v1.vela` | 184 | `58de871bf5ed35355f1f90e848f0a33355914f00a4f3740ae02d427e1b0e13fb` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:230` | `crates/vela_hot_reload/tests/fixtures/reload/closure_initializer_v2.vela` | 184 | `0542b441bb3fb03f4ec331670faf5b8421e1490b5863ec1045b149037a0904d2` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:250` | `crates/vela_hot_reload/tests/fixtures/reload/closure_initializer_v1.vela` | 184 | `58de871bf5ed35355f1f90e848f0a33355914f00a4f3740ae02d427e1b0e13fb` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:262` | `crates/vela_hot_reload/tests/fixtures/reload/closure_initializer_unrelated.vela` | 184 | `b2095725672f58ecfac8ced0d7590292f0e493193aca5c954348701ba7f0e2b8` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:282` | `crates/vela_hot_reload/tests/fixtures/reload/recursive_closure_v1.vela` | 144 | `1d307677f3310012100ee0db14af3cc104f2ea4fffba6a7836568b3e67cdf3ae` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:293` | `crates/vela_hot_reload/tests/fixtures/reload/recursive_closure_v2.vela` | 144 | `9b303192c756e31dd32378a4f376a39d7f00e433d94ad7002fe51d9ad7c8bc9b` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:312` | `crates/vela_hot_reload/tests/fixtures/reload/default_closure_v1.vela` | 158 | `86e084b5018c8373420d96deae27d69eb7f8f265f1d5b466ca6819fc43da799d` |
| `crates/vela_hot_reload/src/tests/state_abi.rs:323` | `crates/vela_hot_reload/tests/fixtures/reload/default_closure_v2.vela` | 158 | `59d22c7b5dc8e786d018d9786a9bd3c1bb1ff1503393e724dff18c84157eac9b` |

## Finite helper byte proof

All actual callers were enumerated before materialization. Rust format brace
escaping and substitutions were evaluated without trimming; both stable-rename
versions, all four trait implementation/reward combinations, and both bonus
versions of each Hot Reload module factory are represented.

| Original helper arguments | Fixture | Bytes | SHA-256 |
|---|---|---:|---|
| `write_stable_schema_rename_module(2, False)` | `crates/vela_engine/tests/fixtures/reload/modules/stable_rename_v1.vela` | 168 | `5b08cb6971e3567ca71bcc7f27dea3e10faddb681d73a21f2ff18e8679a09e94` |
| `write_stable_schema_rename_module(6, True)` | `crates/vela_engine/tests/fixtures/reload/modules/stable_rename_v2.vela` | 197 | `83ec7aa42c0e1119a769b53b22aaa58fcc602085b454f767a4a0811e1473d4de` |
| `write_trait_impl_module(2, False)` | `crates/vela_engine/tests/fixtures/reload/modules/trait_2_absent.vela` | 141 | `c1c614114054fad7d816f23231995eea40d5f4513f7d82ee434b5ddcf4bb5fb7` |
| `write_trait_impl_module(2, True)` | `crates/vela_engine/tests/fixtures/reload/modules/trait_2_implemented.vela` | 171 | `b9462be1627ebd6498ca2762ddecda71d2952b4c83f1fa47baea5cf629294285` |
| `write_trait_impl_module(6, False)` | `crates/vela_engine/tests/fixtures/reload/modules/trait_6_absent.vela` | 141 | `d41dd911e036c1df443e1a2162dbdccc46fb7bc0040d0cfb8f0a6b5038a34234` |
| `write_trait_impl_module(6, True)` | `crates/vela_engine/tests/fixtures/reload/modules/trait_6_implemented.vela` | 171 | `6bdd1ddbf06553bfcb18b6509738cbac88f4e58054819e0889bef15cb2e474a0` |
| `trait_method(self.level + amount)` | `crates/vela_hot_reload/tests/fixtures/modules/trait_method_v1.vela` | 275 | `27172af98eb86055567db19ec6ad3df316cf11092f2d14d57090492b404f5633` |
| `trait_method(self.level + amount + 1)` | `crates/vela_hot_reload/tests/fixtures/modules/trait_method_v2.vela` | 279 | `d42f5abcc21919bfd73c548d01475bfcb827c6863012236a83a73f30406554cc` |
| `inherent_method(self.level + amount)` | `crates/vela_hot_reload/tests/fixtures/modules/inherent_method_v1.vela` | 206 | `e247507d71e044290a0c57fe4493a3317982a79b2f6c48b538b1979734d33ff8` |
| `inherent_method(self.level + amount + 1)` | `crates/vela_hot_reload/tests/fixtures/modules/inherent_method_v2.vela` | 210 | `3a2fee1e14a940266dfc6af742486c106e3bb1cb23f5c820315d110e355e134b` |

## Package tree byte proof

The manifest formula and evaluated string arguments were taken from HEAD,
including the app `api.vela` -> `main.vela` topology. Dependency ABI tests share
the byte-identical initial tree. Visibility/impact tests reuse the public graph
and overwrite only their distinct dependency declaration before loading it.
Native reduction materializes exactly the original single replacement; neither
declarations nor state/closure bodies change. Every file was byte checked.

Paths below are under `crates/vela_engine/tests/fixtures/packages/`.

| Origin | Fixture | Bytes | SHA-256 |
|---|---|---:|---|
| `ordinary_package_imports_public_dependency_function:write_package manifest` | `public_function/vela.toml` | 126 | `759e8ca35ce7c6c488c036c08137dd0f825e8513ab4dea2725361e318f869f00` |
| `ordinary_package_imports_public_dependency_function:write_package source` | `public_function/src/main.vela` | 57 | `086a1696853a32b2b5b5c3e3b0752fd65ebfd00134c95c870dc6374aac908829` |
| `ordinary_package_imports_public_dependency_function:write_package manifest` | `public_function/util/vela.toml` | 88 | `1a82e30b649bf83c7572bee2bfe5777835dcde7718e37c211265a010b9d146e1` |
| `ordinary_package_imports_public_dependency_function:write_package source` | `public_function/util/src/api.vela` | 31 | `9bcf66598832478792e001c29088e27b90f841b69848cad9dff86f9cebc7100b` |
| `ordinary_package_imports_dependency_type_and_method:write_package manifest` | `public_type/vela.toml` | 126 | `759e8ca35ce7c6c488c036c08137dd0f825e8513ab4dea2725361e318f869f00` |
| `ordinary_package_imports_dependency_type_and_method:write_package source` | `public_type/src/main.vela` | 100 | `dd1c369eda88a5e8062bc560e1dd7c4ebe35f0febb45c7538e3be72fa5f948dc` |
| `ordinary_package_imports_dependency_type_and_method:write_package manifest` | `public_type/util/vela.toml` | 88 | `1a82e30b649bf83c7572bee2bfe5777835dcde7718e37c211265a010b9d146e1` |
| `ordinary_package_imports_dependency_type_and_method:write_package source` | `public_type/util/src/api.vela` | 95 | `93150c7b1aa7bf6c271c9230b90f3a115c5502ebfb174c9f2ce947959ddcbd03` |
| `ordinary_package_includes_transitive_dependencies_but_not_their_aliases:write_package manifest` | `transitive/vela.toml` | 130 | `da0a6cfe2c063df67913bbf38e51645bcb2509f9303a1006c63c740952e7b502` |
| `ordinary_package_includes_transitive_dependencies_but_not_their_aliases:write_package source` | `transitive/src/main.vela` | 59 | `9e8f37abe1b797f42bd81d2bbf8fc4b29e86cb6a6a84a4a60881d106f99341c7` |
| `ordinary_package_includes_transitive_dependencies_but_not_their_aliases:write_package manifest` | `transitive/middle/vela.toml` | 135 | `3c00363580fbd35fc0b545cd904d38d846f437c98f01d41887d21256faa811ba` |
| `ordinary_package_includes_transitive_dependencies_but_not_their_aliases:write_package source` | `transitive/middle/src/api.vela` | 57 | `cf4cc69f6b62a77c7544974b852f596b29000ed39d9f9bd026aa0421bc085809` |
| `ordinary_package_includes_transitive_dependencies_but_not_their_aliases:write_package manifest` | `transitive/leaf/vela.toml` | 88 | `e805e3972254b0894be6ae0dbc78ee48f7b5da56a710d943b6b987b145947e80` |
| `ordinary_package_includes_transitive_dependencies_but_not_their_aliases:write_package source` | `transitive/leaf/src/api.vela` | 29 | `c8f98f924851204e08727e05c1cd6bebb5fcf19145fb05b8c08ad5cfbe51917c` |
| `ordinary_dependency_body_reload_updates_root_package_calls:write_package manifest` | `dependency_reload/vela.toml` | 126 | `759e8ca35ce7c6c488c036c08137dd0f825e8513ab4dea2725361e318f869f00` |
| `ordinary_dependency_body_reload_updates_root_package_calls:write_package source` | `dependency_reload/src/main.vela` | 55 | `05c611eff67ceb383bd6d65887ca2cbc63253dd61880bac035d3281ee1cd088d` |
| `ordinary_dependency_body_reload_updates_root_package_calls:write_package manifest` | `dependency_reload/util/vela.toml` | 88 | `1a82e30b649bf83c7572bee2bfe5777835dcde7718e37c211265a010b9d146e1` |
| `ordinary_dependency_body_reload_updates_root_package_calls:write_package source` | `dependency_reload/util/src/api.vela` | 36 | `fac4432bb67d36baee15592f1abd0b45dd8e2450fa34464ee362e2ca4e145766` |
| `src/source/package/tests.rs` | `updates/dependency_v2.vela` | 36 | `9994c8b3b143d234da22eef48ce991ecd59db4c801553bad674813507b7e5119` |
| `src/source/package/tests.rs` | `updates/dependency_rejected.vela` | 47 | `cdc14d2672d597e9843a6634213b966275e0eaeb07cf3561589747e60d8ee2f0` |
| `src/source/package/tests.rs` | `updates/transitive_alias_rejected.vela` | 55 | `16078f87e88a129920274ea2c33ddad5843fe916078036f5e5ab24015d8f6b24` |
| `src/source/package/tests/native_capabilities.rs:write_package manifest` | `native_write_to_read/vela.toml` | 140 | `cec15e11039ff8a02fd6d90c0364297738bc42920ffab0f10be630042358e5d5` |
| `src/source/package/tests/native_capabilities.rs:write_package source` | `native_write_to_read/src/main.vela` | 203 | `76286d1cfdf547b3bd1d6f3f4ece09af55f97d330ed3889214b34abc03da1eda` |
| `src/source/package/tests/native_capabilities.rs:write_package manifest` | `native_read_to_write/vela.toml` | 140 | `cec15e11039ff8a02fd6d90c0364297738bc42920ffab0f10be630042358e5d5` |
| `src/source/package/tests/native_capabilities.rs:write_package source` | `native_read_to_write/src/main.vela` | 45 | `754210ee68be871c89ecf462dbfeb4cdf1f3ae8f54f5f90a81b2640b9bc2c885` |
| `SOURCE.replace: exactly one read+write expression` | `updates/native_read_only.vela` | 187 | `ab6c436ab5ffaf911e6a4a1fa2111a8728453830e68d27e74a7592a8f022d2b2` |
| `src/source/package/tests/native_capabilities.rs:read-to-write update` | `updates/native_write_expansion.vela` | 61 | `07e7813415bb9dda9bc47f0c48efe10e3807a4034e6a50215fbdb60ccc903563` |

## Residual reviewed literal selections

These four multi-entry/metadata programs were selected after reviewing mixed
registration and reflection files; the same exact-byte and whole-file reverse
substitution check passed.

| Original source:line | Fixture | Bytes | SHA-256 |
|---|---|---:|---|
| `crates/vela_engine/src/standard/bindings.rs:674` | `crates/vela_engine/tests/fixtures/native/bound_option_result.vela` | 336 | `0058611e5a2921fdcf56849628dfe523342b169d8768f8241e2548f39c9c4065` |
| `crates/vela_engine/src/standard/bindings.rs:884` | `crates/vela_engine/tests/fixtures/native/bound_collections.vela` | 255 | `d388f486b590299e05d672094cc4f61cdfa0c83754815cab4d6c4e41bbd1c922` |
| `crates/vela_engine/src/tests/reflection/reflect_calls.rs:87` | `crates/vela_engine/tests/fixtures/reflection/native_iterator.vela` | 237 | `758119189c696d62c5b21070b4cf1cac7c061eaa0e63984a77e2cc7b426a2fbf` |
| `crates/vela_engine/src/tests/script_identity.rs:18` | `crates/vela_engine/tests/fixtures/reflection/script_identity.vela` | 260 | `8fa554b0a938f93f3949d152682017b136935104001e68be8c7f205251a9568b` |
