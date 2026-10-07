# Selection bracket recovery

B10.32 fixes two concrete CST defects exposed while starting S9. This is partial
S9 evidence: no catalog cell is certified and the four broad S9 cells, recovery
state, trivia, installed selection and native UX11/UX12 remain open. B10.33 must
continue full error-recovery coverage. B16 remains deferred.

Independent marked sources precede provider observations. The initial eleven
documents produce 44 whole stdio vectors/136 positions in LF/CRLF and shifted
Chinese/non-BMP forms. Eight vectors/twelve positions fail: unclosed arrays drop
the last token from the final field expression; unclosed indexes lack an IndexExpr
and its inner field. An independently authored AST test also fails with `row.`
instead of `row.field`. Preserve all original sources, positions, expectations
and actual responses. No fixture boundary is changed to make these failures pass.

The fix excludes a closing bracket only when its matching end is the bounded
expression end. Otherwise array/index contents include every remaining token.
Outer index recognition accepts an absent closing bracket as the existing call
recovery does. A closed inner bracket never substitutes for the missing outer
one. Original 44 vectors then match unchanged expectations. Array/index AST
tests pin complete source/inner slices, receiver, byte ranges and actual delimiter
presence through damage/repair/redamage, three statement contexts, six operands
and four Unicode/newline forms. Valid closed expressions keep their existing
syntax/semantics; no grammar, public AST/token contract or runtime API is added.

Final corpus has 90 documents/265 queries: 171 complete chains/94 points,
85 positive documents/four point-only/one actual empty vector. Forty-three
damaged documents require nonempty cached parse diagnostics containing only
E_PARSE; forty-seven quiet documents and the helper require an empty array.
Selection itself does not execute code or inspect live host state. Locals,
returns and expression statements cover array and index final field, binary,
named call, closed nested array/index and Map operands. Unclosed/repaired call,
Map and record controls retain their already-correct owners. Explicit redamage,
comments/whitespace/EOF/empty vectors and duplicate positions retain cardinality.
Four forms produce 360 real stdio vectors/1060 positions, all matching complete
independent expectations. The helper's source is intentionally extended from
the pilot to bind its field receiver; original eleven case sources stay fixed.

Main member is UTF16 61..66 versus bytes 65..70, with field 57..66, named argument
49..66, list 48..67, call 35..67, local 22..68, body 20..70, item 8..70 and file
zero. Incomplete array member 41..46 retains field 37..46 and array 35..46;
incomplete index member 45..50 retains field 41..50 and index 35..50. Unclosed
statement/body/item extend through the terminal newline, not merely the last
token. Repaired array includes `]` through 47, semicolon through 48 and body/item
through 50. Helper member 43..48 has field 37..48, binary 37..52, return 30..53,
body 28..55, item 8..55 and full file. Node pins full UTF16/byte chains, membership,
containment, span deduplication, point/empty cardinality and redamage identity.

Both layers repeat entire main/helper/missing/empty vectors three times through
disk, each dirty source and close restoration, compare separately fresh analysis,
and recheck every previous immutable snapshot. Source/CST, explicit parse policy,
two records, absent schema and query counters remain exact. Protocol compares
whole typed envelopes and atomic invalid half-character/outside/mixed errors on
encoded owned physical roots, and verifies unchanged disk. Earlier selection
oracles, assertion drivers and catalog remain unchanged. Fresh source-matching
native/editor/full/strict gates are required before committing this partial child.
