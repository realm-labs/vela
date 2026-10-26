# SF11 Benchmark And Fuzz Workload Slice

Status: implemented, uncommitted handoff for the parent stage owner. This is only
SF11's benchmark/fuzz slice, not acceptance of the separate site/editor/script
work. No shared status document, production crate code, manifest, dependency,
feature, performance capture or tracked malformed corpus was changed. No commit
or staging was performed.

## Review And Storage Decisions

Read the input-organization plan and the runtime, tooling and consumer census
benchmark/fuzz sections. Inspected the initial diff; pre-existing parent changes
outside this slice were preserved. Before extraction, the existing VM external
comparison contract passed all 11 tests and the baseline contract passed both
existing tests.

| Reviewed family / consumers | Disposition and preserved boundary |
|---|---|
| VM `baseline/workload_sources.rs`: collections, state, materialization, record triplets/quads/quints/sextets, callbacks, arrays, bytes, maps and Option/Result | File-backed under `crates/vela_vm/benches/fixtures/baseline`. These are authored programs, often shared by several execution modes, not Rust scale factories. Keep constant names and separate typed/runtime/instrumentation rows. |
| VM `baseline/workload_sources/{calls,ranges,strings,iterators,sets,typed_containers}.rs` | File-backed complete programs. Includes direct/script/native calls, dynamic mono/poly/cache-miss dispatch, script/trait methods, Unicode chars/bytes and host iterable setup, string escapes, set operations and static/dynamic/erased container contracts. All six nested modules reviewed, not only the top-level constants. |
| VM `baseline/workloads.rs` | Extract scalar branch, range, scalar-dispatch mix and GC-pacing sources. The inline and budgeted scalar rows share one file; IDs and modes remain distinct. `workloads/{collection_families,iterator_sequence}` remain Rust registration tables over the same constants. |
| VM `baseline.rs`, configuration/report/cache-delta/cache-support/VM-state helpers; integration `baseline_cache_delta` and target `baseline_workload_contract` | Retain Rust orchestration, native registrations, argument/heap factories, checksums, warmups, GC/state lifetime and cache-mode distinctions. No runner consolidation, timed file loading or new baseline oracle framework. |
| VM `external_compare/workloads/{core,value_keyed,extended}.rs` | All 22 records retain all five language inputs. File-backed `.vela`, `.lua`, `.rhai`, `.js`, `.py` under `benches/fixtures/external_compare`; same evaluated bytes and source order. Consumers remain `external_compare`, `verified_mir_compile` and the existing integration contract. |
| VM `verified_mir_compile.rs` | Retain five lead IDs, child compile/encode route and inventories. Loads the same source records at compile time; no extra authored source or change to compile measurement boundaries. |
| VM `scalar_block_allocations.rs` | Retain hand-built linked-code/scalar-plan unit data, global allocator and one-versus-10,001 iteration result/allocation checks. Not a static script extraction candidate. |
| Engine `hot_reload.rs` | Initial, accepted update and ABI-rejected source files stay together in `benches/fixtures`. Version identity, staged activation, report checksum and compile/apply versus compile/reject timing stay in Rust. |
| Engine `interop.rs` | Extract the four-function owned-value/native/re-entry program. Keep registrations, generated root binding, arguments, checksums and per-call boundaries in Rust. |
| Engine `actor_concurrency.rs` and `actor_memory.rs` | Share their byte-identical `actor_turn.vela`; extract the substantial dynamic contention program and fixed small-memory program. Retain fresh/shared/isolated runtime ownership, barriers, allocator/RSS/child bounds and result collection. |
| Engine `actor_memory::memory_source(Large)` and `generation_memory::{top_level_source,call_heavy_source,lambda_source}` | Retain genuine generators: function count, generation and closure shape are the measurement dimension. No static stand-in or generated-source template extraction. |
| LS `incremental_latency.rs::{module_text,module_document}` | Retain module/function/edit-salt generation, logical paths, edited module 17 and caret coordinates. Retain the short inline `SUPPORT_TEXT` constant/helper pair. No LS edits: cold indexing, edit/query/snapshot timing and environment-controlled rounds remain unchanged. |
| Fuzz `portable_plan::base_artifact` | Extract only the fixed 198-byte cooked source into `fuzz/fixtures/portable_plan/base.vela`; no final newline. Preserve immutable `OnceLock` encoded bytes, fresh Engine compilation and portable-v8 plan/coverage availability. |
| Fuzz parser, direct portable decode, `fuzz_value`, scalar/handle/source/coverage/exit mutations and `encode_unchecked` | Retain generated UTF-8/parser and binary mutation inputs, direct decoder route, fixed-int little-endian encoding and trailing-byte rejection. No claim every mutated artifact rejects. |
| Fuzz `corpus/portable_plan` | Specialized corpus unchanged: 35 tracked files, 239 bytes, including all six named malformed/control seeds and 29 hash-named seeds. Smoke runs use a copy under ignored `fuzz/target`, never rewrite the authored corpus. |

### Sharing And Package Reachability

Within the VM, three Rhai map rows share `map_lookup_update.rhai`. The ordinary
set and string-keyed set rows share their exact five language files. Other
language implementations are not merged merely because results agree. Baseline
inline/budgeted scalar rows share one VM-local source; both Engine actor benches
share one Engine-local source.

The existing bytecode-local `tests/fixtures/compiler/scalar_loop.vela` (265 bytes)
and `computed_slice.vela` (395 bytes) exactly match the VM scalar and array-slice
inputs. Cross-package includes were **not** adopted: a VM Cargo package cannot
include a sibling bytecode package's test paths, and there is no existing
package-safe shared origin/distribution-copy mechanism for these inputs. Keep
independent owner-local inputs rather than adding a production dependency, a
symlink, a fallback copy loader or a new packaging system. The two VM scalar modes
still share their VM-local input. Bytecode files were not modified.

`cargo package --list --allow-dirty --no-verify` succeeded for VM, Engine, LS and
fuzz (only existing missing-package-metadata warnings). The 177 VM, seven Engine
and one fuzz extracted source files are package-listed. An isolated tree under
ignored `fuzz/target/sf11-package` copied the current workspace manifests/lockfile,
path dependencies and existing root test inputs; VM/Engine/LS files were pruned
to their Cargo package lists. Both the three-owner benchmark check and the separate
fuzz-bin check passed from that tree, offline with separate target directories.
This proves source reachability with the repository's path-dependency topology,
not crates.io publication or dependency-version normalization.

## Exact Byte Evidence

Extraction copied raw-string payloads without trimming, reindentation or EOF
normalization; the two cooked literals were decoded before writing. A subsequent
independent audit read pre-change literals from `git show HEAD:<path>`, compared
per-file multisets of evaluated bytes to every new `include_str!` target and
checked that every extracted source has a consumer. All **194 sites / 185 unique
source files** matched. Existing logical filenames, IDs and spans are unchanged.
Scoped fixture `.gitattributes` use `text eol=lf`; `git check-attr` confirmed both
attributes for VM/external, VM/baseline, Engine and fuzz fixtures. No workspace
renormalization or source formatter was run.

Hashes below are SHA-256 over sorted project-relative paths, each followed by NUL
and the file bytes, excluding `.gitattributes`. They are extraction evidence,
not benchmark semantic expectations or a new reporting format.

| Family | Sites | Files | Unique bytes | SHA-256(path NUL bytes) |
|---|---:|---:|---:|---|
| VM baseline | 75 | 74 | 48,336 | `8869ef150bc7431115ec65582356f0e5373702a8b3f43ff428e273190c508988` |
| VM external comparison | 110 | 103 | 53,226 | `f3ff2aa68ddab32a96764aca0cd2ed420cb464d0e47cb1a1760ea2b2ba9a5f56` |
| Engine | 8 | 7 | 1,745 | `08108487ec794e53213fb99cb5868d0619cb2b2337cafdcb9797d8082cb98116` |
| Fuzz static base | 1 | 1 | 198 | `9695c6fab33daa5441c30578baba91f1f3f268aa1a4dd29c342a9e248f882702` |

All 35 tracked corpus files were separately compared byte-for-byte with HEAD.
The raw-string `\n`, `\r`, `\0`, Unicode and byte-literal escapes remain source
text where they originally were source text; the cooked fuzz seed still has no
EOF newline.

## Independent Semantic Preflight

`external_compare/preflight.rs` owns independent integer expectations. Every
selected runtime/workload validates raw results at **0, 1 and 2 iterations** before
warmup or timing. `--preflight-only` performs those checks without measurement
rows. Vela preflight requires an actual `ScalarValue::I64`, not a mixed/fallback
checksum. Lua preflight inspects `mlua::Value` and accepts only `Value::Integer`:
no floating-point truncation or numeric-string coercion. Rhai requires an integer
return, and process output must parse as an integer. Lua's existing warmup/timed
`i64` conversion remains unchanged, as do all timed checksum mixing, seeds,
workload/runtime IDs and measurement modes.

The oracles were derived from the workload arithmetic/collection semantics,
not captured from Vela, another runtime, or a benchmark checksum. One-iteration
results below are multiplied by the preflight iteration count; each workload's
`run_once` owns fresh local state.

| Workload(s) | One iteration | Independent derivation |
|---|---:|---|
| `scalar_branch_loop` | 11,940 | Twice multiples of 3 through 180, plus `(5v) mod 17` for other values through 180; 181 takes the break. |
| `range_iteration` | 67,040 | `8 × (128×127/2) + 64×63/2`. |
| `function_calls` | 116,882 | Start `s=0`; 240 steps `s'=s+4k+1+((s+k+1) mod 17)` for `k=0..239`. |
| `array_scan` | 10,578 | Sum `19 + 5(k mod 5) + (4k mod 17) + (2k mod 17) + (6k mod 17)` for `k=0..199`. |
| `map_lookup_update`, `map_string_index_lookup_update`, `map_string_key_lookup_update`, `map_i64_key_lookup_update` | 15,722 | `120×11 + Σ(120-k)(k mod 5) + 122`, `k=0..119`; alias/key spelling does not change the recurrence. |
| `map_record_identity_lookup_update` | 10,465 | `96×11 + Σ(96-k)(k mod 5) + Σ((11+k) mod 7)`, `k=0..95`; mutation preserves Alice's key identity and the equal-shaped copy is absent. |
| `set_lookup_mutation`, `set_i64_lookup_mutation`, `set_string_lookup_mutation` | 475 | Final cardinality two each step: `96×2 + 13×21 + 10`. |
| `set_i64_large_lookup_mutation` | 12,858 | Cardinality 64 remains after temporary insertion/removal: `192×64 + 27×21 + 3`. |
| `set_record_identity_lookup_mutation` | 670 | Cardinality one, mutated level 11: `96×(1+4) + 19×10`; Bob and the equal-shaped copy are absent. |
| `string_methods` | 1,391 | Four labels total 19 characters; three qualify for the bonus: `50×19 + 3×7×21`. |
| `closure_callbacks` | 45,008 | Start `s=0`; for 120 steps calculate `a=s+2k`, `b=a+k+(a mod 17)`, `s'=b+3k+(b mod 23)`. |
| `recursive_countdown` | 1,620 | Sum triangular numbers `n(n+1)/2` for `n=16..23`. |
| `nested_collection_allocation` | 11,104 | Sum `5k+16` for `k=0..63`. |
| `object_field_methods` | 9,788 | Sum `2k + 2(k mod 5 + 1) + k mod 3` for `k=0..95`. |
| `string_build_split_join` | 2,649 | Joined length 22: `72×32 + 6×55 + 15`. |
| `float_math_loop` | 19,605 | Round `(256×1.25 + 0.5×256×255/2)×7/6 + 256×0.75`; safely away from a half-integer rounding boundary. |
| `array_transform_sort` | 10,388 | Five tick-residue subtotals (filtered sum plus min/max) are `[190,121,140,217,142]`; 13 copies of the first four and 12 of the last. |

The existing integration target now adds twelve tests: exact 22-ID oracle
inventory, all embedded/cache preflights, wrong raw values/types, embedded limits,
all available process preflights plus execution/parse/syntax/wrong-result failures,
optional-versus-required selection, bounded stdout/stderr/time capture, Node
version syntax, rejection of a successful non-Node version-probe response,
invalid explicit runtime selections, supported/default selection semantics, and
runner rejection before workload discovery in both execution modes.
Wrong-type coverage includes Lua returning `11940 * iterations + 0.75` and
`tostring(11940 * iterations)`; both must fail the raw integer-type check.
Existing two-iteration cross-runtime equality, compile/link and fixed opcode/MIR
inventories remain complementary tests, not replaced by the new table.

Vela preflight limits are 1,000,000 execution units, 16 MiB and call depth 256.
Lua uses a hook every 1,000 instructions with a 1,000-hook allowance (failure on
the next hook); Rhai uses 1,000,000 operations and existing depth limits. These
limits apply only to preflight; timed embedded execution is unchanged. Limit
regressions assert actual runtime budget/hook/operation failures, not merely a
compile rejection.

Process discovery/version has a five-second bound, each preflight child ten
seconds, and each existing warmup/measurement child 120 seconds. Capture caps each
stdout/stderr stream at 64 KiB, closes stdin, propagates malformed/nonzero output,
and kills/reaps the direct child on failure. Waiting for child/pipe completion is
bounded, including after child exit. This is not a general process-tree sandbox:
a runtime spawning unrelated descendants is outside the benchmark contract;
reader threads may survive until such inherited pipes close. The bounded capture
uses 1 ms polling inside the existing whole-process timing window; no performance
comparison or claim is made about that implementation overhead.

Only an omitted `--runtime` selects the default all-runtime behavior. Every
explicit selector is validated against `vela`, `vela-cache`, `lua54`, `rhai`,
`node`, and `python3` before headers, workload discovery or execution. Unknown
IDs (including a valid/unknown mixture), empty or whitespace-only values, missing
values and empty comma-separated entries reject the entire invocation. Both
`--runtime=...` and `--runtime ...`, including repeated options, follow this rule;
no valid prefix can run while an invalid selector is ignored.

Missing optional executables (or a Python-2-only fallback) may still be skipped
in the default all-runtime selection. An explicitly requested missing runtime fails.
Node's successful version response must have stable-release `vMAJOR.MINOR.PATCH`
syntax: three nonempty ASCII decimal components without leading zeros, apart
from a single `0`. Surrounding whitespace is allowed; quotes, extra tokens/lines,
missing components and prerelease/nightly suffixes are rejected before sanitizing
report text. This validates the supported version format, not executable identity
or a minimum Node feature version. Existing Python major-version selection is
unchanged. An available runtime with a failed probe or unsupported version format,
compilation, execution, parse or semantic result fails instead of being reported
as missing. A later
measurement error still prints the existing `status=error` row, but now returns a
nonzero process status rather than continuing to `Ok(())`.

## Validation And Limits

All commands were bounded externally; no long performance run was performed.
The initial-slice commands below use the checkout's installed stable toolchain
unless marked nightly; review-follow-up validation is recorded separately below. Actual optional runtimes were Node `v22.20.0` and Python `3.9.6` on
macOS/aarch64.

- `cargo test -p vela_vm --test integration --test baseline_workload_contract --all-features --locked -- --test-threads=4`: **26 + 2 passed**; the integration total includes **18 external-comparison tests**. The same final command without `--all-features` also passed **26 + 2**. Original default comparison/baseline tests passed before extraction.
- `cargo test -p vela_vm --lib --locked -- --test-threads=4`: **827 passed**.
- `cargo test -p vela_engine --test integration --all-features --locked -- --test-threads=4`: **64 passed**.
- `cargo test -p vela_engine --lib --all-features --locked -- --test-threads=4`: **443 passed**.
- `cargo test -p vela_language_service --lib incremental:: --locked -- --test-threads=4`: **19 passed, one existing ignored**.
- `cargo bench -p vela_vm -p vela_engine -p vela_language_service --benches --no-run --locked`: passed, all ten declared benchmark targets compiled; no measurement execution.
- `cargo check -p vela_vm -p vela_engine -p vela_language_service --benches --all-features --locked`: passed.
- Release `external_compare --preflight-only --runtime=vela,vela-cache,lua54,rhai,node,python3`: passed all 22 workloads, no timed rows, 30-second outer bound. The normal header retains configured measurement defaults; preflight does not execute those defaults.
- Bounded CLI failure injection with temporary fake Node executables verified actual nonzero exit for wrong raw value, malformed output, child failure, broken version probe, explicitly missing Node, and a child failing only after successful preflight. Default optional-missing selection exited zero with `status=missing`. The post-preflight injected failure attempted one sample at three iterations and produced no successful timing result; no capture/baseline was generated.
- `cargo check --manifest-path fuzz/Cargo.toml --bins --locked`: passed both targets with existing codec/test-support features.
- `cargo +nightly fuzz run parser fuzz/target/sf11-smoke/parser -- -runs=128 -max_total_time=10 -timeout=2 -max_len=1024 -artifact_prefix=fuzz/target/sf11-smoke/artifacts/`: **128 runs passed** from an empty disposable parser corpus.
- Same bounded command for `portable_plan` using `fuzz/target/sf11-smoke/portable_plan`: **128 runs passed**, starting from the copied 35-file/239-byte corpus. These are finite smoke tests, not a fuzz campaign or new coverage claim.
- Isolated `cargo check --manifest-path fuzz/target/sf11-package/Cargo.toml -p vela_vm -p vela_engine -p vela_language_service --benches --locked --offline --target-dir fuzz/target/sf11-package/build`: passed. Isolated fuzz `--bins --locked --offline` check also passed with its own target directory.
- Scoped Clippy passed for VM benches/integration/baseline contract and Engine/LS benches, all features, `-D warnings`. New-test `unwrap` diagnostics were corrected; no suppression was added. The embedded-limit negative test was corrected to use valid recursive Vela source and assert the typed call-depth failure rather than accepting a syntax rejection.
- Scoped `rustfmt --edition 2024 --config skip_children=true --check` for changed/new Rust files and scoped `git diff --check`: passed. **No workspace formatting command** was run.

A combined owner-test command reached its 180-second outer timeout while running
LS tests. A separate full LS `--lib -- --test-threads=4` attempt reached its
120-second limit. Neither is an aggregate pass; the focused incremental test
result above is the relevant bounded proof for the unchanged LS benchmark family.
No larger timeout was attempted. Full workspace/conformance/installed-editor,
site and browser gates were not run or claimed by this slice.

### Review Follow-Up: Strict Lua Values And Node Version Format

The fractional Lua regression was run against the previous implementation and
failed because preflight incorrectly returned success. Preflight now requests
`mlua::Value`, rejects every non-Integer variant, and compares the integer exactly
to the independent oracle. Both requested wrong-type cases are retained in the
existing negative test. Timed Lua calls still request `i64`; no workload input,
timing boundary or checksum changed.

Node validation now rejects `not-a-node-version` even when the version command
exits successfully. Parser tests cover valid stable versions and malformed,
quoted, multiline, non-ASCII and prerelease responses; a bounded process test
exercises `ProcessRuntime::locate` with a successful invalid response and requires
an error, not an optional-runtime skip.

Fresh bounded follow-up validation:

- `cargo test -p vela_vm --test integration external_compare_contract --all-features --locked -- --test-threads=4`: **20 passed**, 60-second outer bound. The same command without `--all-features` also passed **20**.
- `cargo bench -p vela_vm --bench external_compare --no-run --locked`: passed, 120-second outer bound.
- Rebuilt release `external_compare --preflight-only --runtime=vela,vela-cache,lua54,rhai,node,python3`: passed **all 22 workloads in all six modes**, each at 0/1/2 iterations, within a 30-second outer bound. Node `v22.20.0`, Python `3.9.6`; no measurement rows or performance run.
- `cargo clippy -p vela_vm --bench external_compare --test integration --all-features --locked -- -D warnings`: passed.
- Scoped `rustfmt --edition 2024 --config skip_children=true --check` and scoped whitespace checks passed. No workspace formatting, other-owner edits, staging or commits.

Initial-slice owner/fuzz/isolation results above are retained historical evidence,
not reruns of those broader gates for this focused review correction.

### Residual Audit: Reject Invalid Explicit Runtime Selections

Before correction, bounded release invocations with `--runtime=not-a-runtime`,
`--runtime=vela,not-a-runtime`, and `--runtime=` all reproduced exit status zero.
`BenchConfig::from_iter` now returns a result and validates every explicitly
provided token instead of dropping empty tokens or silently ignoring unknown IDs.
The existing runner body moved to benchmark-local `external_compare/runner.rs`;
the executable and integration contract use that same entry path. Parsing must
finish successfully before the header or any workload/runtime activity. Timed
loops, workload bytes, checksums and default optional-missing semantics did not
change.

Two config tests cover all six supported IDs, default versus explicit selection,
unknown-only and mixed selections, both flag spellings, missing/empty/whitespace
values, repeated options and empty comma-separated entries. The runner regression
passes unknown-only, mixed and empty selections through the real runner in both
preflight and measurement modes, with both flag spellings. A deliberately
unmatched workload filter proves selection rejection precedes workload discovery
rather than being an unrelated no-workload error.

Fresh bounded residual-audit validation:

- `cargo test -p vela_vm --test integration external_compare_contract --locked -- --test-threads=4`: **23 passed**; the same focused contract with `--all-features` also passed **23**, within the 60-second combined test/Clippy bound.
- `cargo bench -p vela_vm --bench external_compare --no-run --locked`: passed without warnings, 120-second outer bound.
- Rebuilt release `external_compare --preflight-only --runtime=vela,vela-cache,lua54,rhai,node,python3`: passed **22 workloads × six modes × 0/1/2 iterations**. Node `v22.20.0`, Python `3.9.6`; no timed rows.
- Actual executable regressions rejected unknown-only, mixed, empty, whitespace-only, comma-only and trailing-empty values in both flag spellings, plus a missing value. Each returned nonzero with a selection diagnostic and **empty stdout**, confirming rejection before the normal header. With an empty `PATH`, omitted selection still succeeded with both process runtimes reported missing, while explicit `node` failed as unavailable. These subprocess checks and the six-mode preflight completed within one 30-second outer bound; each child also had a 10- or 15-second bound.
- `cargo clippy -p vela_vm --bench external_compare --test integration --all-features --locked -- -D warnings`: passed. Scoped Rust formatting and whitespace checks passed; no workspace formatting.

Earlier owner/fuzz/isolation results remain historical evidence, not fresh reruns
for this selector correction. No files outside the original workload slice were
edited; nothing was staged or committed.

Parent owns stage staging/commit and any shared acceptance/status update. The
remaining SF11 editor/site/script work is outside this file's acceptance claim.
