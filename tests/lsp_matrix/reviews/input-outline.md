# Native Outline interaction review

B10.39 closes only `vscode/UX11/outline/input/local` and
`vscode/UX11/outline/render/local`. Document/workspace symbol pickers, no-match
queries and UX12 folding/selection interactions remain pending. Existing 50
native contracts and 27 installed provider checks retain their full scope.

The [manifest](../fixtures/input-outline.json) references the complete authored
declaration and ownership trees. Its private fixture files retain Unicode
comments, declaration attributes, implementation headers, fields, variants,
methods and import-only/unknown-neighbor files. Each LF/CRLF form has nine
queried files, 44 roots and 79 nodes. Twenty-two physical fixture files remain
byte-exact; all eighteen queried buffers remain clean and unchanged.

The independent [contracts](../../../scripts/lsp-matrix/outline-contracts.js)
derive ranges only from authored source markers. The pinned VS Code 1.137.0
primary workbench source owns presentation and selection policy: the Outline
renderer displays symbol names/details, uses SymbolKind-specific icons and
orders by source position. Single click and Enter collapse `selectionRange` to
its start; double click selects the entire declaration `range`. The focus
command's category comes from its Explorer container. These expectations do not
come from a provider result or observed DOM tree.

The installed ordinary workbench receives actual keyboard and pointer events.
Home/Down traverses every node, including virtualized rows; each visible focused
row must match its literal name, detail, icon and level. Parent names are
observed from the actual traversal stack, then compared with the authored tree.
All fifty parent visits check the initial expanded state, physically collapse,
check the collapsed state, then expand again. End must still land on the authored
last node, preventing unreviewed trailing rows from passing.

All 158 single clicks must navigate to the exact identifier caret. Every
nonempty file also receives Enter, which must actually focus the editor and
retain the exact caret, followed by a double click selecting the complete marked
declaration. Empty files require the exact SDK message and no visible rows.
Every file retains PNG and ARIA evidence alongside raw observations, physical
input events and whole contract checks; these artifacts are required and hashed.
The observer only prepares files and reads editor state; it performs none of
the accepted Outline inputs or navigation.

An empty result cannot pass on an unfinished request. The native observer joins
each fresh main-loop document-symbol request with its queued worker generation
and a later completed worker response, using request ID, URI and method. Worker
responses omit the main-loop sequence, so the previous synchronous correlation
helper remains unchanged and a separate worker helper checks this shape.
Independent tests reject mismatched generations/owners, stale origins, missing
queue entries, reordered records and empty dispatch acknowledgements.

Retained prototypes record the original range-start assertion error, worker
sequence assumption, Right-on-an-expanded-parent error, asynchronous Enter/focus
race and incorrect palette category. Their failed screenshots, traces and logs
remain evidence. The complete focused prototype passes 566 physical inputs and
521 registered checks in approximately 69 seconds. The 120-second route budget
was planned before capture; the outer observer budget reserves that interval
in addition to the existing 450 seconds. Acceptance requires fresh full native,
installed VSIX and scoped regression bundles on one unchanged source/profile.
Windows/macOS use their own native bindings and independent fresh evidence.

The first complete native run passed, then the workspace test build changed the
development server fingerprint. Its successful bundle remains retained; final
native/editor acceptance uses matching rebuilt server bytes. A later full native
run exposed an existing pointer-hover layout race: the measured target was at
x631 before type/parameter hints appeared, then shifted to x675. No hover request
was emitted at the old coordinate. The screenshots, geometry and server trace
remain retained. The driver now waits for the complete independently reviewed
source line with its type and parameter labels before measuring the pointer
target. SDK padding is width-only, so its DOM text has no extra ordinary space
after the parameter label. A focused real-input prototype covers both pointer
hover and pointer-leave dismissal. An independent regression rejects the earlier
layout, wrong/missing labels, duplicates and altered source. The original fifty
contracts, accepted input actions, hover assertions and timeouts are unchanged.
