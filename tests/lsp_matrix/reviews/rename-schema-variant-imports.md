# Schema variant import ownership

B04.19 advances 48 S2/S4/S8 positive/negative service/protocol cells for references,
highlight, prepare-rename and rename. IDs follow
`<feature>/syntax/<S2|S4|S8>/<positive|negative>/<service|protocol>`;
broad cells remain unreviewed.

The independent 29-query fixture has five ownership groups. Module aliases,
enum aliases, direct imports and explicit variant aliases join the canonical
schema variant reference set. Explicit alias declarations and uses remain intact
when renaming the variant; the underlying import path changes. References and
edit sets are asserted separately. Alias patterns retain their pattern kind.

A colliding public source enum retains its own declaration, ordinary and pattern
references and cross-file qualified use; it remains non-renamable under the
existing public-enum policy. An independently renamed local shadows an imported
alias without becoming a schema reference. Other metadata-only variants retain
their separate sets and null source/rename results. Unknown paths, comments and
strings remain excluded. The read-only oracle explicitly distinguishes a public
source definition from an absent metadata-only definition.

Both writable groups undergo complete workspace edits, whole-file comparison,
explicit schema regeneration, parse, repeated queries and restoration. Both
layers cover Unicode LF/CRLF; protocol checks UTF-16, encoded paths, open/closed
versions, both WorkspaceEdit forms and schema ABI confirmation. Existing pattern
classification cases remain in the fixture.

Initial failures exposed missing schema alias references and omitted qualified
source variant references. Definition, references and rename now share schema
variant sites with import expansion and source/local precedence. Source reference
lookup also resolves qualified paths to a visible enum owner. Remaining variant
capture/collision, parameter and lifecycle partitions require subsequent work;
UX05/UX06 and the whole B04 gate are still open.
