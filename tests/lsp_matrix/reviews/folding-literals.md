# Folding literals and operators

B10.19 reviews only S7 positive/negative service/protocol, four cells.
Member/constructor, recovery/trivia, installed smoke and native UX12 remain
separate. Previous declaration/body/import proofs remain active.

Fifty-nine independently authored whole sets have forty-seven positives with
164 region spans and twelve genuine empty single-line cases. Arrays, nested
arrays, tuples, maps with expression keys/values, records with shorthand fields,
triple-quoted plain/interpolated strings and array expressions inside
interpolation pin their full outer and inner regions. Plain string contents
that resemble source must remain a single literal region.

Both unary operators and all seventeen binary operators traverse multiline
operand literals without adding an operator region. Assignment target/value,
all five compound writes, field receivers, call callee/arguments, index
receiver/index, try and await preserve the same child ownership. Multiline
scalar operators, calls, index arguments and parenthesized scalars retain only
their enclosing item/body ranges; parenthesized arrays retain the array alone.
Unit, numeric radix/exponent/suffix, character, byte, boolean and ordinary
string literals cannot invent a fold. Every single-line example yields `[]`.

The first literal pins item UTF16 column8 versus byte12, body35 versus39 and
array line1 column14 through line4 column1; item/body end line6 column1.
LF/CRLF and two independently authored Unicode shift lines preserve these
literal coordinates. Service byte coordinates and strict immutable-source
protocol projection remain unchanged.

Both layers check disk, every dirty case and close restoration against current
and independently fresh state, every earlier immutable source/parse/query
snapshot, and complete main/helper/missing sets repeated three times.
Generation/parse/project/HIR counters, two current source records, absent
schema, typed initialize/open/change/close/query envelopes, encoded owned
roots and unchanged physical disk are required. Independent Node assertions
pin full memberships/counts/coordinates and nested versus wrapper ownership.
The actual bounded stdio prototype passes all236 authored UTF16/parse cases,
including188 positive sets and48 genuine empty sets. Fresh installed/native/
full acceptance remains required for the batch. No language/runtime/ownership
change or parent acceptance is claimed.
