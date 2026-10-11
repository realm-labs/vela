# Native workspace symbols and no-match review

B10.41 owns only the Input/Render cells of `vscode/UX11/workspace-picker` and
`vscode/UX11/no-match`. Together with the earlier Outline/document-picker proof,
these finish UX11. All UX12 folding/selection interactions remain separate.

Use the complete authored workspace-symbol-declarations and ownership corpora:
31 and 60 rows, including nineteen distinct URI-only metadata rows in ownership.
Four sequential normal windows run both corpora in LF/CRLF. Their private
workspaces have no manifest; explicit test-owned fallback roots and schema
select the original source/module names. No imports, identifiers or static
expected rows are rewritten to fit a shared workspace. Only schema sourceSpan
bindings are removed in a cloned artifact, matching the existing installed
metadata-only oracle; original fixtures remain unchanged.

Each window verifies the registered platform, architecture, version, locale,
font/settings/display and installed server, plus its owned root configuration.
Windows/macOS use their native bindings and path separators. These are one
selected profile's sequential windows in one fresh bundle, with independent
future evidence on the other platform. They are not cross-profile aggregation.

The pinned VS Code 1.137.0 desktop bundle owns the UI policy. Workspace picks
use the complete qualified name, SymbolKind icon and container plus relative
file label, separated by ` • `. Metadata rows label the normalized empty path
`.` while their raw response retains URI-only `vela-schema:`. The SDK
aggregates by name/kind/URI, sorts picks by fuzzy score then lowercase name and
icon ID, and collapses a returned declaration range to its start on acceptance.
The full returned range is still compared on the actual wire. Source/schema
same-name functions remain distinct; class-versus-struct and duplicate-file
target indices have literal goldens. Metadata-only locations never become fake
source navigation claims.

The empty query compares complete ordered LSP rows and visits all 182 rendered
rows across the four windows. Tail-to-head cycles retain exact boundaries. All
144 source rows require physical pointer or Enter acceptance, exact active URI,
unchanged text/dirty state and the authored UTF-16 declaration-start caret. Each
destination has required PNG/ARIA artifacts. Sixteen first targets must be
absent from both current documents and the complete window's didOpen history.
Warm preview pages may close, so current-open state is not mistaken for never
opened. Complete raw observations, per-window logs and disk bytes are retained.

No-match checks include a known document function before the impossible query,
a completed fresh document-symbol worker request, literal `No matching editor
symbols` and physical Enter retaining the picker/source/caret. Workspace checks
cover all 78 authored single-token empty-name queries, including aliases,
locals, unresolved/dynamic ownership, builtins, detail-only text and Unicode
trivia. Each requires one fresh matched real client request and explicit `[]`,
the sole `No matching workspace symbols` placeholder with no icon/description,
unchanged source and its own PNG/ARIA. Multitoken GUI queries use a different
SDK name/description policy and cannot inherit whole-query backend empty claims.

Every positive source query first uses a deliberate impossible transition to
avoid a same-value cached picker request. These are distinct planned physical
inputs, never retries. Complete ordered response fields and fresh pairs are
checked for both transitions. The backend sorts by name then kind_name only:
same-name/kind file ties inherit opaque document IDs and can change when a
Windows editor opens a differently spelled drive URI. Only contiguous equal
name/kind ties are canonically compared; complete fields, multiplicity and all
non-tie boundaries remain exact. Original ordered raw pairs are retained, and
the SDK's independent URI/icon presentation ordering remains asserted. Negative
tests reject missing/extra rows, changed detail/range, unknown fields and moved
non-tie entries. This does not change the existing installed-provider oracle.
The bridge only sets up author-owned documents,
reads observations and finishes the window; it does not select accepted symbols.

Budgets are planned before capture: the ordinary 810s and trust 210s lanes retain
their previous budgets; the new picker lane uses four finite 120s windows and
480s aggregate proof budgets. All windows run sequentially. Final acceptance
requires fresh native 54, VSIX 27, Node infrastructure, fmt/Clippy/workspace and
automatic plus strict B09 regression on unchanged source/profile/server bytes.

The first retained prototype rejected an empty relative workspace-folder path
in the existing safeFile guard. The new session now uses its owned parent as
the inspector base, preserving the shared path safety contract.
The second retained prototype proves all initial no-match partitions, then
exposes the incorrect new strict-file-tie assumption. Cross-platform self-tests
also initially assumed every expected presentation field was OS-independent;
they now validate the new registered-platform field and only normalize the
reviewed source path separator. Prior52 contracts retain their exact checks.

The third retained prototype completes LF declarations, then rejects the new
metadata-description assumption. The pinned SDK label service falls back to
`uri.fsPath` and normalizes it with the OS path library. `vela-schema:` has an
empty path, so both Windows and POSIX normalize it to `.`; a container adds the
usual bullet prefix. The native oracle now asserts that exact SDK label while
the independently authored raw response still requires URI-only `vela-schema:`
with no source range. Metadata rows remain non-navigation observations.

The fourth retained prototype passes all four windows and all 1,418 physical
actions / 1,192 checks, but its bundle is rejected by the source-identity guard:
the review and driver README were edited during capture. It is behavioral
diagnostic evidence only. Complete acceptance must use a new frozen-source
54-proof run; the guard and failure are preserved without recertifying that
stale bundle.
