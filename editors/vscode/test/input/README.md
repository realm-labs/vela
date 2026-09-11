# Local workbench input evidence

Run `npm --prefix editors/vscode run test:input` from the repository root. This
builds and installs the actual VSIX into a fresh test-owned profile, then controls
that VS Code workbench through pinned Playwright/CDP keyboard and pointer events.
The extension-host bridge only sets up fixtures, reports observations and ends
the run. It does not perform the input action under acceptance.

`profile.json` pins the one local machine/editor configuration, including locale,
keyboard layout, theme, zoom, font and measured viewport/display scale. The runner
checks actual configuration, language, architecture, keyboard layout and display.
A changed profile requires explicit review and fresh proof. Extra environments,
versions and rendering variants remain deferred B16 work. The fixture demonstration
uses a trusted isolated workspace; UX21 must use its own trust-enabled setup.

The demonstrated path types a prefix, opens suggestions by keyboard, inspects the
visible candidate and accepts it with a pointer click. It checks actual editor
focus, exact final source, dirty state and caret against the shared fixture.
Readiness polling only observes state; input actions run once. All waits are
bounded. Every attempt retains traces, accessible widget snapshots, screenshots,
workbench/protocol logs and the installed artifacts in `test-results/input-*`.

The version-2 `results.json` records passing/failing status, clean process exit,
source identity, exact profile, driver/fixture/packaging/server hashes, VSIX hash,
artifact hashes and typed action/assertion receipts. Assertions and actions must
match `scripts/lsp-matrix/local-contracts.js`, which reads independent fixture
expectations. The audit verifies the archive's contents against installed files
and current extension sources, and rejects substituted or stale package bytes.
The proof is not accepted solely from a screenshot or a provider result.

To combine current provider and Input/Render evidence:

```sh
VSCODE_TEST_VERSION=1.137.0 npm --prefix editors/vscode test
npm --prefix editors/vscode run test:input
node scripts/lsp-matrix/run.js --run --batch B01 --accept \
  --editor-results editors/vscode/test-results/run-REPLACE/results.json \
  --local-results editors/vscode/test-results/input-REPLACE/results.json
```

Use the paths printed by those runs. Keep sources unchanged between evidence
capture and acceptance. The source tree fingerprint includes uncommitted files;
only the self-referencing execution checkpoint is excluded. Missing actual input
results cannot be replaced by validator self-tests. Feature batches add their
own exact route contracts and observations; the B01 demonstration does not certify
any of UX01-UX18 or UX21.
