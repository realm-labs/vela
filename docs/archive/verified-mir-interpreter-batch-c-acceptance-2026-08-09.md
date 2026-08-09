# Verified-MIR Interpreter Batch C Acceptance — 2026-08-09

Batch C of the
[Verified-MIR superinstruction and basic-block interpreter plan](../verified-mir-superinstruction-basic-block-interpreter-plan.md)
is accepted at implementation checkpoints `608b884c7` and `d2684bb2b`. It
retains the first measured MIR-native short recipe and atomically moves every
portable artifact surface to format version 5.

## Accepted implementation

The selector recognizes an i64 comparison against an immediate whose result
has exactly one use in the same block's conditional terminator. It proves the
shape directly from verified MIR definition/use, facts, liveness, safepoints,
and terminator structure. An immediate on the left is normalized by reversing
the comparison. Bytecode adjacency is never inspected.

For an accepted unit, the backend omits the ordinary compare statement and
branch terminator and emits one `I64CmpImmJumpIfFalse`. The existing opcode's
unlinked and linked representations, linker projection, verifier rules,
execution helper, cache/profile classification, and disassembly shape remain
the sole runtime path. There is no production peephole, selector flag, fallback
interpreter, or second frame driver.

Each selected physical unit retains:

- its linked instruction offset and selected recipe kind;
- the exact covered MIR statement and terminator identities in process;
- two logical source points and two CFG exits;
- the exact two-operation and two-logical-budget-unit coverage; and
- a portable canonical form that strips process-local MIR identities while
  preserving independently verifiable physical coverage.

The unlinked and linked verifiers reject missing or inconsistent operation,
source, exit, budget, kind, and offset coverage. Source linking separately
checks the selected source points against the exact verified MIR function.
Portable decode applies the format limits and rejects a checksum-valid artifact
whose selected coverage has been corrupted.

## Artifact version 5 hard switch

The ordinary portable program, portable Service artifact, and Service
deployment bundle all move from version 4 to version 5 in the same checkpoint.
Versions 1 through 4 reject at all three boundaries. There is no old reader,
load-time MIR reconstruction, inferred plan, or compatibility expansion.

Canonical `from_compiled`, encode/decode, link, and `from_linked` round trips
retain the selected plan. The Service proof compiles a selected branch, loads
it through the Service artifact boundary, proves the selected unit remains
present, and executes it. Canonical portability also clears process-local frame
slot names, so the linked-to-portable round trip is representation-stable.

## Structural dispatch proof

The frozen `scalar_branch_loop` inventory predicted 606 dynamic hits across
two MIR sites. The accepted artifact has exactly that shape:

| Metric | Batch A | Batch C | Change |
|---|---:|---:|---:|
| Static linked instructions | 58 | 56 | -2 |
| Dynamic outer dispatches | 7,787 | 7,181 | -606 |
| Selected-unit hits | 0 | 606 | +606 |
| Linked code bytes | 7,424 | 7,168 | -256 |

The exact predicted dispatches disappear, while the execution checksum remains
unchanged.

## Runtime retention gate

The focused clean/load-gated capture is
`perf-results/external_compare/20260809T045130Z-verified-mir-batch-c-scalar-candidate.txt`.
At 500,000 iterations, five repeats, and two warmups,
`scalar_branch_loop` improves from 10,980,632,575 ns to 10,008,411,008 ns,
or **8.86%**, passing the recipe's 5% focused retention gate.

The complete five-row capture is
`perf-results/external_compare/20260809T045312Z-verified-mir-batch-c-runtime-candidate.txt`.

| Workload | Batch A mean ns | Batch C mean ns | Delta |
|---|---:|---:|---:|
| `scalar_branch_loop` | 10,980,632,575 | 9,849,382,275 | -10.302% |
| `range_iteration` | 33,108,354,483 | 32,730,217,941 | -1.142% |
| `function_calls` | 36,475,609,558 | 36,409,500,466 | -0.181% |
| `recursive_countdown` | 11,251,672,383 | 11,421,278,608 | +1.507% |
| `float_math_loop` | 14,056,844,083 | 14,209,818,366 | +1.088% |

The scalar-suite geometric mean improves from 18,379,533,498 ns to
18,029,146,709 ns, or **1.906%**. Every checksum is unchanged and no lead row
regresses by 5%.

The target-independent external guardrails were captured in
`perf-results/external_compare/20260809T050538Z-verified-mir-batch-c-external-guardrails.txt`:

| Workload | Delta from Batch A |
|---|---:|
| `array_scan` | -3.491% |
| `string_methods` | -2.632% |
| `map_string_index_lookup_update` | -2.382% |
| `object_field_methods` | +1.450% |

All guardrail checksums remain unchanged.

## Boundary and resource guardrails

The VM guardrail rerun is
`perf-results/commands/20260809T054105Z-verified-mir-batch-c-vm-guardrails-rerun.txt`.
Its target scalar rows improve by 6.42% and 7.75%; the largest non-target mean
regression is 3.02%. Engine interop was rerun in
`perf-results/commands/20260809T054130Z-verified-mir-batch-c-engine-interop-rerun.txt`;
its largest regression is 4.31%, and all checksums match.

Service, async, scoped-task, Actor memory, allocation, and concurrency captures
are retained under the corresponding
`perf-results/commands/20260809T0536*-verified-mir-batch-c-*` and
`20260809T0537*-verified-mir-batch-c-*` files. The Service rerun is
`20260809T054030Z-verified-mir-batch-c-service-boundary-rerun.txt`, and the
stable scoped-task rerun is
`20260809T053952Z-verified-mir-batch-c-scoped-task-stable-rerun.txt`.

- Service checksums and every allocation/byte count are identical; the largest
  timed Vela-boundary regression in the rerun is below 5%.
- Async checksums match; all ordinary async rows are within 3.74%, while the
  deep-call row improves by 30.68%.
- The first scoped-task capture had one scheduling outlier. Its stable rerun
  places that row at +3.07%, with all checksums and pool counts unchanged.
- Actor peak RSS remains within 2.5% across every shape/profile/count row.
- Actor allocation checksums match and allocation counts decrease by exactly
  one per hot/cold actor call in the measured one-worker workloads.
- Actor concurrency checksums, pending results, and overlap proof remain exact.
  Two immediate reruns move the initially slow two-worker cold row back within
  3.6% of baseline and then 2.5% faster, demonstrating scheduler variance
  rather than a stable regression.

No target-independent guardrail shows a stable regression above 5%.

## Compile-resource and portable-size gate

The clean candidate capture is
`perf-results/commands/20260809T053313Z-verified-mir-batch-c-compile-candidate.txt`.

| Workload | Batch C compile mean ns | Delta | v5 artifact bytes | Size change |
|---|---:|---:|---:|---:|
| `scalar_branch_loop` | 3,962,845 | +3.623% | 2,377 | +138 |
| `range_iteration` | 3,380,583 | +8.404% | 2,304 | +16 |
| `function_calls` | 3,252,809 | +6.410% | 2,333 | +32 |
| `recursive_countdown` | 2,887,577 | +0.644% | 2,089 | +24 |
| `float_math_loop` | 3,081,458 | +9.962% | 2,039 | +16 |

The v5 size increase is the selected-plan/coverage representation and new
format framing, not retained MIR. Mean peak child RSS is 11,534,336 bytes,
1.38% above Batch A. Compile timing is recorded for the later whole-track size
and throughput decision; Batch C has no compile-time rejection threshold.

## Validation

```bash
cargo test -p vela_bytecode --all-features
cargo clippy -p vela_bytecode --all-targets --all-features -- -D warnings
cargo test -p vela_vm --all-features
cargo test -p vela_engine --all-features
cargo clippy -p vela_vm -p vela_engine --all-targets --all-features -- -D warnings
cargo test -p vela_vm --test integration external_compare_contract
cargo fmt --all -- --check
git diff --check
```

Batch D may now add compact scalar blocks only through the same verified MIR
selection and physical coverage boundary. It must retain the one frame driver,
ordinary hard-boundary helpers, exact source/budget semantics, and artifact v5
canonicality established here.
