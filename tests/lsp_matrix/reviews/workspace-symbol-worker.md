# Workspace symbol version, cancellation and worker review

B10.11 closes protocol stale_version and cancellation. S0/S11 completeness and
installed/native picker remain open. Use the independently authored complete
workspace lifecycle corpus and a separate literal worker contract inventory.

Equal version1 and lower versions0/-1/i32::MIN are ignored after opening version1.
Even pending real workspace-symbol tasks completed before or after the ignored
update remain publishable. Generation, complete source/schema/outline facts,
physical disk and full current query sets stay unchanged, with no publications
from the rejected update. A newer version2 changes the authored names/ranges,
matches a freshly initialized coordinator and retains the old immutable snapshot.

Cancel numeric boundary IDs, zero, Unicode/empty strings and string20 for whole,
schema-only and no-match queries, both before worker completion and after a
completed result is held. Repeated cancellation yields exactly one RequestCancelled
JSON-RPC error and no symbol result. Cancellation changes no source/schema state
or generation; the same ID immediately succeeds, late cancellation stays quiet,
and that ID can succeed again. Malformed params, unknown IDs and numeric20 versus
string20 cannot poison an unrelated pending result or a future valid request.

Replay every subsequent source/schema lifecycle phase while numeric20 and
string20 tasks are pending. Cancel numeric20 before/after the generation change;
its result is discarded with the exact cancellation error, including when stale.
The string20 old result produces no publication and gets one retry. That real
retry must publish only the current complete authored workspace rows. Check
fresh-server parity, physical disk/schema bytes, previous source/outline/schema
snapshots and whole frozen workspace ownership after every later action.

A separate two-change sequence holds the retried result, advances generation
again, and requires one ContentModified error with no second retry or stale
success. Cancelling that already retried request still takes priority over
exhaustion. Numeric/string ID reuse succeeds with the current full authored set.
All scheduling uses production TestServer queue/receive/publish paths, with the
existing five-second task bound and no timing sleeps. LF/CRLF and two Unicode
shifts reuse independent UTF-16/byte ranges and encoded private roots.

The first compile used an incorrect new-test TaskOutcome::Published spelling;
the existing enum is Completed. The next red run exposed an incorrect test
assumption of immediate ContentModified: workspace-symbol is already retryable
once. Tests now pin that established policy, the current complete retried result
and exact error after retry exhaustion. Both exploratory logs remain artifacts;
no production retry/cancellation policy changes or defects are claimed.

The lifecycle checker is now crate-visible only in tests to retain all original
whole-outline, bound-schema and disk assertions. Node pins worker contract IDs,
versions, malformed params, exact errors, one-retry budget and all nineteen
phase/query memberships. Acceptance requires focused workspace-symbol and
original cancellation regression, Node infrastructure, fmt/relevant all-target
Clippy, frozen Windows VSIX23/native50, automatic full audit, strict B09 refresh
and checkpoint tests under process-local Rust1.97.0. Preserve all prior records,
all50 native contracts and independent macOS history; macOS needs fresh evidence
when used and B16 remains deferred.
