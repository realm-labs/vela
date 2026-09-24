# Imported source type and trait ownership

The later [B04.35 S8 review](rename-s8-imports.md) closes the schema type
import gap recorded below.

B04.34 advances the S8 positive and negative service/protocol paths for
`references`, `highlight`, `prepare-rename` and `rename`. The broad S8 cells
remain unreviewed: schema type imports still lack a complete reference and
rename matrix. Each completed row below has an independent marked fixture and
the same exact oracle at both Rust layers.

| S8 partition | Executed fixture and asserted boundary |
|---|---|
| Source functions | `reference-rename-import-boundaries`: 17 queries cover direct and qualified calls, explicit function and module aliases, closed consumers, private cross-module imports, stdlib-only imports and missing paths. |
| Source values | `reference-rename-imported-values`: 22 queries cover public const and extern state through every import form, compound writes, public const rename rejection, private const internal rename, and exact state ABI annotations. |
| Source types and traits | `reference-rename-imported-types`: 33 queries cover struct and enum type hints, record construction, direct and module aliases, qualified closed-file hints, trait import aliases and cross-module implementations, private struct/trait internal renames, and inaccessible/missing imports. |
| Source variants | `reference-rename-source-variant-imports` and `reference-rename-private-variant-imports` cover public/private enum variants, direct/type/module aliases, constructors and patterns while separating schema owners. |
| Schema functions and variants | `reference-rename-schema-imports` and `reference-rename-schema-variant-imports` mark source-backed schema spans, explicit aliases, metadata-only null rename targets, source/schema ownership collisions and unresolved paths. |
| Ambiguous imports | `reference-rename-variant-ambiguity-removal` and `reference-rename-schema-variant-ambiguity` require empty results for ambiguous bare names and reject edits that would silently change their owner. |
| Pending schema type imports | Source-backed and metadata-only schema types need direct/alias/module import reference, highlight and rename oracles before the broad S8 cells can be certified. |

Both drivers compare complete reference sets with and without declarations,
same-document highlight kinds, prepare eligibility and ranges, complete rename
edits and explicit null/rejection results. Writable groups apply edits across
files, parse the result, query again and restore the original source. The
protocol layer checks UTF-16, LF/CRLF, encoded Unicode paths, open/closed
versions and edit annotations. Public source types and constants retain their
read-only policy; private declarations and source-backed schema targets retain
their distinct rename policies.

The new type fixture exposed five defects: record construction yielded a whole
expression reference range; module aliases in type hints lost source ownership;
trait references omitted imports and aliased impl headers; navigation from an
aliased impl header was null; and private trait rename omitted its impl header.
The service now resolves visible import paths consistently and uses the trait
identity for references, navigation and rename. Schema type imports, other
syntax groups and UX05/UX06 remain open.
