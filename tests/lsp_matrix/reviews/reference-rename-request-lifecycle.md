# Reference and rename request lifecycle

B04.30 established the document-version and request lifecycle fixture. B04.46
completes rename's stale-version and cancellation protocol state cells (two
IDs). The fixture also rechecks the already verified repeat cells for
references, highlight, prepare-rename and rename.

The protocol tests reuse the independent four-file reference fixture with an
open importer and definition, a closed consumer, Unicode source text, and
LF/CRLF variants. They assert complete reference sets, both highlights,
prepare range/placeholder, and every legacy and versioned rename edit after
successive helper changes. Open file versions and closed file null versions
come from explicit fixture state.

After an accepted helper edit at version 4, duplicate and older versions carry
whole-document, incremental, invalid-range and empty changes. Each is ignored:
generation, parse count and HIR rebuild count stay fixed, and all four requests
repeat the exact current results. Closing and reopening resets the version to
-7; a later version -3 edit is accepted and its renamed declaration range and
edit version replace the old values. Cancellation notifications before a new
request, after a source change, after a completed request, and for an unknown
ID do not poison subsequent current requests.

Rename now runs as a cancellable worker snapshot task. The test queues a real
rename request, receives its completed task without publishing it, then sends
`$/cancelRequest`; publication returns only `RequestCancelled`, with no edit,
and a later request returns the exact current edit set. A second queued rename
captures the old generation before a helper edit; its result is discarded as
`ContentModified`, and the next request returns the new declaration range and
version. These deterministic interleavings check the same production dispatch
and publication path as stdio without a timing-dependent sleep. Remaining
syntax partitions and UX05/UX06 remain before B04 acceptance.
B00-B03 snapshots stay fixed; macOS needs fresh evidence when used. B16 remains
deferred.
