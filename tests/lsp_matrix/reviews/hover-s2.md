# Hover S2 executable bodies

Scope: `hover/syntax/S2/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::body_matrix_tests::hover_body_matrix_preserves_scoped_facts_captures_and_callbacks`
and `tests::hover::bodies::hover_body_matrix_preserves_scoped_facts_captures_and_callbacks`.

`hover-s2` independently authors 183 queries: 157 complete hovers and 26 explicit
null results with schema present. Positive tokens run at their start and
interior, negative tokens at their start. LF/CRLF and present/physically absent
schema variants yield 1,358 positions per layer, each repeated. The host field
becomes null without schema; its parameter, copied local and result facts become
unknown. Expected metadata and Markdown are authored; markers independently
supply byte/UTF-16 ranges and exact local declaration locations. Service asserts
the complete label, kind, detail, docs, range and canonical source/local identity.
Protocol asserts the complete Markdown object and range or explicit JSON null
through real initialize/open/request dispatch.

| Partition | Independent expectations |
|---|---|
| Parameters and locals | Typed declaration/use parameters, primitive inferred and annotated locals, aliasing, tuple destructuring, arrays/tuples, if/match expression results, assignments and five compound writes retain their binding facts and declaration identity. |
| Lexical scopes | Initializers reference the outer binding before a shadowing declaration; block and lambda shadows restore outside their scope. Closures capture reads/writes and locals; nested typed/untyped lambdas retain exact parameter identities. |
| Loops and patterns | Numeric and tuple loop bindings, tuple match guards and source enum tuple/record variant patterns have the independently declared facts; locals outside their owning scope return null. |
| Callbacks | Array map/filter, iterator fold and map key/value callbacks infer their parameters, captures and result locals from static contracts. Erased Function and dynamic Any callbacks retain unknown parameters; their members return null. Function-valued locals retain the existing parameter/result fact display. |
| Method bodies | Trait default, inherent and trait-impl bodies resolve implicit receivers, source fields and methods, captured self, typed parameters and inferred locals. Field reads and writes retain exact owner metadata and docs. |
| Host and negative boundaries | Schema-backed host callbacks, copies and fields use registered facts; absent schema cannot invent them. Missing names, unknown/dynamic members and out-of-scope callback/pattern/block locals return null despite unrelated source declarations. |

The authored callback-parameter declaration exposed receiver-first hover routing:
completion context carries a collection receiver, but the exact parameter token
belongs to the local declaration. Declaration dispatch now precedes that member
guard. Method-body cases exposed absent implicit receiver facts in whole-module
analysis. Scoped trait/impl owner seeding now preserves source identity and
propagates member/capture facts. Two analysis tests independently check aliased
source owners against a same-name source type and conflicting registry type,
trait calls, host fields, missing schema, explicit Any, external local contracts
and the separate executable receiver seed boundary.

Container and inferred-lambda expectations follow the existing `TypeFact`
display and semantic function contracts; this batch changes neither formatting
nor language/runtime semantics. Source-free analysis and schema-backed analysis
remain separate cached builds. The protocol driver uses isolated encoded roots,
immutable physical inputs, physically absent schema and repeated requests.
Query prefixes are ASCII; accepted coordinate evidence owns same-line non-BMP
proof. This review claims no other hover syntax/state partition or full UX10
native input/render acceptance. Windows/macOS gates require independent fresh
evidence from one registered profile.
