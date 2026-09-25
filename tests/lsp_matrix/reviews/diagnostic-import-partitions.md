# S8 import diagnostics

B05.12 reviews the S8 diagnostics partition in the shared
`diagnostic-import-partitions` fixture. The positive paths include a public
cross-module function through an alias and a registered, source-backed schema
function through an alias. Neither produces a false unresolved or unused import
diagnostic. A public but unused import produces one warning.

The negative paths include a misspelled export with a specific suggestion, a
private declaration with a related label at its owner, and a missing module.
Both layers assert the complete diagnostic sequence, codes, messages,
severities, labels, candidate/repair metadata and exact source ranges. Service
positions are byte columns; protocol positions are UTF-16 columns after a
Chinese and non-BMP prefix in a percent-encoded workspace URI. LF and CRLF
repeat the same contract.

Changing the defining export reverses which spelling is unresolved and makes
the newly valid but unused alias visible. Deleting the module reports one
unresolved-module error per affected import and clears the old private/unused
diagnostics. Restoring the module restores the original set. Long-lived
protocol publications are checked against a fresh server in each state.
