# Workspace symbol installed editor smoke

This closes only `workspace-symbol/editor/smoke`. Native UX11 picker Input and
Render gates remain independent and open. No script or runtime behavior changes.

## Authored corpus

- Reuse the reviewed workspace-symbol-declarations and workspace-symbol-ownership
  literals: four/eight source files, 31/60 complete rows and 32/49 queries.
- Materialize all sources in LF and CRLF in a fresh private Unicode/space/percent
  workspace. There is no `vela.toml`, added probe source or provider stub.
- Load the ownership schema's literal static facts as metadata-only. Removing
  authored sourceSpan setup never mutates the original fixture. Full backend
  source-bound schema behavior has separate service/protocol evidence.
- Query every authored positive, collision, alias, detail-only, builtin,
  unresolved/dynamic and empty partition three times in disk, dirty and restored
  phases. Dirty editors insert two Unicode prefix lines inside the authored
  whole-file marker; declaration ranges shift, file extents retain their start.
- Close every owned Vela document before switching fallback roots/schema or
  restoring disk. Physical files and schema bytes/facts remain unchanged.

## Installed boundary

The existing editor suite has many unrelated global symbols and a root manifest
that takes precedence over editor fallback settings. Run a second real VS Code
session, using the same installed VSIX and separate user-data directory, in the
manifest-free private workspace. Its first folder stays fixed. Public settings
select only the authored corpus scripts; actual configuration/open/change/close
messages must complete before each feature check. Only startup readiness retries.
Each provider assertion executes once with finite deadlines.

Call `vscode.executeWorkspaceSymbolProvider`; never register a test provider or
forge a request/result. Compare complete editor multisets (all names, kinds,
containers, URI/ranges and duplicate source/schema names). VS Code's workspace
command aggregates and orders provider entries. No observed entries are filtered
or deduplicated by the test. Require one fresh matched actual client request and
response for each repeat, exact query parameters and the complete ordered LSP
rows, including details and absent fields. An empty editor array alone cannot
prove a provider exists: the wire result must be explicit `[]`, not null/error.
Only URI spelling is normalized with the actual client URI type (Rust URL and
VS Code encode a Windows drive letter differently); original raw rows and every
other field are retained, and the complete ordered normalized response is checked.

Schema wire locations are exactly URI-only `vela-schema:` with no range. The
installed language client's zero placeholder editor Location is checked as a
conversion artifact; it is not a metadata source target or navigation claim.

The runner preserves both raw suite results, client/server traces and complete
query observations. It merges results only with identical source/server/platform
provenance and pinned VS Code version, rejecting duplicate names. Failed checks
are retained. The matrix discovers both explicit suite entry files. Node tests
pin corpus sizes, independent UTF16/byte goldens, dirty extents, metadata-only
collisions, multiset multiplicity, incomplete/stale/null/error wire pairs and
source/binary/platform/version/duplicate merge failures. Current Windows and
macOS evidence remains independently fresh; B16 stays deferred.
