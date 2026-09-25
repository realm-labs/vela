# Reference and rename S1 top-level declarations

B04.54 reviews the sixteen S1 positive/negative service/protocol syntax cells
for references, document highlights, prepare rename and rename. The coordinate
matrices distinguish the declaration name from its modifiers and surrounding
syntax, then follow source identity across the sites that can name it.

| Declaration partition | Direct evidence |
|---|---|
| `const`, contextual `state`, `extern state` | The imported-value matrix checks public const read-only policy, editable extern state with hot-reload ABI risk, private const, and newly added private initialized state. The latter has exact declaration, compound-write and read sites; `pub`, `const`, `extern` and both `state` keyword positions are empty/null queries. Imports, aliases, qualified paths and a closed consumer remain distinct. |
| Functions and parameters | The baseline, import-boundary, method-parameter, required-signature and default-binding matrices distinguish source function names from parameter declarations, default-body bindings and named call labels. Private/missing imports, unknown labels and colliding signatures cannot borrow a known owner. |
| Structs, fields, enums, variants and traits | Imported-type and source-variant matrices follow declarations through type hints, constructors, patterns, aliases and impl headers. Field/variant matrices distinguish the member's declaration from its enclosing type; private, missing and mismatched owners remain separate. |
| Inherent and trait `impl` methods | Method and required-parameter matrices distinguish sibling and foreign-owner methods, trait defaults and explicit implementations, including source declaration spans. Source-backed and metadata-only schema facts retain their own ownership and rename policies. |

Each mapped coordinate driver checks complete reference sets with and without
declarations, exact same-document highlight ranges/kinds, prepare ranges and
placeholders, and full edit plans. It applies every writable group, compares
whole files, parses, re-queries and restores under Unicode LF/CRLF. Protocol
uses actual LSP requests, UTF-16, encoded paths, `didChange` and document
versions. Public or metadata-only declarations may have references while
remaining read-only for rename. This review covers editor symbol behavior; it
does not claim runtime state mutation, compiler ABI acceptance or native editor
interaction. Other B04 syntax and interaction gates remain open. B00-B03
accepted snapshots stay fixed, macOS needs independent current evidence when
used, and B16 remains deferred.
