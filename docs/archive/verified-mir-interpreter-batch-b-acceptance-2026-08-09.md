# Verified-MIR Interpreter Batch B Acceptance — 2026-08-09

Batch B of the
[Verified-MIR superinstruction and basic-block interpreter plan](../verified-mir-superinstruction-basic-block-interpreter-plan.md)
is accepted at implementation checkpoint `53bb193ea`. It introduces the
physical-selection and coverage-verification boundary while deliberately
emitting only the pre-existing ordinary bytecode.

## Accepted implementation

`vela_bytecode::compiler::mir_backend` now performs this sequence for every
verified root before ordinary physical lowering:

```text
MirBackendHandoff
  -> deterministic dense ordinary-unit plan
  -> independent coverage verification against sealed MIR and analyses
  -> unchanged canonical MIR-to-bytecode backend
```

The selector accepts only `MirBackendHandoff`. It records one dense ordinary
unit for each current MIR block, a block-entry map, and physical coverage for:

- exact function, block, statement order, and terminator identity;
- every statement, terminator, and charged CFG-edge budget point including
  origin, class, and unit count;
- every safepoint and its sealed live-root set;
- every CFG exit and its conditional edge charge;
- every statement/terminator source origin;
- block live-in/live-out, statement live-before/live-after, and debug
  availability; and
- the measured `I64CompareImmediateBranch` candidate reason while the unit
  remains ordinary.

The verifier is a separate module and does not call the selector's coverage
construction helper. It derives expected coverage again from the sealed MIR
and analyses, so an omission in selection cannot validate itself by sharing an
incomplete expected manifest. The canonical CFG-successor interpretation is
shared by selector, verifier, and the existing backend reachability scan.

Selection reports are test-only. Production exposes no optimization flag,
legacy route, plan mutation, HIR/source query, runtime-value query, or
bytecode-adjacency pass. Batch B plans are compile-time proof objects and are
not yet retained in ordinary or portable artifacts.

## Negative proof

Focused malformed fixtures independently corrupt and reject:

```text
missing block
duplicate block
wrong function identity
missing statement
duplicate statement
wrong terminator
invalid CFG exit
reordered/moved budget sites
swallowed safepoint roots
missing/reordered source points
missing liveness/debug coverage
invalid dense block entry
```

Repeated selection over the same handoff is structurally equal. The fixture
also retains a nested lambda and reports the measured deferred i64 immediate
compare/branch candidate while every physical unit remains ordinary.

## Canonical-output proof

The frozen external-compare contract passes without updating any snapshot.
For all five lead workloads, verified MIR counts, CFG edges, budgets,
safepoints, trap/source points, static instruction count, code bytes, dynamic
dispatch count, candidate frequencies, portable bytes/checksum, execution
checksum, and linked opcode reports remain unchanged.

## Compile-resource gate

The raw candidate capture is
`perf-results/commands/20260809T041647Z-verified-mir-batch-b-compile-candidate.txt`.
It used the same two warmups, seven fresh child samples, and 2 ms RSS sampling
as Batch A.

| Workload | Batch A mean ns | Batch B mean ns | Delta | Artifact bytes |
|---|---:|---:|---:|---:|
| `scalar_branch_loop` | 3,824,285 | 3,793,250 | -0.81% | 2,239 |
| `range_iteration` | 3,118,494 | 3,100,339 | -0.58% | 2,288 |
| `function_calls` | 3,056,863 | 3,133,077 | +2.49% | 2,301 |
| `recursive_countdown` | 2,869,113 | 2,788,404 | -2.81% | 2,065 |
| `float_math_loop` | 2,802,291 | 2,786,125 | -0.58% | 2,023 |

Every portable artifact checksum and byte length is identical to Batch A.
Mean peak child RSS is 11,375,177 bytes versus 11,377,517 before, effectively
flat.

## Loaded-runtime gate

The raw clean/load-gated candidate is
`perf-results/external_compare/20260809T041758Z-verified-mir-batch-b-runtime-candidate.txt`.
It used exact Vela selection, 500,000 iterations, five repeats, and two warmups.

| Workload | Batch A mean ns | Batch B mean ns | Delta |
|---|---:|---:|---:|
| `scalar_branch_loop` | 10,980,632,575 | 10,990,904,558 | +0.094% |
| `range_iteration` | 33,108,354,483 | 33,079,402,641 | -0.087% |
| `function_calls` | 36,475,609,558 | 36,672,642,308 | +0.540% |
| `recursive_countdown` | 11,251,672,383 | 11,563,282,358 | +2.769% |
| `float_math_loop` | 14,056,844,083 | 13,810,687,925 | -1.751% |

The five-row geometric-mean delta is +0.302%. Checksums are unchanged and no
row crosses the 5% regression gate. Because emitted and linked code is
structurally identical, these differences are measurement noise rather than a
new runtime path.

## Validation

```bash
cargo test -p vela_bytecode --all-features
cargo clippy -p vela_bytecode --all-targets --all-features -- -D warnings
cargo test -p vela_vm --test integration external_compare_contract
cargo fmt --all -- --check
git diff --check
```

Batch C may now replace only measured candidate units, starting with the
MIR-native i64 immediate compare plus conditional branch proof. Artifact
portability moves atomically from version 4 to version 5 when that selected
representation becomes serialized.
