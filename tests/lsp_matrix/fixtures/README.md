# Shared fixture corpus

JSON strings preserve line endings and Unicode. `marker-golden.json` contains
hand-authored stripped text, byte offsets, UTF-16 positions and malformed cases;
neither parser generates its expected coordinates. The Rust support module is
included under `cfg(test)` in both LSP crates. The installed VSIX runner imports
`scripts/lsp-matrix/fixtures.js` and consumes the same lifecycle JSON.

Marker syntax is reserved in fixture input:

- `[[name]]` identifies a zero-width cursor.
- `[[name:start]]text[[name:end]]` identifies an exact range.
- Names use lowercase ASCII letters, digits and hyphens, starting with a letter.
- Names are unique within each document. Proper nesting is allowed; duplicates,
  unclosed/crossing ranges, invalid names and markers splitting CRLF are errors.

Positions are computed after stripping markers. `byte` is an absolute UTF-8
offset; `line` and `character` are zero-based UTF-16 coordinates. Service APIs
use byte columns, derived independently from the same absolute offset; protocol
and editor APIs use the UTF-16 position. Oracles never call production LineIndex
or providers to obtain expected ranges. Edit helpers reject reversed/overlapping
ranges, split surrogate pairs and out-of-bounds positions before applying edits.

Complete semantic-token oracles normally use range markers. They may use a
cursor marker before a token whose literal `[` would collide with the following
range marker. In that case the independently authored `text` determines its
byte/UTF-16 length; the oracle still checks exact source text and scalar bounds.

`semantic-token-members.json` pins complete source/schema member, constructor
and variant streams, including explicit label versus shorthand binding roles
and negative same-named source/schema ownership collisions.

`semantic-token-calls.json` pins complete call and argument streams across
source/schema/stdlib functions and methods, named/default/reordered positions,
callback values and conservative unknown/dynamic/collision boundaries.

A workspace has `version`, `id`, `files`, ordered `actions`, and an independent
`oracle`. Each relative file path rejects traversal, absolute/drive paths and
backslashes. `open`, `change`, `save`, `close`, disk `write`, and disk `delete`
actions distinguish overlays from disk state. A disk change cannot replace an
open overlay; close restores current disk, and save writes the current overlay.
Missing or invalid action preconditions fail. Materialization requires a new
isolated root and refuses to overwrite an existing directory.

`shared-unicode-lifecycle.json` drives eleven service/protocol transitions for
both LF and CRLF. Its exact target ranges are authored in `afterEachAction`.
The installed extension also reads this corpus and checks initial, unsaved and
close-restored target ranges and source text. These are fixture/provider proof;
workbench Input/Render routes are separately required.

Navigation query oracles name each method's exact marker (or explicit null).
`target-file` defaults all methods to one file; optional `target-files` entries
keyed by `definition`, `declaration`, or `type-definition` override it when a
local parameter and its declared type live in different files. The service and
protocol consumers independently project those targets. `knownCallables`
checks builtin callable resolution before source-navigation null assertions,
so an unresolved fixture cannot accidentally prove the builtin policy.

An optional `source-symbol` checks the independently authored qualified member
identity on service definition/declaration results, alongside exact locations.
Protocol navigation has no symbol field and continues to assert its full URI
and range response.
