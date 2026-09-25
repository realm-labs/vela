# Unresolved and dynamic diagnostic/action states

B05.6 covers the four protocol state cells for diagnostics and code actions at
`unresolved` and `dynamic`. The shared Unicode fixture puts an unknown method
on a known `Array<i64>` receiver, a `levle` member on a source function with an
explicit `Any` return, and a separate `foobar` error in one document. Under LF
and CRLF, the server must publish exactly the two known-receiver diagnostics
with marker-derived UTF-16 ranges, error severities, codes and messages. The
dynamic member must not be diagnosed from guessed facts.

At the unresolved `frist` range the server must return exactly three ordered
quick fixes, each with one exact replacement in both edit projections and the
current document version. At the dynamic range it must return an empty action
array even when the request context contains both real diagnostics. Applying
the first fix must produce the complete expected source and leave only the
unrelated diagnostic. Requerying the repaired and dynamic sites must return no
stale actions. Existing editor interaction and URI/lifecycle tests cover
different axes; this test claims only these four protocol state cells.
