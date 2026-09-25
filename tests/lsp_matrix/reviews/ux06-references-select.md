# UX06 references panel and selection

B04.38 proves the `references-select` Input and Render route through the
installed VSIX. The independent fixture has a public `grant` declaration, a
direct import and call in the open file, and a qualified call in a closed file.
It also contains local `score` reads, writes, and a same-named shadow for the
remaining UX06 routes. Chinese and emoji precede the identifiers, so all
visible locations must use UTF-16 columns.

The driver positions the caret inside `grant` and presses Shift+F12. It checks
that the References peek contains four results grouped 1/2/1 across the three
files, then expands the groups and reads every visible result's file, line,
column, and highlighted token. The set must exactly match the fixture's four
marked sites. It double-clicks each result and checks the active document,
unchanged text, clean state, and exact selected identifier range. Each result
is selected from a fresh peek opened by the physical shortcut. The source and
closed target files are unopened before the first query.

The action trace, accessible panel snapshot, screenshot, normalized row record,
workbench log, LSP log, extension-host log, and server trace are retained.
The `highlights-caret` and `shadow-exclusion` routes remain open in B04.
