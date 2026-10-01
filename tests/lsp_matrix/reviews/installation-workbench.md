# Installed extension activation and file association

B08.6 covers all four UX01 Input/Command obligations. The external driver
installs the actual VSIX into a new isolated profile and verifies archive,
installed source and bundled binary identities. The bridge becomes ready before
opening any Vela document and reports the installed extension as inactive.

`input-installation.json` independently authors identical Vela-shaped source
bytes in `.txt` and `.vela`, with Chinese/emoji content and literal call and
definition coordinates checked against test-only markers. The driver first
opens `.txt` through native Quick Open and Go to Line, presses F12, and verifies
plaintext language mode, unchanged source/caret, inactive extension and absent
server startup. VS Code guards F12 when there is no definition provider; UX01
does not require a nonexistent navigation message. A separate `vscode.open`
command and definition-provider query must preserve plaintext and return an
exact empty array without activating Vela.

The first native `scripts/installation/main.vela` open owns on-language activation.
It preserves the UX01 main.vela opening contract without overwriting the existing
completion-driver main.vela fixture. Startup readiness only
observes the installed extension and its startup log. Native F12 must produce
one completed real-client request with the authored URI, UTF-16 call position,
definition range and target text, and move the actual editor to that target.
A separate `vscode.open` and definition command repeat the full source/caret and
wire checks. Neither readiness nor the bridge sends a substitute native query.
Both routes verify original disk bytes, typed action/check receipts and retained
screenshots and logs. Every action runs once and the proof has a 45-second bound.

Windows and macOS use their registered native Quick Open bindings with the same
independent expectations. Evidence remains fresh and independent per platform.
The initial failed investigation preserved a screenshot showing plaintext with
no navigation message; it is not accepted evidence. UX18 server-stop/reload and
invalid-path recovery remain separate work, as do deferred B16 environments.

Live bundle validation also exposed the original completion driver's unscoped
trace filter after startup proofs were added. Scope its action receipts to its
own proof ID; do not weaken the shared validator or mix startup actions into
completion evidence. Superseded captures/audits remain outside acceptance.
