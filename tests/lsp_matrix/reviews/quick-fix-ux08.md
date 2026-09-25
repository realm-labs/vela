# UX08 Quick Fix interaction

B05.5 covers all eight local Input/Render obligations for UX08. The shared
Unicode method-typo fixture defines the complete source, target and unrelated
diagnostics, three ordered action titles and the exact one-range replacement.
The installed VSIX native driver opens Quick Fix with the platform default
shortcut or a pointer click on the visible lightbulb. It checks all visible
menu rows and the focused first choice before pressing Enter. The complete
dirty source changes only `frist` to `first`; only `foobar` remains diagnosed.
After a physical editor-focus shortcut and undo, source and both diagnostics
return to the disk state. VS Code clears the dirty flag when undo reaches that
state.

The dismiss route opens the same menu and presses Escape, then verifies the
menu is hidden and the source and diagnostics unchanged. The no-fix route
places the caret at the valid `main` function name, invokes Quick Fix, checks
that no action menu or unsafe candidate appears, and verifies source and
diagnostics remain unchanged. Each route records its own actions, assertions,
screenshots, accessibility snapshot and installed VSIX/server logs. The bridge
only sets up a fixture or observes state; accepted input is physical keyboard
or pointer input. Windows x64 proof here does not close the independent macOS
profile. Other B05 cells and deferred B16 remain open.
