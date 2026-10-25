# Local LSP execution inventory

`catalog.json` and its feature files own semantic applicability and evidence.
`execution-manifest.json` assigns every current semantic, local interaction and
batch-deliverable ID to one stable batch. `interactions.json` records reviewed
workbench routes, independent expected outcomes, fixture identities and minimum
evidence levels. Fixture IDs are reserved here; B01 and feature batches implement
the shared fixtures and action drivers. A registered route is not executed proof.

## Checkpoint E Inventory Join (Pending Re-Audit)

[Inventory review](reviews/checkpoint-e-inventory.md) and its
[exact contract journal](reviews/checkpoint-e-inventory.json) reconcile the actual
reduced protocol and grammar with the existing execution migrations. Every
original obligation ID and owner remains; `execution-baseline.json` is unchanged.
The manifest names exact old/new hashes for 224 changed contracts and four
unchanged S10 contracts whose scoped-view test evidence was retired. Historical
assertions/test links are preserved, with 158 proof records explicitly pending.

B02–B08 are reopened with their original acceptance snapshots intact. Unaffected
B00/B01/B09 acceptance, completed children, profile audits and native artifact
references are preserved; the next task is B02. No native capture was refreshed.
Bare inventory auditing is usable, but strict acceptance and parent E closure
remain pending.

## Checkpoint E Site/Editor Cleanup (Scoped, Not Acceptance)

[Checkpoint E evidence](reviews/checkpoint-e-site-editor.md) records the bilingual
owned-value documentation regression, removed extern-state grammar/highlighting,
regenerated parser and error corpus, and non-destructive Zed cache validation.
Package validators and source-level checks are not installed/native proof.
Affected pre-removal captures remain stale. That source-only cleanup preserved
acceptance flags; the subsequent inventory join above explicitly reopens affected
contracts without discarding historical evidence. Parent final gates/status
still own E closure.

## Checkpoint D Consumer Changes

Tooling now uses strict schema **4** and retained registered Value facts. Host,
view, scoped-resource, constructor and dependent extern-state projections are
removed. Ordinary VM state, source structs/traits, collection methods,
reflection and native LSP worker scheduling remain. Removed-feature rows are
rejection/recovery contracts, not supported query successes.

[Checkpoint D evidence](reviews/checkpoint-d-boundary.md) records exact current
counts, the two parent-owned highlighting failures, seven parent Node golden
failures, and the completed native LSP test gate. No fresh installed/native
captures were taken; old hashes for changed fixtures are historical and have
not been rewritten to imply current acceptance. Unrelated evidence stays intact.

## Checkpoint B Consumer Changes (Historical)

Service hotfixing and Provider discovery/selection are removed capabilities,
not language-service or LSP protocol providers. The five `completion-service-*`
fixtures and their feature-specific catalog test registrations are retired.
Mixed call, import, expression and signature matrices retain ordinary coverage;
removed paths have null callable/expected-parameter facts. The known-callable
mapping list retains 41 ordinary cases; its three removed-call positions are
covered as explicit null queries in `signature-s5` instead. The Provider-named
trait/default/return scenarios remain ordinary traits without `#[provider]`.
Selection keeps removed path spellings only as unresolved, non-executing CST
ancestry inputs. Schema artifact consumers use format 2 without `serviceSet`.
`examples/tests/removed_entrypoints.rs` retains source-boundary rejection proof.

Original contracts, accepted unrelated evidence and historical checkpoint
summaries remain intact. Old Service-specific results are historical, not current
support or fresh proof. Changed schema fixtures and S5 assertions require fresh
current-source audits; no native/installed capture is claimed by this removal.
The consumer guard runs with `node --test editors/vscode/scripts/check-removed-consumers.test.js`
and in the VS Code CI matrix. It checks retirement, schema format and retained
ordinary fixture coverage; it does not replace compiler/protocol/native gates.

The language-service and protocol count guards cover the retained 101 signature
positions (72 positive, 29 null). The independent invalid-schema Node golden
expects format 2 in its diagnostic. Removed Service positions are not restored
through replacement metadata or fabricated query cases.

`execution-baseline.json` preserves the initial local contracts. Keep this dense
requirement data intact when adding coverage. Changed or split requirements need
an explicit migration in the manifest: exact original and replacement IDs/hashes,
a reason, and the behavior preserved. Normal evidence additions do not change
requirement IDs or contract hashes. New requirements need explicit owners.

B16 and its scenario families are deferred environment work. They have no local
execution obligations and cannot absorb missing local proof. All other batches
remain pending until their strict acceptance passes. The runner validates ownership
and includes the expanded inventory in the JSON report. `gate-evidence.json` links
infrastructure obligations to exact executed Node, Rust and installed-editor test identities. Full strict
acceptance also rejects unverified interaction and later
deliverables, even if all original semantic cells are verified.

Run the inventory self-tests and live service/protocol audit:

```bash
node --test "scripts/lsp-matrix/*.test.js"
node scripts/lsp-matrix/run.js --run
node scripts/lsp-matrix/run.js --run --batch B00
node scripts/lsp-matrix/run.js --run --batch B00 --accept
node scripts/lsp-matrix/run.js --reopen B02 --reason "reviewed scope expansion"
```

Each audit keeps its reports and command logs in a separate `target/lsp-matrix/run-*`
directory. Root-level reports point readers to the latest run; retries never
overwrite earlier command logs. These are regenerable artifacts,
not the durable batch status. `checkpoint.json` owns accepted batches, source
identities, exact remaining batch obligations, reopened scope and the next child.
`--accept` requires a passing scoped gate and advances only the first incomplete
local batch. Accepted batches need fresh proof in every later scoped run. `--local-results <results.json>` ingests exact installed-workbench evidence with
source/profile/driver/fixture/package integrity checks. Missing route results
stay unreviewed; validator self-tests cannot replace actual input proof.
Child commits record `LSP-Batch`, exact requirements, validation and remaining work.

Checkpoint v2 registers both alternating development profiles. Acceptance and
the active child remain shared; each batch retains its original validation and
`profileAudits` retains the latest successful scoped run per platform/architecture.
An accepted batch may be passed to `--accept` to record a fresh current-platform
audit without moving the next task. Every such run must re-prove all already
accepted batches on that platform. Neither historical audits nor another
platform's bundles can satisfy missing current proof.
