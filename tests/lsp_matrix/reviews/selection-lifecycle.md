# Selection source/dependency recovery review

B10.36 maps `selection/states/recovery/protocol` to an independent literal
source/overlay model, cached CST and complete selection vectors. Eleven variants
run through 43 states and 46 operations in each Unicode LF/CRLF and shifted form.
The main file imports the helper; a separate unchanged file and an absent file
must retain their own whole query vectors throughout every transition.

Dirty main/helper changes include errors before and after healthy syntax,
repair, repeated damage, and simultaneous errors followed by separate repairs.
Disk changes hidden by overlays, closing onto damaged disk, closed-file repair,
deletion with and without a surviving overlay, recreation, physical rename,
version-reset reopen, and saving damaged/repaired text are distinct states.
`didSave` remains a no-op notification; the test independently writes actual
disk text before sending it. The final close restores the complete baseline.

Both Rust layers require every literal CST node's kind/extent and the selected
token's entire ancestry, including equal-span TypeHint/Literal nodes omitted
from the deduplicated range chain. Every state queries all five document IDs
three times with full duplicate/right-boundary/comment/EOF vectors and actual
empty vectors. Current and fresh databases and every previous immutable
snapshot retain exact sources, cached diagnostic ownership/spans, record counts
and absent schema facts. Queries cannot change generation or parse/project/HIR
counts. Protocol results compare complete typed response envelopes; physical
disk and encoded private-root URIs are checked independently.

Syntax publications have exact E_PARSE code, message, severity, source and
UTF16 range, or an exact empty syntax list. Every diagnostic URI is published
at most once per transition. Open/change/close, prior closed publication owners
and known deletions require one receipt. Writes to the never-opened rename target
retain the existing no-source-publication policy; save requires no receipt.
The fixture records these expected cardinalities before execution.

The original whole lifecycle test exposed a real stale diagnostic after close:
closing onto `fn broken(items:) {}` publishes `expected type annotation`, but
repairing that closed dependency through the watcher omitted its empty clear.
A separate regression independently fixes the literal damaged text and exact
UTF16 span at line 1, columns 18..24, then requires exactly one empty publication
after disk repair. Its original red is retained. Publication ownership must
survive overlay closure; current diagnostics still come solely from the current
database after the entire coalesced watcher batch. A set of previously closed
publication URIs records client ownership, without copying source/analysis facts.
Open overlays still win, removed owners clear, and metadata/source facts share
one typed publication per URI. Never-opened closed sources keep their policy.

The initial test's demand for a receipt from the never-opened rename target was
an author policy error, preserved separately from the confirmed closed-source
defect. Primary watcher policy and existing exact tests determine its correction.
Four independent Node tests pin literal state membership/counts, publication
policy, complete UTF16/byte geometries and equal-span ownership; no provider
output supplies expected selection ranges.

The existing accepted batches, earlier selection partitions, native/installed
contracts and independent macOS history remain intact. This child does not accept
B10; trivia, installed selection and UX11/UX12 remain pending. Fresh same-source
Windows native, installed editor, automatic matrix and strict B09 receipts are
required before recording the child checkpoint.
