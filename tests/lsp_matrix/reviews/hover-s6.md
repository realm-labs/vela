# Hover S6 patterns and control flow

Scope: `hover/syntax/S6/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::pattern_matrix_tests::hover_pattern_matrix_preserves_bindings_variants_and_control_scopes`
and `tests::hover::patterns::hover_pattern_matrix_preserves_bindings_variants_and_control_scopes`.

The independent `hover-s6` oracle authors 212 queries: 177 complete hovers and
35 explicit null results with schema present. Positive tokens run at start and
interior positions; negative tokens run at their start. LF/CRLF and present/
physically absent schema variants yield 1,532 positions per layer, each repeated.
Twelve registered queries become null without schema; nineteen host/schema
parameter or pattern/loop local queries retain unknown instead of unavailable
registered facts. Markers independently supply exact ranges and local declaration
locations. Service compares whole metadata and source/schema/local identity;
protocol compares complete Markdown and UTF-16 ranges or explicit JSON null.

| Partition | Independent assertions |
|---|---|
| Source enum patterns | Qualified, type-aliased and namespace-aliased unit/tuple/record variants retain canonical owner, shape and docs. Namespace prefixes retain module identity; type prefixes retain enum identity. Foreign same-named enum variants and fields keep their distinct types and docs. |
| Payload bindings | Tuple, explicit record and shorthand bindings retain local declaration identity and source/registered payload facts. Explicit labels retain field metadata; shorthand tokens retain lexical binding identity. Source payload records retain their own member docs and types. |
| Nested patterns and scopes | Nested enum, nested tuple, whole-value binding and let destructuring preserve facts. Guards and arm bodies use the same exact binding; nested block shadows and subsequent restoration retain distinct identities. Sibling arms and queries after a match cannot see another arm's locals. |
| Expressions and branches | Match-expression results retain i64, if-expression results retain String, and each branch's same-named locals retain distinct declarations. Branch and expression-pattern locals are unavailable outside their scopes. |
| Registered patterns | Unit, tuple, explicit record and shorthand schema variants retain exact metadata, field facts and host payload types. Physically absent schema removes metadata hovers and leaves affected locals unknown. Source enum payloads referring to host types follow the same availability boundary. |
| Iteration | Array items, index/value pairs, tuple destructuring, ranges, strings, bytes, Set, Iterator, MapEntry, source-record arrays and foreign-record iterators retain independently derived item facts. Registered arrays/iterators obey schema availability. Nested loops retain distinct shadow identities and restore the outer binding. |
| Negative boundaries | Unknown/private variants and labels, invalid unit/tuple record fields and unrelated qualified paths are null despite registry decoys; their payload locals remain unknown. Source loop receivers reject absent fields from same-spelled registry metadata. Any/unknown iteration retains Any/unknown and has no guessed member. Control-flow keywords, wildcard and literal-pattern tokens are null. |

The fixture first reproduced wrong schema variant ownership for source patterns,
schema fallback for a missing source loop field, and registry payload facts leaking
through a private source enum. Qualified pattern paths now enter the existing
canonical path resolver and own unresolved results. Missing members with an exact
canonical source type are rejected even when iteration omitted source-origin
metadata; host facts and short names do not acquire source identity. Pattern
constructor analysis checks source visibility before registry fallback. An
independent analysis regression covers direct, type-aliased and namespace-aliased
private patterns beside a public source pattern, with and without registry decoys.

Oracle spelling follows independent existing contracts: Set iteration uses
`set::from_array`, Map iteration yields the logical `MapEntry` record, module
identity includes its package, and a record constructor used as a match scrutinee
is parenthesized to distinguish its braces from the arm list. Ordinary struct
pattern labels expose static field metadata, while their local payload inference
remains unknown under the existing pattern analysis contract. No provider response
is copied into expectations. This batch does not add pattern runtime semantics or
modify the language grammar.

Protocol uses actual initialize/open/request dispatch, encoded isolated roots,
immutable physical inputs, physically absent schema and repeated requests.
Query prefixes are ASCII; accepted coordinate evidence owns same-line non-BMP
proof. No other hover syntax/state group or full UX10 acceptance is claimed.
Windows/macOS acceptance needs independent fresh evidence from one registered
profile.
