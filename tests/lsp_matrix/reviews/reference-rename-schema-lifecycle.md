# Reference and rename schema lifecycle

B04.29 established the schema lifecycle fixture. B04.45 completes its eight
missing-schema and stale-schema protocol state cells for references, highlight,
prepare-rename and rename. Dynamic state cells were reviewed in B04.41;
syntax partitions and editor interactions remain separate obligations.

The independent fixture has eight explicit schema states: source-backed
`host::grant`, retarget to `host::award`, removal of the host fact while
metadata-only `meta::ping` remains, invalid artifact, restoration of grant,
schema deletion, recreation with award, and restoration of grant. A same-named
call on an unknown module and a dynamic receiver never join either host owner.
The two source-backed owners have separate declarations and calls in open and
closed documents; the metadata-only function has a call but no source target.
An independent source-only `helpers::stable` declaration and calls in the open
importer and closed consumer remain resolvable through every schema state.

At every state, service and protocol repeat complete references with and
without declarations, document highlights, definition, prepare range and
placeholder, and rename edits. Active source-backed functions produce only
their own three-site edit set and a schema ABI warning. Inactive functions have
no target, references or edits. Metadata-only calls remain referenceable when
the artifact is valid but cannot be prepared or renamed. Unknown and dynamic
calls remain unowned. The service compares incremental and fresh databases.
The source-only control keeps its exact three-site references and rename edits,
single-document highlight, definition and prepare range without a schema ABI
warning even while the artifact is invalid or deleted.
Protocol exercises actual watched-file notifications, including invalid and
deleted diagnostics, and compares a long-lived server with a fresh server at
each state. Both layers run LF and CRLF with Chinese and non-BMP prefixes;
protocol also checks encoded Unicode paths, UTF-16 positions, both edit forms,
and open versus closed document versions.

Stale-version, cancellation, remaining syntax partitions and UX05/UX06 remain
before B04 acceptance. B00-B03 snapshots stay fixed; macOS
needs fresh evidence when used. B16 remains deferred.
