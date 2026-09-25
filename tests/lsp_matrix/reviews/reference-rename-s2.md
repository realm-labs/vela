# Reference and rename S2 function and method bodies

B04.55 reviews the sixteen S2 positive/negative service/protocol syntax cells
for references, document highlights, prepare rename and rename. A new
independent body matrix separates a function parameter, a writable local and a
same-spelled local inside an `if` branch. Eighteen query points check exact
ownership through initializer and compound assignments, `if/else`, a capturing
callback closure, guarded `match`, `for`, and `return`. `let`, `break` and
`return` keyword positions have no symbol target.

The baseline coordinate matrix adds nested block shadowing, reads, writes and
calls. Imported-type matrices distinguish explicit body type hints from local
variables and similarly named source types. Method-parameter and default-
binding matrices visit inherent, trait and implementation bodies, closures,
default expressions and caller values. Record-field and named-parameter
matrices separate member writes and callback argument labels from local values.
Local shorthand and collision tests check complete applied edits across
captures, nested patterns and sibling scopes, and reject capture of another
binding or unresolved name. The S6 control-flow matrix independently confirms
that guarded arm and loop bindings cannot borrow a function-body local owner.

The coordinate drivers repeat complete reference sets with and without
declarations, same-document highlight ranges/kinds, prepare ranges and
placeholders, and full edit plans. Every writable group is applied, compared
against whole-file expected source, parsed, re-queried and restored under
Unicode LF/CRLF. Protocol uses actual LSP requests, UTF-16, encoded paths,
`didChange`, versions and both edit forms. This is an editor ownership review;
it does not claim runtime control-flow behavior, compiler legality or native
editor interaction. Native B04 interaction gates remain open. B00-B03 accepted
snapshots stay fixed, macOS needs independent current evidence when used, and
B16 remains deferred.
