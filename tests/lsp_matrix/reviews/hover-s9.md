# Hover S9 error recovery

Scope: `hover/syntax/S9/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::recovery_matrix_tests::hover_recovery_matrix_preserves_known_facts_and_null_boundaries`
and `tests::hover::recovery::hover_recovery_matrix_preserves_known_facts_and_null_boundaries`.
Both run independently authored static and state fixtures without script execution.

`hover-s9` contains 56 physical inputs and 377 queries: 336 complete hovers and
41 explicit nulls with schema. Start/interior positions, LF/CRLF and schema
present/physically absent produce 2,848 positions per layer, each repeated.
Without schema, two registered queries become null, 42 parameter/use facts retain
unknown, and one type hint retains Any without docs or identity. Markers supply
independent byte/UTF-16 coordinates and exact local declaration identities.
Same-line CJK/non-BMP prefixes precede selected tokens. Every query explicitly
asserts whether its current file has parser/lexer diagnostics.

| Static partition | Independent assertions |
|---|---|
| Incomplete calls | Empty, value, named-value, nested, multiline, qualified, factory-returned, source method, schema and builtin callees preserve complete metadata. Named source parameters retain their declaration identity. Unknown/partial callees, invalid labels and shadowing noncallables cannot borrow callable metadata. A known shadowing local retains its own hover even when it is called incorrectly. |
| Members and constructors | Known source/schema fields and source/builtin methods retain types/docs. Missing/partial members, Any/unknown receivers and builtin-looking names are null despite same-spelled registered types. Open record fields retain exact source ownership; missing field labels remain null. |
| Patterns | Open match arms preserve source type and unit/tuple variant identities. A tuple payload retains its exact local declaration and i64 fact before an absent arm body. Unknown source variants reject an exact same-spelled registry variant. Arrows remain null. |
| Declarations and hints | Unnamed struct/enum/trait declarations and an unclosed function parameter list before/after a healthy function cannot consume that neighbor's parameters and calls. Free function, impl, trait and enum headers preserve actual parameter/field and builtin hint ownership; impl/trait self retains its owner. Missing hint arguments retain only the primary known container shape. Source and schema leaves retain exact facts; unavailable schema degrades instead of guessing. |
| Lexical boundaries | Empty/trivia files, unfinished comments/strings, their word contents and exact EOF remain null. Fake call delimiters and same-spelled names inside comments/strings do not become code. A malformed unrelated file cannot contaminate healthy metadata. |
| Diagnostic candidates | `Array.frist` publishes a real `analysis::unknown_method` candidate `first`; hover at `frist` remains explicit null. Service checks structured candidates, protocol checks the actual DidOpen publication. A repair suggestion never becomes a resolved member. |

`hover-s9-recovery` authors 14 queries through 15 phases: 189 complete hovers and
21 nulls across schema-present phase expectations. Across newline/schema modes,
there are 1,536 positions per layer on the incremental instance and the same
positions on an independently rebuilt instance, all repeated. Optional definition
oracles contribute 336 positions on each instance, also repeated, comparing exact
document/range or null. Whole hover results match independently authored phase
expectations before incremental/fresh comparison.

The caller opens, changes to an incomplete known call, changes to an unresolved
callee, repairs and closes. The defining file opens and changes parameter/return
types, docs and field type; an incomplete body retains its current known signature.
Function, type and field removals suppress old metadata and locations. Repairs
and close restore baseline facts and exact targets. A neighbor opens, becomes
malformed, repairs and closes without contaminating the caller. Selected defining
and neighbor files additionally assert diagnostic presence. Every phase preserves
the original disk bytes; protocol materializes separate encoded fresh roots and
initializes real servers with the current overlays. Final results equal baseline
and all overlays are closed. Physical schema absence is independently exercised.

The fixture reproduced `fn broken( { }` swallowing a subsequent healthy function.
Function-item recovery now uses the same body brace already selected by the
function parser when its parameter list has no closing delimiter; the recovered
body ends the item rather than letting the unmatched paren hide its boundary.
Complete parameter lists use the original boundary path. A focused syntax test
checks both functions, the neighbor's exact parameter/type, lossless text and the
retained missing-paren diagnostic in LF/CRLF.

The fixture also reproduced missing parameter hover in bodyless free-function
and impl headers. A focused module queries only an actual direct signature
parameter name token, its concrete source owner and current module. It lowers
the retained CST hint through the existing schema-aware converter; impl self uses
the actual target path. Exact physical local identity does not require inventing
an executable LocalBinding. Nested lambda/default parameters cannot borrow this
header's owner. Required trait and enum metadata paths remain independently tested.

Oracle corrections follow primary contracts: builtin method labels and identities
include the instantiated receiver (`Array(i64).first`); malformed container hints
retain known outer shapes and unknown leaves; enum fields display their retained
raw declaration hint. An exact registered bare type name is real metadata even
in an invalid call, so the unresolved `gr` case deliberately leaves it unregistered.
Member/variant decoys remain registered. No expectation was copied from a provider
response, and no grammar, runtime or host mutation capability is added.

These fixtures certify the four S9 syntax obligations. Other hover states,
S10/S11 partitions and full UX10 remain separate work. Windows/macOS gates require
independent fresh evidence on one registered profile and never mix bundles.
