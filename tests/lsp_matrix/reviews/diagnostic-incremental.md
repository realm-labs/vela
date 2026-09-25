# S11 incremental diagnostics

B05.15 uses a shared four-module fixture. The importer contains a persistent
Array method error; its helper first changes only a function body, then renames
the exported declaration. The importer then rebinds its import to the new
declaration. A relay imports the importer, while an unrelated module provides
an independent control. LF and CRLF run each state separately with a Chinese
and non-BMP prefix; protocol paths contain encoded Unicode, spaces and `%`.

Service assertions pin complete diagnostic sets, codes, messages, severities,
byte ranges and replacement candidates at each state. A body-only edit leaves
declaration/import fingerprints and the project index unchanged and invalidates
only the helper. Declaration and import edits change their respective
fingerprints, rebuild the project index and invalidate exact reverse dependents.
Old-generation diagnostic requests return `Stale` with no facts. Repeated
queries leave generation unchanged, and every live result equals fresh
analysis for all four modules.

Protocol assertions pin the exact set of published document URIs and complete
diagnostic objects, including UTF-16 ranges and related labels. The unrelated
file never republishes. Same-version invalid replacement text produces no
publication; an unknown cancellation notification produces no response. A
newer same-text update then republishes the current result, which still agrees
with a fresh server. `publishDiagnostics` is a notification rather than a
request, so request cancellation cannot cancel the diagnostic publication
itself; the test covers its effect on subsequent synchronization instead.
