# UX05 keyboard rename confirmation

B04.36 adds independent Input and Render evidence for the `rename-confirm`
route. The installed VSIX is opened in a fresh Windows x64 VS Code 1.137.0
profile. The input driver positions the caret in `grant`, presses the default
F2 shortcut, checks the visible Rename input and its focus, types `award`, and
confirms with Enter. The independent marked fixture expects exactly four edits
in three files: the source declaration, direct import, open-file call, and
unopened-file qualified call. A same-named local declaration and use remain
unchanged. The driver checks each editor document and each disk file, including
UTF-16 after Chinese and emoji in an encoded workspace path.

VS Code saves the workspace edit to disk. Ctrl+Z reopens the previously closed
files as dirty documents containing the original text while their disk files
still contain the renamed text. Ctrl+Y reapplies the edit and clears the dirty
state. The driver checks both document and disk layers at every step and
requires the final diagnostics to clear. It retains the action trace,
accessible widget snapshot, screenshots, exact state observations, diagnostic
observation, LSP trace, extension-host log, and server trace.

The first live run exposed two server-side problems. Closed edit URIs used an
uppercase drive with a literal colon while the client opened the same drive
as lowercase `%3A`; the rename projection now preserves the client's drive
URI spelling. More materially, VS Code closes and saves a source document
after applying a workspace rename; `didClose` previously restored the stale
pre-rename disk snapshot, leaving the updated import unresolved. Closing a
source now reads its current disk content before recomputing workspace facts.
A protocol regression closes a saved renamed declaration and asserts that the
import and call retain the same owner. The live route checks that no stale
diagnostic survives confirmation and redo.

This child proves only `rename-confirm`. UX05 cancellation, invalid and
colliding names, plus UX06 references and highlights remain open in B04.
