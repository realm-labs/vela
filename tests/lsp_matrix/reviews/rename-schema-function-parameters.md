# Source-backed schema function parameters

B04.26 advances the S5 positive/negative service and protocol partitions for
references, highlights, prepare-rename and rename. These broad cells remain
unreviewed until the complete B04 gate.

The expanded schema-function fixture has 23 queries and six ownership groups,
five of which are writable. The explicit source span for `host::grant` points to
`helpers::grant(amount: i64)`. A bare call in a closed consumer and qualified
calls in another file use named `amount` labels. The oracle separates that
parameter from the caller's same-spelled local, a same-named source function,
unknown labels and a metadata-only schema function. Renaming `amount` to
`bogus` is rejected because an existing unresolved label on the same callable
would become owned. All writable groups undergo whole-file applied edits,
schema reload, exact requery and restoration.

Both service and protocol assert exact reference sets and kinds, highlights,
definitions, prepare targets and complete rename edits under LF/CRLF. Protocol
also checks UTF-16, encoded paths, open/closed versions, both edit forms and
schema ABI markers. The initial empty parameter reference set is retained in
the fixture. Callable parameter resolution now joins a schema callee to a HIR
function signature only when its explicit source span exactly matches the
function name span and its terminal name matches the source declaration.
Unrelated origin stubs and metadata-only callables cannot lend parameters.

Schema method parameters, broader semantic/lifecycle partitions and UX05/UX06
remain pending. Existing B00-B03 snapshots stay fixed; fresh macOS evidence is
required when that machine is used. B16 remains deferred.
