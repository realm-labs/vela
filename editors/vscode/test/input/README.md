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

UX17 root/settings input runs after the existing routes. Native Add Folder,
Save Workspace As, workspace JSON editing and Remove Folder use the actual
workbench. File-dialog input paths are resolved only against the owned fixture
base, recorded verbatim as observations and typed by the physical input driver.
The observer only reads folder/settings state and executes the separate definition
commands; it does not update folders or accepted settings. Full workspace JSON
preserves each pinned profile's preferences, with application dialog settings
remaining in the private user profile. The roots proof has a 120-second budget.
Before expanded captures, the runner and observer budgets are set to 450 seconds:
180 for existing routes, 120 for roots, 120 for invalid configuration/schema,
and 30 for transitions. These are finite planned budgets, not measured relaxations.
Both profiles now also pin `files.simpleDialog.enable: true`. The inspected
pinned editor uses this setting for file/folder pickers; `window.dialogStyle`
only controls confirmation dialogs. Workbench pickers remain actual VS Code
UI and accept physical keyboard/pointer input. The change requires fresh profile
verification and captures; it does not reuse older profile fingerprints.

Both also pin `window.dialogStyle: "custom"` and `explorer.confirmDelete: true`
for Explorer file-operation acceptance. Delete still requires the real workbench
confirmation. Dialog style has APPLICATION scope in the pinned VS Code; the
runner writes it only into the isolated user-data/User/settings.json. Other
preferences stay in workspace settings, and effective settings are verified.
The developer's own user profile is never edited. This adjustment requires fresh
evidence; macOS and Windows captures remain independent.

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

UX17 Explorer routes create, rename, delete and recreate actual dependencies.
They check exact current/null definitions from separate command queries and real
F12 requests, complete diagnostic appearance/clearing, owned file membership,
Unicode ranges and unchanged dirty caller source. Investigate a selected route
with `--proof ux17-explorer-create`, `--proof ux17-explorer-rename` or
`--proof ux17-explorer-delete`; a strict gate still requires every owned proof.
Roots/settings/schema and trust routes have independent installed contracts;
the checkpoint owns their current acceptance status and next task.
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

UX17 static schema replacement uses Quick Open pointer selection, actual editor
text insertion and Save. Independent fixtures retain existing completion metadata
while changing only a distinct host type's fields. Full SDK and native keyboard
hover responses, exact diagnostic publications, dirty caller/disk ownership and
byte-for-byte schema restoration are checked. A null schema hover still permits
the workbench's exact diagnostic tooltip. The schema bridge performs read-only
queries; accepted replacements never use it to write artifacts. A focused run is
`npm --prefix editors/vscode run test:input -- --proof ux17-schema-replace`;
batch validation still requires all current native proofs. See the
[schema review](../../../../tests/lsp_matrix/reviews/workspace-schema-workbench.md).

UX17 invalid configuration/schema appends twelve finite recovery states after
roots/settings, with 36 command/native keyboard hover pairs, complete current
source/metadata diagnostics, exact rejection logs and native schema/manifest
deletion/recreation. Four unsaved overlays retain their original disk bytes.
An empty manifest establishes a valid graph before invalid TOML is typed, so
retention and package repair cannot accidentally reuse an earlier fallback graph.
A focused run includes its owned workspace prerequisite:
`npm --prefix editors/vscode run test:input -- --proof ux17-roots-settings --proof ux17-invalid-config-schema`.
The complete suite now contains 48 proofs; strict gates require the full fresh
suite on the selected registered profile. See the
[invalid configuration review](../../../../tests/lsp_matrix/reviews/invalid-config-schema-workbench.md).

UX21 uses a second sequential normal window, separate private user/shared data,
and enabled Workspace Trust. It declines the startup prompt, checks Vela's
visible disabled policy and Restricted Mode, and grants trust through the actual
trust editor. The owned configured binary must stay unstarted until trust;
afterward the exact server, dirty caller, native F12 and diagnostics recover.
The read-only trust observer never changes trust/configuration through its API.
The trusted 450-second lane and new 210-second lane keep separate finite budgets;
the latter covers two 90-second proofs plus setup. No native windows run together.
A focused run is
`npm --prefix editors/vscode run test:input -- --proof ux21-untrusted-open --proof ux21-grant-trust`.
Full acceptance now requires all 50 proofs on one frozen source/profile/server.
See the [trust review](../../../../tests/lsp_matrix/reviews/workspace-trust-workbench.md).
