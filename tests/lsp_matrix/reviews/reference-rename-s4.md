# Reference and rename S4 members and constructors

B04.51 reviews the sixteen S4 positive and negative service/protocol syntax
cells for references, document highlights, prepare rename and rename. The
coordinate drivers compare complete owner-specific reference sets with and
without declarations, same-document highlight ranges and kinds, exact prepare
targets and complete workspace edits. Editable groups are renamed, applied to
full source files, parsed, queried again and restored under LF/CRLF. Protocol
checks UTF-16 positions, encoded Unicode URIs and open/closed versions.

| Syntax partition | Independent assertions |
|---|---|
| Source fields and record constructors | Two same-named structs retain distinct fields through imported aliases, qualified/local constructors, explicit and shorthand labels, dot reads and writes, compound assignments and match patterns. Local shorthand bindings retain their own identity when a field is renamed. Unknown labels/owners and collisions do not adopt a neighboring field. |
| Source enum record and tuple fields | Record variants distinguish sibling and same-named foreign enum owners through constructors and patterns. Tuple-variant positional and named forms keep parameter ownership separate from caller and pattern locals. Unit/tuple/record wrong-form labels, invalid owners and local shadows produce no field target. |
| Source and schema methods | Inherent and trait/default methods, source/schema receiver ownership, call sites and returned receivers have independent exact sites. Same-named methods on distinct owners and metadata-only methods remain separate; wrong/dynamic receivers do not acquire a method. |
| Source and schema variants | Unit, tuple and record variants in value and pattern positions retain enum and variant identity across direct, aliased and qualified imports. Private/source-backed variants follow their editable policy; metadata-only, missing, ambiguous and wrong-owner targets do not produce an edit. |
| Schema fields and shorthand | Source-backed and metadata-only schema fields are separated from a same-named source field. Explicit/shorthand constructors, explicit/shorthand patterns, dot reads and compound writes have exact kinds and edits. The fixture also queries `Any.value` and `i64.value` beside real `value` fields and requires empty references/highlights and null prepare/rename results. |

The service layer also checks canonical local/source/schema symbol identities;
the protocol layer checks both legacy and versioned workspace edit forms and
reissues requests after real `didChange` notifications. This review closes
these S4 cells only. Other B04 syntax groups and local editor interaction
gates stay open. B00-B03 accepted snapshots stay fixed, macOS needs independent
current evidence when used, and B16 remains deferred.
