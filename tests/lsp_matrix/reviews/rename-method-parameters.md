# Body-backed method parameter ownership

B04.09 advances S4/S5 references, highlights, prepareRename and rename at service
and protocol layers. Broad cells stay unreviewed; signature-only, schema and
remaining callable forms require separate evidence.

The independent three-file fixture has 23 query positions and six binding groups:
an inherent method, its sibling, a same-named method on another type, a trait
default, an explicit trait implementation and a caller parameter. It includes
aliases, a closed consumer, closure calls, defaults and a missing argument value.
Unknown labels, an unknown receiver and a string produce empty results.

Both drivers repeat exact reference sets with and without declarations, reference
kinds, highlights, definitions, prepare targets and complete edit plans. All six
groups are actually renamed; entire files are compared and parsed, then queried
again after edits and restoration. Existing-parameter and owned unknown-label
capture reject, while a sibling method can independently adopt that label name.
Service checks canonical local identity. Protocol checks LF/CRLF, UTF-16, encoded
Unicode paths, both edit forms, didChange and open/closed document versions.

The initial service test exposed missing method declaration/body references.
Method metadata body spans excluded parameter declarations, and consumers selected
the enclosing impl declaration instead of the method's body binding map. Queries
now include parameter spans and use canonical body maps. Named labels join resolved
source signature spans to those maps; body IDs keep sibling owners independent.

Original B00-B03 snapshots remain fixed. Windows evidence is fresh for this child;
macOS requires its own fresh evidence when used. Resume B04.10 remaining callable
and schema-variant ownership, then semantic/lifecycle and UX05/UX06. B16 is deferred.
