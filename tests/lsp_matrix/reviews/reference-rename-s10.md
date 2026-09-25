# Reference and rename S10 ownership

B04.49 reviews the sixteen S10 positive and negative service/protocol syntax
cells for references, document highlights, prepare rename and rename. Each
positive query must keep the intended symbol owner and complete exact sites;
negative queries must not borrow a similarly spelled owner or create an edit.

| Ownership partition | Independent evidence |
|---|---|
| Local and parameter | The baseline coordinate, named-parameter, method-parameter and default-binding matrices distinguish declarations, reads, writes, calls, shadowed bindings and labels. Service asserts `SymbolRef::Local` identity; both layers compare complete sites, highlight kinds, prepare ranges and applied edits. |
| Source declaration and member | Imported values/types, source fields and source methods check qualified source identity across files. Private and missing imports, wrong member receivers and colliding names stay outside the source-owned set. |
| Source and schema variants | Source-variant and schema-variant import matrices distinguish unit, tuple and record variants, pattern uses, aliases and separate source/schema owners. Unknown, private and ambiguous variants remain empty/null. |
| Schema and host fact | Schema method, field, function and type-import matrices assert `SymbolRef::Schema` where source-backed, complete editable sites and ABI risks. Metadata-only facts are read-only; similarly named source facts do not absorb them. |
| Standard library and builtin | The import-boundary matrix checks stdlib terminal, alias, call and module positions as non-targets. A separate service/protocol test checks `max` and `i64` directly: no references, highlights, prepare range or rename edit. |
| Module | The import-module tests check exact source-module segments across files and same-document highlights. The ownership test checks one module symbol across both imports and confirms that the module path itself cannot be prepared or renamed. |
| Dynamic `Any` and unresolved | The dynamic return matrix proves known source/schema callables remain owned while their `Any` member suffixes do not acquire an owner. Coordinate no-group oracles, missing imports and malformed-neighbor recovery require empty references/highlights and null prepare/rename results. |

The coordinate drivers repeat complete queries with and without declarations,
compare byte/UTF-16 marker ranges and URI ownership under LF/CRLF, apply actual
workspace edits and re-query. Schema-only and source-backed facts use separate
policies; no test query executes scripts or reads live host state. This review
does not close other B04 syntax groups or native editor interaction gates.
B00-B03 accepted snapshots stay fixed; macOS needs independent current evidence
when used, and B16 remains deferred.
