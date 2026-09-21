# Declaration references across expression binding maps

B04.12 advances 64 service/protocol positive/negative partitions for references,
highlight, prepare-rename and rename under S2/S4/S5/S6. IDs follow
`<feature>/syntax/<S2|S4|S5|S6>/<positive|negative>/<service|protocol>`; broad cells
remain unreviewed pending other semantic and lifecycle partitions.

The independent fixture has 26 query points and six groups: a private module
constant, same-spelled tuple fields on two variants, tuple-default locals,
required-trait-default locals and caller values. Tuple field defaults have no
parameter environment: their `value` resolves to the module constant. Required
trait defaults separately bind their parameters. HIR default-body construction
and the existing schema-field-default path-facts test establish this distinction;
the fixture preserves these semantics rather than inventing tuple parameter uses.

The constant is used in tuple defaults (including a closure), a required trait
default, constant/state initializers, inherent/trait-default methods and a normal
function default. Collection previously visited only ordinary function binding
maps. It now visits every distinct canonical binding map, avoiding duplicate
edits when nested closures and defaults share a map. Both reference and rename
collection use this same enumeration.

The first full workspace run exposed older imported-type expectations in both
layers that omitted the constructor inside a constant initializer. Assertions include
that exact Read site (seven references, six active-document highlights), preserving
all prior expected sites and symbol checks.

Both drivers repeat exact sets/kinds, highlights, definition/prepare targets and
complete edits, apply all six groups, compare whole files, parse, requery and
restore. The local groups include compound writes. Unknown labels/owners and
strings remain empty; same-owner label/parameter collisions and local/global
capture reject. Collision targets that are themselves renamed follow their
explicit oracle group, so applied-state assertions use the current spelling.
The sibling variant may adopt another variant's unknown label spelling.

Protocol checks LF/CRLF, UTF-16, encoded paths, both edit forms and open/closed
versions. Original B00-B03 snapshots remain fixed; Windows/macOS evidence stays
independent and B16 remains deferred. Schema callable ownership remains open.
