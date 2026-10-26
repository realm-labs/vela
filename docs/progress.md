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

## Test Organization: Implemented, Validation Limits Open

The previous TO00–TO09 commits completed a bounded parameterization/helper pass,
**not a complete repository-wide input-organization review**. Eighteen crates use dev-only
`rstest`, eleven new shared `.vela` fixtures serve compiler/VM/Engine tests and
HIR/analysis share fresh graph setup. Those changes remain useful and are retained.
However, many complete programs still live inside Rust cases/helpers, lifecycle
scenarios, shared tooling JSON and repository consumers. The earlier completion
claim was too broad. Inline source is not itself a defect: review readability and
reuse rather than treating either parameterization or blanket extraction as completion.

The [input-organization plan](test-organization-plan.md) has now been executed
through SF11 with the SF12 residual audit complete, one commit per stage. The
[SF execution record](archive/test-input-organization-acceptance.md) owns concrete
changes and proof. Substantial/reused programs and source trees are extracted;
short local cases, tiny reload pairs, explicit byte tests and useful JSON stay
inline. No zero-inline target, custom framework or runtime feature was introduced.

Implemented parser UI-P, HIR/analysis/compiler UI-S and full real-process CLI UI-C,
with typed assertions, positive controls, golden inventories and negative checks.
Formatter/source-artifact execution comparisons, bounded CLI/temp ownership,
actual site-input execution, six shipped queries and untimed benchmark correctness
preflight are present. GC/budget/re-entry/reload ownership and reclamation assertions
remain. Matrix readers resolve 515 references to 198 marked files; all 218 resolved
JSON trees match their original values. Existing specialized runners remain owners.

[Final SF12 validation](archive/test-input-sf12.md):

- Reconciled all-feature Rust identities: **4,644 passed, two existing ignored,
  zero failed/missing**. This is bounded partition evidence after a **900-second
  aggregate timeout**, not an aggregate exit-zero run. Unit/integration partitions
  use workspace-built binaries; per-owner doctest feature unification may differ.
- Workspace formatting, all-target/all-feature Clippy, docs, 29 example tests and
  example Clippy pass. Temporary proof-copy pollution of the unsafe audit was
  cleaned up; the unchanged audit passes. Bench/fuzz build and bounded smoke pass.
- **340 Node tests**, **21 process/selection guards**, site docs/syntax/145-page
  build, four actual site programs, grammar/query/package checks and wasm32 pass.
  Actual browser Playground execution remains separately scoped and unimplemented.
- Fresh installed VS Code proof is **26/27**, zero recorded failures, one incomplete
  lifecycle check. Aggregate 900-second and focused 300-second timeouts remain;
  unchanged readiness waits alone require more than 300 seconds. No longer rerun
  was made. Completed partitions match final input/server/platform provenance.
- Full installed/strict/native/Windows acceptance and registry package archives
  remain unproven. Missing `observer.vsix` still blocks physical Input/Render;
  existing versionless path dependencies block archive creation. Remote CI retains
  the full gates and now explicitly selects new repository-consumer checks.

[Previous execution evidence](archive/test-organization-acceptance.md) records
actual TO changes and tests. These results are historical to that pass, not fresh
validation of the proposed selective migration:

- Rust discovery reconciled against bounded passes: **4,532 all-feature / 4,517
  default**, zero failed or missing identities, two existing ignored. These are
  partition totals, not exit-zero aggregate workspace commands; aggregate and
  several group runs timed out. Per-owner feature resolution may differ from
  aggregate feature unification; owning CI still retains the aggregate gates.
- Workspace all-feature Clippy/docs/formatting; examples tests/Clippy (all 14 bins);
  wasm32 release check; benchmark/fuzz compilation; site docs/syntax/145-page build
  and package validators pass. These build checks are not performance/browser proof.
- **335 Node tests** pass. Fresh installed VS Code partitions pass **27/27** with
  identical final-input/server/platform provenance. The aggregate timeout remains
  recorded. Native-input failed before interaction due to missing `observer.vsix`;
  strict/native/Windows evidence remains incomplete and is not promoted.

## Runtime Removal Validation (Historical Baseline)

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
