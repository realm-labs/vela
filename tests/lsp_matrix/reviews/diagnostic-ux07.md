# UX07 Problems interaction

B05.4 closes the six local Input/Render obligations for UX07's
`problems-navigate`, `repair-unsaved` and `valid-location` routes. The separate
`input-diagnostics.json` fixture begins with a valid `first` method and an
unrelated `foobar` error. Real keyboard input inserts `@` in the function body,
producing the exact `E_LEX_CHAR` error. An independent protocol test first
checks the complete diagnostic sets and marked UTF-16 ranges under LF and CRLF,
then checks repair and the valid neighboring identifier.

The installed VSIX native driver types the error, opens Problems with the
platform's default shortcut, checks both rendered messages, codes, Error labels
and one-based locations against marker oracles, then double-clicks the target
row. The resulting editor selection and squiggly underline must cover exactly
the typed character. The repair route focuses the editor by its default group
shortcut before sending Backspace; this is a recorded physical input. The
complete source remains dirty and equals the disk fixture after the deletion,
while the Problems panel and visual decorations remove only the target error.
The valid-location route clicks the marked `first` token and requires unchanged
text, diagnostics and no error decoration on that token.

Each route records exact actions and input/render assertions, screenshots, an
accessible panel snapshot, observed DOM geometry and retained VSIX/server logs.
The extension-host bridge is used only for fixture setup and observations; it
does not perform the accepted input. The tests run in the pinned Windows x64
profile here; macOS needs its own fresh proof before a macOS gate. UX08 Quick
Fix interaction, other B05 cells and deferred B16 remain open.
