# Semantic-token coordinates and repeated requests

B06.1 closes the twelve `tokens-{full,delta,range}` protocol environment
(`utf16`, `crlf`, `encoded_uri`) and `repeat` state obligations. Syntax
partitions and the other lifecycle states remain open.

The shared fixture independently enumerates every token, including both slices
of a multiline Unicode comment, a same-line function after Chinese text,
parentheses/braces, local declarations/uses, strings with a non-BMP character,
assignment, return and semicolons. Service assertions use byte coordinates;
protocol assertions decode the entire relative stream against UTF-16 markers.
Both layers check exact text/type/modifier sets, nonzero lengths, source bounds,
line confinement, ordering and nonoverlap. LF and CRLF use the same logical
oracle without including carriage returns in any token.

An unsaved edit inserts a Unicode header and a blank line, renames the local,
and lengthens its Unicode string. Applying service and encoded protocol deltas
to the preceding streams must equal the independently checked current stream.
Fresh instances agree. Closing the dirty protocol document restores the exact
disk stream and original ID. Full and range requests repeat identically; a
delta against the current ID has no edits. Actual encoded root URIs retain one
identity across open, change, close and fresh disk loading.

A separate dirty edit replaces `"文😀"` with `"éééa"`: both occupy nine UTF-8
bytes, but their UTF-16 lengths differ. The old byte-only result ID incorrectly
produced zero edits, leaving stale client lengths and following columns. IDs
now include the source fingerprint, retained by range filtering, so applying
this delta also matches the independently checked fresh client stream.

Range requests check whole-token partial overlaps, exact later-line function
and string spans, prefix and interior empty ranges, and rejected reverse or
split-surrogate requests followed by unaffected full queries. Service ranges
also check whitespace and reverse spans. These assertions exposed byte-based
protocol token positions/lengths and nonempty results for interior empty spans.
All three output modes now convert token starts and ends using current snapshot
text before relative encoding; projection failures follow the shared typed
response error path. Empty or reverse service ranges return an empty stream.
