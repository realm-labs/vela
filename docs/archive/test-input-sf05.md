# SF05: VM And Reflection Test Inputs

## Scope And Result

SF05 owner-local implementation against starting HEAD
`bdc23b480a9b15f9aba074b8a4bbcec937a3fb3c`, following
[the plan](../test-organization-plan.md) and
[the runtime census](../test-input-census-runtime.md).

- Initial owner-local extraction: **106 literal sites -> 104 fixtures**:
  103 VM sites / 101 VM fixtures and three reflection sites / three reflection
  fixtures, in 30 Rust files. The authorized SF04 follow-up below adds one VM
  consumer of an existing canonical root fixture: **107 relocated sites total**,
  with no additional owner-local fixture.
- Only test source storage changed. Existing named tests, entry names, logical
  SourceIds/module paths, registrations, helpers, budgets and assertions remain.
  No runtime implementation, feature, dependency, target, framework or schema change.
- Writes are confined to `crates/vela_vm` (no benches or external-comparison
  contracts), `crates/vela_reflect`, and this evidence document. No shared root
  fixture, status document, other owner or commit was written by this task.
  Concurrent changes in other owners were observed and left alone.
- No test/helper merging or new glob discovery: existing explicit includes keep
  ordinary Cargo input tracking. The temporary extraction aid was removed.

### Cross-owner Set-combination Handoff — Closed

After SF04 supplied the canonical
`tests/fixtures/conformance/collections/set_combinations.vela`, the authorized
follow-up switched only the source in
`src/set_methods.rs::runs_compiled_set_combination_methods` to
`include_str!("../../../tests/fixtures/conformance/collections/set_combinations.vela")`.
Before replacing the literal, direct byte comparison passed: **931 bytes**,
SHA-256 `dc37196944bf1f1115e73e468ce724638b32dd900d0a08bb81c4c3e922ced967`,
matching the supplied hash. Leading/trailing newlines and all source bytes remain
unchanged. Compilation, native registration, execution and the exact assertion
`OwnedValue::String("daily,quest".to_owned())` remain unchanged.

The independent 687-byte managed set-combination fixture still returns I64 `34`;
it is not merged with this shared program. No root fixture/attribute or other
owner was written, and no commit was made. The conditional cross-owner
enum/qualified-record pattern groups remain untouched; their graph identity and
legacy false-arm/budget observations are not consolidated or claimed closed.

Follow-up validation:

- `cargo test -p vela_vm --lib --locked set_methods::tests::runs_compiled_set_combination_methods -- --exact --test-threads=4`
  — **1 passed**, 826 filtered; 180 s bound.
- Same command with `--all-features` — **1 passed**, 836 filtered; 180 s bound.
- `rustfmt --edition 2024 --config skip_children=true --check crates/vela_vm/src/set_methods.rs`
  initially requested only an include-line wrap adjustment; scoped rustfmt applied
  it and the same check passed (initial check bound 60 s; format/check/all-feature
  command group bound 180 s).
- No timeouts. Prior full-owner results below are the initial extraction evidence,
  not newly rerun full-owner suites for this one-source follow-up.

## Family Review And Dispositions

Paths below are relative to `crates/vela_vm` unless explicitly prefixed.
The ledger below identifies **every extracted literal** by original location,
fixture and bytes. Remaining raw-source site counts refer to the final source
files, not test counts or a claim that every Rust string is a program. Cooked
strings, includes and format-generated sources were reviewed separately too.
There is no size threshold or zero-inline target.

| Source family | Decision and remaining input rationale |
|---|---|
| `src/tests/execution_core.rs` | File-back four fixed deep recursion programs (ordinary, trait, callback, iterator); depth 10,000 and execution routes unchanged. Retain the 13 remaining raw sites: tiny recursion budget control, numeric/Unicode/unary/logical examples and local reads/writes. Long logical-chain `format!` generation remains generated scale input. Hand-built instruction budget cases remain Rust. |
| `src/tests/control_flow.rs` | Extract multi-entry returning-operands document without combining the five observations. Keep 16 local block/if/match/guard/variant source sites, including the short returning-initializer pair and nested record match, adjacent to their selected branches/results. |
| `src/tests/program_execution.rs` | Extract large value-method and record/closure/iterator/float-key scenarios, managed trait defaults, nested captures and Option/Result propagation documents. Cross-family typed try document goes under contracts. Keep 22 raw sites for small loop/native/method/default/adder controls; cooked native wrappers remain local. Keep first-inserted negative-zero key bit/sign observation, budget resets, export and cleanup checks. |
| `src/tests/script_methods.rs` | Extract composite collection methods, named/default arguments, trait defaults and record/tuple variant dispatch documents. Keep 11 narrow receiver/impl/self/capture/module source sites and existing root self-compound-assignment input. Similar managed/owned programs with different values are not shared. |
| `src/tests/records_enums.rs` | Extract explicit PartialEq/PartialOrd implementations, derived equality versus identity lookup, constructor defaults and shared enum-match program. Keep 30 raw sites: tiny constructors, tuple patterns, slots, derived comparison operator controls, error paths and exact legacy pattern budget cases. The repeated 320-byte census candidate is actually a tag-match program, so its fixture is named `records/enum_match.vela`, not `enum_defaults`. |
| `src/tests/iteration.rs` | Extract mixed sequence iteration, record-key set/map collectors, lazy for consumption and callback short-circuit sequences. Keep 24 local sites for simple for/pattern/index/range cases, native iterators, individual collect/fold/adapter operations, collection/budget errors and short snapshot/current-value controls. Read/consume/mutate order is unchanged in both groups. |
| `src/tests/modules.rs` | Retain all 18 raw documents plus the tiny cooked dependency. The eight graphs (imported call, same-name function, const chain, type constructors, constructor defaults, match patterns, qualified-record legacy case, qualified paths) are compact and clearer beside explicit ordered ModuleSources. No graph files or global pooling of small matching text. |
| `src/tests/consts.rs` | Retain six raw documents: two short aggregate-copy/read examples, constant-opcode example and the three-node imported aggregate graph. Their mutation/fresh-copy and declaration dependencies remain visible beside assertions. |
| `src/tests/runtime_semantics.rs` | Extract the three substantial assignment/effect/alias scenarios. Keep five raw and two cooked budget/operation microprograms (including the small Option present/absent pair). Exact edge charges and stop-before-mismatch checks remain unchanged. |
| `src/tests/bytes.rs` | Extract length/slice/get/hex and iterator programs. Keep eight raw sites for index/U8, out-of-bounds, immutable assignment, endian/U32 and hex Result cases. Backslashes in byte literals remain source bytes, not decoded byte strings. |
| `src/tests/{numeric_conversions,char,deferred_literals,scalar_numeric_ops}.rs` | Retain all 23/1/6/6 raw sites respectively. Numeric widening/narrowing, wrapping/bits, Unicode char predicates, contextual literals and short multi-entry scalar operator matrices belong next to exact tags/results/errors. No stringification of typed expectations. |
| `src/tests/type_guards.rs` | Extract the composite Function forwarding-across-containers program. Keep 28 raw sites covering one guard/entry/container mismatch, identity-key controls, lazy iterator checks and Option/Result payload matrices; preserve raw poisoned-guard and enum guard builders and their distinct checked/unchecked entry observations. |
| `src/tests/reflection_metadata.rs` | Extract eight metadata/query documents, including 4,078-byte methods/traits/variants and 2,089-byte name/kind/fields. Keep two removed-async target expressions with their small `format!` wrapper and the 332-byte source-span query beside its explicit `(7,20,42)` descriptor. |
| `src/tests/reflection_members.rs` | Extract four variant/member access-policy queries. Keep eight unknown/denied/type-descriptor controls; exact candidate order, related spans, method IDs and field permissions remain typed Rust observations. |
| `src/tests/reflection_modules.rs` | Extract module/function/export query (2,597 bytes) and export-policy query. Keep four unknown/call-policy snippets next to the differently configured registries. |
| `src/tests/reflection_permissions.rs` | Extract composite script-value native query. Keep 12 small deny/active-policy/unknown-permission/lookup-limit source sites. Permission failure must precede value access exactly as before. |
| `src/tests/reflection_values.rs` | Share the two identical 496-byte trait/value queries; extract updated-copy and registered-module-trait documents separately. Keep four tiny closure/metadata-denial/unknown-field/permission sources and cooked execution-unit-edge source. Managed-budget and ordinary consumers still compile and set up independently. |
| `src/tests.rs` | Extract shared helper's attributed reward-module input, retaining fresh graph and registry creation on each call. Keep the tiny old/new stable-state slot pair and all typed reflection registry builders. |
| `src/string_methods.rs` | Extract two full utility chains and the 1,112-byte scalar/Unicode parse-options program. Keep 25 local single-method success/error/parse-range snippets, including byte-index, UTF-8 boundary and escaped line/whitespace cases. |
| `src/map_methods.rs` | Extract higher-order programs (owned and managed), record identity and three introspection/lookup documents. Keep 16 one-operation/find/zero-argument-callback/merge/extend/limit controls. Function-only versus program compilation and native installation are untouched. |
| `src/set_methods.rs` | Extract managed numeric combination, string predicates and three record-identity/callback programs. The authorized follow-up uses SF04's canonical root set-combination fixture. Keep 18 raw sites for short has/filter/map/predicate/clear/extend/snapshot/error/limit examples. Near-matching combinations retain independent bytes and results. |
| `src/math_stdlib.rs` | Retain all 22 raw sites: distance/pow/sqrt/sign/move-towards numeric examples and finite/domain/type error controls. Extracting these compact matrices would separate operands from expectations without meaningful reuse. |
| `src/array_methods/tests/aggregation_and_ordering.rs` | Extract explicit/derived Ord and sort-by-key implementations. Keep 20 sum/group/extrema/sort one-operation and failure sites, including numeric/identity group keys; managed results are not mistaken for duplicates. |
| `src/array_methods/tests/higher_order_and_mutation.rs` | Extract two substantial callback programs. Keep 13 endpoint/remove/insert/extend/clear and growth/error controls; mutation and returned removed-value sequences remain local. |
| `src/array_methods/tests/lookup_and_transform.rs` | Extract predicate-versus-identity query, managed distinct and both reverse/slice documents. Keep 14 get/contains/index/scalar-lookup/distinct/join/error sites; identity equality is still distinguished from PartialEq. |
| `src/stdlib/tests/core.rs` | Extract managed option filter and large option/result helpers. Keep seven raw wrong-shape/flatten/callback failures and compact set construction/iteration/identity controls. |
| `src/stdlib/tests/option_result.rs` | Extract large native/method helper matrices and multi-function and_then/or_else programs. Keep six compact math/try/map/map_err/filter programs. Native free functions and value methods remain distinct routes. |
| `src/stdlib/tests/option_result_chains.rs` | Extract ok_or propagation, tuple destructuring and large managed helper/and_then/or_else programs. Keep three compact Result-try/map/map_err programs. No flattening of success/short-circuit/error paths. |
| `src/tests/dynamic_method_dispatch.rs` | Extract six complete script/heterogeneous/named-default/error/cache-guard programs. Keep 13 tiny receiver/error/cache-site inputs including the bounded two-adapter format wrapper. Cache cold/hot/set counts and receiver guards are unchanged. |
| `src/tests/evaluation_order.rs` | Retain four compact raw sources and existing root named-argument fixture. `run_marked` still registers a fresh effect recorder; ordered traces `[1,2,3]`, `[2,1]` and skipped RHS are not collapsed into result-only rows. |
| `src/tests/linked_context_execution.rs` | File-back the helper's three-entry shared context/callback source, not the VM/session. Keep tiny old/new closure-owner sources and two callback-recursion format variants inline. Owner pointer equality, depth transitions, explicit GC collection, rooted result, pending-native cleanup and error stack remain intact. |
| `src/tests/linked_bytecode_profiler.rs`, `linked_execution_core.rs` | Retain 88-byte callback seed, two tiny cooked linked-entry/cache sources and all raw linked-code builders. Exact profiler offsets, dense handles, cache capacities and instruction charges need their Rust construction. |
| `src/tests/standard_{map_set,option_result}_id_dispatch.rs` | Extract three full transform/helper programs. Leave debug-name poisoning tables and ID-based resolution checks in Rust. |
| `src/tests/standard_{array,string,callback}_id_dispatch.rs`, `standard_id_dispatch.rs`, `standard_borrowed_native_dispatch.rs` | Retain raw code/ID fixtures, 47 callback-cache snippets (at most 196 bytes), one 100-byte callback source and tiny cooked native wrappers. They identify one cache/method boundary beside precise expectations. Internal invocation borrowing is not script-visible host borrowing. |
| `src/tests/linked_{array,map,set,string,readonly}_standard_method_cache.rs`, `linked_standard_method_cache.rs`, `option_result_standard_method_cache.rs` | Unit-data: retain linked instructions, runtime inputs, guards, cold/hot runs, native accounting, denied permissions and allocation rollback. No script-text substitutes. |
| `src/tests/linked_standard_method_cache_{fixtures,support}.rs`, `linked_standard_method_cache_fixtures/array.rs`, `src/tests/scalar_blocks.rs` | Unit-data builders, despite the word fixture. Keep malformed registers/plans, source points, optimized/ordinary comparisons, exact 4/3 charges and first-trap spans. |
| `tests/conformance.rs`, `tests/diagnostic_fixtures.rs`, `src/tests/block_tail_semantics.rs` | Existing shared/file-backed inputs retained. Preserve positive/compile-negative/runtime-negative phase selection and complete diagnostic rendering; local CRLF/trim policy is not applied to new fixtures. No UI-P/UI-S delivery is claimed here. |
| `tests/integration.rs`, `src/test_support.rs` | Existing target/helper boundaries retained. `autotests=false` aggregator continues to own diagnostics/cache-delta/external comparisons; no new test target or bench helper. |
| VM benches and `tests/{external_compare_contract,baseline_workload_contract,baseline_cache_delta}.rs` | No writes: SF11 reserved benchmark/workload/contract ownership. Existing owner test commands execute these current contracts, but this is not SF11 semantic-preflight implementation or performance evidence. |

### Runtime, Host And Reflection Unit-data Audit (No Change)

The census unit-data families were audited for source consumers, not exempted
based on their crate names. No replacements were selected:

- VM `src/{heap,heap_execution,heap_graph,heap_values,container_contracts,
  collection_mutation,equality,frame,owned_contract,owned_value,runtime_type_guards,
  script_object,small_storage,value,value_key,budget,constant_loads,closure_calls,
  try_propagation}.rs`, their test submodules, `frame/registers.rs`,
  `owned_export/tests.rs`, `serde_tests.rs`, `option_result_methods/access.rs`
  and `ordered_keyed.rs`: Rust values, graph/cycle/GC/root builders, bounded
  growth, rollback, identity and memory accounting. Preserve serde/platform gates.
- `src/tests/owned_boundary.rs`: six 42-byte identity wrappers and a tiny cooked
  update wrapper intentionally stay with nested owned data, rejected keys,
  mutation and released boundary-memory assertions. `script_builtin_methods.rs`
  keeps its two UTF-8 byte-length controls. `error.rs` keeps the cooked continued
  stack-source string next to explicit spans and its tiny reflection diagnostic
  sources; these are diagnostic labels/spans, not hidden compilation helpers.
- Reflect `src/tests/{calls,fields,implements}.rs`,
  `src/members/tests/{candidates,metadata,policy}.rs`,
  `src/{metadata,permissions,types}.rs` and `tests/value_access.rs`: typed
  descriptors, records, policy and expected diagnostics stay in Rust. Source API
  audit needles are not Vela programs. Independent updated-copy/original-value
  and denied-write observations remain. `src/modules/records.rs` constructs
  metadata records; its names/keys are production vocabulary, not source fixtures.
- Reflect `src/script_types.rs`: file-back attributed struct/enum and trait/impl
  documents; retain three raw short explicit-ID rename/inherent-method sources
  and four cooked reorder/schema-change versions. Keep field/variant order,
  stable IDs, changed/equal schema hashes, docs/attrs and source identities.
  `src/modules/tests.rs`: extract the attributed function/export document;
  retain the 52-byte public/private state pair. All other module tests build
  typed registries, not source graphs. No synthetic module graph is introduced.
- Host `src/call_value.rs`: owned carrier/U8/bytes/map/tuple round-trip is unit
  data. `tests/unsafe_boundaries.rs` traverses and parses actual Rust with `syn`;
  paths, source text and unsafe needles are not Vela inputs. No host changes.
- Stdlib `src/{manifest,ids,reflection_natives}.rs`: manifest definitions,
  derived IDs, primitive mappings, duplicates and effect/policy data stay in
  Rust; signature/doc strings are production vocabulary. Manifest counts
  (24 types, 4 variants, 3 fields, 52 functions, 164 methods) and all 49 reflection
  native definitions remain checked by their existing tests.
- Stdlib runtime `src/lib.rs`: keep whole-manifest function/method binding
  lengths, typed implementations and ID uniqueness. No representative script
  replaces these exhaustive unit-data checks. Neither stdlib owner was edited.

### Helper, Scenario And Byte Proof

`src/test_support.rs` still builds fresh HIR, preserves frontend diagnostics as
`InvalidHirGraph`, selects the function from that same source set and compiles
with the existing options/registry. The owned-result, supplied heap/budget,
host, cache, profiler and metadata-only routes remain separate. There is no
cached registry/program/Runtime or new universal runner. Reflection compile
helpers retain their different native-name sets and policy setup.

Every selected literal was raw UTF-8, so its interior bytes were copied directly.
All leading/trailing newlines, indentation, escapes and Unicode are preserved.
The two duplicate groups were directly compared before sharing:
`reflection/script_trait.vela` (496 bytes, two sites) and
`records/enum_match.vela` (320 bytes, two sites). No assertions were removed.
After rustfmt, inverse-expanding includes, formatting the reconstructed originals
and comparing tokens with exact raw/cooked string hashes passed for all 30 files.
This checks the surrounding helpers, budget settings, effect/cleanup/GC/cache/
profiler sequences and assertions as well as source equality.

All 104 new `.vela` files have actual `include_str!` consumers; strict UTF-8 and
no-CR checks passed. Both owner fixture directories have scoped `.gitattributes`;
`git check-attr text eol` reports `text: set`, `eol: lf` for every file. No root
attributes, normalization pass, new reader or golden was introduced. Cargo's
package lists contain all 101 VM and three reflect fixtures. These are test-only
inputs; no production/build-script include now depends on repository fixtures.
Package-list reachability is proven, **not an isolated packaged build**.

## Validation

All commands ran from the workspace root with finite process bounds. Tests use
four threads; no command below timed out. No workspace fmt/Clippy or commit ran.

| Actual command | Bound | Result |
|---|---:|---|
| `cargo test -p vela_vm --lib execution_core --locked -- --test-threads=4` before edits | 180 s | 53 passed, 774 filtered; includes linked execution-core matches. |
| `cargo test -p vela_reflect --locked -- --test-threads=4` before edits | 180 s | 54 unit + 3 integration passed, 0 doctests. |
| `cargo test -p vela_vm --locked -- --test-threads=4` after edits | 240 s | 827 unit + 2 workload-contract + 11 conformance + 19 integration + 13 doctests = **872 passed**, 26.22 s wall time. |
| `cargo test -p vela_vm --locked --all-features -- --test-threads=4` | 240 s | 837 + 2 + 11 + 19 + 13 = **882 passed**, 27.96 s. |
| `cargo test -p vela_reflect --locked -- --test-threads=4` after edits | 240 s | **57 passed**, 8.82 s; 0 doctests. |
| `cargo test -p vela_reflect --locked --all-features -- --test-threads=4` | 240 s | **57 passed**, 0.38 s; 0 doctests. |
| `cargo clippy -p vela_vm -p vela_reflect --lib --tests --no-deps --locked -- -D warnings` | 240 s | Passed, 7.95 s. No bench target or dependency linting requested. |
| Same scoped Clippy command with `--all-features` before `-- -D warnings` | 240 s | Passed, 8.97 s. |
| `rustfmt --edition 2024 --config skip_children=true` on the explicit 30 changed Rust paths, then the same paths with `--check` | 60 s each | Passed; no workspace formatting or recursive child formatting. |
| `cargo package -p vela_vm --list --allow-dirty --no-verify --locked` and corresponding `vela_reflect` command | 120 s each | Passed; 101/3 fixtures included. Existing missing package-description/homepage metadata warnings only. |
| `cargo test -p OWNER --lib --locked [--all-features] -- --list`, for both owners, before and after | 180 s each | Nonempty identities unchanged; exact hashes below. |

Discovery hashes are SHA-256 of sorted `: test` output lines joined by LF,
without a final LF. Names and counts match before/after (not merely totals):

| Owner/lane | Count | SHA-256 |
|---|---:|---|
| VM default | 827 | `c2b101e0965af1e64abd4427c49da517315064660b0dbc25eb5f9da8e511a839` |
| VM all features | 837 | `8a20f9d77ad45e3a8e4415561f80047ce98876c068766749ebd3dc8128cebed5` |
| Reflect default/all features | 54 each | `678d29294162996484fa00711b5397ff6dc61918339190671b46a63ff8f64b7a` |

Review-tool corrections: an initial inventory one-liner had a Python syntax
error and was corrected before use. Initial reconstructed-text/token checks
were too strict about rustfmt's preserved indentation and optional trailing
argument commas; the final formatted-token comparison preserves string hashes
and passes. These were review-tool failures, not failing owner tests, and no
runtime/assertion changes were made to satisfy them.

Initial extraction final audit (90 s outer bound, 60 s formatting subprocess bound):
`git diff --check -- crates/vela_vm crates/vela_reflect docs/archive/test-input-sf05.md`
passed. A Python ledger audit reread all 106 original raw literals from starting
HEAD and directly compared them with the 104 fixture files, checking lengths and
SHA-256 values; all passed. It also compared the entire pending set-combination
test with its original bytes, verified temporary-aid removal and documentation
links, and reran the explicit 30-file rustfmt check successfully.

The final source-consumer scan (10 s bound) covered 191 VM, 37 reflect, four host,
six stdlib and one stdlib-runtime Rust source files. The compile/ModuleSource/
DiagnosticSource signals identified 45 VM and two reflect files (script_types
and modules/tests), with zero host/stdlib/runtime source consumers. This signal
scan complements the raw/cooked/include/format and body review above; it is not
claimed to be a Rust parser or a replacement for reviewing typed unit-data.

No full workspace, installed-editor, browser, performance, SF11 or independent
host/stdlib test acceptance is claimed. This is an owner-local handoff for parent
staging/committing; the shared set-combination follow-up is now closed as recorded above.

## Exact-byte extraction ledger

Original locations are at the starting HEAD. Every selected literal is a UTF-8 Rust raw string; payload bytes were copied without decoding escapes, trimming, reindentation or newline insertion. Direct byte equality was checked before replacing each literal. Inverting the replacements reproduced each original Rust file exactly before Rust formatting.

| Original Rust location | Fixture (owner-local unless root path shown) | Bytes | SHA-256 |
|---|---|---:|---|
| `crates/vela_vm/src/tests/bytes.rs:76` | `crates/vela_vm/tests/fixtures/bytes/methods.vela` | 254 | `275b59e0afbe59298fabd9ca0c6c00f3a7484bce5cc649fbcf6f5da46dc93ef2` |
| `crates/vela_vm/src/tests/bytes.rs:95` | `crates/vela_vm/tests/fixtures/bytes/iterators.vela` | 470 | `2c748063c11ce2fbdbe9e5f63cbe20bd61162b5202ce2e7e1392824a61c2c951` |
| `crates/vela_vm/src/tests/execution_core.rs:339` | `crates/vela_vm/tests/fixtures/execution/deep_recursion.vela` | 136 | `b069e62fdc6252c5f10822bb4ff5a49705e655b0d5e82dc8e379e4c216abe609` |
| `crates/vela_vm/src/tests/execution_core.rs:363` | `crates/vela_vm/tests/fixtures/execution/deep_trait_recursion.vela` | 349 | `00bbcb07ec08a3e0beb41e6cfc76cb391eed8cec251c25323bb9816b1cf78dae` |
| `crates/vela_vm/src/tests/execution_core.rs:392` | `crates/vela_vm/tests/fixtures/execution/deep_callback_recursion.vela` | 142 | `63f33d1fecdf1f967743a78015c3e01c00987a5ebd9e1ddd416dbc751d92f07b` |
| `crates/vela_vm/src/tests/execution_core.rs:416` | `crates/vela_vm/tests/fixtures/execution/deep_iterator_recursion.vela` | 209 | `b0a185a415bda1bcddd7f88f9e91d83d8c0234f861fb3445b967c87bd8fb590e` |
| `crates/vela_vm/src/tests/control_flow.rs:131` | `crates/vela_vm/tests/fixtures/execution/returning_operands.vela` | 326 | `cf0ae3b41cd3042812a48332812827f434c66d072896180caa8cbc7fda92f329` |
| `crates/vela_vm/src/tests/program_execution.rs:134` | `crates/vela_vm/tests/fixtures/execution/managed_value_methods.vela` | 1344 | `bbcea8438212d5a3a2c4fad76a45bc07b9efd087f6b6b7f4dd834ba800085e1b` |
| `crates/vela_vm/src/tests/program_execution.rs:184` | `crates/vela_vm/tests/fixtures/execution/record_identity_keys.vela` | 1112 | `5307bdad69fa113d2dbbc2e4c67af16d871a684db7861a19716c760d18e34a70` |
| `crates/vela_vm/src/tests/program_execution.rs:234` | `crates/vela_vm/tests/fixtures/execution/closure_iterator_identity_keys.vela` | 1499 | `cb43ea79f1c4db15f5dc830630ed01230a5d6c5be552efae8408655962e50811` |
| `crates/vela_vm/src/tests/program_execution.rs:297` | `crates/vela_vm/tests/fixtures/execution/record_index_identity.vela` | 557 | `aae923afb7f95fda0c37e920b33b17083526c55d2e505fbf712f6f197e86904a` |
| `crates/vela_vm/src/tests/program_execution.rs:337` | `crates/vela_vm/tests/fixtures/execution/finite_float_keys.vela` | 689 | `02e2f071f780fecf833ea1524578e13f894ba12530603be8368a2a1d40524042` |
| `crates/vela_vm/src/tests/program_execution.rs:448` | `crates/vela_vm/tests/fixtures/execution/managed_trait_defaults.vela` | 372 | `3d96a1a17a959381ca67101041f78ac23f9c83789fa48daca31e726f7415f98d` |
| `crates/vela_vm/src/tests/program_execution.rs:849` | `crates/vela_vm/tests/fixtures/execution/nested_captures.vela` | 221 | `9c9fdbba7537a06121bcfce1f9f6884d18759399de8abd4eca6728aee5640aca` |
| `crates/vela_vm/src/tests/program_execution.rs:898` | `crates/vela_vm/tests/fixtures/execution/option_propagation.vela` | 313 | `9f06207592fae2bf829356d9e175be2f9ae1be8489085121c0fd3b5997ad04ac` |
| `crates/vela_vm/src/tests/program_execution.rs:953` | `crates/vela_vm/tests/fixtures/execution/result_propagation.vela` | 326 | `a24b370aef42dc53f1797a90b60fa363bfe0c4e56188cd5d95f7e844bf8035c6` |
| `crates/vela_vm/src/tests/program_execution.rs:1013` | `crates/vela_vm/tests/fixtures/contracts/cross_family_try.vela` | 553 | `bcc5cbd7d0e4f4c33ffd45d54fb63c47fed7654a8e464fa07824de9f33ffe895` |
| `crates/vela_vm/src/tests/script_methods.rs:18` | `crates/vela_vm/tests/fixtures/execution/value_methods.vela` | 1231 | `127ca42d83ba7f29b13fbd0ee2f59b84b420252b9caf8db4b0e55403f6712755` |
| `crates/vela_vm/src/tests/script_methods.rs:134` | `crates/vela_vm/tests/fixtures/execution/method_defaults.vela` | 426 | `d05c505e36b3c0d8577aaba4a6363b9fa5ef0d032f0cc21cce238da2a6ab3ccb` |
| `crates/vela_vm/src/tests/script_methods.rs:229` | `crates/vela_vm/tests/fixtures/execution/trait_defaults.vela` | 372 | `c9bc6051d26f553ee91644b4e57ef88f42e261f1b044190a70f68e220fa8b63a` |
| `crates/vela_vm/src/tests/script_methods.rs:348` | `crates/vela_vm/tests/fixtures/execution/record_variant_method.vela` | 427 | `61d0af32abcf9aab6f42a5e66a2e4194e422928d91cdefc6dfa01345218751a3` |
| `crates/vela_vm/src/tests/script_methods.rs:384` | `crates/vela_vm/tests/fixtures/execution/tuple_variant_method.vela` | 410 | `3d5ad2ceeb4a8dba39912ff7317e82b444ae4ff96f3e0d1b9a0427789067fc8f` |
| `crates/vela_vm/src/tests/runtime_semantics.rs:55` | `crates/vela_vm/tests/fixtures/execution/assignment_order.vela` | 361 | `fc9503798b5572f8462eacf5a1117f377f40713ee326f48b757c72d2c13dfc98` |
| `crates/vela_vm/src/tests/runtime_semantics.rs:95` | `crates/vela_vm/tests/fixtures/execution/compound_assignment_order.vela` | 417 | `ad2d8e477c64ee2b2c146cc7bb6e719024ecea9393ba5aff8554054fafde908f` |
| `crates/vela_vm/src/tests/runtime_semantics.rs:136` | `crates/vela_vm/tests/fixtures/execution/assignment_target_reassignment.vela` | 721 | `3fce798ee77e7b7eb32006727f97fcada8b7b4173a570ab7d08aced38119e72e` |
| `crates/vela_vm/src/tests/iteration.rs:308` | `crates/vela_vm/tests/fixtures/execution/sequence_iterators.vela` | 546 | `b3987fc8c382310e208f6c84409d0bd5d897bfabcd6838416b2cb6f6a9dc2e37` |
| `crates/vela_vm/src/tests/iteration.rs:451` | `crates/vela_vm/tests/fixtures/execution/collect_set_identity.vela` | 405 | `da209f4c398aeaf0c6b09b51cc95738f54a9f9fb776defbb0d088cd98a8d8d23` |
| `crates/vela_vm/src/tests/iteration.rs:510` | `crates/vela_vm/tests/fixtures/execution/collect_map_identity.vela` | 533 | `8e594be5d2e062548ca5a9a3df12d45f5188028ccbd60034d7a3d8882ad4a5c9` |
| `crates/vela_vm/src/tests/iteration.rs:771` | `crates/vela_vm/tests/fixtures/execution/lazy_for_consumption.vela` | 341 | `45cde8107a6a1a447eee51cf66280add359a74d87dd5b23c405e8e05feca9393` |
| `crates/vela_vm/src/tests/iteration.rs:825` | `crates/vela_vm/tests/fixtures/execution/iterator_short_circuit.vela` | 434 | `8b54c69e283de1503bcf89b6c875bcb1d8a60ff0f0f5242a2718421c4f8f8d2d` |
| `crates/vela_vm/src/tests/records_enums.rs:271` | `crates/vela_vm/tests/fixtures/records/partial_eq.vela` | 435 | `53e5cfafe144dd375ab4e91213d72c8fa35d3e0015e3cebb6de41acf6d409ff2` |
| `crates/vela_vm/src/tests/records_enums.rs:329` | `crates/vela_vm/tests/fixtures/records/derived_equality_lookup.vela` | 802 | `d21a0796aaf42377447a4882d45f362dd19c696ecc3146a95988705a6384ddf1` |
| `crates/vela_vm/src/tests/records_enums.rs:452` | `crates/vela_vm/tests/fixtures/records/partial_ord.vela` | 582 | `a77b85578cadf9b3288c74836da280b625e0dfda34ada1b10f04df31b5855fce` |
| `crates/vela_vm/src/tests/records_enums.rs:1081` | `crates/vela_vm/tests/fixtures/records/constructor_defaults.vela` | 550 | `e15560a08ec690fb4779042ebab306e09b6970a04b0b9f87532c90bdd16aaf26` |
| `crates/vela_vm/src/tests/records_enums.rs:1114` | `crates/vela_vm/tests/fixtures/records/enum_match.vela` | 320 | `c66c2fa82881ef51025271fe768b8c541d163eed0befedaa6ac61eaed6185b15` |
| `crates/vela_vm/src/tests/records_enums.rs:1142` | `crates/vela_vm/tests/fixtures/records/enum_match.vela` | 320 | `c66c2fa82881ef51025271fe768b8c541d163eed0befedaa6ac61eaed6185b15` |
| `crates/vela_vm/src/tests/type_guards.rs:27` | `crates/vela_vm/tests/fixtures/contracts/forward_container_callbacks.vela` | 504 | `7692c664ff6d612c2338832f293f80cb86efabb87dee27c3f86ef74d29ded58f` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:64` | `crates/vela_vm/tests/fixtures/reflection/field_metadata.vela` | 437 | `5086bac4efc8c80818b7baa6b3983d44a6ba7b7987ab3a43310107fd4bb858c9` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:100` | `crates/vela_vm/tests/fixtures/reflection/name_kind_fields.vela` | 2089 | `27d6677eebf599946da939503f778c7da464b9b653960473163010dae3c8fb1a` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:171` | `crates/vela_vm/tests/fixtures/reflection/required_permissions.vela` | 768 | `a91c31cb89a7439a8ea2cfcb5cd7be5f38e690339daeca589a5f5cb8573e8f42` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:207` | `crates/vela_vm/tests/fixtures/reflection/signature.vela` | 534 | `60c94b487eab1ecee5576cb14f548e19971eebf8412e59f2b4bed84b645d08fc` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:272` | `crates/vela_vm/tests/fixtures/reflection/field_access.vela` | 452 | `629c6090ccdabc207e84f467a310e5ea7377735fd0fa4062def88f0258e49f13` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:320` | `crates/vela_vm/tests/fixtures/reflection/methods_traits_variants.vela` | 4078 | `184b0127cea07d7aeffb98cdb2899d34da84059289568c784ede82ccb42a38b7` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:417` | `crates/vela_vm/tests/fixtures/reflection/registered_trait.vela` | 737 | `f980ceb91ac5acc8771df889a751c8e78ec20b2e5a4aa393429b992985121b8e` |
| `crates/vela_vm/src/tests/reflection_metadata.rs:452` | `crates/vela_vm/tests/fixtures/reflection/registered_type.vela` | 835 | `41ec2ba2e215595c0cc2cbe1af0311452c05ef9ea0bd06b45afb0232ede7c8ed` |
| `crates/vela_vm/src/tests/reflection_members.rs:100` | `crates/vela_vm/tests/fixtures/reflection/variant_access.vela` | 760 | `51cb6cc1b574771570dc0f4a96295e2954a2c85b696c2738f21443c1a5781668` |
| `crates/vela_vm/src/tests/reflection_members.rs:163` | `crates/vela_vm/tests/fixtures/reflection/variant_permissions_denied.vela` | 514 | `e56a2ad9474b647153d72459ef05dc9526fb3e898041c8294c6ece1f848155a6` |
| `crates/vela_vm/src/tests/reflection_members.rs:198` | `crates/vela_vm/tests/fixtures/reflection/variant_permissions_granted.vela` | 883 | `b93f9d800ac987d30c2ddac3ff6d6b1c8b08eddf27d556fc4095c8d69a4484db` |
| `crates/vela_vm/src/tests/reflection_members.rs:239` | `crates/vela_vm/tests/fixtures/reflection/method_policy.vela` | 549 | `a2200c84f98c308484a32c0f5daab6e333801b70f4606cb577da5b8bbee2d088` |
| `crates/vela_vm/src/tests/reflection_modules.rs:48` | `crates/vela_vm/tests/fixtures/reflection/modules_functions_exports.vela` | 2597 | `4db64c4a1d3748e00678021ba88f757c7189777cc400e1eff02573ce8bfdec78` |
| `crates/vela_vm/src/tests/reflection_modules.rs:280` | `crates/vela_vm/tests/fixtures/reflection/export_policy.vela` | 1066 | `c6d7cd6d8e5a40034eb80a8ebc8e2a6c162bf003c9e82a2fcf73208373702e51` |
| `crates/vela_vm/src/tests/reflection_permissions.rs:42` | `crates/vela_vm/tests/fixtures/reflection/script_value_natives.vela` | 453 | `3eecf60e6d0489501d11f80dbac944c06d6ce03413b506bf4feb61d80946ad99` |
| `crates/vela_vm/src/tests/reflection_values.rs:167` | `crates/vela_vm/tests/fixtures/reflection/script_trait.vela` | 496 | `c897077351ad82fa6845c313a1aa9e05c25fa8ca0018cef103192c4aab307f9b` |
| `crates/vela_vm/src/tests/reflection_values.rs:354` | `crates/vela_vm/tests/fixtures/reflection/script_trait.vela` | 496 | `c897077351ad82fa6845c313a1aa9e05c25fa8ca0018cef103192c4aab307f9b` |
| `crates/vela_vm/src/tests/reflection_values.rs:227` | `crates/vela_vm/tests/fixtures/reflection/updated_record.vela` | 391 | `cc237ac97f65f998cda9ba78b2982d419b96bdc222a80a84dfe448d18cf02b5a` |
| `crates/vela_vm/src/tests/reflection_values.rs:402` | `crates/vela_vm/tests/fixtures/reflection/registered_script_trait.vela` | 600 | `8150569df0f5fee620183c9f423d05515d75e443042ba956e92349cc217f4233` |
| `crates/vela_vm/src/tests.rs:362` | `crates/vela_vm/tests/fixtures/reflection/reward_module.vela` | 149 | `54d722e528da2161a521969c183b5717fd8ac801f3c426409cfcc6515c96fe00` |
| `crates/vela_vm/src/tests/dynamic_method_dispatch.rs:204` | `crates/vela_vm/tests/fixtures/dispatch/runtime_script_receiver.vela` | 339 | `c39fab5df3fc9b1954986c6a90f1baaef635efece33345ab291ee133797294a7` |
| `crates/vela_vm/src/tests/dynamic_method_dispatch.rs:259` | `crates/vela_vm/tests/fixtures/dispatch/heterogeneous_receivers.vela` | 419 | `f7fd6d0289c2178577f3e5baf634d28ec0d5ba35221978fd80b83cf58105d418` |
| `crates/vela_vm/src/tests/dynamic_method_dispatch.rs:297` | `crates/vela_vm/tests/fixtures/dispatch/named_default_args.vela` | 447 | `93160fc81545d1243433dec9d23efca013dbe1b5e632888ba5c9b85c9e1ead28` |
| `crates/vela_vm/src/tests/dynamic_method_dispatch.rs:341` | `crates/vela_vm/tests/fixtures/dispatch/named_arg_errors.vela` | 675 | `52b14e851c1d1d9c35d98bf88ad7a51defb6f4e46a5907241dde13043f40e2f9` |
| `crates/vela_vm/src/tests/dynamic_method_dispatch.rs:482` | `crates/vela_vm/tests/fixtures/dispatch/standard_to_script_cache.vela` | 313 | `4be4053915224d8a87edfe06b0e62947913c57ee29201095428cfbe1c03d580f` |
| `crates/vela_vm/src/tests/dynamic_method_dispatch.rs:567` | `crates/vela_vm/tests/fixtures/dispatch/script_receiver_cache.vela` | 482 | `d406821482c8ab0b357a2143d8182fab4bb92ddfad6822c6fd13c9c999ab9363` |
| `crates/vela_vm/src/tests/linked_context_execution.rs:6` | `crates/vela_vm/tests/fixtures/dispatch/context_callback.vela` | 138 | `668afdabde7bb4d4eff2c6220115391180bbaacddb6de6fd975125e9bcc3cf9e` |
| `crates/vela_vm/src/tests/standard_map_set_id_dispatch.rs:8` | `crates/vela_vm/tests/fixtures/dispatch/map_transform_ids.vela` | 454 | `c31eae872bc9b6b3def7edd9f629386525e93bcfdcd04e901f73559a80dba3b9` |
| `crates/vela_vm/src/tests/standard_map_set_id_dispatch.rs:48` | `crates/vela_vm/tests/fixtures/dispatch/set_transform_ids.vela` | 710 | `8c95a73c5c9f7923ae07974371333ceace31d2d790c97ea0d0b1a74465ca16ec` |
| `crates/vela_vm/src/tests/standard_option_result_id_dispatch.rs:8` | `crates/vela_vm/tests/fixtures/dispatch/option_result_ids.vela` | 821 | `558a64abd1b6174d51e6c61c47a89ac69cf19aa0f6203b95e15329fa54842d9e` |
| `crates/vela_vm/src/array_methods/tests/aggregation_and_ordering.rs:225` | `crates/vela_vm/tests/fixtures/stdlib/array_ord.vela` | 1121 | `52163baca747ce58ad9f076e78370d38cc3d653106ffc786cf2564808104513b` |
| `crates/vela_vm/src/array_methods/tests/aggregation_and_ordering.rs:281` | `crates/vela_vm/tests/fixtures/stdlib/array_derived_ord.vela` | 702 | `e7852b08edfcf5596b280e46c14ff48161f438b674ac47ad55feffe5844b08d3` |
| `crates/vela_vm/src/array_methods/tests/aggregation_and_ordering.rs:316` | `crates/vela_vm/tests/fixtures/stdlib/array_sort_by_ord.vela` | 691 | `b9011e689da16f15cc7017686959c612961b90b6adf9f73f5c311b7a229125cc` |
| `crates/vela_vm/src/array_methods/tests/higher_order_and_mutation.rs:5` | `crates/vela_vm/tests/fixtures/stdlib/array_callbacks.vela` | 588 | `52e20b2147f311f41b5f33b4fb6736fde65abf859054657775327e736ea457dc` |
| `crates/vela_vm/src/array_methods/tests/higher_order_and_mutation.rs:36` | `crates/vela_vm/tests/fixtures/stdlib/managed_array_callbacks.vela` | 668 | `4ad73c006946226161b490131f9096bf80ffa9ed3399d6b105a0b0b2800d6370` |
| `crates/vela_vm/src/array_methods/tests/lookup_and_transform.rs:202` | `crates/vela_vm/tests/fixtures/stdlib/array_predicate_identity.vela` | 1020 | `aa7203a5687f7633c2fcca21980bf87d74a6685f4da6e5f38753525dfc550ba7` |
| `crates/vela_vm/src/array_methods/tests/lookup_and_transform.rs:269` | `crates/vela_vm/tests/fixtures/stdlib/managed_array_distinct.vela` | 495 | `2ec26dd194e5cd9274765abf13b0d8f9a3b48f7827ef61a113ac8c43137b707c` |
| `crates/vela_vm/src/array_methods/tests/lookup_and_transform.rs:299` | `crates/vela_vm/tests/fixtures/stdlib/array_reverse.vela` | 425 | `fc22da413cdd9e507bc626ba2fd8063500a7ba94cbca0fd49a0b1645d8328348` |
| `crates/vela_vm/src/array_methods/tests/lookup_and_transform.rs:326` | `crates/vela_vm/tests/fixtures/stdlib/managed_array_reverse.vela` | 554 | `64d6188b53a21a6133f3685a1068f04d25ced5546e9440070fdea8c116e1f613` |
| `crates/vela_vm/src/array_methods/tests/lookup_and_transform.rs:359` | `crates/vela_vm/tests/fixtures/stdlib/array_slice.vela` | 520 | `d22b0fb9664f627ddbb6dfd52a26b94e90a48442983d0d25c07137152fc5e9fb` |
| `crates/vela_vm/src/array_methods/tests/lookup_and_transform.rs:389` | `crates/vela_vm/tests/fixtures/stdlib/managed_array_slice.vela` | 536 | `2c9f502165310fd6de50cc20cbeaef308cb9ddb396984a52bddfeb70a3551085` |
| `crates/vela_vm/src/map_methods.rs:157` | `crates/vela_vm/tests/fixtures/stdlib/map_callbacks.vela` | 709 | `3bdbefe4bd050a4a819be1feec0d08795832de0afda2483e91c4bf937cba7caa` |
| `crates/vela_vm/src/map_methods.rs:185` | `crates/vela_vm/tests/fixtures/stdlib/managed_map_callbacks.vela` | 1071 | `879c86b0d0311e07b14a5d35fd396f6cb5a6d73dafbe62601f98e48085a02f02` |
| `crates/vela_vm/src/map_methods.rs:251` | `crates/vela_vm/tests/fixtures/stdlib/map_record_identity.vela` | 568 | `e2dcc4c517e0455e07ffee88ae888eeab65ad14cb9498ec84c39d7eb3cb8bbcd` |
| `crates/vela_vm/src/map_methods.rs:423` | `crates/vela_vm/tests/fixtures/stdlib/map_introspection.vela` | 561 | `aedceb0ee44f0fb619248714a3b228fb014ff919435b8fe3b423649798ec1fdd` |
| `crates/vela_vm/src/map_methods.rs:458` | `crates/vela_vm/tests/fixtures/stdlib/managed_map_introspection.vela` | 564 | `936c977acbd417a5b2eaea8911e586d5a409dfaa37dd13b6e186303adb9ce41a` |
| `crates/vela_vm/src/map_methods.rs:490` | `crates/vela_vm/tests/fixtures/stdlib/managed_map_lookup.vela` | 561 | `db5255befb3b2196537db16ac118473be4acefdd6f58d7d2c33e987289d8e96c` |
| `crates/vela_vm/src/set_methods.rs:198` | `tests/fixtures/conformance/collections/set_combinations.vela` | 931 | `dc37196944bf1f1115e73e468ce724638b32dd900d0a08bb81c4c3e922ced967` |
| `crates/vela_vm/src/set_methods.rs:233` | `crates/vela_vm/tests/fixtures/stdlib/managed_set_combination.vela` | 687 | `6dda85c6fcec79254c4c11a82627f59624ab99545956ad15fdc1a906e7b8677b` |
| `crates/vela_vm/src/set_methods.rs:268` | `crates/vela_vm/tests/fixtures/stdlib/string_set_predicates.vela` | 916 | `bf159e49c6ce01e9d935b888b4f1b14136d5a03ab40d1d3502f8af21c03e823e` |
| `crates/vela_vm/src/set_methods.rs:334` | `crates/vela_vm/tests/fixtures/stdlib/set_record_identity.vela` | 467 | `82550915c480e187b9aa6ddb167bf51943a9ee018c0cde2ea993f39632ca6603` |
| `crates/vela_vm/src/set_methods.rs:370` | `crates/vela_vm/tests/fixtures/stdlib/set_combination_identity.vela` | 879 | `9d6a9efb738ff5fc3da48a3ce74ab3f8d3a651b20ffde1f8edf5acfd1b6bd51a` |
| `crates/vela_vm/src/set_methods.rs:560` | `crates/vela_vm/tests/fixtures/stdlib/set_callback_identity.vela` | 698 | `3d5494f2e5188a0e2cb4d3ceaa47bcb92b439b3c693e20b3aa173b9121f19034` |
| `crates/vela_vm/src/string_methods.rs:161` | `crates/vela_vm/tests/fixtures/stdlib/string_utilities.vela` | 346 | `80bcff52501c68c370d78852f0c9cc9331a99598ee9193eca74f99b06833bafb` |
| `crates/vela_vm/src/string_methods.rs:186` | `crates/vela_vm/tests/fixtures/stdlib/managed_string_utilities.vela` | 372 | `b885f05d68c5d1821e3aa5246b8b21a1ffdd2dcad09449b55a27a575bda71aad` |
| `crates/vela_vm/src/string_methods.rs:626` | `crates/vela_vm/tests/fixtures/stdlib/string_scalar_parsing.vela` | 1112 | `bf60affa8c494ff3281eb661611e4960b124d28e6282c1f1f0304584d8332526` |
| `crates/vela_vm/src/stdlib/tests/core.rs:6` | `crates/vela_vm/tests/fixtures/stdlib/managed_option_filter.vela` | 537 | `60abf5e7595fa084dd6ffa23e2c01796dfe367145f6b4ebfbbfb854298d1b204` |
| `crates/vela_vm/src/stdlib/tests/core.rs:34` | `crates/vela_vm/tests/fixtures/stdlib/managed_option_result_helpers.vela` | 1723 | `f2d85f2fd2bd702cc81e2f44c954a9941602393399665602b912e7f8191699ff` |
| `crates/vela_vm/src/stdlib/tests/option_result.rs:105` | `crates/vela_vm/tests/fixtures/stdlib/option_result_native_helpers.vela` | 1461 | `410dac414874f083832a16f64145d42daf79335a646352d4c1a8b599858ece17` |
| `crates/vela_vm/src/stdlib/tests/option_result.rs:220` | `crates/vela_vm/tests/fixtures/stdlib/option_result_and_then.vela` | 700 | `98dd6128f5df28da93bbc518df06f51e026a92f0991b714c977ad46694d4c43c` |
| `crates/vela_vm/src/stdlib/tests/option_result.rs:263` | `crates/vela_vm/tests/fixtures/stdlib/option_result_or_else.vela` | 562 | `c87cffcb0a43877acfb50eb4a581b2e4d725971e8ae4eeba614d239f895978b9` |
| `crates/vela_vm/src/stdlib/tests/option_result.rs:327` | `crates/vela_vm/tests/fixtures/stdlib/option_result_helpers.vela` | 1373 | `e8b944b90d54baa23a6371f3d4f3b175721a811416dcde5c4c63752520f84df9` |
| `crates/vela_vm/src/stdlib/tests/option_result_chains.rs:6` | `crates/vela_vm/tests/fixtures/stdlib/option_ok_or_try.vela` | 405 | `1f12f16fd1ebc8d1202b1222006587560e5c98fc007da5c9c3cc3e8ecccb269a` |
| `crates/vela_vm/src/stdlib/tests/option_result_chains.rs:77` | `crates/vela_vm/tests/fixtures/stdlib/result_tuple_try.vela` | 463 | `172fc97a95308effb68f68c98816f181ee4129af2089b569100d93f6d5be0c31` |
| `crates/vela_vm/src/stdlib/tests/option_result_chains.rs:112` | `crates/vela_vm/tests/fixtures/stdlib/managed_option_result_native_helpers.vela` | 1687 | `8d610d48446fbb3be9ca5a52c7bb94ee21a55ade85bd13681359d4c8078e0154` |
| `crates/vela_vm/src/stdlib/tests/option_result_chains.rs:213` | `crates/vela_vm/tests/fixtures/stdlib/managed_option_result_and_then.vela` | 599 | `f5bdcd4f3dc4375467fa5b18b60d50549fcb919bdacb021a70fffbd242629814` |
| `crates/vela_vm/src/stdlib/tests/option_result_chains.rs:249` | `crates/vela_vm/tests/fixtures/stdlib/managed_option_result_or_else.vela` | 642 | `4438d411d3de7ba1b234551706ae2dd9eaa8c295fb5156008456568d764fe14c` |
| `crates/vela_reflect/src/script_types.rs:519` | `crates/vela_reflect/tests/fixtures/metadata/struct_enum.vela` | 336 | `a2bbe5104bb0a9af5e952b055f022fd9d1f24d74f080f24fa11ec60076fae68c` |
| `crates/vela_reflect/src/script_types.rs:924` | `crates/vela_reflect/tests/fixtures/metadata/traits_impls.vela` | 302 | `4285b5375173ecf90d2e70740b0511cf3f35a04270b283bcbb19e6edc5784493` |
| `crates/vela_reflect/src/modules/tests.rs:33` | `crates/vela_reflect/tests/fixtures/modules/functions_exports.vela` | 157 | `cd2e35d2816f3634ba07ed6f99125ebf69a0aa7a3103a7253beffc902618e9cf` |
