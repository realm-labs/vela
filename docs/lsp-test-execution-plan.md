# LSP Test Matrix Execution Plan

Status: ready for a future goal; implementation batches have not started.
The [strategy](lsp-test-strategy.md) owns required behavior and P0-P5 acceptance.
This plan owns work boundaries and verification checkpoints. Follow
[goal.md](goal.md), [architecture.md](architecture.md), and repository agent
instructions throughout. The implementation baseline is commit `33451ec80`.

## Goal Contract

Build executable proof for the existing syntax and LSP contracts, repair defects
that this proof exposes, and pass all strategy exit gates. Test queries must
never execute scripts or access live host state. Preserve the service/server/
editor boundaries; unrelated runtime optimization and new language or LSP
features are outside this goal.

Use this prompt to start later in a client that supports `/goal`:

```text
/goal Implement the LSP test matrix according to docs/lsp-test-execution-plan.md.
Read the checkpoint, resume the first incomplete batch, and complete B00-B19
in order. For each batch, add independent assertions, fix exposed defects,
run its acceptance checks, and create small, coherent Conventional Commits.
Record the LSP-Batch ID and validation results in each commit body.
Do not pass acceptance by deleting requirements, weakening assertions,
ignoring tests, or marking defects as N/A. Continue until full acceptance.
If interrupted, preserve a resumable checkpoint and keep unexecuted
environment checks pending.
```

For a bounded first goal, replace "complete B00-B19" with "complete only B00-B01".
Completing that bounded goal does not complete the full strategy. A token/time
limit is a resource boundary, not evidence of acceptance. Goal mode maintains
the objective; repository checkpoints and Git provide the durable resume state.
See OpenAI's [Follow a goal](https://learn.chatgpt.com/use-cases/follow-goals).

## Stable Batches

Execute B00, then B01, then B02-B14 in order, followed by B15-B19. B02-B13 each
own all existing obligations for the listed exact catalog feature IDs, including
their syntax, state, environment, and local installed-editor cells. These are
vertical slices across P1-P4, so range or negative assertions are not deferred
until a later phase. B15-B18 add cross-feature and execution-lane acceptance;
they do not excuse gaps in earlier batches.

| Batch | Scope | Batch-specific exit proof |
|---|---|---|
| B00 | Batch manifest and acceptance enforcement | Every current obligation has exactly one owner; explicit scope expansion, completed-batch regression checks, and machine-readable checkpoint validation have self-tests. Implement scoped strict acceptance without weakening the full gate. |
| B01 | Shared fixtures and independent assertions | Marker stripping, byte/UTF-16 positions, multi-file disk/overlay actions, and exact range/edit oracles feed both Rust and installed VSIX runners. Chinese/non-BMP, LF/CRLF, and malformed fixture self-tests pass. |
| B02 | `definition`, `declaration`, `type-definition`, `implementation` | Distinguish target kinds and source/schema ownership; exact URI/range/text and negative no-jump policy. Cross-file dirty Unicode cases and installed navigation pass; unsupported implementation requests have rejection proof. |
| B03 | `completion`, `completion-resolve` | Candidate inclusion/exclusion, insertion/replacement edits, ownership and resolved documentation; apply edits and re-query. |
| B04 | `references`, `highlight`, `prepare-rename`, `rename` | Exact reference sets, shadow/read/write separation, collision rejection, and applied multi-file Unicode edits preserve intended references. |
| B05 | `diagnostics`, `code-action` | Exact diagnostics and clearing across edits; applied fixes remove the intended error and preserve unrelated source. |
| B06 | `tokens-full`, `tokens-delta`, `tokens-range` | Decode and check token text/type/modifiers/bounds; delta application equals fresh full output after multiline Unicode edits. |
| B07 | `hover`, `signature-help` | Exact types, spans, callable/parameter identity, defaults/named arguments, and dynamic/unknown suppression. |
| B08 | `initialize`, `initialized`, `shutdown-exit`, `cancel`, `did-open`, `did-change`, `did-close`, `did-save`, `stdio` | Capability options, real framed transport, bounded termination, synchronization, stale/cancelled request policy, and malformed-message handling. |
| B09 | `watched-files`, `configuration-change`, `workspace-folders`, `configuration-request` | Dependency create/change/delete, multi-root/config/schema invalidation and conditional configuration policy; no stale locations or publications. |
| B10 | `document-symbol`, `workspace-symbol`, `folding`, `selection` | Exact kinds, ownership, range ancestry and selection nesting; incomplete syntax and Unicode boundaries. |
| B11 | `prepare-call-hierarchy`, `incoming-calls`, `outgoing-calls` | Exact incoming/outgoing edges and ranges, correct receiver ownership, unknown-target exclusion. |
| B12 | `formatting`, `range-formatting`, `on-type-formatting` | Applied exact output, valid-input parseability, trivia preservation, range containment and idempotence. |
| B13 | `inlay` | Exact positions, labels/kinds, parameter identity, range containment and suppression rules. |
| B14 | Semantic partition completeness review | Audit all S0-S14 groups against grammar/AST partitions and each feature contract; split broad cells where necessary and add missing assertions. Owns new partition obligations; existing cells retain B02-B13 ownership. |
| B15 | Cross-feature state machines | Incremental versus fresh-workspace equivalence through disk/dirty/close/reopen, dependency and schema replacement, malformed neighbors, cancellation and stale generations. Add explicit obligations for required high-risk combinations. |
| B16 | Installed-editor environment lanes | Run shared user workflows on Windows/Linux/macOS and minimum/current VS Code. Pin actual versions in results, enforce profile identity and complete profile aggregation, retain logs and provenance. Implement PR/nightly/release selection and scheduling. |
| B17 | Constrained generation and oracle sensitivity | Fixed-seed valid pair coverage, finite state sequences, reproducible minimized failures, and bounded mutation checks that demonstrate wrong targets/ranges/stale results are detected. Add machine-checkable acceptance obligations. |
| B18 | Scale budgets | Fixed workspace sizes and edit/query workloads, separate cold/warm runs, explicit latency/memory budgets and machine context. Calibrate and commit thresholds before acceptance measurement; report violations without relaxing thresholds to fit the result. |
| B19 | Full acceptance | All batches accepted, whole-repository checks pass, full strict matrix passes for every required profile, generated/scale evidence passes, and a durable acceptance report records commands, revisions, profiles and artifacts. |

B00/B01 are infrastructure gates, not claims of semantic coverage. The current
41 feature IDs are assigned once across B02-B13. The initial 1327 obligations
are a starting inventory, not a frozen denominator or the final P5 workload.
B14-B18 must register additional requirements before claiming their acceptance.

Batch numbers and completed scope remain stable. If a batch needs multiple
independently verifiable changes, use child IDs such as `B04.1`, `B04.2`, and
record their exact obligation IDs before editing. Do not renumber later batches.
Each child normally produces one coherent commit; a parent closes only when all
its requirements pass. Stable batches do not promise exactly twenty commits.
New requirements for an accepted batch reopen its affected scope explicitly.

## B00 Implementation Requirements

The existing runner supports `--run`, `--strict`, and `--editor-results` only.
Batch/profile selection and regression enforcement described here are planned
work, not commands that work today. Until B00 lands, do not mark feature batches
accepted using an ordinary successful audit.

B00 must add a versioned manifest under `tests/lsp_matrix/` that records batch
ownership and exact obligation IDs. Preserve IDs through normal edits; splits
record the old-to-new mapping and preserve the complete original requirement.
Validate missing owners, duplicate ownership, empty semantic batches, removed
requirements, and unjustified applicability changes. Candidate test prefixes
must never become automatic evidence mappings.

A scoped acceptance run must validate the whole catalog, fail any executed test
failure, and require every applicable obligation in the selected batch and all
previously accepted batches to be verified by current evidence. Pending batches
may remain open. A missing editor result is pending evidence, not a pass.
Completed cells cannot silently become unreviewed, mapped-only, removed, or N/A.
Legitimate contract changes require an explicit recorded migration and re-audit.
Include negative self-tests for each way these gates could falsely report success.

The manifest must also declare B14-B19 deliverables so accepting all initial
feature cells cannot accidentally close the entire goal. B16 extends evidence
identity to required profiles and aggregates them; current local provenance
alone does not certify multiple OS/editor versions.

## Commit And Validation Contract

Before each child task, inspect the diff and checkpoint, identify exact missing
requirements, inspect existing assertions, and run the closest relevant test.
Write independent expected behavior before implementing a correction. A failure
may be investigated locally, but an accepted checkpoint must pass its checks.
Keep a reproducer, minimal fix, and corresponding evidence coherent; do not commit
a knowingly failing test as an accepted batch or bundle unrelated product fixes.

Each implementation commit records this information in its body:

```text
LSP-Batch: B02.1
Requirements: <exact IDs or a versioned manifest group>
Behavior: <contract and observable result>
Validation: <commands and results>
Remaining: <parent-batch gaps and unexecuted profiles, or none>
```

Use subjects such as `test(lsp): cover cross-file type definition partitions`,
`fix(lsp): preserve target ranges after overlay edits`, or
`ci(lsp): enforce accepted matrix batches`. Stage explicit task files and inspect
the staged diff. Preserve unrelated work. Commit locally; publication is not a
requirement of this plan.

Run focused tests for every behavior change. For each batch close, run the audit
self-tests and service/protocol audit below, plus the B00 scoped gate once it
exists. Rust changes require formatting and relevant Clippy checks. Run installed
VSIX tests when the batch owns editor obligations or changes fixtures, packaging,
provenance, or editor behavior. B19 also requires all workspace validation commands.

```bash
node --test scripts/lsp-matrix/model.test.js
node scripts/lsp-matrix/run.js --run
npm --prefix editors/vscode test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For final acceptance, additionally run the strategy's full `--strict` command
with explicit matching editor results for each required profile, plus generated
and scale gates implemented by B17/B18. Local checks do not certify unavailable
OS lanes. Retain their pending status until actual matching execution evidence
exists. A recorded unrelated baseline failure is still an open final gate.

## Checkpoint And Resume Contract

The initial checkpoint is: **no B batch accepted; next task B00; no active child;
P0 implementation baseline `33451ec80`; full acceptance pending**. This is a
prepared plan, not an active goal or a newly measured coverage result.

B00 creates the machine-readable execution checkpoint alongside its manifest.
Thereafter that checkpoint is the single source of batch status; keep this plan
stable rather than appending a per-commit narrative. Each checkpoint records:

- Accepted batches, active child, remaining exact requirement IDs, and next task.
- Manifest version, tested source revision/tree identity, validation commands and
  results, editor profiles, and artifact locations/provenance.
- Uncommitted work and reproducible failures when interrupted, with external
  blockers distinguished from implementable failures.

Update the checkpoint with each accepted child commit. Its own commit can be
found by Git history and the `LSP-Batch` trailer; do not store a self-referential
commit hash. Do not check generated logs or machine-specific absolute paths into
the repository. Ignored `target/` reports are regenerable evidence, not the sole
resume record. Phase acceptance reports preserve the relevant durable summary.

On resumption, read repository instructions, the strategy, this plan, and the
checkpoint; reconcile them with Git history and the working diff. Re-run the
nearest incomplete/failing gate when inputs changed or evidence is missing.
Continue the active child or first incomplete batch, preserving completed work.
Update `progress.md` only for phase status/current-focus changes and
`decisions.md` only for durable design decisions. Budget exhaustion, interruption,
or unavailable CI keeps unverified work pending; only the requested goal's actual
exit gates justify marking that goal complete.
