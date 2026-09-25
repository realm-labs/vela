# Reference and rename S6 patterns and control flow

B04.53 reviews the sixteen S6 positive/negative service/protocol syntax cells
for references, document highlights, prepare rename and rename. The independent
control-flow matrix gives an outer parameter, a `for` binding and a guarded
`match` binding the identical spelling `value`. Its eleven owned query points
assert three separate complete reference sets, highlight ranges/kinds, prepare
targets and applied edits. It also queries `for`, `in`, `match`, guard and
ordinary `if`, `_`, `break` and `continue`: each has empty references and
highlights and no prepare or rename target.

Existing source and schema record/enum matrices cover constructor versus
pattern ownership, explicit and shorthand record/variant fields, tuple
variants, aliases, qualified paths, private and missing owners, and collision
boundaries. Local shorthand cases independently check guarded patterns, nested
tuple/record bindings and `for` patterns while preserving field labels during
rename. Local collision cases reject capture across loop iterable/body and
match guard/arm boundaries. These checks are mapped only where their actual
test methods assert the claimed result.

Both coordinate drivers run Unicode LF and CRLF, repeat exact reference sets
with and without declarations, apply every editable group, compare whole
files, parse them, re-query and restore. Protocol additionally sends real
`didChange`, checks UTF-16 ranges, encoded URIs, client versions and both
workspace edit forms. This review covers symbol ownership and editor requests;
it does not certify runtime control-flow execution or compiler legality.
Other B04 syntax and native interaction gates remain open. B00-B03 accepted
snapshots stay fixed, macOS requires independent current evidence when used,
and B16 remains deferred.
