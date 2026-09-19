# Completion editor acceptance

The shared `input-completion.json` fixture owns two schema functions with distinct
documentation and three independent source files. Chinese and a non-BMP character
precede each cursor on the same line. The cursor is inside `coWrong`; typing `m`
leaves `comWrong`, so accepting a suggestion must replace the entire identifier.
Expected labels, documentation, snippet, resulting text and caret come from the
fixture and marker coordinates, never from provider output or rendered content.

Installed-provider smoke queries dirty LF and CRLF documents through the real
language client. It checks the complete candidate set, lazy documentation before
resolve, exact owned documentation after resolve, function kinds/details,
snippet strings and full UTF-16 replacement ranges. The query must not edit source.

UX04 has separate Input and Render contracts for Enter, Tab and Escape. The
external workbench driver types the prefix, triggers suggestions and moves to the
second candidate using the keyboard. Enter and Tab each expose the resolved docs,
close the detail pane and accept. Exact document text and the caret inside the
inserted call are checked, then keyboard undo restores `comWrong` and its caret.
Escape closes the visible widget, preserves that typed source and retains editor
focus. Screenshots, accessibility snapshots, action receipts and server/client
logs are retained and hashed. Fixture setup may open a document and place the
initial cursor; it does not accept, dismiss, undo or simulate a provider result.

Each proof is bounded and its required actions/checks are validated by the local
evidence gate. A partial run cannot certify another route. The two registered
profiles translate the undo key independently; acceptance requires fresh evidence
from one exact profile. Historical macOS evidence cannot supplement Windows
results. Generated combinations, scale gates and B16 environment expansion remain
separate obligations.
