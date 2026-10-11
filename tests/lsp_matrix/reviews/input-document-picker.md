# Native document-symbol picker review

B10.40 owns only `vscode/UX11/document-picker/input/local` and
`vscode/UX11/document-picker/render/local`. Workspace-picker and the combined
no-match route, plus all UX12 folding/selection interactions, remain pending.
Document no-match assertions are prerequisites here and cannot close the
workspace half of that separate route.

The independent authored document-symbol declarations and ownership fixtures
provide complete names, kinds, ranges and identifier selections. The picker
materializes a separate prefix so each of eighteen LF/CRLF documents obtains a
fresh worker result; it never reuses an Outline cache or derives expectations
from provider/DOM output. The same 44 roots and 79 nodes per form include all
authored parents/children, duplicate method names, malformed ownership recovery,
attributes and Unicode before identifiers. Twenty-two disk files retain their
literal bytes, including helper/import neighbors.

Primary source is the pinned VS Code 1.137.0 workbench desktop bundle and its
NLS messages. `asListOfDocumentSymbols` flattens the entire tree and sorts by
range start ascending, then range end descending. `_flattenDocumentSymbols`
uses the immediate parent's name as container description. The document quick
access provider labels names with SymbolKind icons, uses `symbols ({count})`
for the unfiltered separator, and collapses selectionRange to its start upon
acceptance. Reopening restores the last containing source range. The native
list loops from its last entry to the first; every authored row is visited and
the End/tail/Down/head cycle checks the complete boundary.

All 158 destinations require actual physical acceptance, alternating 40 pointer
clicks and 118 Enter keys. Every name, immediate parent description and kind icon
is asserted before acceptance and after reopening. Exact active URI, unchanged
text/dirty state and UTF-16 caret are observed through the read-only bridge.
No bridge command performs accepted navigation. Initial source setup is a
precondition. Windows and macOS have separately registered native key bindings.

Empty results require a completed fresh main request/queued worker generation/
response correlation. NLS literals are `No editor symbols` and `No matching
editor symbols`. SDK placeholder rows are ordinary list items without a range,
so they are not marked aria-disabled. Physical Enter must keep the picker open
and the complete editor selection/source unchanged. Sixteen populated documents
also type an independently authored impossible query and reject stale selection.
Every initial list and no-match state requires its own PNG/ARIA artifact plus
raw observations, input trace, logs and artifact hashes.

The new route has a finite 240-second budget planned before native capture. The
ordinary runner/observer budget adds it to the existing 570 seconds; the separate
trust window retains its previous budget. Readiness polling only reads state;
accepted inputs are issued once. Acceptance requires all 52 native proofs,
installed VSIX27, full Node infrastructure, formatting/Clippy/workspace and
fresh automatic plus strict B09 regression on one frozen Windows source/profile
and server fingerprint. macOS requires its own fresh proof bundle.

Retained authoring failures include an incorrect pointer-count golden (38 rather
than 40), an aggregate accept count that included placeholder Enter, and the
first real prototype's selector matching hidden recycled separator elements.
The inspected SDK assigns `display:none` to unused separator nodes. Selecting
the visible separator observes the actual complete count without hiding any
symbol rows or filtering provider output.

The corrected real prototype passed all 592 physical inputs and 631 registered
checks in 80.6 seconds, including 158 exact destinations and eighteen rejected
placeholder acceptances. Its PNG/ARIA states and complete raw observations
remain retained. The full frozen-source bundle must be recaptured separately.

The first full native capture passed through Problems navigation, then the old
repair-unsaved path observed a transient unpainted target squiggle after the
correct pointer selection. Its screenshot/trace retain the correct `@` buffer
and caret but a preceding diagnostic decoration state. The observer now waits
within the existing bounded observation interval for the original exact target
decoration to align before asserting it. No accepted input is retried, and the
prior diagnostic actions/assertions/budgets stay unchanged. A focused three-route
diagnostic capture and a new complete frozen-source capture validate this fix.
