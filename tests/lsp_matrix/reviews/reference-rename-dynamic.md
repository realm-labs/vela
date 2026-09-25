# Dynamic Any return boundaries

B04.41 closes the four `states/dynamic/protocol` cells for references,
document highlights, prepare rename and rename. The protocol test uses the
independent `navigation-dynamic` fixture, whose navigation matrix also asserts
the exact source and schema callable origins before treating downstream
members as dynamic. The schema contains real `item` fields and `get` methods,
and the source defines corresponding members, so a guessed owner would yield a
detectable wrong result.

The twelve negative cursor positions cover direct source and imported
functions returning `Any`, source methods returning `Any`, schema functions and
methods returning `Any`, local variables bound to those returns, a shadowed
callable and both field and method suffixes. Each position must return empty
references with and without the declaration, empty document highlights and
null prepare/rename responses. Seven nearby source/schema callable and member
positions remain resolved controls. The fixture runs with LF and CRLF, Chinese
and non-BMP prefixes, and percent-encoded workspace URIs. The existing schema
lifecycle matrix separately checks dynamic isolation while schema artifacts
are replaced, invalidated, deleted and restored.

The four syntax-level dynamic/ownership cells remain open until their other
partitions are reviewed. This state slice does not accept B04 or claim B16
environment expansion.
