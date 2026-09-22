# Schema variant lexical capture

B04.20 advances 48 S2/S4/S8 positive/negative service/protocol cells for references,
highlight, prepare-rename and rename. IDs follow
`<feature>/syntax/<S2|S4|S8>/<positive|negative>/<service|protocol>`;
broad cells remain unreviewed.

The independent fixture has 38 queries across six ownership groups. Distinct
proposed names reject capture by a function parameter, closure parameter, later
default parameter, module declaration and imported binding. An unused direct
variant import independently rejects a duplicate module binding; the source-backed
wrapper declaration also rejects a duplicate declaration name. Existing sibling
variant conflicts remain rejected.

A separate writable variant is safely renamed to the name of an unrelated local:
its qualified use and explicit alias retain ownership. The import path changes,
while alias spelling and alias uses do not. This guards against rejecting all
renames merely because the proposed name occurs somewhere in a source file.

The complete reference and edit sets retain prior source/schema/local isolation,
read-only source definitions, metadata-only nulls and pattern classification.
Both layers apply all three writable groups, compare whole files, regenerate
schema metadata, parse, requery and restore. Protocol also covers Unicode LF/CRLF,
UTF-16, encoded paths, open/closed versions, both edit forms and ABI confirmation.

The initial failure permitted parameter capture. Function and variant rename now
share the same lexical guard using their canonical editable sites. Reverse capture
of unresolved paths, short-owner ambiguity, remaining parameter/lifecycle coverage
and UX05/UX06 still require subsequent work; this child does not close B04.
