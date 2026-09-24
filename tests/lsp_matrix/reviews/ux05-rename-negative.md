# UX05 rename cancellation and rejection

B04.37 adds separate Input and Render proof for `rename-cancel`, `invalid-name`,
and `colliding-name` using the same three-file fixture as confirmation. Each
route starts from the unchanged fixture in the installed VSIX session.
The driver opens the default F2 rename widget at the `grant` use, checks
visible input and focus, and types the route's candidate name. Escape cancels
`award`; Enter submits malformed `1bad` or the colliding `call` declaration.

For rejected names, the driver requires the visible VS Code alert to contain
the exact server response. Invalid identifier text uses LSP `InvalidParams`;
a prepared target whose valid new name would conflict or change reference
ownership uses `InvalidRequest`. The widget is then dismissed. Every route
checks that the open document, unopened origin and closed importer, and all
disk files still contain the original text, and that diagnostics remain empty.
The action trace, alert text, accessible widget snapshot, screenshots, document
and disk observations, LSP trace, extension-host log, and server trace are
retained as evidence.

Protocol tests assert that malformed names return the explicit error while
unknown targets retain a null response. The existing collision and reference
coordinate matrices now check the explicit rejection code and message before
confirming that no workspace edit is produced. The service layer still uses
`None` for rejected edits; the LSP boundary supplies the user-visible reason.
