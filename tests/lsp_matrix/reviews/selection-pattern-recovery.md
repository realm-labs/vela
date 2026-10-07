# Selection tuple-pattern recovery

B10.34 continues S9 with an independently exposed CST defect. This is partial
evidence; no broad catalog cell is certified. The tuple-pattern body previously
excluded the final token even when no closing parenthesis existed. Simple final
bindings disappeared from the AST; qualified paths and closed nested tuple/record
patterns lost their last token and complete syntax ancestry.

The independent AST test first fails because an outer tuple has only one of its
two authored patterns. Six final patterns (binding, wildcard, boolean literal,
qualified unit path, closed nested tuple and closed record), qualified/naked
outer tuples, local/return/direct match contexts, damage/repair/redamage and four
Unicode LF/CRLF forms produce 432 observations. Whole source/node/final-pattern
slices, first binding, separator, owner path, byte bounds, real closing tokens
and diagnostic appearance/clearing are exact. The fix reuses paren_contents_end:
exclude only the actual matching closing token belonging to this bounded tuple.
Inner delimiters remain owned. No grammar, public AST/token contract, runtime,
selection provider or folding behavior is changed.

Keep authoring mistakes separate from product proof. Initial four-document
selection expectations repeated identical pattern/arm spans when no arrow was
present: eight vectors failed, but corrected 16 vectors all match before the fix.
The expanded draft additionally omitted record-field trailing space, invented an
ExprStmt parent around direct match statements, and misplaced their terminal
newline. Existing primary CST statement/record ownership contracts establish the
correct policy. Marker/chain corrections preserve all clean bytes, query positions
and actual responses, with original expectations retained as artifacts.

After those corrections, 72 complete real stdio vectors/144 positions still fail
before the product fix: incomplete final qualified paths and closed tuple/record
patterns in every context and outer form. Preserve their complete sources,
coordinates and expected chains unchanged. All 332 vectors/1284 positions then
match after the one-line fix. Simple missing bindings are genuine AST regressions
even though selection deduplication made their pilot vectors already correct.

The final marked corpus contains 83 documents/321 queries: 234 complete chains
and 87 points; 78 positive documents, four point-only and one actual empty vector.
Forty damaged sources require nonempty E_PARSE-only cached diagnostics; 43 quiet
sources and the helper require empty arrays. Explicit redamage, duplicate queries,
comments, whitespace and EOF/empty policies retain cardinality. Unknown qualified
pattern targets remain source syntax ancestors, never semantic declaration ranges.

Repaired main binding is UTF16 67..71 versus bytes 71..75, with tuple 47..72,
arm 47..77, list 45..80, match 35..80, local 22..81, body 20..83, item 8..83 and
full file zero. Missing arrow gives pattern/arm the same 47..71 span, emitted once;
list 45..71 and match 35..71 retain complete bounds while local/body/item own the
terminal newline. Naked tuple binding 54..58 retains tuple 47..58 when incomplete.
Direct match has no invented expression-statement parent; repaired match ends
before its semicolon, while incomplete arm/list/match retain the available newline.
The helper's complete field/binary/return/body/item/file chain remains fixed.
Node pins literal UTF16/byte goldens, owner deduplication, context membership,
containment, stable ordering and exact point/empty vectors across all four forms.

Both layers repeat whole main/helper/missing/empty vectors three times through
disk, each dirty source and close restoration, compare independent fresh state,
and recheck every earlier immutable snapshot. Source/CST/parse policy, two records,
absent schema and fixed query counters stay exact. Protocol compares complete typed
envelopes and atomic half-character/outside/mixed invalid-position errors on encoded
owned physical roots, with unchanged disk. Common marker/oracles, all previous
selection and folding assertions/catalog/native contracts remain unchanged.
The first complete audit also exposes one outdated B06 semantic-token expectation:
`pattern-eof`'s final `bound` is now a real HIR pattern binding and requires
`declaration` and `source`. Preserve its source, marker, type and range, all other
tokens and diagnostics. The reviewed semantic-token fixture and an independent
Node golden strengthen that single modifier set; requirement contracts remain
unchanged. Preserve the failed audit and renew all source-matching evidence.
Fresh source-matching native/editor/full/strict proof is required for this child.
Remaining S9 partitions, recovery state, trivia, installed selection and native
UX11/UX12 stay open; B16 stays deferred and historical platform proof is independent.
