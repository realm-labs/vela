# Selection literals and operators

B10.31 closes only S7 positive/negative service/protocol. Selection recovery,
trivia, installed selection and native UX11/UX12 remain open; B16 deferred.

117 independently marked documents author 244 queries: 230 complete chains and
14 points. 112 documents have positive chains, four have only points, and one
has a genuinely empty vector. Four LF/CRLF and Unicode prefix forms produce
468 whole stdio vectors and 976 positions. Selection reads syntax, never executes
operators, scoped tasks, collection constructors or host state.

Cover decimal/hex/binary/underscored integers, decimal/exponent floats, all eight
integer and two float suffixes, booleans, ASCII/Chinese/non-BMP/escaped characters,
empty/Unicode/escaped/multiline strings, empty/ASCII/escaped bytes and interpolated
text. Unary not/negate/nesting, all 17 binary/logical/range operators and all six
assignments retain actual precedence, left binary and right assignment association.
Unit/parenthesized/tuple/nested/trailing-comma expressions, array elements/nesting/
empty/multiline/trivia and indexed receivers retain delimiters and punctuation.
Map string/bare/integer/float/character/boolean/qualified-path keys, colon/value
ownership, nested arrays/maps, multiline entries and trailing trivia are explicit.
Records include explicit/shorthand fields, empty lists, nested maps and comments.
Set uses the current `set::from_array(...)` entry point, confirmed by compiler
collection-method tests; the old plan's `Set::new()` is not introduced. Empty `{}`
is a Block in `braced_expression_kind`, so its document is named empty-braced-block.
No nonexistent empty Map syntax owner is claimed.

Interpolation covers text chunks, escaped braces/Unicode, braces and values,
unary/binary/assignment expressions, arrays/maps, nested and multiline strings,
inner comments, lambda-as-callee and named calls. Index, path/member separators,
try/await, method calls and same-spelled neighboring literals remain syntax owned.
Comments, whitespace and EOF select points; the empty input vector stays empty.

Main member is UTF16 46..52 versus bytes 54..60. Its parents are field 42..52,
index 42..59, binary 42..66, interpolation 41..67, literal 35..99, local 22..100,
block 20..117, item 8..117 and file zero. Named label 85..89 has argument 85..96,
list 84..97, call 71..97 and interpolation 70..98. Text chunks are 35..41 and
67..70, not whole-literal leaves. Left bracket 52..53 has index/binary/interpolation/
literal/local/block/item/file parents. Its byte leaf is 60..61. Return result
108..114 has ReturnStmt 101..115. Helper member 43..48 has field 39..48, binary
39..52, interpolation 38..53, literal 32..54, return 25..55, block 23..57, item
8..57 and full file including the separate public apply function. Helper text
chunk 32..38 retains literal/return/body/item/file ancestry.

The marker DSL cannot terminate a range immediately after literal `[`. Five
queries explicitly author its start point, ASCII token and complete parent chain.
Separate Rust/Node test-only wrappers assert source byte 91 and prefix a one-byte/
one-UTF16-unit leaf. They never read CST/provider output or production LineIndex.
Fixed whole-chain UTF16/byte goldens pin both leaf and parents in all four forms;
wrong token, empty parents or a non-bracket source point must reject. Common
marker parsing and selection oracles remain unchanged.

Preserve the initial 102-document/408-vector/896-position draft with four failed
vectors/four positions. The nested inner tuple's marker accidentally included the
outer closing delimiter. Primary parenthesized-expression delimiter ownership
requires the inner matching close. Move only that marker, preserving every clean
source byte, query coordinate and actual response. Rename the empty-braced case
to reflect its Block owner; all 408 vectors then match. Partition review deliberately
adds 15 suffix/key documents; all 468 final vectors match. This exposes no product
defect and changes no production/parser/grammar/runtime/capability behavior.

Both Rust layers repeat whole main/helper/missing/empty vectors three times through
disk, every dirty variant and close restoration, compare separately fresh analysis,
and recheck every previous immutable snapshot. Exact source/CST, quiet cached parse
diagnostics, two records, absent schema and generation/parse/project/HIR counters
are required. Protocol pins complete typed result/error envelopes, actual initialize/
open/change/close, encoded owned roots, unchanged physical disk and atomic invalid
half-character/outside/mixed position errors. Stdio lexical/parse-code filters alone
cannot certify valid syntax policy. Node independently pins membership, fixed ranges,
containment, span deduplication, order, duplicate queries and point/empty cardinality.
All earlier generic/import/declaration/body/type/member/call/pattern assertions and
accepted platform history remain intact. Fresh native/editor/full/strict gates must
match the final source before the checkpoint advances.
