# Hover S7 literals and operators

Scope: `hover/syntax/S7/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::literal_matrix_tests::hover_literal_matrix_preserves_expression_facts_and_lexical_boundaries`
and `tests::hover::literals::hover_literal_matrix_preserves_expression_facts_and_lexical_boundaries`.

The independent `hover-s7` oracle authors 439 queries: 309 complete hovers and
130 explicit null results with schema present. Positive tokens run at start and
interior positions; negative tokens run at their start. LF/CRLF and present/
physically absent schema variants yield 2,978 positions per layer, each repeated.
Seven registered queries become null without schema; eight local/parameter
queries retain unknown instead of unavailable host facts. Markers independently
supply exact ranges and local declaration locations. Service compares whole
metadata and source/schema/local identity; protocol compares complete Markdown
and UTF-16 ranges or explicit JSON null.

| Partition | Independent assertions |
|---|---|
| Primitive facts | Signed/unsigned integer suffixes, f32/f64, default and context-typed numbers, hex/binary/octal, separators and scientific notation retain exact inferred facts on their locals. ASCII/CJK and escaped chars, ordinary/escaped/multiline strings, byte escapes, bool and unit retain existing types. |
| Literal boundaries | Literal bodies, suffixes, byte prefixes, escapes, quoted builtin/qualified/member spellings and documentation payloads return null despite same-spelled schema decoys. Closing quotes and literal end characters remain null. |
| Interpolation | Ordinary, nested and multiline interpolation text is null; embedded variables retain exact identity and types. Operators remain null. Registered calls and indexed host fields retain metadata only with schema, while the enclosing string fact stays String. |
| Containers and constructors | Homogeneous/mixed/nested/empty/context-typed arrays, tuples, maps with varied key spellings and sets retain independently derived facts. Source and registered record constructors distinguish field labels from shorthand caller bindings. Bare and qualified path-shaped map keys are static spellings and return null; values and expressions inside interpolated keys remain code. |
| Indexing and fields | Array/Map/String/Bytes, nested and mixed collections, source record arrays and returned rows, schema index capabilities, Any/unknown/empty values retain exact result facts. Tuple indexing keeps the existing conservative union; numeric tuple fields select a fact without inventing a symbol hover. Missing source fields reject schema decoys. |
| Operators | Arithmetic, comparison, logical, unary, float, string, range and compound assignment cases preserve operand and result facts. Operator characters immediately following an identifier return null without borrowing that identifier. |
| Other boundaries | Delimiters, dots, commas, semicolons, pipes, call/namespace separators, type-hint angles, return and match arrows, and Option/Result question suffixes are null. Typed lambda/call and try-payload locals retain exact facts. A final identifier remains hoverable, while the exact end-of-file position is null. |

The fixture first reproduced a byte literal prefix borrowing a registered type
named `b`. Hover now requires the actual CST Ident/SelfKw token to contain the
cursor and match its identifier range. The half-open token range prevents a
preceding identifier from owning punctuation, trivia or EOF. A focused map-entry
check follows HIR's existing static path-key contract. Specialized provider-ID
literal hover still runs first; real interpolation expressions retain their CST
symbol ownership. Completion/signature prefix handling and language/runtime
semantics are unchanged.

The schema cache regression previously queried whitespace after `return` and
depended on incidental analysis warming. It now queries the actual `grant`
callee and asserts its canonical source label before and after schema reload.
The original exact analysis-build counts remain asserted.

Oracle corrections follow primary contracts: bare `{}` is a unit-valued block,
tuple indexing yields a conservative union, and source module identity includes
its package. Fixture transport reserves `[[`/`]]`, so nested array brackets and
markers use source whitespace to remain unambiguous. This whitespace does not
change type facts; the shared fixture parser validates every marker before the
fixture is written. No provider response is copied into expectations.

Protocol uses actual initialize/open/request dispatch, encoded isolated roots,
immutable physical inputs, physically absent schema and repeated requests.
Same-line CJK precedes selected queries; accepted coordinate evidence owns
same-line non-BMP proof. No other hover syntax/state group or full UX10 acceptance
is claimed. Windows/macOS acceptance needs independent fresh evidence from one
registered profile.

An initial editor-launcher audit had seven protocol request timeouts despite
the preceding successful workspace run. All seven tests passed in individual
reruns with their existing request budgets and assertions. That failed audit is
retained locally and excluded from acceptance. Fresh gates run with
`RUST_TEST_THREADS=8` to bound local test concurrency; no request/test timeout or
expected result is relaxed.
