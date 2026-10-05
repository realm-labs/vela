# Installed folding provider smoke

Close only `folding/editor/smoke`. UX12 native folding Input/Render remains
independent and open. The production language server and runtime are unchanged.

Reuse eight independently marked rows from the imports, bodies, literals,
members, trivia and recovery reviews: six positive sets with 19 raw ranges and
two true empties. Node tests pin complete literal public line models (14 ranges)
and the Unicode import columns: 8/37 UTF16 versus 12/41 UTF8 bytes. Fixtures are
copied unchanged from reviewed source markers, never from provider output.

Materialize four fresh Unicode/space/percent roots for LF/CRLF and two-line
Unicode shifts, with unique package identities and actual main/helper files.
Preserve the shebang's first line when shifting. A separate on-disk prefix makes
every corpus replacement unsaved. Check disk, all eight dirty variants, and
actual close/reopen restoration; physical bytes stay unchanged. The helper owns
an empty result in every phase. Preserve the original first workspace folder.

Call the installed client's public `vscode.executeFoldingRangeProvider` three
times per document/phase. The primary VS Code command computes the folding
model: stable start-line ordering retains the first range at a shared start
line. The language client's folding converter preserves start/end lines and
kinds; the UI model combines coincident item/body starts. Independently authored
markers determine both complete expected public models and complete ordered
wire sets; no observed range is filtered or deduplicated by the test.
The public API uses the numeric `FoldingRangeKind` enum (Imports=2, Region=3),
while wire kinds are strings. The first real run exposed a test-side `.value`
mapping mistake; retain its failed results and traces, then explicitly map the
actual API enum and reject unsupported/missing kinds. Node assertions pin both
numeric mappings and reject the mistaken object/string forms. Raw public rows
are also retained before assertions, including partial failed observations.

Require fresh complete matched `textDocument/foldingRange` request/response
pairs after every invocation boundary, exact owned document parameters and
every UTF16 endpoint/kind. Validate all complete owned pairs when automatic
background folding overlaps the command, without choosing a passing response.
IDs cannot be reused across queries. Empty public arrays require real wire `[]`,
not null, missing providers, indentation fallback or errors. Require actual
open/change/close/workspace mutations with existing finite deadlines and quiet
settling; check effective text, EOL, model version and physical disk each time.
The effective editor folding settings must be enabled/auto.

The runner retains full per-root observations and client logs plus the actual
server trace. Eighty query observations cover 240 public invocations across
the four forms. Each action/observation is bounded; assertions are not retried.
Shared document lifecycle helpers preserve existing symbol assertions and
cleanup behavior. Existing installed and native checks remain present. Matrix
progress is shared, but Windows and macOS evidence is independently fresh;
B16 remains deferred. This does not close selection or native UX11/UX12 gates.
