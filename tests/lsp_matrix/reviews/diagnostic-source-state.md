# S0 diagnostic source states

B05.14 reviews the four S0 diagnostics cells across configured roots and
outside-root scratch files. The `diagnostic-source-state` fixture uses a valid
two-module disk project, a dirty main overlay, and a Unicode scratch path. The
service asserts the complete diagnostic objects, including byte ranges, labels
and replacement candidates; the protocol asserts complete publications with
UTF-16 ranges and encoded URIs. LF and CRLF run independently. Closing the
overlay restores valid disk diagnostics, while closing the scratch document
clears its diagnostics. An opened scratch source is analyzed even when a
workspace root is configured; it cannot change the valid main or helper result.
Live and fresh protocol servers agree on both dirty states.

The import fixture supplies multi-file dependency transitions. A changed
export changes the exact unresolved import; deletion removes stale private and
unused warnings. Moving the defining source to a different module path leaves
the old import unresolved without treating the new path as the old module.
Restoration returns the original complete diagnostic set. Service and protocol
assert the source owner and missing-module results with byte and UTF-16 ranges.

The schema lifecycle fixture supplies absent, replaced, invalid and restored
schema states. Service assertions pin the whole diagnostic sequence, message,
severity, byte range and candidate replacements; protocol assertions pin the
published UTF-16 sequence and fresh-server parity. Missing or invalid schema
keeps the independent Array method error, emits one controlled warning, and
does not invent host-field errors. A valid replacement changes the field
candidate from `level` to `rank` and restoration returns `level`.
