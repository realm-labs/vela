# Diagnostic and quick-fix Unicode projection

B05.1 verifies the UTF-16 and CRLF protocol environment cells for diagnostics
and code actions with one shared service/protocol fixture. Chinese text and an
emoji precede `frist` on its line; a separate `foobar` method typo remains in
the same file. Both layers assert the complete initial two-diagnostic set,
including code, message, severity and exact ranges. The quick-fix request
requires exactly three ranked candidates (`first`, `find`, `last`), one edit per
candidate and no unrelated edit. The protocol checks both legacy `changes` and
versioned `documentChanges` ranges against the independent UTF-16 markers.

Applying `frist` to `first` must yield the complete expected source, parse
cleanly, publish only the unchanged `foobar` diagnostic, and offer no action at
the repaired site. This runs under LF and CRLF; the service asserts byte-column
ranges while the protocol asserts UTF-16 columns. The new test exposed that
code-action edits bypassed the existing workspace-edit UTF-16 projection even
though diagnostics already used correct ranges. Code actions now use the same
snapshot-aware projection as rename, including open versions and client URI
spelling.

The four environment obligations are closed only for this exact representative
range workflow. Other B05 syntax, state, encoded-URI, installed editor and
native UX07/UX08 obligations remain open. B00-B04 accepted snapshots stay fixed;
macOS requires independent current evidence when used, and B16 remains deferred.
