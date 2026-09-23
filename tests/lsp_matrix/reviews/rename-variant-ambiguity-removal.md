# Source variant import ambiguity removal

B04.24 advances the 48 S2/S4/S8 positive/negative service/protocol
reference, highlight, prepare-rename and rename cells. These broad cells remain
unreviewed pending the complete B04 gate.

The independent 55-query fixture has nine ownership groups and six writable
groups. Two private source enums each expose a `Clash` variant through an
unaliased direct import in the same module. The bare `Clash` is unresolved
because both imports bind it; declaration, import and qualified use positions
still have distinct exact reference sets. Renaming either variant to `Fresh`
must be rejected at all three owned positions: otherwise the previously
ambiguous bare name would silently resolve to the other variant. An unrelated
private `Solo::Unique` variant can safely become `Fresh` with full applied-edit
verification, establishing that the candidate spelling itself is valid.

The service and protocol layers assert complete references and prepare results
for both owners, null references/prepare/rename for the ambiguous bare path, and
rejection of all six owner rename requests. The surrounding fixture retains
schema, public source, private source and local ownership controls. Its six
writable groups undergo whole-file applied edits, parse, exact requery and
restore. Protocol checks LF/CRLF, UTF-16, encoded paths, open/closed versions,
both edit forms and schema ABI markers.

The initial source declaration rename incorrectly succeeded. Counterfactual
source lookup now simulates import bindings after the proposed rename and
compares every expression/pattern path, including ones that were unresolved
because their original imports were ambiguous. Remaining schema short-owner
ambiguity and parameter/lifecycle coverage plus UX05/UX06 still precede B04
acceptance.
