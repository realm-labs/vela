# Signature help: S8 and source lifecycle review

Scope: four `signature-help/syntax/S8/{positive,negative}/{service,protocol}`
obligations and protocol states `dirty`, `close_restore`, `dependency_change`,
`dependency_delete`. Exact matrix identities are
`signature::import_tests::signature_package_matrix_preserves_import_ownership_and_lifecycle_facts`
and `tests::signature::imports::signature_package_matrix_preserves_import_ownership_and_lifecycle_facts`.

`signature-s8` independently authors 27 complete signature templates and 34
positions through 16 sequential phases, for 544 positions per layer and line
ending. Each driver runs LF and CRLF, repeats complete signature/definition
results and compares every phase with a fresh database/server, in addition to
the authored expectations. Expected values do not come from provider output.
Protocol uses actual initialization, open/change/close and watched-file
notifications, UTF-16 positions and isolated encoded Chinese/space/percent paths.

| Partition | Independent proof |
|---|---|
| Qualified paths and imports | Root and direct dependency `api::choose` have different parameter names/types and return facts. Direct aliases, namespace aliases, `crate::` and dependency-qualified calls preserve their complete defining-file signatures. An unimported same-short-name function is a decoy. Supported individual import syntax is used; no grouped-import syntax is introduced. |
| Imported members | Root/dependency record aliases, direct trait receivers, trait defaults on records and factory-returned receivers select exact inherent or trait owners. Tuple variant aliases retain defining-file parameter types and positional-only policy. Both packages deliberately share module/type/member spellings, so service owner symbols are supplemented by exact target document and both byte endpoints. |
| Static schema and builtin owners | Imported and namespace-aliased async schema functions retain metadata's required/defaulted parameters and return facts despite a conflicting source declaration signature. A schema function with a source span has an exact closed-file target; an ordinary schema function colliding with `api::choose` cannot replace source ownership. Aliased builtin math calls retain exact numeric-union signatures and no invented definition. |
| Visibility and unresolved calls | Private root/dependency functions, unavailable members/modules, unreachable transitive and nested-transitive package paths, broken imports, duplicate aliases, unimported short names and `Any` receivers have explicit null signatures and no callable owners/definitions. A local noncallable shadows an imported function; a noncallable imported const retains its real definition but has no signature. A current-module function outranks an imported same-name function. |
| Unsaved definitions | Open root/dependency definitions, change names/types/defaulted signatures and root asyncness, then close to disk. Every query has full expected results during dirty states and after restoration. Physical disk bytes stay unchanged for open/change/close. |
| Unsaved importer | A Unicode multiline prefix shift and an import retarget only the affected alias; other root/dependency calls retain their owners. Closing restores the original disk import and query positions. |
| Dependency lifecycle | Modify a closed dependency; delete, recreate and rename it; explicitly retarget one importer alias to the renamed module; close and restore. All dependent signatures disappear or change exactly as authored. Unrelated source/schema/builtin results remain exact, including source-backed schema targets while internal source numbering changes. Created/changed/deleted notifications have their actual event kinds. |
| Fresh equivalence and restoration | Service rebuilds a fresh database/source graph from current disk plus overlays at every phase. Protocol independently materializes a fresh root, initializes a fresh server and opens the current overlays. Full signatures and individually checked exact targets agree, without relying on equivalence as the sole oracle. Final results equal the initial baseline, all original disk bytes return and overlays are closed. |

The matrix exposed schema source spans following a reused numeric SourceId after
dependency deletion, causing navigation into an unrelated decoy file. The shared
schema location cache now binds document owners at load time and projects spans
into the current source table. The incremental matrix keeps the same loaded
schema throughout; fresh setup binds the authored schema into its own new table.
`incremental::schema_sources::tests::schema_spans_keep_document_owners_across_source_table_replacement`
also checks all eight span categories through renumbering, identical artifact
rereads, deleted-owner suppression, recreation and explicit replacement. Only
SourceId changes; authored byte bounds and existing text-boundary checks remain.

This review does not certify signature recovery, stale/missing schema state,
S10/S11 or installed signature UX. Those remain separate B07 work. Acceptance
requires fresh current-source execution and independent evidence on its selected
registered platform; macOS evidence cannot be supplied by this Windows run.
