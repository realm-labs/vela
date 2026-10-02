# Workspace symbol declaration review

B10.6 closes S1 positive service/protocol, repeat and UTF-16/CRLF/encoded-URI
protocol checks. S0, schema/import ownership, malformed recovery, lifecycle
states, installed workspace-symbol smoke and native UX11/UX12 remain open.

The authored corpus has four sources and 31 complete ordered rows: four files,
four modules and 23 top-level declarations. It reuses independently authored
declaration source/markers, adds real helper declarations and a nested same-named
file, and records separate workspace identities, kinds, details, containers and
full source locations. Public/private consts, hinted/unhinted values, local and
extern state, zero/untyped/defaulted/async functions, structs with fields,
unit/tuple/record enums, required/default traits and inherent/trait impls retain
their top-level owners. Imports, aliases, locals, parameters, source members and
variant uses do not manufacture additional workspace declarations. Three Widget
owners, two LIMIT declarations and two api.vela files keep separate locations.

Thirty-two query strings have authored ordered result IDs, not results derived
from a provider or from the query matcher. Whole/trimmed-empty results, ASCII
case folding, edge whitespace, qualified substring matches, nested scopes,
duplicate names/files, private/async/state/trait/enum names and exact empties
compare every row and every omitted optional field. Signature/type details,
source text, attributes and Unicode trivia are not name matches; interior spaces
and wildcard-like text do not broaden matching. Source members remain in document
outlines; this source-only corpus does not claim schema member behavior.

Every query repeats three times in LF/CRLF, with and without a two-line Unicode
prefix. Service compares complete byte-column results, canonical identities and
rendered display parts, with unchanged source records. Protocol compares complete
JSON-RPC envelopes from real TestServer dispatch against independent UTF-16
markers, checks all four disk sources, encoded Chinese/space/percent roots and
unchanged physical files. File/module ranges include the whole current source;
declaration locations preserve their complete source extents. The literal LIMIT
start is line 1 (or 3 after prefix), UTF-16 column 8 versus byte column 12.
Three Node checks pin corpus counts/partitions, literal query memberships and
exclusions, and marker/whole-file goldens through both line endings and shifts.

The service first passes. The initial protocol comparison fails because the old
workspace location projection copies byte columns directly. Projection now uses
the same immutable database snapshot as the symbol query and builds at most one
checked UTF-16 index per requested source. It rejects unavailable sources,
invalid URI/bounds and reversed ranges through the established projected-response
error path. Schema's existing vela-schema: location and optional detail payload
remain unchanged. A focused projection test rejects absent snapshot records and
invalid source URIs. Symbol projection lives in the existing focused symbols
module instead of growing the generic to_proto entrypoint.
The protocol JSON boundary guard names this exact projection module for the
existing workspace-symbol detail extension payload; unrelated modules still
cannot use JSON. The first full audit exposed the omitted boundary registration,
which is retained with the initial failed protocol comparison.

Final acceptance requires focused Rust tests, relevant all-target Clippy, fmt,
Node infrastructure, one frozen source with fresh installed VSIX23/native50 and
full audit plus strict B09 refresh. Preserve all previous accepted snapshots,
completed children and native contracts. macOS needs independent fresh evidence;
B16 remains deferred. This fixture never executes scripts or host state.
