# Diagnostic and Quick Fix dependency lifecycle

B05.9 covers the four protocol state cells for `dependency_change` and
`dependency_delete` across diagnostics and code actions. A closed defining
module exports `grant`, while its importer is opened and then made dirty without
changing its disk source. A second, private `grant` declaration tests visibility.
The defining module changes its public export to `award`, is deleted, then is
restored with `grant`. The importer keeps unresolved references to both names
and an independent `Array<i64>.frist()` error. Each phase runs under LF and CRLF
in a percent-encoded Unicode workspace.

Every transition publishes the complete importer diagnostic set. The test pins
exact codes, messages, severities, UTF-16 ranges and candidate replacements,
and compares the full publication with a freshly initialized server using the
same disk dependency and dirty importer text. Import actions follow only the
currently public export: `grant`, then `award`, neither after deletion, then
`grant` again. The independent Array action set always has exactly three
repairs. Each action checks its title, kind, sole target URI, single edit range
and text, plus the versioned edit projection; full action arrays equal fresh
servers at every phase.

Applying the current import action to a fresh dirty importer matches an
independent whole-source oracle, clears only its selected unresolved name and
leaves no stale action at that name. The other missing name and the Array error
remain. For CRLF sources, the import edit's LF insertion is checked explicitly
against the resulting mixed-line-ending source. The test does not claim
unopened-importer coverage or separate watched-file obligations.
