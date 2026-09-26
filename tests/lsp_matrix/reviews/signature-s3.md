# Signature help: S3 review

Scope: the four `signature-help/syntax/S3/{positive,negative}/{service,protocol}`
obligations. Exact tests are
`signature::call_matrix_tests::signature_type_matrix_preserves_complete_hints_and_unknown_boundaries`
and `tests::signature::call_matrix::signature_type_matrix_preserves_complete_hints_and_unknown_boundaries`.

`signature-s3` independently authors 94 full signature templates over 102 query
positions: 95 signatures and seven explicit null results. Both drivers execute
every position with LF and CRLF and repeat complete results. All signature labels,
parameter names/types/labels/defaults, active indexes and static owner policies
are exact. Service additionally compares every parameter and callable return
against independently authored structural TypeFacts, including container leaves,
iterator items, mutation modes, tuple elements, callable parameters/returns and
the distinction between source records and schema host types. Its test-only
decoder constructs enum variants directly; it does not call a production hint,
schema artifact or signature parser. Protocol checks the full wire object using
independently marked UTF-16 positions and preserves physical source bytes.

| S3 partition | Positions / authored assertions |
|---|---|
| Primitive and builtin type hints | New `types.vela` has all 15 primitive contracts, Range, typed Array/Map/Set/Iterator/Option/Result, tuple, erased Function/Closure/Array, and explicit Any. Parameters and returns retain exact structural facts and full labels. Iterator display remains `Iterator`, while the structural oracle checks its otherwise hidden item type. No general callable or script generic syntax is added. |
| Source and schema named types | New source/schema record, enum and trait alias cases compare canonical type names and distinct structural variants. The reviewed `completion-callable-hints` source inputs add direct, alias and namespace hints, source precedence over colliding schema names and provider impl/default ownership. |
| Nested collections and views | The 29 reviewed hint positions include shared/mutable Array/Map/Set views, fixed/growable mutation shape, nested Result/Array and tuple hints. The whole parameter list and return facts are compared, rather than just the eligible completion parameter. |
| Unavailable type leaves | Repeat all 29 hint positions without schema: 25 source-owned callables retain their signature and authored unknown leaves; four absent native calls return null. Private/non-type/missing/ambiguous/qualified-miss imports cannot borrow same-short-name schema facts. Invalid non-builtin type arguments retain unknown facts. Known callables with unknown or unhinted parameters still show only their known signature facts. |
| Structured schema callables | Free function, host method and trait method independently specify an async three-parameter signature with a nested Function/Option callback, Array, defaulted Result parameter and Result/Array return. Active slots zero, one and two retain their distinct names and exact facts. |
| Any and unknown boundaries | Explicit Any parameter/return facts remain Any; unresolved types and nested leaves remain Unknown. Dynamic/unknown receiver and missing callee requests return explicit null and an empty callable-owner list. |
| Range builtin ownership | Range hints resolve to `TypeFact::Range` with or without schema. An ordinary schema host type named Range and a colliding `Range.len(bool) -> bool` cannot replace the builtin. `Range.len() -> i64` retains zero parameters and its builtin owner. |

This matrix exposed a shared hint conversion defect: legal `Range` hints yielded
unknown parameter/return facts. The builtin converter now maps Range directly.
Existing analysis proofs also check Range and Array-of-Range through function,
trait/default-provider, field, const and state facts. The old standard completion
Range case had inherited an unfiltered expression set from the unknown receiver;
it now requires the known zero-parameter signature and an exact empty eligible
argument set, with explicit absent expected name/type. The case is retained and
strengthened. This is a correction to the existing builtin/zero-argument contract,
and does not change language syntax or runtime semantics.

This review closes S3 only. Other syntax dimensions, lifecycle, capabilities,
installed signature provider smoke and UX10 remain separate B07 work. Current
source execution and fresh independent platform evidence are required to accept.
