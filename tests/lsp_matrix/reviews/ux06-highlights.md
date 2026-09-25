# UX06 read/write highlights and shadow exclusion

B04.39 proves the `highlights-caret` and `shadow-exclusion` Input and Render
routes through the installed VSIX. The shared independent UX06 fixture has a
local `score` declaration, compound write, and read in one function, plus a
different local `score` declaration and read in another function.

The driver clicks the rendered identifiers with physical pointer events and
checks the resulting caret. It reads VS Code's visible decoration rectangles,
aligns each against the exact source token geometry, and requires the default
Dark Modern read/write classes: `wordHighlightText` for the declaration,
`wordHighlightStrong` for the compound write, and `wordHighlight` for reads.
The complete observed decoration set is compared against the marked sites,
first at the write and read occurrences, then at the shadowed occurrence. After
moving to the shadow, the three unrelated local decorations must disappear.

VS Code inserts parameter and type inlay hints into the rendered source line.
The geometry reader excludes those hint spans when translating fixture UTF-16
columns to rendered character rectangles, and checks that the remaining text
exactly equals the source line. It retains screenshots for both caret positions
in each route, the decoration geometry record, physical action and assertion
trace, workbench/LSP logs, extension-host log, and server trace.
