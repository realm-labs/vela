# Diagnostic and Quick Fix schema lifecycle

B05.8 covers the four protocol state cells for `missing_schema` and
`stale_schema` across diagnostics and code actions. One encoded Unicode
workspace has an open script with two misspelled `Player` fields and an
independent `Array<i64>.frist()` error. The host schema moves through `level`,
deletion, `rank`, malformed JSON and restored `level` under LF and CRLF.

Every phase asserts the complete source diagnostic publication: exact codes,
messages, severities, UTF-16 ranges and candidate replacements. Missing or
invalid schema produces one controlled warning and suppresses guesses about
`Player`, while the source-owned Array diagnostic and its three exact Quick Fix
edits remain available. Replacing `level` with `rank` changes both host field
candidates and one-edit actions to `rank`; restoring the old schema returns
`level`. Every action checks its title, kind, only target URI, marked edit
range, replacement text and versioned edit projection. A freshly initialized
server at each schema phase must agree with the long-lived server on complete
diagnostics and actions.

In the initial and changed valid-schema phases, the test applies one host
field action in a fresh open document. The whole result must match an
independent marked source oracle, remove only the selected field diagnostic,
preserve the other host and Array errors, and return no stale action at the
repaired location. This proof does not claim the separate watched-file or
workspace-configuration obligations.
