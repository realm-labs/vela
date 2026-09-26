# Signature help: S5 review

Scope: the four `signature-help/syntax/S5/{positive,negative}/{service,protocol}`
obligations. Both exact compiled test identities end with
`signature_call_matrix_preserves_full_parameters_owners_and_static_boundaries`;
the service module is `signature::call_matrix_tests`, and the protocol module is
`tests::signature::call_matrix`.

`signature-s5` authors 27 complete signature templates and 161 query positions.
The templates independently specify the whole label, every parameter's name,
type and label, default markers, canonical owner and named-argument policy.
Each query specifies its current semantic parameter or explicit null. Reviewed
source fixtures are reused as inputs; no provider output creates expected data.
The test-only expander selects authored templates and projects parameter labels
to the wire fields. Both drivers execute all 161 positions with LF and CRLF,
repeat each request and compare complete signature lists and both active indexes.
Service also compares the complete callable owner list and named-argument policy;
protocol dispatches real requests, requires an explicit JSON result and preserves
physical source bytes. Fixture configuration is parsed and its diagnostics must
be empty. Protocol uses isolated Chinese/space/percent paths and UTF-16 markers.

| S5 partition | Fixture / positions | Complete assertions |
|---|---|---|
| Source functions, defaults and active arguments | `call-parameter-mapping`, 44 | All parameters remain present, including defaults and zero-parameter functions. Reordered named slots, mixed arguments, unknown/duplicate names, positional overflow, comments, multiline calls, strings containing commas, nested calls/lambdas and a missing close use authored active slots. Invalid mappings retain the known callable and the documented presentation fallback zero. |
| Source, schema and standard imports | `callable-imports`, 32 (15 positive, 17 null) | Direct, imported, aliased and namespace-qualified calls retain canonical owner and full parameters. Current-module source functions take precedence. Private, missing, ambiguous, unimported, non-callable and shadowing paths return null and no callable owners; unrelated same-name declarations cannot supply a signature. Source tuple variants retain positional-only policy. |
| Service static call boundaries | `completion-service-calls`, 20 in each of full, ordinary-only and missing schema modes | Full mode has ten positive and ten null results. Base, sibling and pinned calls retain contract parameters, asyncness and schema owner rather than colliding ordinary registry functions. Missing origin, ambiguous helpers, nested lambdas/defaults, absent service/member/method and wrong shape stay null. All 40 queries without a service set stay null, even with colliding ordinary functions still registered. |
| Source and schema methods | New `methods.vela`, 19 (11 positive, eight null) | Inherent/defaulted, trait receiver, trait default, trait override and source-returned receivers; async source function; explicit named/defaulted async schema method, schema-returned receiver, type-only method and trait fallback. Type-only `arg0`/`arg1` are presentation names: named syntax cannot create semantic authority and active slot falls back to zero. Wrong-owner, missing member/callee, non-callable, unknown and Any receivers, and source/schema Any returns remain null with no owners. |
| Standard functions and methods | Imported `math::max` cases plus new `standard.vela`, six (five positive, one null) | Numeric union parameters survive direct/alias/namespace and named calls. String split, Array callback, Map insertion and Option/Result fallback methods retain every specialized parameter and return fact plus builtin owner. Dynamic callback receiver stays null. |

Standard Option/Result `unwrap_or` method metadata names its parameter `default`
and exposes `Any`; static signature facts retain `i64 | Any` as the return union.
They do not infer a narrower signature from a concrete argument. Free standard
functions use their own manifest names. Semantic type display uses `Array(i64)`;
source hints use the restricted builtin `Array<i64>` syntax. Member symbol
identities use `owner.member`, while functions and variants use `::`.

The 85 positive and 76 null positions run at both layers. This review certifies
S5 only; other syntax partitions, schema lifecycle, capabilities, installed
signature provider smoke and UX10 remain separate B07 obligations. Acceptance
requires a current-source executed audit with independent platform evidence.
