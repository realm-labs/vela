# Hover S5 calls and arguments

Scope: `hover/syntax/S5/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::call_matrix_tests::hover_call_matrix_preserves_parameter_labels_and_lexical_arguments`
and `tests::hover::calls::hover_call_matrix_preserves_parameter_labels_and_lexical_arguments`.

The independent `hover-s5` oracle authors 165 queries: 130 complete hovers and
35 explicit null results with schema present. Positive tokens run at start and
interior positions; negative tokens run at their start. LF/CRLF and present/
physically absent schema variants yield 1,154 positions per layer, each repeated.
Thirteen registered callable/parameter queries become null without schema;
the host-typed parameter retains its existing unknown fact. Source and builtin
queries remain available. Markers independently supply exact ranges and source
local declaration locations. Service compares whole metadata and identity;
protocol compares complete Markdown and UTF-16 ranges or explicit JSON null.

| Partition | Independent assertions |
|---|---|
| Source functions | Qualified, imported and namespace-aliased calls retain defining-file signatures and docs. A foreign same-named function retains distinct parameter types, return and docs. Positional, defaulted and reordered named arguments retain their exact callee and parameter owner. |
| Source methods | Inherent, returned/chained, foreign same-named, required trait, default trait and override methods retain signatures/docs and existing identities. Named labels retain their own parameter declaration, including required signatures without a body and default/override separation. |
| Labels and values | Callee parameter labels and same-spelled caller values have different exact declaration identities. Untyped and explicit Any source parameters retain unknown/Any rather than types guessed from actual arguments. Default expressions are not executed or substituted into hover display. |
| Async calls | Source functions and methods retain async signatures; named labels and caller values retain their separate owners. Await-result locals retain existing inferred facts. Registered async functions/methods retain asyncness, complete available metadata and named parameter types. |
| Static schema | Functions, imported aliases and methods retain exact docs, unknown-effect metadata and method permissions. Named/defaulted metadata parameters retain types with no fabricated source symbol. Physically absent schema removes those callable/label hovers. |
| Stdlib | Qualified and aliased Option functions, no-argument time functions and typed Array get/push/map methods retain builtin identity and exact static contracts. Named labels retain registered types; callbacks retain inferred parameters and exact lexical identities. Method metadata preserves its existing erased callback return, rather than inventing a call-specific signature. |
| Local callables | Typed lambdas and copied source functions retain local identity and structural Function facts on declarations and calls; argument values keep caller identity. Copies without source parameter-name ownership, explicit Any locals, Any parameters and untyped parameters do not invent named-label contracts. |
| Negative boundaries | Unknown labels on every callable family; unnamed schema signatures; dynamic/missing/Any-returned members; unrelated qualified leaves; private source functions and imported private aliases despite registry decoys; shadowed namespace/import heads; and different/mixed/incomplete receiver alternatives return explicit null. Typed argument values still retain their lexical facts beside invalid labels or targets. |

The initial authored fixture exposed absent source named-label hover and absent
schema/builtin import-use hover. Labels now consume the existing scoped callable
contracts, canonical named-argument site collector and signature-only parameter
spans. Metadata-only labels expose types without invented source identity.
Unresolved HIR imports expand their actual scoped alias before static lookup;
source visibility stays authoritative. Constructor field-label ownership remains
in the existing S4 handler. Shared drivers and all accepted exact test identities
are unchanged.

Two oracle details use independent existing contracts: `TypeFact::display_name`
uses structural `Array(...)` / `Option(...)` fact display, while source signatures
retain their hint spelling; accepted S2 and the hint/fact contract preserve an
unknown host parameter when schema is absent. The fixture uses the validated
schema callable-signature artifact shape (`typeFact`, `requirement`). None of
these adjustments copy a provider response into expected values.

Protocol uses actual initialize/open/request dispatch, encoded isolated roots,
immutable physical inputs, physically absent schema and repeated requests.
Query prefixes are ASCII; accepted coordinate evidence owns same-line non-BMP
proof. No other hover syntax/state group or full UX10 acceptance is claimed.
Signature-help active-parameter/default/named behavior retains its separately
accepted proof. Windows/macOS acceptance needs independent fresh evidence from
one registered profile.
