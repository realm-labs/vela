# Local LSP execution inventory

`catalog.json` and its feature files own semantic applicability and evidence.
`execution-manifest.json` assigns every current semantic, local interaction and
batch-deliverable ID to one stable batch. `interactions.json` records reviewed
workbench routes, independent expected outcomes, fixture identities and minimum
evidence levels. Fixture IDs are reserved here; B01 and feature batches implement
the shared fixtures and action drivers. A registered route is not executed proof.

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
