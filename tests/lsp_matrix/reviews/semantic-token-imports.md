# B06 S8 module and import token review

The shared `semantic-token-imports` fixture independently authors complete
463-token positive and 177-token negative streams in a four-file script
workspace. Literal text, kinds, complete modifiers and byte/UTF-16 coordinates
come from markers rather than a provider. The defining files remain closed in
protocol queries. Nested modules and marked static schema metadata are loaded
at both layers; schema source IDs come from setup and spans from the corpus.

| Partition | Exact expectations |
| --- | --- |
| Source imports | Functions, readonly consts, extern states, structs, enums and traits retain their defining-file ownership at import leaves and uses. Aliases retain the target kind/provenance and add declaration, without definition. Plain and aliased imports coexist. |
| Namespaces | Deep qualified paths and source file/directory module aliases resolve through the workspace graph. Known value/call path prefixes are namespaces without terminal provenance; const/state/function values never lend their kind to prefixes. A direct source module import has source ownership. |
| Cross-file expressions | Imported function values/calls, const/state reads/writes, aliased type hints and record constructors, unit/tuple/record variants, directly imported unit/tuple variant aliases and match payloads, field/method access, source traits/default methods and closure captures retain exact roles. |
| Registered imports | Source-backed schema functions/types/modules, metadata-only functions/traits and builtin function/module aliases retain host/schema or defaultLibrary provenance. Source spans do not turn schema ownership into source ownership. Exact public source declarations win schema collisions; exact private source ownership blocks fallback. Source enum ownership blocks registry functions at nonexistent variant paths. |
| Negative boundaries | Unknown/wrong-path/private import leaves and aliases are unresolved. Duplicate aliases cannot choose a target; a local declaration takes precedence over its conflicting import. Private/unknown constructor type hints gain no provenance, explicit labels remain lexical variable tokens, and field values retain their own bindings. Local parameters shadow namespace-looking paths: only the first segment inherits the local role, unknown suffixes gain no function/property provenance. Literal name occurrences remain strings and Any members remain conservative. |

The shared drivers compare complete LF/CRLF streams, apply actual deltas, check
unchanged IDs and fresh parity, and pin every line/token/empty-end range.
Changing the importer replaces its import fingerprint and alias scope; repair
restores the entire original stream and ID. Protocol uses encoded Unicode,
space and percent URIs, a negative unsaved overlay and close-to-disk restore.
Dependency edits/deletion are separate lifecycle obligations.

Production collects exact import/alias spans once per source. A shared scoped
target resolver prioritizes visible source ownership, blocks private source
and invalid source descendant fallback and consults immutable builtin names and exact registry identities.
Known value paths assign target attributes only to the terminal token. Local
paths preserve only the first binding's role. Explicit record labels start
without provenance and gain property ownership only from known field facts.
All changes remain read-only tooling projections.

Only the twelve S8 positive/negative full/delta/range service/protocol obligations
use this evidence. Other B06 cells require separate proof; macOS evidence stays
independent and B16 stays deferred.
