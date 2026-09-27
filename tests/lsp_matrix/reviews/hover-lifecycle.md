# Hover source and schema lifecycle

B07.17 owns nine `hover/states/{dirty,close_restore,recovery,unresolved,dynamic,
missing_schema,stale_schema,dependency_change,dependency_delete}/protocol` cells.
Exact independent tests are
`hover::lifecycle_states::hover_state_matrix_preserves_current_facts_through_source_and_schema_lifecycle`
and `tests::hover::lifecycle_states::hover_state_matrix_preserves_current_facts_through_source_and_schema_lifecycle`.
Only the protocol identity maps these protocol cells; service proof supplements it.

`hover-lifecycle` authors 34 queries through 30 finite phases and five physical
inputs. LF/CRLF and positive token start/interior versus null start positions
yield 3,748 positions on the incremental instance and the same on a separately
rebuilt instance per layer, all repeated. Optional independently marked definition
or null assertions add 1,836 positions per instance, also repeated. Whole hover
metadata/identity at service and whole Markdown/UTF-16 ranges or JSON null at
protocol must match authored phase expectations before fresh equivalence.

| State partition | Observable independent assertions |
|---|---|
| Disk, dirty and close | Query a closed caller and unopened definition. Open/shift the caller with Unicode unsaved lines; hover ranges and lexical/capture identities use the overlay. Open/edit the definition body, then replace signature, field, method and docs with unsaved String facts and moved declaration markers. A bool disk replacement behind an i64 overlay cannot replace its current hover. Closing the definition selects the current bool disk facts, and final caller close restores its original coordinates. |
| Recovery and unresolved | A missing-paren declaration before healthy definitions retains current String signatures, fields, docs and targets while parser errors remain present. Repair restores baseline facts. An unresolved caller callee and named label are null; healthy receivers, source members and host facts remain available. Repair restores exact ownership. An unrelated malformed disk neighbor has diagnostics without contaminating caller hovers. |
| Dependency change/delete | Physical watched writes, deletions and recreations invalidate a closed imported module. Removing it clears functions, named labels, fields, methods, returned methods, variants and definition ranges; typed source locals retain unknown and unresolved hint leaves retain Any without guessed identity. Recreating restores exact facts/targets. A second deletion while its dirty overlay remains open retains String facts; closing that deleted overlay removes them. |
| Dynamic/unknown | Any and unknown caller parameters retain exact local facts and physical identity throughout every phase; suffixes cannot borrow same-spelled registered types. Known source and host Any-return functions keep their callable facts but their members remain null. Changing the source factory return hint to Any clears its chained member and target despite the concrete body. Replacing a host factory's concrete return with Any removes its formerly known chained method while its new callable docs/type remain available. |
| Schema replacement | Watched schema writes replace field/method/callable types and docs and remove old returned-receiver facts. Separate member, type and function removal steps reject stale metadata and named-label contracts; unavailable hints/parameters degrade independently. Malformed JSON and physical deletion remove all registered facts, publish controlled `schema::unavailable`, preserve source/stdlib facts and never retain old docs. Repair clears unavailability and restores baseline metadata. |

Actual typed DidOpen/DidChange/DidClose and DidChangeWatchedFiles notifications
feed the live server. Changed existing, created and deleted files use the correct
event types. Each fresh protocol instance uses a new encoded Chinese/space/percent
root, current disk inputs and independently opened current overlays. The service
assembles source snapshots without production filesystem IO. Known absent Vela
files additionally disappear from SourceDb. Selected recovery files assert actual
parser/lexer error presence; every caller query asserts its own parse state.

Caller publication expectations are authored separately from metadata. Opening
an unchanged definition and changing only its body do not invalidate caller
exports and therefore do not republish caller diagnostics. Caller edits, changed
definition exports and watched file/schema events publish the affected caller;
schema unavailability appears once and clears on repair. These checks inspect
actual notifications, not only a queried diagnostic model. Disk-backed close
publishes current disk diagnostics; scratch close's empty policy does not apply.
The malformed declaration uses the primary CST missing-delimiter contract rather
than assuming a lazily lowered expression error is a CST parse diagnostic.

Only authored actions change disk files. Queries/fresh comparisons preserve every
current physical input and every physical absence. Owned temporary roots and
file mutations are checked before cleanup/deletion. Final disk bytes and complete
hover results equal baseline and all overlays are closed. No expectation comes
from a provider, and no scripts or host values are executed/read.

This closes the nine hover state cells, without claiming B15's cross-feature
scope, cancellation/generation scope, or full UX10 input/render coverage.
Windows/macOS gates require independent fresh evidence on one registered profile.
