# Source enum record-field ownership

B04.07 advances S4/S6 references, highlights, prepareRename and rename partitions
at service and protocol layers. The broad cells remain unreviewed; schema fields,
methods and other syntax/semantic/lifecycle partitions remain separate work.

The independent three-file fixture has 15 query positions, 12 field sites across
three owners, and three preserved-local probes. It distinguishes two same-named
enums and two record variants of one enum. Enum and variant import aliases,
qualified/local constructors, explicit/shorthand match patterns, an initially
closed consumer and Unicode LF/CRLF are exercised. Negative labels cover literals,
unknown fields, unknown owners, and known enums' unit, tuple and missing variants.
Rename rejects existing-field and unknown-label capture within one variant;
sibling variants and other enums may independently use the same spelling.

Both drivers repeat exact reference sets with/without declarations, kinds,
highlights, definitions, prepare targets and full rename edits. All three groups
are actually renamed; full documents are compared, parsed, queried, restored and
queried again. Constructor shorthand parameters and a shorthand pattern binding
retain their original local ownership after expansion. Protocol verifies actual
didChange, encoded Unicode URIs, both UTF-16 edit forms and open/closed versions.
The existing source-struct matrix runs against the same unified implementation.

The fixture exposed missing aliased constructor/pattern references and rename
targets, and enclosing-enum fallback for invalid record labels. Shared canonical
record owners now retain the enum variant identity, replacing the old independent
name-based enum-field search. Reference pattern classification is preserved.
No runtime, grammar or host behavior is changed.

Original B00-B03 snapshots stay fixed; fresh evidence is tied to the current
Windows profile and source identity. macOS requires independent fresh evidence
when used. Resume B04.08 schema/callable ownership and subsequent semantic,
lifecycle and UX05/UX06 gates. B16 remains deferred.
