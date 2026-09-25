# Diagnostic S9 recovery partitions

B05.10 covers the four service/protocol positive and negative S9 diagnostic
cells. A marked source contains an unresolved name and a misspelled Array
method after Chinese and emoji. Consecutive edits append an incomplete member
(`scores.`), call (`scores.first(`), type (`Array<>`) or declaration (`fn
broken(`), then add a Unicode prefix line and restore the original source.
Every state runs with LF and CRLF.

The service test pins the complete ordered diagnostics: codes, messages, error
severity, byte ranges and candidate replacements. The protocol test pins the
same facts after LSP UTF-16 projection and compares the full publication with
a fresh server at the same text and version. Both tests require the adjacent
unresolved name and method typo to survive every incomplete neighbor. The type
form reports one exact type-arity error; the declaration reports one exact
parse error. Restoration removes these new errors without clearing the
independent ones. The service also verifies that every stale-generation query
returns an empty stale result.

The test exposed an unknown-field diagnostic with an empty name for `scores.`. Diagnostics
now suppress an empty member name, so the incomplete member and call forms do
not invent unknown-member or call errors. Exact whole-set assertions guard this
negative behavior. Separate state and code-action recovery obligations remain
owned by their own reviewed tests.
