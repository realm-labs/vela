# Progress

This file records current implementation truth and remaining work, not a changelog.
The [goal](goal.md), [architecture](architecture.md), and
[removal plan](runtime-simplification-plan.md) define the supported scope.

## Current Baseline

The A–E **removal implementation and software acceptance are complete**.
Vela is synchronous and interpreted, with ordinary script hot reload and existing
owned-value native APIs. No independent opaque-object runtime survived the
removal; replacement objects, conversion APIs, and a new re-entry model remain
separately deferred and were not implemented.

Removed: Service/Provider, script async/tasks, caller-borrowed Rust roots and
arguments/returns/views, HostRef/PathProxy and transparent host paths, leases and
release operations, dependent registration macros/extern bindings, and JIT-only
preparation. Old source/API/artifact boundaries reject; no compatibility runtime
or wrapper remains.

Retained: ordinary modules/packages/traits, functions/closures and script
collections/iterators, script reflection, VM-owned state, GC and finite budgets,
synchronous native/context calls and existing re-entry, native effects and
permissions, interpreter MIR/root/budget/cache/physical plans, CLI, WASM, native
LSP workers, and planned debugger/DAP support.

Compatible state and exact old executable generations remain owned by active
calls and retained closures, including nested code, native dispatch and plans.
Native host-read/write effects remain enforced independently of removed host
paths through package declarations, reload policy and binding ceilings.

| Checkpoint | Result |
|---|---|
| A — contracts/baseline | `ff8163a19`: authoritative removal contract and 27 existing baseline tests. |
| B — Service/Provider | `bc40c9258`: production APIs, compiler targets, dispatch, schemas and consumers removed. |
| C — tasks/async | `f55b2d770`: admission, scheduling, syntax, native futures, async APIs and metadata removed. |
| D — borrowing/paths | `226900e81`: borrowed/path runtime, leases and dependent bindings removed without opaque replacements. |
| E — repository cleanup | JIT-only APIs, stale contracts/grammar, dependencies and consumers cleaned; final software gates and provenance-checked installed-editor partitions pass. |

Current formats are **portable program 8 / Rust-binding schema 7 / tooling
schema 4**. Incompatible formats reject before use/link/activation. Package
metadata remains unsupported by the portable codec; no new encoding was added.

## Current Task: Test Organization

The [repository test organization plan](test-organization-plan.md) inventories
all 23 root-workspace crates and the separate example/fuzz/editor/site owners.
Execution is authorized in TO00–TO09 order, with one commit per stage.

TO00 accepts the compiler/VM record-writeback pilot: two byte-preserved shared
sources, four independently named cases, unchanged lowering/result assertions.
The fast-lane filter now names `removed_macros_and_borrow_apis_are_unavailable`;
full validation retains both trybuild runners. Focused pilot tests passed again.
TO01 completes bounded foundational/front-end families and shares the HIR/analysis
conformance graph without sharing assertions. All seven owners passed 545 tests
in both feature modes; package lifecycle/audits intentionally remain unchanged.
[Execution evidence](archive/test-organization-acceptance.md) records dispositions
and validation. Later stages preserve stateful/safety scenarios, specialized
runners and LSP evidence obligations; final acceptance remains pending.

TO02 shares nine more exact source inputs across 18 compiler/VM/Engine consumers
and exposes homogeneous MIR/compiler cases. Raw verifier/artifact boundaries and
separate lowering/runtime assertions remain intact; focused owner gates pass.

TO03 completes VM numeric/bytes, reflection absence and typed stdlib lookup cases,
plus fresh standard-runner reuse. Host/stdlib-runtime exhaustive audits and VM
lifecycle/safety scenarios remain unchanged; conformance still distinguishes
compile-time rejection from runtime failure and retains its fast-lane prefix.

TO04 completes Engine string/conversion and bindgen keyword/diagnostic families.
Reload, generated-call, affinity and authorization scenarios retain their original
observations; generated reload now guards the source replacement count. Both
feature modes and scoped Clippy pass.

TO05 completes ordinary macro signature/classification cases with exact diagnostics.
Trybuild and stderr expectations are unchanged; both feature modes pass all 27 UI
fixtures and the native/value execution suites.

## Final Validation

Detailed commands, failures resolved during validation, evidence paths and limits
are in the [acceptance report](archive/runtime-simplification-acceptance.md).
Rust validation used installed stable **1.99.1**; focused gates also ran on 1.98.0.

- Full workspace tests, all features, locked, no exclusions: **4,333 passed,
  zero failed, two existing ignored**. Macro UI, conformance and unsafe-boundary
  audits ran; test threads were bounded to four.
- Workspace Clippy, all targets with both default and all features, `-D warnings`;
  Rust formatting; workspace documentation: passed.
- Full examples tests/Clippy and direct execution of all **14 retained bins**:
  passed. The reflection-schema mutation example now reaches its intended
  rejection through a valid primitive descriptor, not an unregistered host type.
- WASM release cross-target check, benchmark compilation and fuzz-bin compilation:
  passed. Fuzz smoke tests are not a coverage-guided campaign.
- Node matrix/editor guards (**321 tests**), site syntax/docs/build (**145
  pages**), VS Code/Zed package validators and Tree-sitter regeneration/corpus/
  queries: passed.
- Installed VS Code 1.137.0 on macOS arm64: **27/27 declared checks passed in
  bounded partitions**, merged only after identical input/server/platform
  provenance was verified. The aggregate `npm test` run timed out at 900 seconds;
  no aggregate exit-zero run is claimed.

## Editor Evidence And Remaining Gaps

Software/removal acceptance is **not full strict/native LSP acceptance**.
The [reviewed inventory migration](../tests/lsp_matrix/reviews/checkpoint-e-inventory.md)
preserves all 1,327 semantic and 1,477 execution obligation IDs, the immutable
execution baseline and historical captures. Changed contracts have explicit
old/new hashes; stale evidence cannot become verified from an old passing test.

Affected B02–B08 batches are reopened. B00/B01/B09 retain their historical
acceptance without certifying changed source. The native checkpoint resumes at
**B02**, with 1,423 remaining execution obligations. Fresh installed-provider
proof does not substitute for physical Input/Render evidence or an independent
Windows run. The current editor-result audit has zero failures but still reports
mapped/unreviewed obligations; strict coverage is not declared complete.

Remaining retained tracks include native LSP evidence/incrementality, ordinary
container conformance, measured interpreter performance, debugger/DAP and release
hardening. None reinstates deleted capabilities or approves deferred interop.
See [blocked.md](blocked.md) for evidence/environment limitations.

## History And Update Rules

The [previous progress snapshot](archive/progress-before-simplification-finalization.md)
preserves the old implementation, milestone records and transition evidence;
older detail remains in [archive](archive/). Removed-feature acceptance there is
historical, not current support.

Update this file only for supported-baseline, checkpoint, validation or remaining-
gap changes. Keep routine logs and per-change details in Git or the acceptance
report; preserve ownership and current request commit limits.
