# Source struct field ownership

B04.06 advances S4/S6 references, highlights, prepareRename and rename partitions
at service and protocol layers. Broad cells remain unreviewed: this matrix covers
source structs, not all enum/schema fields, methods or other S4/S6 syntax.

The independent three-file fixture has two same-named public structs, 13 query
positions and 13 field sites. It includes an imported alias, qualified and local
constructors, shorthand constructors, explicit and shorthand match patterns,
dot reads, writes and compound assignments, an initially closed consumer, literal
contents, an unknown field on a known owner, and an unknown constructor. It rejects
collision with an existing field and capture of an unknown label of the same
owner; another struct can independently adopt that unknown label's spelling.

Both drivers repeat exact reference sets with/without declarations, highlight
ranges/kinds, definition targets, prepare ranges/placeholders, and complete edits.
They run Unicode LF/CRLF fixtures, apply both field groups, compare whole files,
parse, requery, restore and query again. Three additional local-ownership probes
check constructor shorthand parameters and a shorthand pattern binding before
and after expansion. Service checks canonical local symbols as well. Protocol
uses encoded Unicode paths, actual didChange, both edit forms and open/closed
client versions. No source-language or runtime behavior changes.

The matrix exposed omitted alias/pattern references and constructor/pattern edits,
loss of qualified receiver identity between same-named structs, and fallback from
an unknown field label to the enclosing struct. Label sites now share scoped
declaration ownership across references and rename; dot targets use a common
qualified-owner resolver. Field edits preserve the local side of shorthand.

Original B00-B03 acceptance snapshots remain fixed. Fresh Windows evidence is
recorded in the checkpoint; macOS requires its own fresh run when used. Remaining
field/variant/schema/callable, semantic/lifecycle and UX05/UX06 work continues in
B04.07 and later children. B16 stays deferred.
