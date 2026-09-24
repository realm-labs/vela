# Reference and rename request lifecycle

B04.30 advances rename's stale-version and cancellation protocol state cells
(two IDs). It also rechecks the already verified repeat cells for references,
highlight, prepare-rename and rename. The two broad state cells remain
unreviewed until their other interactions are covered.

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

These four methods currently use synchronous snapshot dispatch, so there is no
pending task to cancel or retry in the test harness. This batch asserts their
actual request boundary. Retry and stale-discard behavior for asynchronous
requests belongs to the transport and cross-feature batches. Recovery syntax,
remaining syntax partitions and UX05/UX06 remain before B04 acceptance.
B00-B03 snapshots stay fixed; macOS needs fresh evidence when used. B16 remains
deferred.
