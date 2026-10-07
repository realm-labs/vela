# Selection recovery partition review

B10.35 consolidates S9 service/protocol positive and negative proof. The new
fixture contains 250 independently authored documents: 82 damage/repair/redamage
triplets, an actual empty request vector, and empty/whitespace/comment files.
Its 820 queries contain 729 complete chains and 91 points. Four Unicode LF/CRLF
and shifted forms produce 1000 complete real stdio vectors / 3280 positions.
The earlier bracket, parenthesis and tuple-pattern assertions remain mandatory
parts of these four cells, rather than being replaced by the new corpus.

| Partition | Complete independently authored ownership |
| --- | --- |
| Incomplete expressions | Local and return member/static/service paths, positional/named/empty-value calls, lambda headers/bodies, binary/unary operands and source records preserve every available expression/argument/statement/body/item/file ancestor. |
| Delimiter recovery | Retained bracket and parenthesis corpora cover complete final field/binary/call/array/index/map operands in local, return and direct statement contexts. Retained tuple-pattern recovery covers bindings, wildcards, literals, qualified paths and closed nested tuple/record patterns in both outer tuple forms and all three match contexts. |
| Type recovery | Open parameter/return/local hints, Array/Map/Set/Iterator/Option/Result, nested Result/Array and tuple hints retain exact TypeHint/TypeArgList/parameter/declaration owners. An unclosed tuple hint intentionally has no inner hint nodes; an incomplete Result has the independently required arity diagnostic. No script generics are introduced. |
| Declarations and defaults | Missing function/struct names, parameter/struct/enum-record types, parameter/default calls, enum tuple fields, trait/impl method hints, field defaults, const/state values and unfinished imports retain whole available CST owners. Sixteen damaged-neighbor partitions place malformed fn/struct/enum/trait/const/use before and after an intact function, with complete healthy ancestry. |
| Guard/control/map | Match guards with an actual arrow, if conditions, for iterables and map missing-key/value/closer states retain available field/arm/list/match/control/entry owners. Direct control statements have no invented ExprStmt parent. |
| Lexical recovery | Ordinary/multiline strings, bytes, chars, formatted strings and unfinished interpolation remain whole opaque PathExpr tokens until repaired. Repaired interpolation owns Literal/Interpolation/PathExpr. Unclosed block comments remain point queries and retain exact lexical errors. |
| Ownership and negatives | Unknown source/static/service receivers retain syntactic ancestry without borrowing semantic declaration parents. No schema facts exist. Duplicate queries preserve order. Right significant tokens win at token boundaries; trivia and true empty files yield points, empty request vectors stay empty, and missing documents have explicit point/empty policies. |

Every authored node has literal source bounds and a fixed SyntaxKind. Tests inspect
the cached complete CST and every selected token's full ancestor list, including
same-span nodes omitted by selection range deduplication. This independently
catches lost AST structure even where an already-green range chain would hide it.
Both layers check repeated whole main/helper/missing vectors, disk/every dirty
state/close, fresh databases and every earlier immutable snapshot. Owned source
and full CST text, two source records, absent schema, unchanged query generation,
parse/project/HIR counters and unchanged disk are required. Protocol additionally
checks typed complete response envelopes and atomic rejection of split UTF16,
outside-line/document and mixed valid/invalid position vectors in encoded roots.

Keep authoring errors separate from product defects. The initial 184-document
probe had 480 endpoint vector mismatches: the author assumed the left token at a
boundary. Correct right-token expectations preserve all clean text, coordinates
and actual responses. A lone `!` has an intentionally opaque PathExpr until an
operand exists. Lexical errors and nested Result arity errors must remain in the
exact diagnostic code sets. These corrections do not establish product red.

Independent syntax red does establish missing required-child errors: explicit
empty parameter/field annotations and imports ending in `::` silently passed.
The grammar requires a type after an explicit colon and a segment after `::`.
Minimal validators report those missing children without changing CST recovery,
optional unannotated parameters/fields or accepted quiet partial expressions.
Nine contexts, four Unicode/newline forms and three states produce 108 direct
observations with exact diagnostic messages, codes, spans and retained nodes.
The original red and corrected expectations are retained independently.

This strengthens the old folding import case from quiet to diagnosed. Both old
layers first fail on `unclosed-import-group`; change only its parse-error boolean
and corresponding count assertion. All 54 sources, all 265 folds, existing CST
checks and other policies remain unchanged. Forty-four sets now require errors;
ten remain valid or accepted quiet. Historical B10.22 quiet import evidence is
superseded by the required-segment diagnostic, with the failed runs preserved.

S9 syntax proof does not close source/dependency recovery state, S12 trivia,
installed selection or UX11/UX12. They remain separate B10 requirements. Windows
proof is fresh on the current source; macOS audits stay independent and B16 stays
deferred. The parent batch is not accepted by these four cells.
