# Unresolved reference and rename state

B04.42 reviews the four `states/unresolved/protocol` cells for references,
document highlights, prepare rename and rename. All 26 shared
`reference-rename-*` coordinate fixtures contain explicit no-group queries.
The protocol driver treats those as independent empty-site oracles and, under
both LF and CRLF, compares the complete reference set with and without
declarations, the complete same-document highlight set, and null prepare and
rename responses. Every fixture also has owned controls, so a missing target
cannot silently borrow a similarly named source or schema owner.

| Missing or non-target partition | Fixture evidence |
|---|---|
| Local/parameter and lexical exclusions | Baseline coordinates, named/method/required parameters and default bindings distinguish missing names or labels from live locals, signatures, literals, comments and keywords. |
| Source members and constructors | Record, tuple and variant field matrices reject missing labels, wrong owners and invalid constructor forms beside owned read/write sites. |
| Imports and visibility | Import boundaries, imported values/types, source/private variant imports and ambiguity removal reject missing/private paths and unowned stdlib terminals beside exact imported controls. |
| Schema source and metadata | Schema function, method, field, import, type and lookup matrices reject unknown owner/label/path combinations while keeping source-backed and metadata-only identities separate. |
| Variants and patterns | Schema variant, import, lookup, ambiguity and capture matrices reject unknown constructors, patterns, import terminals and ambiguous short names beside exact owned variants. |
| Incremental and malformed neighbors | Source lifecycle deletes/recreates a dependency while a same-name decoy remains; schema lifecycle invalidates/deletes/restores an artifact; recovery damages and repairs neighboring syntax. Their unresolved queries remain empty/null and current diagnostics are checked where applicable. |

The schema lifecycle tests additionally assert empty highlights for the
unknown-module call at every schema state. The service uses the same shared
coordinate fixtures and now checks that unknown highlight boundary too. This
review closes the unresolved *state* cells; syntax ownership and the separate
missing/stale schema state cells retain their own acceptance requirements.
