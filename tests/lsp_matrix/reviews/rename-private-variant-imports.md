# Private source variant rename and imports

B04.23 advances the 48 S2/S4/S8 positive/negative service/protocol
reference, highlight, prepare-rename and rename cells. The broad cells remain
unreviewed until their complete batch gate. Definition and type-definition
assertions protect shared navigation behavior.

The independent fixture has 53 queries across eight ownership groups. A private
source enum variant is referenced from its declaration, a direct import, an
explicit alias, an enum type alias, a value use and a pattern. Exact reference
sets include the alias binding and use. Rename changes the variant declaration,
the underlying terminal of both imports and the direct/qualified uses; retained
alias spelling stays intact. A local parameter with the same alias spelling has
its own independently applied rename. Public source variants stay non-renamable;
schema and metadata-only owners remain separate despite identical qualified
metadata names.

Distinct rejection probes cover a sibling variant, an existing import binding,
a local parameter at an editable use and an unresolved qualified path that
would become a reference after rename. Source enum rename now checks both
lexical capture and counterfactual path ownership. Paths originally owned by
the target must still resolve to its renamed identity; all other paths and
imports must keep their identity or unresolved state.

Both Rust layers compare complete reference/highlight sets, apply five writable
groups, compare whole files, regenerate schema metadata, parse, requery and
restore. Protocol also checks LF/CRLF, UTF-16, encoded paths, versions, both
edit forms and ABI markers. Initial failures for omitted private references,
local capture and reverse capture are retained in the Windows evidence logs.
Ambiguity-removal, remaining schema parameter/lifecycle coverage and UX05/UX06
remain before B04 acceptance.
