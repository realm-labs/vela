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
remain pending until their strict acceptance is implemented and passes. The
current runner validates ownership and includes the expanded inventory in the
JSON report; scoped acceptance and resumable checkpoint enforcement are the next
B00 child. Full strict acceptance also rejects unverified interaction and later
deliverables, even if all original semantic cells are verified.

Run the inventory self-tests and live service/protocol audit:

```bash
node --test "scripts/lsp-matrix/*.test.js"
node scripts/lsp-matrix/run.js --run
```

The generated report and logs under `target/lsp-matrix/` are regenerable artifacts,
not the durable batch status. Child commits record `LSP-Batch`, exact requirements,
validation and remaining work until the B00 checkpoint becomes authoritative.
