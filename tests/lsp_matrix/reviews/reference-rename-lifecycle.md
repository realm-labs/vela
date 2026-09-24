# Reference and rename source lifecycle

B04.28 advances the dirty, close-restore, dependency-change and
dependency-delete protocol state cells for references, highlight,
prepare-rename and rename (16 IDs). These broad state cells remain unreviewed
until their remaining source/schema and interaction cases are complete.

The independent four-file fixture has 13 explicit owner states, run under both
LF and CRLF with Chinese and non-BMP prefixes. It starts with an imported
`helper::increment`, a closed qualified consumer and a separate
`other::increment`. Unsaved helper changes shift its declaration; a disk write
while the overlay is open must leave the overlay authoritative. Closing the
helper exposes the disk target. An unsaved main import then binds to the other
owner, and closing main restores the helper binding. Deleting helper makes the
main call unresolved; recreating it restores the original complete site set.

At every state, service and protocol repeat exact references with and without
the declaration, document highlights, prepare range/placeholder and complete
rename edits. The oracle identifies the expected owner and enumerates every
site independently, including the other owner while main is rebound. The
service compares its incrementally updated database with a fresh database for
each state. Protocol uses actual didOpen/didChange/didClose/didSave and watched
file notifications, a percent-encoded Unicode path, UTF-16 ranges, legacy and
versioned edits, and null versions for closed files. Unresolved states require
empty references/highlights and null prepare/rename.
The full workspace check also enforces the single `GlobalState` coordinator;
the protocol test driver uses the existing endpoint naming convention.

Recovery, dynamic, missing/stale schema, stale-version and cancellation state
cells, syntax-partition completeness and UX05/UX06 remain before B04 acceptance.
Original B00-B03 snapshots stay fixed; macOS needs fresh evidence when used.
B16 remains deferred.
