# Schema function lookup preservation

B04.17 advances 48 S2/S5/S8 positive/negative service/protocol cells for references,
highlight, prepare-rename and rename. Requirement IDs follow
`<feature>/syntax/<S2|S5|S8>/<positive|negative>/<service|protocol>`;
broad cells remain unreviewed.

The independent oracle rejects introducing a function at an unresolved call,
function value, qualified call or unused import. It rejects making other schema
short-name calls ambiguous and renaming a bare target use into an ambiguous or
different exact-name function. Qualified target edits can safely coexist with an
exact unqualified schema name, whose binding must remain unchanged. Local and
source owners retain priority. Both layers check exact references/targets, apply
safe edits, regenerate explicit schema metadata, compare whole files, requery and
restore under Unicode LF/CRLF; protocol includes UTF-16, versions and ABI markers.

The fixture has 24 queries across seven groups; all four writable groups undergo
applied edits. Unknown calls and values use different proposed names so their
rejection assertions are independent. The safe exact-name case exposed a span
lookup defect: metadata-only functions borrowed a qualified owner's source span.
Source locations now require canonical identities. The initial capture and span
failures, final validation and Windows evidence are recorded with B04.17.
