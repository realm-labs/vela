# Document symbol lifecycle review

B10.4 closes protocol dirty, close_restore, missing_schema, stale_schema,
dependency_change and dependency_delete. Installed document-symbol interaction
and the other B10 features remain open. Service parity strengthens these six
protocol obligations without counting additional cells.

The independently authored fixture defines eight source variants across three
files and nineteen finite phases. Each phase specifies disk bytes, open overlays,
effective sources, schema status and complete ordered outline trees. Expectations
pin names, kinds, details, canonical source identities, declaration extents,
actual selections and parent-child ownership. They do not come from provider
results. Fresh fixtures are assembled from phase state maps, independently of
the replayed actions.

Actual didOpen/didChange requests replace the whole main-file outline, including
function names, field types and added members. A watched disk edit underneath
the dirty buffer remains hidden until didClose, which restores the newer disk
tree. Reopen, a second dirty edit, a second close and final restoration check
that stale names, fields and ranges never return. Watched dependency replacement,
deletion and recreation change that file's own declarations while the main
file's actual alias impl remains owned by main. Deleted source results are empty
and the current source database has no deleted record.

Schema facts are genuinely loaded: assertions pin one type and field, field
name, i64/String/bool display, and a bound source location. Replacement changes
the field name/type; unsupported format version 99 and deletion clear facts
and locations with the established diagnostic and Any fallback. Restoration
loads facts again. Outline trees remain authored from real source declarations,
including the origins file that supplies deliberately different schema spans.
Schema mutations cannot substitute a metadata name, field or signature.

Every phase compares repeated complete JSON-RPC responses with its authored
UTF-16 tree and an independently initialized server. Service checks exact texts,
parse success and repeated complete byte-range trees against a freshly assembled
database. Every previous immutable database/server snapshot is rechecked after
each later action, including its source text, schema status and entire tree.
LF and CRLF, encoded Chinese/space/percent private roots and a literal Chinese
plus non-BMP name golden (line 1, UTF-16 13..16 versus byte column 17) cover
range conversion and line shifts. Physical disk bytes are checked separately
from effective overlays. Test-owned temporary roots are removed after use.

Initial setup exposed an oracle error: serialized primitive names use `string`,
while display names use `String`. B10.3's schema fixture is also corrected to
`string` and now explicitly checks every loaded field displays `String`, so
unknown facts cannot silently satisfy its collision proof. Source outline
expectations stay fixed. The failed setup/compile/display logs are retained;
these corrections do not claim a production defect or change schema behavior.
The first full audit also rejected the harness field spelling `self.server`
under the existing single-owner architecture guard. The harness now explicitly
names its production `TestServer` field `test_server`; dispatch behavior and
the guard remain unchanged. Retain that failed audit and its first native/VSIX
artifacts, and recapture all final evidence against the corrected source.

Acceptance requires focused whole-tree tests, formatting, relevant all-target
Clippy, Node matrix checks and fresh Windows native/VSIX evidence followed by
the strict prior-batch gate on one frozen source. Preserve accepted snapshots,
prior children, all native contract hashes and independent historical macOS
evidence. macOS requires fresh evidence on that registered profile; B16 remains
deferred.
