# Completion: imports and recovery review

Scope: `completion/syntax/{S8,S9}/{positive,negative}/{service,protocol}`.
The catalog binds the exact tests for each partition below. Only a passing
executed audit certifies those mappings. The new shared fixture is
`completion-import-sites`: 26 queries, twenty candidate queries and six empty
queries, repeated with LF/CRLF and Chinese/non-BMP prefixes. Seven queries
explicitly assert parse errors before querying unfinished syntax.

| S8 partition | Shared fixture(s) | Assertions |
|---|---|---|
| Qualified and imported declarations | `completion-import-sites`, `completion-import-aliases`, `completion-type-ownership` | Source function, typed constant, state and extern-state import paths; source/schema/stdlib aliases and namespaces; types, enums and traits. Complete candidate sets, identity, detail, insertion format, resolve and applied target checks distinguish the owners. |
| Import insertion | `completion-import-sites` | Source/schema/stdlib functions insert a plain path in `use`, preserving comments, aliases, multiline layout, the whole word at a mid-token cursor and an unfinished import. Applying it parses. A schema function alias in an expression retains a call snippet and its canonical signature. |
| Package and visibility | `completion-package-type-ownership`, `completion-package-callables`, `completion-package-members` | Current/dependency qualification, imported aliases, private/public visibility and exact applied declarations/signatures. Missing dependencies, unreachable transitive packages, shadowed imports and wrong receiver owners cannot supply unrelated candidates. |
| Variant and member aliases | `completion-enum-aliases`, `completion-package-members` | Imported type/namespace aliases retain unit/tuple/record variant and inherent/trait/default member ownership; private, duplicate, missing and non-enum paths have explicit empty sets. |
| Schema source spans | `completion-import-sites` | Function/type aliases used in expressions/type hints keep schema identity and owned lazy docs; applying completion navigates to independent marked source spans. A metadata-only function has no source target. Raw schema imports retain the existing source-import navigation null policy; this fixture does not claim schema navigation inside `use`. |
| Unresolved imports | `completion-import-sites`, `completion-import-aliases`, `completion-type-ownership` | Missing leaf/module, private target, duplicate alias and unknown namespace cannot borrow another declaration, callable or schema docs. Complete sets and actual resolve outcomes remain explicit. |

The qualified cursor keeps its module base while recording the `Import` role
from `SyntaxUsePath`, including comments and multiline syntax. Cursor tests
also prove a subsequent expression path stays an expression even when a comment
contains import-like text. Callable insertion consumes this role to suppress
call parentheses. Existing expression calls and existing argument lists retain
their prior insertion rules.

| S9 partition | Shared fixture(s) | Assertions |
|---|---|---|
| Incomplete type/member/record/import | `completion-import-sites` | Known unfinished type/member/record sites have exact owned candidates and independent byte/UTF-16 edits; application with explicitly authored closing syntax parses and preserves target facts. Unknown type/constructor and dynamic member receivers have exact empty sets. A record ending at EOF, including trailing whitespace and an empty prefix, retains its fields. |
| Incomplete call | `completion-call-expressions` | Missing call close retains the complete parameter/expression choices; the returned edit plus explicit recovery suffix parses. Nested calls, unknown target, occupied/duplicate argument labels and dynamic receivers cannot acquire guessed callable facts. |
| Incomplete member and constructor boundaries | `completion-members` | Empty-prefix dot and record queries preserve owner-specific sets; missing/dynamic/erased receivers and unknown constructors remain conservative. Used fields, different owners and source/schema variants of these contexts have independent expected results. |
| Malformed declarations and partial facts | `completion-source-recovery` | Six states cover malformed neighboring declaration, repair, redamage and close restoration. Candidate, resolve, signature and definition oracles compare cached analysis with fresh analysis; actual syntax diagnostics appear and clear at both service and protocol boundaries. |
| Replaced schema facts | `completion-schema-callable-lifecycle` | Current/replaced/removed/restored schema functions and methods keep only current parameters, return ownership and lazy docs, including retained resolve payloads. |

EOF recovery only includes the unfinished source boundary. It may step over
trailing whitespace, but a record with a closing brace is rejected at or beyond
that brace. A direct recovery test checks closed records with and without an
outer closing brace and trailing whitespace, preventing stale constructor reuse.
Cursor assertions also keep EOF positions after a field colon or value in the
value context, including trailing whitespace, so fields do not reappear there.

Schema source IDs are obtained from the loaded workspace; byte spans come from
the independent fixture markers in `schema-markers.json`. The same marked
sources generate LF and CRLF metadata, with protocol assertions checking the
exact UTF-16 target ranges. This supplies source-backed metadata without fixed
machine paths or guessed source IDs.

This review does not certify S1/S2/S6/S7/S11/S13/S14, UX04 interactions, generated
combinations or scale gates. B03 stays open.
