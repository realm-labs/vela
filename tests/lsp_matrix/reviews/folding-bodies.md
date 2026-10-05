# Folding function bodies and control flow

B10.18 reviews only S2/S6 positive/negative service/protocol, eight cells.
Member/constructor, literal/operator, recovery/trivia, installed smoke and
native UX12 requirements remain separate.

Thirty-four independent complete sets have twenty-eight positives (103 region
spans) and six genuine empty sets. Locals/type hints/assignments/compound writes,
returns and shadow blocks, statement/expression/return if/else/else-if, iteration
with tuple or two bindings, multiline iterable and nested break/continue, match
scalar/block arms, unit/tuple/record patterns, aliases/shorthand/multiline pattern
fields/guards, expression arms containing if, closures/captures, named callbacks,
lambda array expressions, block expressions through assignment/return/field/
index/call, async await callbacks and inherent/default-trait method bodies have
every range authored independently. Existing declaration tests retain parameter
defaults and single-line method negatives alongside these body-specific cases.

Normal if and for statements have only their nested block/value regions;
match expression arms retain their whole expression region as well as branch
blocks. Multiline patterns/parenthesized scalar guards cannot invent folds.
Lambda and block regions remain distinct; repeated traversal of the same range
does not duplicate it. Single-line bodies/control-flow/callbacks yield `[]`.

LF/CRLF and two Unicode shifts pin item UTF16 8 versus byte12 and body35 versus39,
with literal end line4 column1. Actual stdio prototype compares all136 authored
UTF16/parse combinations:112 positive sets and24 empties. An initial ignored
draft used a colon for a named call argument; retain that failed capture and
source, correct it to the documented `name = value` syntax, and retain subsequent
120/128/136 passing expanded corpora. This is fixture correction, not a claimed
production defect or response-derived oracle.

Both layers repeat whole main/helper/missing sets three times through disk/every
dirty case/close restoration, independent fresh state and every prior immutable
source/parse/query snapshot. Generation/parse/project/HIR counters, two current
records, absent schema and exact real disk bytes stay pinned. Typed initialize/
open/change/close/query dispatch uses encoded owned roots and complete envelopes.
Node pins every membership/count and literal coordinate/ownership policy.
Existing tests/contracts and independent platform evidence remain preserved;
this child does not claim full B10 acceptance.
