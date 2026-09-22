# Schema variant expressions and patterns

B04.18 advances 32 S4/S8 positive/negative service/protocol cells for references,
highlight, prepare-rename and rename. Requirement IDs follow
`<feature>/syntax/<S4|S8>/<positive|negative>/<service|protocol>`;
the broad cells remain unreviewed.

The independent fixture has 14 queries across three canonical schema owners.
It distinguishes match scrutinees and arm expressions from patterns on the same
line, recognizes a pattern whose arrow is on the following line, and excludes
arrows in comments from classification. Same-named variants of other owners keep
separate reference sets. Metadata-only owners have no source definition or rename;
unknown owners, strings and comments have no references or edits.

Both layers repeat exact reference/highlight and definition queries. The writable
owner is renamed from every reference, including patterns; its complete edit plan
is applied, whole files compared, schema variant name and value fact regenerated,
sources parsed, queried again and restored. The protocol also checks UTF-16,
Unicode LF/CRLF, encoded paths, open/closed versions and ABI confirmation markers.

The initial oracle exposed two defects: line-based arrow scanning misclassified
four sites, and prepare-rename excluded HIR pattern paths even though edit
collection included them. Classification now uses HIR path kind, and rename target
selection includes patterns. Import aliases, source-owner collisions and broader
schema parameter/variant lifecycle coverage remain for subsequent children.
