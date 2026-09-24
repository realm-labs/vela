# S8 references, highlights and rename ownership

B04.35 completes the S8 modules-and-imports review for `references`,
`highlight`, `prepare-rename` and `rename`, at service and protocol layers for
positive and negative cells. The syntax contract covers qualified paths,
imported declarations and aliases, public/private visibility, source-backed
schema spans and unresolved imports. Every row below has marked source and
frozen complete sets in independent LF/CRLF fixtures.

| Partition | Exact evidence |
|---|---|
| Source functions and values | `reference-rename-import-boundaries` and `reference-rename-imported-values` cover direct, explicit and module aliases, qualified and closed-file calls/reads/writes, public const read-only policy, private const internal rename, external state edits and ABI annotations. |
| Source types, traits and variants | `reference-rename-imported-types`, `reference-rename-source-variant-imports` and `reference-rename-private-variant-imports` cover struct/enum/trait annotations, record construction, enum constructors/patterns, direct/type/module aliases, trait implementations, public read-only types and private internal rename. |
| Schema functions and variants | `reference-rename-schema-imports` and `reference-rename-schema-variant-imports` separate source-backed declarations, metadata-only targets, explicit aliases, source/schema collisions and unresolved paths. |
| Schema types and traits | `reference-rename-schema-type-imports` runs 32 queries across source-backed host struct, enum and trait facts plus metadata-only host type. It checks import alias and terminal cursors, direct and module aliases, qualified closed-file hints, trait implementation headers, missing imports and exact source-backed versus null navigation. |
| Ambiguity and visibility | `reference-rename-variant-ambiguity-removal`, `reference-rename-schema-variant-ambiguity`, plus the source import fixtures require empty results for ambiguous/missing/private paths and reject a rename that would change the resolved owner. |

Both drivers assert complete reference sets with and without declarations,
same-document highlight kinds, prepare eligibility and exact range/placeholder,
all workspace edits and explicit null/rejection outcomes. Editable groups apply
the edit across files, parse, reload schema facts, requery and restore. The
protocol driver checks UTF-16 after Chinese and emoji, LF/CRLF, encoded paths,
closed-file versions and edit annotations. Metadata-only schema types retain
references but have no fabricated definition or rename target.

The new type fixture exposed absent schema type references, alias and module
ownership, missing import-terminal queries, incomplete trait impl references,
null import-alias navigation, and namespaced type collision checks. One shared
site resolver now supplies reference, navigation and rename identity and edit
ranges. The existing completion import fixture now expects a source-backed
schema type terminal to navigate to its registered source span, matching its
alias case. Other syntax groups, state partitions and UX05/UX06 remain open.
