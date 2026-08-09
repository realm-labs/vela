# Verified-MIR Interpreter Batch D Acceptance — 2026-08-09

Batch D of the
[Verified-MIR superinstruction and basic-block interpreter plan](../verified-mir-superinstruction-basic-block-interpreter-plan.md)
is accepted at implementation checkpoints `e61c18c89`, `750b40b86`,
`9f839142a`, `b46a5c59c`, `7d3fdbc4c`, and `8fb0b7ab8`. It adds compact scalar
basic blocks to the one production interpreter and passes the quiet-machine
semantic, performance, allocation, artifact, and host-boundary gates.

## Accepted physical model

Each `LinkedCodeObject` owns immutable, bounded scalar-plan tables addressed by
a dense `ScalarBlockPlanId`. One `RunScalarBlock` instruction enters a focused
executor from the existing frame driver. A plan contains compact Bool/i64
operations, a fused exit, exact logical source subpoints, ordered operation and
edge charges, and bounded target offsets.

Selection consumes only `MirBackendHandoff`. It gives retained short
superinstructions priority, then accepts a complete block when all of these are
true:

- the block belongs to a verified CFG cycle;
- it contains at least three eligible Bool/i64 operations;
- it contains no call, allocation, safepoint, HostAccess, reflection, state,
  task, await, or other hard boundary; and
- it ends in a Jump or a Branch consuming the last operation's proven Bool
  result.

The cyclic rule is the initial profitability boundary: setup blocks executed
only once or a few times remain ordinary, while their enclosing hot cyclic
blocks retain essentially all measured scalar-block hits. The independent
selection verifier re-derives cycle membership rather than trusting the
selector. Batch E owns wider natural-loop region formation.

Unlinked, portable, and linked verifiers reject invalid plan handles,
registers, constants, targets, source coverage, budget coverage, duplicate or
orphan plans, malformed exits, and excess table sizes. Source linking
independently compares every selected operation, terminator, edge, source, and
budget placement with the exact same-generation verified MIR. Portable v5
canonicalization removes process-local MIR IDs but retains all physical plan
metadata required to load and verify without MIR. Versions 1-4 remain rejected
for ordinary, Service, and deployment artifacts.

## Executor and unsafe boundary

The executor borrows the fixed frame register slice once, checks runtime tags,
uses the canonical checked i64 helpers, preserves writes completed before a
later trap, and reports the exact failing source subpoint. Budgeted and
unbounded frame-driver specializations remain separate. Profiled execution
records every logical operation and exit without introducing a second
interpreter or mutable plan state.

Two private unchecked register-slot helpers are the only new unsafe boundary.
Their precondition is established by unlinked, portable, and linked plan
verification plus the fixed, non-resizing frame register count. A malformed
typed entry still returns a structured type error before unchecked slot reuse.
Miri is unavailable on the installed stable Rust 1.97.1
`aarch64-apple-darwin` toolchain; the verifier matrix, malformed-entry test,
fixed-frame audit, and full VM suite are the executable proof on this machine.

## Semantic differential proof

Focused ordinary-versus-selected fixtures cover:

- successful add/multiply execution and identical output;
- exact budget success and exhaustion at the exit source;
- checked overflow, partial-write order, and exact source span;
- true/false fused branches;
- a loop's continue backedge and break exit; and
- malformed entry tags returning a type mismatch rather than panic or UB.

Profiler tests prove one logical subpoint per operation and exit. Production
control-flow tests continue to cover authored break/continue lowering, while
the focused selected fixture executes the same continue/break targets through
`RunScalarBlock` and compares its result with ordinary instructions.

## Structural dispatch proof

The stable inventory at `7d3fdbc4c` is:

| Workload | Batch A static / profiled dispatches | Batch D static / profiled dispatches | Selected blocks / ops / hits |
|---|---:|---:|---:|
| `scalar_branch_loop` | 58 / 7,787 | 40 / 4,167 | 2 / 12 / 362 |
| `range_iteration` | 63 / 23,947 | 43 / 6,699 | 3 / 15 / 2,192 |

On covered blocks, scalar replaces 12 operations plus two terminators with two
outer entries, an 85.7% reduction. Range replaces 15 operations plus three
terminators with three entries, an 83.3% reduction. Total profiled outer
dispatch falls 46.5% and 72.0% respectively. Checksums are unchanged.

The non-target lead artifacts are deliberately ordinary again:
`function_calls`, `recursive_countdown`, and `float_math_loop` have their Batch
A static instruction counts and no selected scalar block. `RunScalarBlock` is
appended after existing instruction variants so pre-Batch-D variant ordering is
preserved.

## Runtime retention gate

The final clean/load-gated capture is
`perf-results/commands/20260809T073747Z-verified-mir-batch-d-cyclic-runtime-candidate.txt`.
It uses 500,000 iterations, five repeats, and two warmups at `7d3fdbc4c`.

| Workload | Batch A mean ns | Batch D mean ns | Delta |
|---|---:|---:|---:|
| `scalar_branch_loop` | 10,980,632,575 | 7,303,558,274 | -33.487% |
| `range_iteration` | 33,108,354,483 | 17,471,631,425 | -47.229% |
| `function_calls` | 36,475,609,558 | 36,234,959,558 | -0.660% |
| `recursive_countdown` | 11,251,672,383 | 11,436,486,258 | +1.643% |
| `float_math_loop` | 14,056,844,083 | 13,953,171,883 | -0.738% |

The geometric mean improves from 18,379,533,498 ns to 14,913,935,453 ns, or
**18.856%**, exceeding the 15% Batch D gate. Every checksum matches Batch A.

## Target-independent guardrails

The stable VM rerun is
`perf-results/commands/20260809T074921Z-verified-mir-batch-d-cyclic-vm-guardrails-rerun.txt`.
Target scalar/range rows improve 33.45%/47.41%, the budgeted scalar row improves
7.84%, and the largest positive non-target mean change is 4.58%. The preceding
capture ran immediately after release recompilation and transiently slowed the
first scalar row; an immediate isolated rerun and the no-recompile full rerun
both restored it. All checksums and profile hit counts match.

The final external proof combines
`20260809T074951Z-verified-mir-batch-d-cyclic-external-guardrails.txt` for the
stable array/object rows and the isolated
`20260809T081648Z-verified-mir-batch-d-cyclic-external-guardrails-rerun.txt`
for string/map after the first long process showed non-repeating tail outliers:

| Workload | Stable delta from Batch A |
|---|---:|
| `array_scan` | -10.36% |
| `string_methods` | -3.74% |
| `map_string_index_lookup_update` | -6.75% |
| `object_field_methods` | -0.16% |

All checksums match. No target-independent row has a stable regression above
5%.

## Host, resource, and compile guardrails

Final captures are retained under `perf-results/commands/`:

- `20260809T082510Z-verified-mir-batch-d-engine-interop.txt`
- `20260809T082607Z-verified-mir-batch-d-service-boundary.txt`
- `20260809T082629Z-verified-mir-batch-d-async-execution.txt`
- `20260809T082701Z-verified-mir-batch-d-scoped-task.txt`
- `20260809T082725Z-verified-mir-batch-d-actor-memory.txt`
- `20260809T082739Z-verified-mir-batch-d-actor-allocations.txt`
- `20260809T082745Z-verified-mir-batch-d-actor-concurrency.txt`
- `20260809T082757Z-verified-mir-batch-d-compile-resources.txt`

Engine interpreter rows are within 0.37% or improve. Every Service checksum,
allocation count, allocated byte count, and deallocated byte count is exact;
timed Vela rows are within 0.36% or improve. Async and scoped-task checksums and
pool counts match, with no positive regression. Actor 10,000-Runtime retained
RSS changes by +0.03%/-0.31% for the small profile modes and +1.19%/+0.07% for
the large modes; allocation counts and bytes remain exact. Concurrency retains
the pending-overlap proof and exact checksums.

Compile mean changes range from -0.33% to +2.89%, and mean peak child RSS falls
0.27%. Portable artifact sizes increase by 180, 111, 64, 48, and 32 bytes for
the five rows, representing bounded v5 plan/source/coverage data rather than
retained MIR or Runtime-local mutable state.

## Zero-allocation block entry

The clean capture
`perf-results/commands/20260809T083436Z-verified-mir-batch-d-scalar-block-allocations.txt`
executes the production frame driver after warmup with one and 10,001 scalar
block entries. Both roots allocate exactly three times and 340 bytes; the
additional 10,000 entries add **zero allocations and zero bytes**. The result
checksum is 10,001.

## Validation

```bash
cargo test -p vela_bytecode
cargo test -p vela_vm
cargo clippy -p vela_bytecode -p vela_vm --all-targets -- -D warnings
cargo bench -p vela_vm --bench scalar_block_allocations
cargo fmt --all -- --check
git diff --check
```

Batch E may now form single-entry/single-latch scalar loop regions from
verified CFG structure. It must preserve the accepted block executor's source,
budget, trap, artifact, profiling, allocation, and ordinary-fallback contracts
while meeting the stronger 25% five-row geometric-mean gate.
