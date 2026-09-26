# B06 semantic-token workbench review

The three UX09 routes own six native Input/Render obligations: edit-scroll,
toggle-off-on and unresolved-token. They run in an isolated owned profile with
the installed production VSIX, the pinned editor version and Dark Modern theme.
Provider smoke remains separately reviewed in [semantic-token-editor.md](semantic-token-editor.md).

The independent fixture marks complete spans in multiline Chinese/non-BMP source:
a type declaration, function declaration/call, Unicode comment, local, typed
field, unresolved qualified owner/member and dynamic field. Expected colors
come from the bundled Dark Modern/Dark+/Dark VS theme chain and the Vela grammar,
not a provider or renderer capture. The standard client's unresolved references
fall back to variable blue; it does not advertise the custom unresolved modifier.
Dark Modern's plain foreground is #CCCCCC. Disabling semantics restores lexical
namespace green, function yellow and plain local text; these distinguish the
enabled and disabled states without test-authored styling.

DOM-only observations map full marker UTF-16 ranges across rendered text nodes.
Every covered node must agree on color, font style, weight and decoration. Exact
text, logical line, columns, viewport visibility and alignment with the containing
line are asserted. Unique padding lines distinguish virtualized rows. Raw glyph
rectangles, visible lines and viewport bounds are retained. No Monaco model or
token API supplies expected classifications or input actions.

Edit-scroll inserts two Unicode lines through the keyboard, checks exact dirty
text and shifted styles, executes 36 declared wheel ticks in each direction and
requires all marked head spans to leave the viewport while the tail is visible.
The fixed scroll distance also covers the shorter code viewport when a previous
diagnostics route leaves the Problems panel open; it does not depend on that
panel being closed or retry actions until the assertion passes.
Returning restores every exact shifted style. The pinned keyboard input path
submits code points; its seven native undo groups are independently specified as
`//`, ` edited`, ` 中😀`, newline plus `/*`, ` 新😀`, ` */`, and final newline.
Each undo checks the full intermediate document. Final undo restores clean disk
text and original styles. Source checks never retry an input action.

Toggle-off-on opens Settings by the native platform shortcut, searches the exact
setting ID, reads the actual visible category/title and changes the dropdown with
pointer and Home/Down/Enter keys. The pinned editor declares values in order
true, false, configuredByTheme. A read-only bridge observes the effective setting;
it never changes configuration. Switching back to code checks unchanged clean
source and lexical styles, then the same UI enables semantics and restores all
original styles. The settings accessibility tree and disabled screenshot persist.

Unresolved-token double-clicks the actual call glyph, checks its exact selection,
types `missing`, and requires function yellow to become variable blue. Other
marked styles remain fixed. A closed, unimported file declares a same-named
function and must remain unopened; it cannot lend a resolved color. Native undo
restores the full clean source and original styles.

Each route retains input receipts, independent assertions, before/after and
special-state screenshots, DOM observations and client/server logs. Source and
server fingerprints invalidate stale proof after test/oracle changes. Windows
and macOS share the recipe and checkpoint, but each requires its own fresh
installed editor/native evidence. Windows execution does not certify macOS.
B16 stays deferred.
