# Schema variant lookup preservation

B04.21 advances 48 S2/S4/S8 positive/negative service/protocol cells for references,
highlight, prepare-rename and rename. IDs follow
`<feature>/syntax/<S2|S4|S8>/<positive|negative>/<service|protocol>`;
broad cells remain unreviewed.

The independent fixture has 38 queries across seven ownership groups. Distinct
candidate names reject turning an unknown qualified expression, qualified pattern,
unused import or bare expression into a reference to the renamed variant. A
previously unique short-owner reference to another variant must not become
ambiguous. The unknown path on another fully qualified owner remains unknown
after a safe rename; unrelated locals with the proposed name keep their bindings.

The lookup check compares canonical variant identities before and after a proposed
schema name change. Target paths retain the renamed identity; other paths retain
their identity or unresolved state. It uses the same scoped expansion and variant
resolver as reference collection. It additionally models the new local binding
introduced by an unaliased import. Import paths use exact registered identities.
No scripts or host calls execute during comparison.

Both layers apply all three writable groups, compare complete file contents,
regenerate schema metadata, parse, repeat exact queries and restore. Existing
source/metadata/local separation and independent reference/edit sets remain in the
fixture. Protocol checks Unicode LF/CRLF, UTF-16, encoded paths, versions, both
WorkspaceEdit forms and schema ABI confirmation.

The initial failure allowed capture of an unknown qualified value. Broad variant
ownership and lifecycle partitions remain open, including independent coverage of
removing an ambiguity and source-import edge cases; parameter coverage and
UX05/UX06 also remain. This child does not close B04.
