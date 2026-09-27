# Hover S8 modules and imports

Scope: `hover/syntax/S8/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::module_matrix_tests::hover_module_matrix_preserves_import_paths_aliases_and_visibility`
and `tests::hover::modules::hover_module_matrix_preserves_import_paths_aliases_and_visibility`.

Two independently authored fixtures contain 319 queries: `hover-s8` has 260
queries (201 complete hovers and 59 null); `hover-s8-package` has 59 (39 complete
hovers and 20 null). Start/interior positions, LF/CRLF, schema present/physically
absent and repeats yield 2,132 hover positions per layer, each repeated. Without
schema, 52 registered queries become null; one terminal type hint retains Type
`Row` with Any, no docs and no identity, following the existing S3 contract.
Service compares complete metadata and identity; protocol compares whole Markdown,
UTF-16 ranges and explicit JSON null. Markers independently own coordinates;
same-line CJK/non-BMP comment prefixes precede selected tokens.

| Partition | Independent assertions |
|---|---|
| Source imports | Public function, async function, const, VM/extern state, struct, enum and trait declarations retain signature/type/docs and source identity through imports, aliases, uses and qualified segments. Nested and foreign namespaces remain distinct. |
| Enum paths | Module, type and unit/tuple variant segments retain their own kinds, docs and identities. Import and alias declarations do not lend the terminal variant's metadata to a type prefix. |
| Registry and stdlib | Registered modules, types, traits, functions and unit variants retain exact schema metadata. Known math namespace and function aliases retain builtin metadata. Missing schema does not manufacture registry ownership. |
| Type hints | Source/host namespace aliases resolve prefix modules and terminal types in type scope, without borrowing a local value or an unrelated function/const/state fact. |
| Null ownership | Private declarations outside their module, missing targets/modules, duplicate aliases, local namespace/function shadows and invalid descendants own null despite registry decoys. Leaving a lexical shadow restores the original alias. Private owners remain hoverable inside their module. |
| Packages | Root and dependency packages both declare `api::grant` and `api::Row` with deliberately different signatures/docs. Direct dependency aliases select the dependency's metadata. A hidden transitive dependency is unavailable to the root but visible to its actual direct importer. Same-named local declarations cannot replace dependency/private owners. |

The package fixture additionally authors 31 definition oracles (23 exact physical
owners and eight nulls). Its 216 definition positions per layer, each repeated,
compare document identity and declaration range. This distinguishes declarations
whose existing SourceSymbol labels share a module-qualified spelling across
packages. Four schema location oracles bind type, trait, module and function spans
to an independently selected source file and raw byte bounds: 16 service checks
across newline/schema variants, including eight exact spans and eight absences.
Both protocol fixtures use real dispatch and immutable physical inputs.

The fixture exposed dependency `grant` hover displaying the root package's docs
and i64 signature. Qualified source hover now resolves the selected path through
the current package and dependency aliases instead of searching globally by
SourceSymbol text. Import declarations, aliases and uses share selected-prefix
classification, including enum type/variant prefixes and private ownership.
Registered module facts and known stdlib namespace prefixes provide module
metadata; type-hint prefixes expand aliases and accept only module/type/trait
metadata. Declaration SourceSymbol naming and runtime semantics are unchanged.

Oracle corrections follow primary contracts: dependency-root aliases without a
concrete root module have null module hover; a following actual `api` module has
its package identity. A missing registered terminal hint retains Any without
identity. Math function facts retain the primary i64/f64 unions. None of these
expectations comes from copying a provider response. Fixture IDs only normalize
marker transport spelling; they do not change source identifiers.

No other hover state or full UX10 acceptance is claimed. Windows and macOS use
independent fresh evidence on their registered profiles; evidence is never mixed.
