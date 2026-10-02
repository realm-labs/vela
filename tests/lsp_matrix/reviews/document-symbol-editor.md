# Installed document symbol review

B10.5 closes `document-symbol/editor/smoke` through three installed VSIX checks:
complete declarations/ownership, all recovery partitions, and source lifecycle.
Native UX11 Outline/picker and UX12 folding/selection interaction, workspace
symbols and the other B10 features remain separate open requirements.

The provider uses the existing independent source/marker/whole-tree oracles.
VS Code's public DocumentSymbol model has zero-based kinds and normalizes absent
details/children to empty strings/lists. Every ordered tree comparison includes
names, kinds, details, complete extents, actual selections and recursive children.
Empty public command results may be `undefined`; each repeat also requires a
complete matching installed-client request and exact wire `[]` after the current
model's open/edit boundary, excluding absent providers, nulls, errors and
incomplete logs. VS Code's OutlineModel caches by document version; the three
public queries may reuse that current pair, never a previous model's response.
Hierarchy and selections must fit
their parents. Canonical source identities
are not exposed by this API; their service assertions remain in B10.1-B10.4.

Declaration proof pins all 19 roots/33 nodes. Member/import ownership pins eight
files with 25 roots/46 nodes, including exact imports-only empties, real alias
impl declarations, variants, traits and source/schema collisions. Every file is
checked on disk, after an unsaved two-line Unicode prefix, and after actual close
and disk restoration, under LF and CRLF. Actual temporary workspace folders load
the private package manifests; an independent type-definition setup probe verifies the
source alias still selects the real Widget URI and its marked name range.
An independent hover setup probe pins the loaded `host::Box.value` String fact.
Installed metadata deliberately omits source spans rather than guessing IDs;
source-backed metadata ownership remains covered by the Rust fixtures.

All 56 independently authored malformed/recovery cases run at both line endings.
Each damaged source has a Unicode line prefix, then repairs to the original
source before the next damage. Empty/trivia, missing/misplaced names, incomplete
headers/members/types/defaults, malformed neighbors and Any/unresolved owners
retain precisely their authored trees. Closing restores the physical baseline.
Eight Node oracle/trace checks pin literal kind/selection/byte goldens, complete counts,
LF/CRLF line shifts, all recovery and state maps, non-mutating metadata setup,
rejection of empty selections/escaped child extents, and complete empty wire
pairs with URI/id boundaries, LF/CRLF, null/error and incomplete-log distinctions.

Thirteen source phases reuse B10.4's independent disk/open/effective maps: two
dirty/close cycles, disk edits under overlays, dependency replace/delete/recreate,
reopen and final restoration. Actual file writes await completed watcher events;
actual close awaits didClose. Deleted resources are physically absent and public
provider resource acquisition rejects them; this does not substitute for the
backend's exact empty deleted-source results. The six schema-action phases and
immutable/fresh-server parity retain their B10.4 service/protocol proof rather
than being claimed from this installed source-only sequence.

Each complete response repeats three times. Buffer text and physical disk bytes
are checked independently, and queries cannot edit either. Encoded private roots
include Chinese, spaces and percent signs. Observations retain independently
authored expectations, actual projected trees, mutation completion receipts and
client protocol logs. Each request/readiness wait has a 15-second budget;
finite stage budgets cover the explicitly enumerated corpus. Added workspace
folders are removed, trace settings restored, and test documents close through real editor
commands. The fixture remains static analysis and never runs scripts/host state.

Initial failures are retained. A Node golden first mixed UTF-16 String indices
with byte offsets; its byte-column calculation now uses UTF-8 Buffer. First GUI
cleanup passed a Windows backslash path to the safe relative-path guard and
masked an original close failure. Cleanup normalizes native separators and
preserves original plus cleanup errors. Opening through `openTextDocument` did
not yield actual didClose in this run; real editor opens now produce verified
close notifications. The next GUI setup inherited the earlier UX04 schema and
its known-field probe failed. Explicit editor roots/schema could not override
the primary package manifest under the established configuration contract.
Adding a root to the original folder window stalled its workspace conversion.
The runner now starts with a private one-folder `.code-workspace`, retaining the
original first folder/settings so added roots do not convert the window.
Actual added workspace folders now load private manifests with their schema,
preserving importer context and known metadata; neither source
trees nor provider expectations were weakened. Failed GUI results cannot accept
their automatically started audits, which are stopped and retained before the
next exploratory run. The fifth run reached imports-only outline queries and
confirmed wire `[]` while VS Code's public command returned `undefined`; the
helper now supports that conversion only with complete actual empty pairs.
The sixth run demonstrated OutlineModel caching: requiring a new transport
request for each command was incorrect. Evidence now starts before the current
document's real open/edit and pins its model version through all three queries.
The seventh run passed declarations/ownership and all recovery, then exposed
VS Code's deleted-resource error spelling, `Unable to resolve nonexistent file`.
The rejection guard accepts this spelling and separately requires the actual
deleted owned path, so an unrelated resource/provider error cannot satisfy it.
No production fix is inferred from these harness errors.

Final acceptance requires one frozen source, complete fresh Windows native and
installed VSIX results, full LSP/Node audit and strict prior-batch refresh. Keep
all fifty prior native contract hashes, accepted snapshots and completed children
unchanged. macOS needs its own fresh evidence when used; B16 stays deferred.
