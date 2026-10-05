# Folding declaration regions

B10.17 reviews only S1 positive/negative service/protocol. Member/constructor,
control-flow/literal/trivia/recovery partitions, installed smoke and native UX12
remain separate requirements.

Twenty-two independently authored complete sets have sixteen positive cases
(twenty-five region spans) and six exact negatives. Public/private/defaulted/
untyped/async functions, multiline parameter headers, default arrays, attributes,
public/private structs, all enum variant forms, required/default/async trait
methods, inherent/trait implementations, method defaults and constant arrays
retain literal whole spans. Function item/body ranges remain distinct; trait/
impl method bodies and parameter defaults have no invented method-item region.
Multiline field/record-variant internals retain the existing enclosing-only
declaration policy. State initializer arrays and multiline extern-state hints,
single-line declarations, comment/string decoys and empty files return `[]`.

LF/CRLF and two Unicode prefix lines pin item UTF16 column8 versus byte12 and
body45 versus49, with the same complete end at line2 column1. The ignored actual
stdio prototype compares all88 authored complete UTF16/parse combinations,
including64 positive sets and24 exact empties; no provider-derived expectation.
Service retains byte coordinates and protocol uses the strict projection fixed
in B10.16. Independent Node tests pin every marker membership, complete counts,
literal item/body geometry and inclusion/exclusion policy.

Disk, every dirty variant, close restoration and all prior immutable source/
parse/query snapshots are compared against independent fresh databases and
protocol coordinators. Whole main/helper/missing sets repeat three times without
generation/parse/project/HIR counter changes. Actual typed initialize/open/change/
close/query dispatch checks complete envelopes, two current sources, no schema
facts, private encoded roots and unchanged physical main/helper bytes. Existing
folding/import tests, contracts and platform evidence remain independently
preserved; this child does not claim full B10 or native acceptance.
