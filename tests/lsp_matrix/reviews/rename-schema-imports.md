# Schema function import ownership

B04.15 targets 48 positive/negative service/protocol cells across S5/S8/S10 for
references, highlight, prepare-rename and rename. IDs follow
`<feature>/syntax/<S5|S8|S10>/<positive|negative>/<service|protocol>`.
Broad cells remain unreviewed.

The independent fixture distinguishes direct imports, function aliases and module
aliases. References include imported bindings and their calls/values; renaming the
schema function edits import terminal paths and non-aliased uses while preserving
explicit aliases. Local shadows, source functions and metadata-only imports retain
their own behavior. Unknown imports do not borrow same-spelled schema functions.
The oracle declares reference and edit sets separately. Both drivers compare exact
sets, apply complete edits, regenerate schema metadata, requery and restore under
Unicode LF/CRLF; protocol also checks UTF-16, versions and ABI annotations.

Thirty queries across five groups include explicit aliases whose spelling initially
equals the function name, closed-file imports and local alias shadows. All four
writable groups undergo complete applied edits and repeated/restored queries.
The initial missing import/use references are retained as a reproducer. Final
validation and Windows evidence are recorded in the B04.15 checkpoint; schema
capture checks, parameters, variants and lifecycle partitions remain open.

Full regression exposed a source namespace alias colliding with schema metadata.
Expanded paths now check canonical visible source declarations before schema
lookup. Existing completion/import matrices retain exact source definitions.
The source-backed schema import completion oracle now checks its explicit source
span instead of the previous null result; metadata-only cases retain null checks.
