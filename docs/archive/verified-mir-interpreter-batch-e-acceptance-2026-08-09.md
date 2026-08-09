# Verified-MIR Interpreter Batch E Acceptance — 2026-08-09

Batch E of the
[Verified-MIR superinstruction and basic-block interpreter plan](../verified-mir-superinstruction-basic-block-interpreter-plan.md)
is accepted at implementation checkpoints `4988175d6` and `23e76999c`.
It internalizes eligible proven-i64 range iterations in the existing scalar
executor and passes the quiet-machine dispatch, semantic, budget, trap,
profiling, allocation, and performance gates.

## Accepted loop shape

Selection starts from a cyclic scalar latch already accepted by Batch D and
recognizes its loop from verified MIR CFG facts, not emitted instruction
adjacency. The initial loop family requires:

- a proven-i64 `RangeNext` header;
- one scalar body/latch reached only from that header;
- one unconditional latch edge back to the header;
- exactly one dominated cyclic predecessor of the header;
- an explicit finite done successor; and
- no call, allocation, safepoint, HostAccess, reflection, state, task, await,
  try region, dynamic range, internal branch, or additional latch inside the
  selected region.

The independent selection verifier recomputes predecessors and dominators and
re-derives the complete recipe. Dynamic ranges and ranges whose body contains
authored break/continue control flow remain canonical ordinary instructions.
Inner leaf loops may be selected independently, but no plan contains another
loop; outer multi-block and multiple-latch regions remain deferred.

The immutable v5 `ScalarBlockPlan` now optionally carries exact range cursor,
end, exhausted flag, yielded item, inclusive mode, header source and charge,
next-edge charge, done target/charge, and process-local verified-MIR header
identity. Portable canonicalization removes only the process-local identity.
Unlinked, portable, linked, source-link, and physical-reference verification
reject malformed registers, sources, charges, headers, body paths, backedges,
done paths, and plan handles without reconstructing a plan from bytecode.

## One interpreter and exact semantic boundaries

The ordinary `I64RangeNext` executes the first header turn. On the first taken
body edge, the existing `RunScalarBlock` entry enters the focused scalar
executor. It executes the body, charges the latch, executes the next logical
range header, charges the selected next/done edge, and repeats without
returning to the large frame-driver match. The executor returns to the same
ordinary instruction offset on the finite exit. Calls, heap ownership,
HostAccess, suspension, and frame driving remain outside this module.

The original physical header and edge stubs retain unique ownership of each
MIR budget-layout site for the first turn. The loop plan carries a separately
verified copy used only for subsequent internal turns. This avoids duplicate
artifact budget sites while preserving the exact order:

```text
body operations -> latch/backedge -> range header -> selected next/done edge
```

The ordinary-versus-selected differential matrix covers every budget limit
from zero through successful completion with nonzero header, next, latch, and
done charges. It also covers empty and one-element ranges, inclusive and
exclusive bounds, `i64::MAX`, unbounded execution, overflow on a later
iteration, and a structurally valid but runtime-tag-corrupted internal header.
Results, consumed units, error kinds, and source spans match ordinary
execution. Selection excludes all safepoints; Runtime deadline/cancellation
observation therefore remains at the same existing call/future boundaries.

## Profiling and dispatch proof

Profiled execution now reports scalar-loop entry, iteration, exit, and charged
backedge events in addition to ordinary instruction, scalar-block, and logical
subpoint hits. With two workload iterations, the frozen inventory is:

| Workload | Batch A static / profiled dispatches | Batch D | Batch E | Loop entries / iterations / exits / backedges |
|---|---:|---:|---:|---:|
| `scalar_branch_loop` | 58 / 7,787 | 40 / 4,167 | 38 / 3,801 | 0 / 0 / 0 / 0 |
| `range_iteration` | 63 / 23,947 | 43 / 6,699 | 39 / 171 | 18 / 2,176 / 18 / 2,176 |

`range_iteration` retains two scalar range plans. Its total profiled outer
dispatch falls 97.45% from Batch D and 99.29% from Batch A. The selected body
entries fall from 2,192 physical block dispatches to 34, while 2,176 internal
iterations and charged backedges remain explicitly observable. Checksums are
unchanged. MIR-aware lowering also omits a redundant post-`RangeNext` jump
only when the verified next edge is physical fallthrough and owns no budget
charge; this is canonical lowering, not a bytecode-adjacency selector.

## Runtime retention gate

The clean/load-gated capture is
`perf-results/commands/20260809T091110Z-verified-mir-batch-e-runtime-candidate.txt`.
It uses 500,000 iterations, five repeats, and two warmups at `4988175d6`.

| Workload | Batch A mean ns | Batch E mean ns | Delta |
|---|---:|---:|---:|
| `scalar_branch_loop` | 10,980,632,575 | 6,769,912,916 | -38.347% |
| `range_iteration` | 33,108,354,483 | 6,938,647,716 | -79.043% |
| `function_calls` | 36,475,609,558 | 37,010,253,575 | +1.466% |
| `recursive_countdown` | 11,251,672,383 | 11,353,763,300 | +0.907% |
| `float_math_loop` | 14,056,844,083 | 12,907,163,958 | -8.179% |

The five-row geometric mean improves from 18,379,533,498 ns to
12,056,746,475 ns, or **34.401%**. Scalar and range exceed their individual
35% gates; the suite exceeds its 25% gate; all checksums match Batch A.

The stable short VM guardrail rerun is
`perf-results/commands/20260809T092054Z-verified-mir-batch-e-vm-guardrails-rerun.txt`.
Target scalar/range rows improve 5.16%/60.34% from accepted Batch D, and the
budgeted scalar row improves 7.70%. All non-target rows remain below the 5%
guardrail; the largest positive delta is `managed_heap_materialization` at
4.82%. The immediately preceding capture retained a transient scalar/host
tail outlier after release recompilation, while its min and the no-recompile
rerun remained stable. Checksums and profile-count equality hold.

## Zero-allocation loop iteration

The clean capture
`perf-results/commands/20260809T092021Z-verified-mir-batch-e-scalar-loop-allocations.txt`
compares one and 10,001 production loop iterations after warmup. Both allocate
three times and 388 bytes; the additional 10,000 iterations add **zero
allocations and zero bytes**. The existing scalar-block proof remains three
allocations and 340 bytes for both one and 10,001 block entries.

## Validation

```bash
cargo test -p vela_bytecode
cargo test -p vela_vm --lib
cargo test -p vela_vm --test integration \
  lead_workloads_have_reproducible_verified_mir_inventories
cargo clippy -p vela_bytecode -p vela_vm --all-targets -- -D warnings
cargo bench -p vela_vm --bench scalar_block_allocations
cargo fmt --all -- --check
git diff --check
```

Batch F may now close generation ownership, reload, async, Service, durable
profiling, v5 portability/corruption, and shared-plan memory without changing
the accepted one-frame-driver execution model.
