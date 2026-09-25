# Reference and rename S11 incrementality

B04.47 reviews the eight S11 positive and negative service/protocol cells for
references and rename. S11 requires repeated requests, body-only edits,
declaration/import changes, reverse-dependency invalidation, cancellation and
generation rejection. Highlight and prepare-rename do not own S11 cells.

The shared four-file lifecycle fixture has an imported `helper::increment`, a
closed qualified consumer and an independent same-named `other::increment`.
Each action checks exact reference sets with and without the declaration and
complete rename edits against independent site markers, including a shifted
helper declaration, dirty importer rebinding, close-to-disk restoration,
dependency deletion and recreation. Both layers compare their long-lived state
with a fresh workspace. LF/CRLF and Unicode prefixes are exercised; protocol
also checks encoded URIs and client versions for open and closed files.

The additional body-only test changes the helper function body without moving
its declaration, verifies that a new generation is accepted, and repeats every
current reference and rename query. Service wraps real reference and edit
results in cancellable background tokens: current results are accepted,
cancelled results are rejected, and results from a pre-edit generation are
rejected. Protocol ignores duplicate/older document versions without parsing
or rebuilding and accepts a reopened document's new version order. Real queued
reference and rename tasks publish only `RequestCancelled` or
`ContentModified` after cancellation or a source edit, with no stale locations
or edits; later requests return exact current results.

Positive cells require current complete locations and edits after each valid
change. Negative cells require empty unresolved results, separate decoy owners,
rejected old document versions, and no publication of cancelled or stale task
results. Remaining B04 syntax dimensions and current-profile interaction gates
still need acceptance; B00-B03 snapshots stay fixed and B16 remains deferred.
