# Native keyboard folding review

Task B10.43 closes only `vscode/UX12/keyboard-fold/input/local` and
`vscode/UX12/keyboard-fold/render/local`. Selection expand/shrink and the combined
malformed-neighbor workflow remain open. No language/runtime or folding provider
policy changes belong to this task.

Expected behavior is authored before the driver: retain all eight original
folding cases plus the disk/helper views in all four LF/CRLF original/Unicode
shifted forms. Every view requires a complete fresh ordered raw folding response,
including empty responses and coincident item/body starts. Preserve all 60 public
ranges independently of the existing pinned workbench projection's 56 eligible
controls; crossing else intervals do not become workbench folds.

For each eligible region, physically press the platform document-start key and
the exact number of Down keys to place a single zero-column caret on its source
header. Observe the actual file, focus, caret and unchanged buffer before the
fold shortcut. Physically collapse, expand, then expand again. Every state must
match the complete painted source line numbers and gutter icon set, actual public
visibleRanges, actual caret, unchanged full source and clean buffer. Repeated
expand is a no-op. All twelve no-control views receive real collapse and expand
shortcuts with identical source, caret and visible state before and after.

Windows uses Ctrl+Shift+[ / Ctrl+Shift+]; macOS uses Option+Command+[ / ] from
the pinned 1.137.0 action registrations. Commands are not executed through the
observer bridge. The bridge only opens pristine owned fixtures and reads public
editor state. Keyboard positioning is part of the recorded accepted input.
Actual DOM source line geometry and icons are read independently of expectations.
Each rendered state retains a PNG and ARIA snapshot; each keyboard action and
state observation retains its trace and hashes. All forty physical fixture files
must remain unchanged, and each private normal window must exit cleanly.

Four sequential private windows keep the original 120-second window and
480-second aggregate limits. All previous native contracts, installed 27 SDK
checks, profiles and historical evidence stay intact. Acceptance requires fresh
same-source native 56 / SDK 27 evidence, Node, fmt/Clippy/workspace and automatic
plus strict B09 regression on the current registered platform. macOS evidence is
independent and cannot be mixed into Windows acceptance.

The first retained Node run rejects two incorrectly authored literal test
expectations: the inner array header is source line 2 (zero based), and the
line-comment sample has four lines including EOF. Correct these against the
original fixture text; no corpus, action, behavior or deadline is reduced.
