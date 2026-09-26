# B06 S10 semantic-token ownership review

The shared `semantic-token-ownership` fixture independently authors a complete
322-token positive stream and three negative streams containing 1013 tokens.
Markers supply exact text, byte/UTF-16 coordinates, kinds and complete modifier
sets. Closed source definitions and static schema facts provide different owners;
no script or live host state is queried. Schema source spans point to independently
marked, real declarations with different names, so a nonzero source ID alone
cannot masquerade as source ownership.

| Partition | Exact expectations |
| --- | --- |
| Locals and parameters | Declarations retain declaration/source modifiers and uses retain source ownership. Const uses carry readonly; source state writes keep the current legend's source classification. Receiver ownership does not transfer to unknown members. |
| Source declarations | Same-file and imported functions, structs, enums and traits win over contradictory schema entries. A source `print` wins over the default library spelling. Source module aliases carry source provenance; structural path prefixes remain namespaces. |
| Source members and variants | Same-file/imported fields, inherent methods, trait methods, constructors and variants carry source ownership. Missing fields/methods cannot borrow same-owner schema entries. A nonexistent source enum variant stays unresolved even when schema metadata claims it exists. Private source callees cannot be exposed by contradictory metadata. |
| Schema and host facts | Source-backed host types, fields, methods, traits, variants and module aliases retain host/schema provenance. Metadata-only functions also retain host/schema. A schema record field carries schema without host; its schema method callable carries the established host/schema callable classification. Real source spans do not add source modifiers to registry facts. |
| Standard library and builtins | The math module alias/function, Array and String methods, primitive/container hints and Option variants carry defaultLibrary. Same-name schema functions cannot override these facts. |
| Dynamic and unresolved | Any receiver members stay lexical variables with no provenance. Any parameters shadow source/stdlib/schema module aliases and source callables; path suffixes lose guessed ownership while unrelated source/schema/builtin facts remain intact. Unknown free/qualified value references and private callees retain unresolved modifiers. Unknown type hints, record owners/labels and members retain their conservative lexical roles without invented ownership. A string containing symbol spellings remains opaque. |
| Removal and repair | Removing Local and its impl changes its surviving type/constructor/field/method uses to the real registry owner. Restoring the source restores every source modifier, without stale schema or source tokens. |

Both layers alternate the positive document with each negative document and repair
it, under LF and CRLF: fourteen queried states in each layer, with the final
protocol repair performed by closing the overlay and restoring disk source.
Every state compares complete independent tokens against both incremental and
fresh workspaces, applies actual deltas, checks repeated IDs/unchanged deltas,
and verifies every line/token range and token-end/document-start empty range.
Protocol documents use encoded Unicode, space and percent URIs and monotonically
increasing overlay versions. Closing restores the complete disk stream and ID.

This child adds tests and evidence; it changes no production semantic policy.
Only the twelve S10 full/delta/range positive/negative service/protocol obligations
use this proof. S11 cancellation/dependency invalidation and other B06 cells remain
separate. macOS requires independent fresh evidence; B16 remains deferred.
