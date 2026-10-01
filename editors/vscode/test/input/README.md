# Local workbench input evidence

Run `npm --prefix editors/vscode run test:input` from the repository root. This
builds and installs the actual VSIX into a fresh test-owned profile, then controls
that VS Code workbench through pinned Playwright/CDP keyboard and pointer events.
The extension-host bridge only sets up fixtures, reports observations and ends
the run. It does not perform the input action under acceptance.
The native runner uses an ordinary window with Vela and the test-only observer
installed into private extension, user-data and shared-data directories. It
does not use the development extension host for native acceptance. Reload starts
a new observer session and keeps the ordinary workbench's dirty-buffer backups.
Both registered profiles pin `workbench.editor.useModal: "off"` so Settings uses
the regular tab required by the existing Ctrl+Tab return route. VS Code's
[documented setting](https://code.visualstudio.com/docs/editing/getting-started/userinterface#modal-editors)
matches the inspected pinned editor; default modal behavior differs from the
development test host. Profile fingerprints and all acceptance captures must
be regenerated after this reviewed adjustment.

`profiles/darwin-arm64.json` and `profiles/win32-x64.json` pin the local
machine/editor configurations, selected automatically using the platform and
architecture registered in `tests/lsp_matrix/checkpoint.json`, including locale,
keyboard layout, theme, zoom, font and measured viewport/display scale. The runner
checks actual configuration, language, architecture, keyboard layout and display.
A changed profile requires explicit review and fresh proof. Switching between
registered machines does not edit shared progress or reuse another platform's
evidence. Extra environments,
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

UX01 starts with an inactive installed extension. Native Quick Open first opens
Vela-shaped `.txt` source and verifies plaintext, unchanged caret and no Vela
activation; an independent command open/query also returns an empty definition
set. The first native `.vela` open activates the bundled server, then native F12
and a separate command open/definition query must return the exact authored
target and move the actual editor there. Startup polling only observes logs and
extension state. These two proofs precede the existing driver prerequisites even
for a selected investigation such as `--proof ux01-install-open`.

UX10 runs pointer and keyboard hover, named/defaulted signature argument changes,
pointer leave and Escape dismissal, and unknown-receiver suppression. Its marked
Unicode source, exact rendered documentation, highlighted range, active parameter,
caret, dirty state and unchanged disk inputs have independent fixture expectations.
Unknown requests must actually complete before widget absence is accepted.
Use `--proof ux10-pointer-hover` (or another registered route) for investigation;
batch acceptance requires fresh evidence for all owned routes.

UX18 types an unsaved Unicode prefix, checks exact definition facts and then
suspends only the captured test-owned server. One real definition request must
remain pending before that same process is stopped. It must finish empty within
five seconds, show the Vela connection error and leave the server stopped across
three observations. Native Developer: Reload Window must preserve the full dirty
source and caret and restore exact command and F12 results. A separate route
configures an absent executable under the private workspace, reloads, checks the
visible error including the exact path, then removes the override and repeats
recovery. Windows suspends the captured process's threads; macOS uses SIGSTOP.
The helper verifies executable, creation identity and test-window ancestry before
mutation. Cleanup can stop only a server captured by this attempt.

Each host writes an identifying output marker. Session metadata points to that
host's marked output directory, even when VS Code reuses its exthost parent.
Raw trace/output/host/server snapshots survive every reload and are hashed in
the bundle alongside failure and recovery screenshots and typed receipts.
Use `--proof ux18-server-stop-reload --proof ux18-invalid-server-path` for a
focused investigation; final acceptance still requires every local route.

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

For alternating machines, sync committed source and the shared checkpoint through
Git. Run `npm ci --prefix editors/vscode` after dependency changes. The ordinary
installed suite (`npm --prefix editors/vscode test`) defaults to the current
registered profile's exact VS Code version; CI can still explicitly select its
own version with `VSCODE_TEST_VERSION`.

The same commands work in PowerShell and macOS shells:

```text
node --test "scripts/lsp-matrix/*.test.js"
node scripts/lsp-matrix/run.js --run
npm --prefix editors/vscode test
npm --prefix editors/vscode run test:input
node scripts/lsp-matrix/run.js --run --batch B02 --accept --editor-results <provider-results.json> --local-results <input-results.json>
```

The final command revalidates B00-B02 on the current platform and records its
profile audit, preserving the B03 child and original Mac acceptance. Use the next
incomplete batch ID when actually closing new scope. Logs, VSIX files and binaries
are local ignored artifacts; regenerate them after switching machines. Profile
audit records describe historical successful runs, not current-source freshness.

Windows uses Ctrl for the command palette/modifier click, Ctrl+Home and Alt+Left
for navigation, and the rendered Electron context menu for actual Peek pointer
input. The Windows helper checks the interactive desktop and changes/restores
the keyboard layout only on the test-owned window. macOS retains its Swift native
menu and input-source helpers. Both require an unlocked interactive session.
Display observations are retained as `observed-display.json`; changed screen,
scale, font or editor settings require a reviewed profile update and fresh proof.
