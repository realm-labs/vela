# Selection parenthesis recovery

B10.33 continues S9 with a concrete CST defect, without certifying any broad
catalog cell. Missing right parentheses leave grouped/tuple expressions as raw
PathExpr nodes; their inner field and other final operands have no syntax owner.
Simply admitting the incomplete expression would also truncate its final token:
the old bodies and tuple classifier unconditionally excluded the final token.

Four independently marked pilot documents precede provider observation. Their
LF/CRLF and shifted Chinese/non-BMP forms produce 16 whole stdio vectors and
64 positions. Eight incomplete vectors fail, while eight repaired controls pass.
An independent AST test fails because the grouping node is missing. Preserve
original sources, coordinates, expectations and actual responses; no marker is
moved to reconcile these product failures.

The minimal fix recognizes a grouping/tuple when its opening parenthesis has
no matching end, as existing call/index recovery does. Both bodies and the tuple
classifier exclude a token only when the matching closing delimiter belongs to
the bounded expression. A closed inner call cannot substitute for an absent outer
parenthesis. All original 16 vectors then match unchanged expectations. The AST
test checks whole nodes, complete final operands, first tuple member/separator,
byte ranges and actual closing tokens across damage/repair/redamage, three
statement contexts, six operands and four forms: 432 observations.

The final corpus contains 83 documents and 321 queries: 234 complete chains and
87 points; 78 positive documents, four point-only documents and one actual empty
vector. Forty damaged documents require nonempty E_PARSE-only cached diagnostics;
43 quiet documents and the bound helper require empty cached arrays. Locals,
returns and expression statements cover grouping/tuple final field, binary,
named call, closed nested array/index and Map. Explicit redamage, comments,
whitespace, EOF, empty and duplicate queries preserve cardinality. Four forms
produce 332 real stdio vectors and 1284 positions, all matching the authored
complete chains. The marker-authoring script initially used an invalid colon
in point names; it was corrected before any pilot provider observation, without
changing clean source or intended positions. This is separate from the AST/CST
defect and cannot count as product regression proof.

Repaired main member is UTF16 41..46 versus bytes 45..50, with field 37..46,
group 35..47, local 22..48, body 20..50, item 8..50 and file zero. Unclosed group
ends at 46; unclosed tuple member 43..48 retains field 39..48 and tuple 35..48.
Unclosed local/body/item own the terminal newline. Repaired tuple includes its
closing delimiter through 49, semicolon through 50 and body/item through 52.
Helper member 43..48 retains field 37..48, binary 37..52, return 30..53, body
28..55, item 8..55 and full file. Node pins full UTF16/byte goldens, membership,
containment, deduplication, point/empty cardinality and redamage identity.

Service/protocol repeat all main/helper/missing/empty vectors three times across
disk, every dirty source and close restoration; they compare separately fresh
analysis and all previous immutable snapshots. Both pin exact source/CST, parse
policy, two records, absent schema and fixed query counters. Protocol checks
typed whole envelopes and atomic invalid half-character/outside/mixed errors on
encoded owned roots with unchanged actual disk. Existing selection drivers,
common oracles and bracket recovery remain unchanged. No grammar,
public AST/token shape, runtime or selection provider change is introduced.

The first full audit exposes the older folding fixture's PathExpr expectation
for an unclosed multiline tuple. Update this one case explicitly: preserve clean
bytes, all 264 original folds and error policies, require complete TupleExpr with
first/final literal children and add its owned multiline fold (265 total).
Node pins literal coordinates/slices; the five folding S9/recovery-state catalog
descriptions record this stronger contract. Other folding cases and historical
accepted parent snapshots remain fixed. Preserve both-layer failures, initial
source identity and failed audit; regenerate all gates for the corrected source.

Fresh source-matching native/editor/full/strict evidence is required for this
child. All four S9 cells, remaining recovery partitions including tuple patterns,
recovery state, trivia, installed selection and native UX11/UX12 remain open.
B16 stays deferred; implementation progress is shared across registered machines,
but their historical evidence is not combined to satisfy current gates.
