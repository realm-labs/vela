# Diagnostic and Quick Fix recovery state

B05.7 covers the `diagnostics/states/recovery/protocol` and
`code-action/states/recovery/protocol` cells. One marker fixture holds two
independent unknown methods after Chinese and emoji on a valid function. The
state sequence opens that source, appends an unclosed function declaration,
applies the first Quick Fix while the declaration remains malformed, restores
the typo, then repairs only the malformed declaration. LF and CRLF run the
same sequence.

Every diagnostic publication has an exact complete code, message, severity
and marker-derived UTF-16 range oracle: the damaged states add one `E_PARSE`
for the unclosed parenthesis, while both valid method diagnostics remain.
The valid typo keeps its three ordered, versioned one-edit Quick Fix actions
through damage and repair. The malformed parenthesis offers no guessed action.
Applying the first action to the damaged source produces the complete expected
text, clears only its method diagnostic and leaves the parse and unrelated
errors; a second action request at that fixed location is empty. Restoring the
typo restores its actions. For both damaged and repaired states, a fresh server
opened at the same text and version produces identical complete diagnostics
and actions to the long-lived server.
