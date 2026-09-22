# Source variant import ownership

B04.22 advances 48 S2/S4/S8 positive/negative service/protocol cells for references,
highlight, prepare-rename and rename. IDs follow
`<feature>/syntax/<S2|S4|S8>/<positive|negative>/<service|protocol>`;
broad cells remain unreviewed. Definition and type-definition assertions also
protect the shared navigation behavior.

The independent fixture has 42 queries across six ownership groups. A public
source enum owns its direct variant imports, explicit aliases, type-alias paths,
ordinary uses and patterns even when schema facts declare the identical qualified
variant. Every source query has an exact source definition and enum type target.
Public source enum variants retain their existing non-renamable policy.

Imported alias spelling does not turn a local parameter into an enum reference.
Unknown source variants and inaccessible private enum imports remain unresolved
even when schema facts offer those names. Schema rename changes only its own
imports and uses, leaving the public source enum imports unchanged. Metadata-only
schema groups retain null definitions and rename results.

Both layers compare complete reference/highlight sets, apply all three writable
groups, compare whole files, regenerate schema metadata, parse, requery and restore.
The source import and enum declaration include Chinese/non-BMP prefixes. Protocol
also checks LF/CRLF, UTF-16, encoded paths, versions, both edit forms and ABI markers.

Initial failures exposed omitted source import/alias references and missing
type-definition navigation. Source variant sites now resolve scoped paths and
imports to a visible source enum; navigation and references share this result.
Schema import collection excludes paths already owned by source declarations.
The previous separate source-reference path scanner is removed. Remaining private
variant rename/import edges, ambiguity-removal and schema parameter/lifecycle
coverage and UX05/UX06 are still required before B04 closes.
