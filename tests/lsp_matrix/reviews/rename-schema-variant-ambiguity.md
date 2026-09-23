# Schema variant ambiguity after rename

B04.25 advances the 48 S2/S4/S8 positive/negative service/protocol
reference, highlight, prepare-rename and rename cells. These broad cells stay
unreviewed until the complete B04 gate.

The independent 40-query fixture retains seven ownership groups and adds two
ambiguity cases. `alpha::Choice::Shared` and `beta::Choice::Shared` make
`Choice::Shared` unresolved; renaming the source-backed alpha variant would
otherwise make that short path refer to beta. Two direct schema imports of
`gamma::Token::Item` and `delta::Token::Item` make bare `Item` unresolved;
renaming gamma would otherwise make the bare path refer to delta. Both cases
assert exact owned reference sets, separate other-owner sets, prepare results,
null results at unresolved paths, and rename rejection from each writable
owner position. The existing schema variant owner remains a safely applied
rename control.

Service and protocol run the same independent oracle under LF/CRLF. The
surrounding schema/source/local groups apply their edits, compare whole files,
parse, requery and restore. Protocol also checks UTF-16, encoded paths,
open/closed versions, both edit forms and schema ABI markers.

The initial gamma declaration rename incorrectly succeeded, while short-name
ambiguity was already rejected. Schema variant rename now simulates the
post-rename import bindings even for paths that were originally ambiguous,
then compares lookup identities under the existing local and source priority
rules. Remaining parameter and lifecycle coverage plus UX05/UX06 are still
required before B04 acceptance.
