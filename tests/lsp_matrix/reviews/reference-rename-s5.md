# Reference and rename S5 calls and arguments

B04.52 reviews the sixteen S5 positive and negative service/protocol syntax
cells for references, document highlights, prepare rename and rename. The
shared coordinate drivers repeat complete reference sets with and without
declarations, exact same-document highlights and kinds, prepare ranges and
placeholders, and complete applied workspace edits. They parse and re-query
each renamed file, restore it, and repeat under LF/CRLF; protocol uses real
`didChange`, UTF-16, encoded URIs and open/closed client versions.

| Call partition | Direct evidence |
|---|---|
| Source functions | The baseline and import-boundary matrices distinguish declaration, import, bare/qualified and aliased calls across files. Private/missing imports and stdlib-only terminals cannot inherit a source callable's sites or edit plan. |
| Named and defaulted arguments | The named, required and default-binding matrices separate callee parameter declarations, call labels, caller values, closure/default-body reads and same-spelled parameters on other callables. Missing argument values retain owned labels where the callee is known; unknown labels/callees and capture collisions reject edits. |
| Source methods | Inherent, sibling, foreign-owner, trait-default and explicit-implementation methods and their parameters remain distinct across named labels, closures, defaults and closed consumers. The method-parameter fixture also queries `Any.add(amount = 1)` at both `add` and `amount`; neither borrows the real method or parameter owner. |
| Schema functions and methods | Source-backed schema callable spans and their named parameters retain schema or canonical source/local ownership; metadata-only callables remain read-only. Same-named source/schema/foreign-owner methods, unknown labels and dynamic receivers stay separate. |
| Tuple-variant calls | Positional and named tuple-variant argument labels retain variant parameter ownership rather than caller or pattern locals. Invalid constructor forms and missing arguments do not invent an owner. |
| Standard library and unresolved calls | Stdlib functions/methods, builtin facts, missing callees and dynamic `Any` downstream calls have no source rename target. The import-boundary and dynamic-return matrices exercise them beside known source/schema callables. |

The S5 dimension also mentions active-parameter tracking. That output belongs to
signature help, so these B04 cells assert the ownership of callable and label
tokens at their actual cursor positions; they do not claim a signature-help
active-parameter result. This review closes only the four methods' S5 cells.
Other B04 syntax groups and native editor interaction gates remain open.
B00-B03 accepted snapshots stay fixed; macOS needs independent current evidence
when used, and B16 remains deferred.
