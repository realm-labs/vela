# Watched-file invalidation

B09.1 owns eleven protocol obligations for watched-files: S0/S8/S9/S11
positive and negative, stale work, cancellation and client capabilities. Other
B09 features and installed workspace/trust interactions remain open.

The independently authored watched-files fixture pins module declaration/call
markers, shifted and unsaved declarations, removed imports, alternate roots,
invalid manifest byte spans and seven schema states. A Node golden checks literal
UTF-16 locations and UTF-8 manifest spans under LF/CRLF, including Chinese and
non-BMP prefixes. No provider result supplies an expected fact.

Source notifications cover create/change/delete, delete+create rename, five
duplicate/coalesced batches and missing-import repair. Each batch asserts the
entire diagnostic publication URI set and complete diagnostic facts, including
labels, candidates and repair hints. Deleted known closed sources receive one
empty clearing publication; intermediate batch states cannot publish. Repeated
definition requests compare complete responses with the marked current target.
A fresh server over the same final disk state independently repeats those facts.
Old database snapshots retain their exact original source bytes.

Disk replacement and deletion cannot supersede dirty version-19 overlays.
Complete workspace/source text and version remain authoritative until close;
close restores the current disk declaration or the exact missing-module facts.
Manifest changes switch source roots, invalid input preserves the last valid
graph and emits the complete project error, repair clears it, and deletion uses
the existing workspace fallback. A position in an excluded closed document
returns the existing exact InvalidRequest error; it is not a known unresolved
binding. Schema replacement, unsupported version, truncated input, missing,
empty and restored facts assert full metadata diagnostics and exact repeated
hover/null definitions. A fresh server agrees with each independently authored
state; the frozen original metadata remains immutable.

The malformed-notification oracle exposed two defects. FileChangeType accepts
unknown numeric values in the wire library, and the scheduler previously treated
them as upserts. The handler now validates every type before scheduling any
event: only Created=1, Changed=2 and Deleted=3 are accepted. Mixed valid/invalid
batches in either order, including -1/0/4, remain silent and preserve generation,
source bytes, overlay version and repeated navigation. The pre-fix failure is
retained in target/b09-1-oracle-review.log. After that fix, a no-op notification
exposed an initialization reload flag surviving a completed database refresh.
Every completed refresh now consumes that flag; empty, unrelated and absent-file
events preserve the current generation and exact navigation while retaining the
existing publication policy. Both failures preceded the corresponding fixes;
their independent assertions were retained.

Real queued references tasks are held before and after worker completion across
watch mutations. Their old results publish only the full ContentModified error.
Typed integer/string cancellation IDs isolate targets; matching cancellation
takes priority over stale generation and publishes only RequestCancelled. New
requests return the shifted current target. Definition itself is synchronous;
references use the actual worker lane and the existing five-second receive bound.

Three client profiles retain exact source/publication behavior; progress-capable
clients assert complete begin/end notifications. The existing exact initialization
matrix additionally checks complete capabilities and conditional watcher
registration, enabled/disabled policy and encoded owned root patterns. Reusing
that proof does not turn candidate discovery into coverage.

Fixtures own canonical temporary roots, marked paths and cleanup. Fresh installed
VSIX/native evidence must match this child source and server binary on the current
registered Windows profile. macOS uses its own fresh captures when resumed;
historical accepted snapshots and shared progress are preserved. B16 is deferred.
