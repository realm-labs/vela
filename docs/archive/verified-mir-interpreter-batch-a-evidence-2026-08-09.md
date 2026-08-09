# Verified-MIR Interpreter Batch A Evidence — 2026-08-09

This report records the first valid measurement checkpoint for Batch A of the
[Verified-MIR superinstruction and basic-block interpreter plan](../verified-mir-superinstruction-basic-block-interpreter-plan.md).
It is an intermediate checkpoint, not Batch A acceptance: lead scalar rows,
runtime instruction frequencies, profiles, and the cancellation/budget boundary
are frozen, while the complete guardrail, compile-resource, artifact-size, and
Runtime/Actor-memory matrix remains outstanding.

## Invalidated captures

All earlier measurements from the overloaded machine are excluded. They are not
used for a baseline, threshold, candidate choice, or historical comparison. The
old tracked baseline and intermediate smoke captures were moved to the
recoverable temporary directory `/tmp/vela-invalid-perf.fDN1yq` before this
capture.

The capture helper now prevents the two measurement errors found during the
restart:

- runtime filters use exact names, so `--runtime vela` cannot silently include
  `vela-cache`;
- stable publication may require a clean worktree and reject a machine whose
  one-minute load per logical CPU exceeds an explicit ceiling; and
- every published capture records the commit, branch, worktree state, Rust and
  Cargo versions, platform, CPU, load, and exact quoted command.

## Valid lead baseline

The frozen raw capture is
[`perf-baselines/verified_mir_scalar_macos_aarch64.txt`](../../perf-baselines/verified_mir_scalar_macos_aarch64.txt).
It was taken from clean commit `cced68d6171c671cd58474efaca01713e1b28c53`
with Rust/Cargo 1.97.1 on an Apple M1 Max with ten logical CPUs. The one-minute
load was 9.94, or 0.994 per logical CPU, below the frozen 1.50 publication
ceiling. The command used 500,000 iterations, five measured repetitions, and
two warmups in the optimized bench profile.

| Workload | Vela mean ns/iteration | Lua 5.4 mean ns/iteration | Vela/Lua | Vela min-to-p95 spread |
|---|---:|---:|---:|---:|
| `scalar_branch_loop` | 21,961 | 2,395 | 9.170x | 3.95% |
| `range_iteration` | 66,216 | 10,168 | 6.512x | 1.59% |
| `function_calls` | 72,951 | 10,522 | 6.933x | 9.61% |
| `recursive_countdown` | 22,503 | 3,902 | 5.767x | 1.05% |
| `float_math_loop` | 28,113 | 3,819 | 7.361x | 2.22% |

The Vela lead-suite geometric mean is 36,758.58 ns/iteration. The embedded Lua
5.4 geometric mean is 5,204.45 ns/iteration, giving a within-capture ratio of
7.063x. These ratios are directional context, not a requirement that every row
match Lua.

`function_calls` is the noisiest Vela lead row. Candidate comparisons involving
it must use fresh-process interleaving and consider median/minimum as well as
mean rather than treating this five-repeat result as a narrow noise floor.

The frozen external guardrail capture is
[`perf-baselines/verified_mir_guardrails_macos_aarch64.txt`](../../perf-baselines/verified_mir_guardrails_macos_aarch64.txt).
It uses the same machine, toolchain, exact runtimes, iteration count, repeats,
warmups, cleanliness rule, and load ceiling at measurement commit
`7fd7ce7d9`. These rows are regression guards rather than scalar-selection
targets:

| Workload | Vela mean ns/iteration | Lua 5.4 mean ns/iteration | Vela/Lua | Vela min-to-p95 spread |
|---|---:|---:|---:|---:|
| `array_scan` | 279,958 | 58,274 | 4.804x | 2.27% |
| `string_methods` | 84,033 | 25,280 | 3.324x | 2.40% |
| `map_string_index_lookup_update` | 57,277 | 3,839 | 14.920x | 9.84% |
| `object_field_methods` | 58,874 | 14,085 | 4.180x | 2.01% |

The Vela Map row and the Lua string/Map rows retain high process-state noise;
the Lua min-to-p95 spreads are 17.61% and 18.27%. Their raw checksums and
directional ratios are frozen, but a later 5% retention decision must use
fresh-process interleaving with a noise floor rather than this single-process
mean. The other Vela guardrail rows are sufficiently tight for the initial
regression screen.

The frozen focused VM guardrail capture is
[`perf-baselines/verified_mir_vm_guardrails_macos_aarch64.txt`](../../perf-baselines/verified_mir_vm_guardrails_macos_aarch64.txt).
It uses the baseline harness's stable shape of seven repeats, 100 calls per
repeat, and ten warmups. All ordinary/profile/cache variants report matching
checksums and paired profile-hit counts. Representative mean sample times are:

| Boundary | Mean ns per 100-call sample |
|---|---:|
| scalar branch / budgeted scalar | 2,180,000 / 1,675,571 |
| range iteration / scalar dispatch mix | 6,666,559 / 9,155,595 |
| script small-argument call / direct closure | 7,283,946 / 14,213,589 |
| managed-heap direct closure / materialization | 14,600,988 / 4,166,887 |
| GC pacing | 29,657,202 |
| host aggregate / field read-write | 228,690 / 1,162,886 |

The capture also freezes profile-only and cache-enabled script-call, closure,
host-field, host-state, and host-aggregate detail rows. These boundaries remain
ordinary execution in the initial selector and are the inexpensive per-batch
screen for accidental non-target regressions.

## Verified-MIR and dynamic-dispatch inventory

The reproducible inventory test compiles each workload, examines its verified
MIR and linked artifact, then executes two workload iterations through the
public profiled VM entry. `profiled_dispatches` is the sum of physical linked
instruction hits, not a semantic execution-unit budget.

| Workload | MIR functions / blocks / statements | CFG edges | Budget sites | Safepoints | Trap / source points | Static dispatches | Code bytes | Profiled dispatches |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `scalar_branch_loop` | 2 / 14 / 37 | 16 | 7 | 1 | 10 / 51 | 58 | 7,424 | 7,787 |
| `range_iteration` | 2 / 14 / 35 | 16 | 10 | 1 | 6 / 49 | 63 | 8,064 | 23,947 |
| `function_calls` | 4 / 10 / 29 | 8 | 11 | 3 | 10 / 39 | 45 | 5,760 | 9,645 |
| `recursive_countdown` | 3 / 12 / 27 | 11 | 10 | 3 | 8 / 39 | 44 | 5,632 | 2,453 |
| `float_math_loop` | 2 / 8 / 34 | 8 | 7 | 2 | 9 / 42 | 48 | 6,144 | 9,781 |

Static eligible-run candidates found directly in MIR are:

| Workload | Length 2 | Length 3 | Length 4 | Length 5+ |
|---|---:|---:|---:|---:|
| `scalar_branch_loop` | 0 | 1 | 0 | 5 |
| `range_iteration` | 0 | 0 | 2 | 4 |
| `function_calls` | 1 | 1 | 0 | 2 |
| `recursive_countdown` | 2 | 0 | 0 | 2 |
| `float_math_loop` | 0 | 0 | 0 | 3 |

`scalar_branch_loop` has two verified-MIR compare-immediate-plus-branch sites.
Their existing linked `I64CmpImm` plus `JumpIfFalse` pair executes 606 times in
the two-iteration profile, 7.78% of all linked dispatches. Replacing each pair
with one MIR-selected instruction predicts 606 eliminated outer dispatches in
that run. No other lead workload contains that exact linked pair.

The test intentionally reports all snapshot mismatches in one failure so a
compiler change cannot hide later workload drift behind the first mismatch.

## Dominant sampled stacks

Each row was rebuilt and sampled separately with exact Vela runtime selection,
500,000 iterations, one measured repetition, one warmup, and a ten-second macOS
`sample` window. Raw samples live under `perf-results/profiles/` and remain
machine evidence rather than tracked product artifacts.

| Workload | Dominant main-thread stacks |
|---|---|
| `scalar_branch_loop` | frame driver 85.0%; constant loads 14.3% |
| `range_iteration` | frame driver 99.6% |
| `function_calls` | frame driver 40.2%; session driver 15.3%; `memmove` 9.3%; frame preparation 5.7%; allocation/recycle/call-argument work remains visible |
| `recursive_countdown` | frame driver 30.5%; session driver 15.4%; `memmove` 10.5%; clock read 7.3%; frame preparation 5.8%; allocation/recycle/call-argument work remains visible |
| `float_math_loop` | frame driver 78.2%; constant loads 10.5%; numeric helpers about 10.6% |

The measurements support reducing dispatch/value plumbing for scalar regions.
They do not support fusing calls in this track: the call rows expose separate
frame, copying, and allocation costs and remain guardrails for ordinary
execution. `range_iteration` is the strongest initial scalar-block/loop target;
the float row can benefit from reduced dispatch but will retain measurable
numeric-helper work.

## Cancellation, deadline, budget, and safepoint boundary

The current async call wrapper checks cancellation/deadline immediately before
polling its runtime future and again after the inner future becomes ready. The
linked frame driver executes synchronous instructions in one continuous loop.
It records profiling at each physical instruction, charges instruction metadata
or explicit `ChargeExecutionUnits` at their semantic sites, and does not poll
the outer future between ordinary scalar instructions.

Consequently the current observable schedule is:

```text
outer Runtime future poll
  -> cancellation/deadline check
  -> synchronous linked frame execution until return, suspension, or error
       -> exact MIR-derived budget charges and ordinary GC safepoints
       -> no cancellation/deadline poll merely because an instruction ran
  -> cancellation/deadline check after Ready
```

A selected unit must preserve every budget site, charged edge, safepoint split,
trap/source point, and suspension boundary. Initial pure scalar blocks do not
remove a current mid-loop cancellation poll because none exists. Scalar loop
regions may not introduce an unbudgeted backedge; adding a new mid-loop
cancellation/deadline policy would be a separate semantic decision rather than
an optimization detail.

## Candidate decision and remaining Batch A work

The first bounded vertical proof is the MIR-native
`I64CmpImmJumpIfFalse` recipe. It has two verified sites and measured dynamic
frequency, exercises definition/use, terminator, coverage, source, profiler,
and portable-artifact boundaries, and is small enough to validate the selector
without creating a second execution engine. Its individual retention still
depends on the Batch C 5% gate or demonstrated necessity for the later accepted
block family.

Batch B should build the deterministic ordinary-unit selector and independent
coverage verifier first. Batch D should prioritize the long pure scalar regions
in `range_iteration`; Batch E may then internalize only eligible natural loops
while charging every taken backedge. Call fusion remains deferred.

Batch A is not complete until the engine guardrail rows, artifact bytes,
compile time, peak compile RSS, and Runtime/Actor memory are captured and the
complete stable-checksum/geometric-mean report is archived. No runtime or
artifact behavior changed in this checkpoint.

## Validation

```bash
cargo test -p vela_vm --test integration external_compare_contract
cargo clippy -p vela_vm --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```
