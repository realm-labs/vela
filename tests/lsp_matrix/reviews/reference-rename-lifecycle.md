# Reference and rename source lifecycle

B04.28 established the source lifecycle matrix. B04.43 completes its sixteen
dirty, close-restore, dependency-change and dependency-delete protocol state
cells for references, highlight, prepare-rename and rename. Missing and stale
schema states remain separate obligations.

The independent four-file fixture has 16 explicit owner states, run under both
LF and CRLF with Chinese and non-BMP prefixes. It starts with an imported
`helper::increment`, a closed qualified consumer and a separate
`other::increment`. Unsaved helper changes shift its declaration; a disk write
while the overlay is open must leave the overlay authoritative. Closing the
helper exposes the disk target. An unsaved main import then binds to the other
owner, and closing main restores the helper binding. Deleting helper makes the
main call unresolved; reopening the importer publishes missing-dependency
diagnostics. Closing it and recreating helper restores the original complete
site set; reopening the importer clears those diagnostics.

At every state, service and protocol repeat exact references with and without
the declaration, document highlights, prepare range/placeholder and complete
rename edits. The oracle identifies the expected owner and enumerates every
site independently, including the other owner while main is rebound. The
service compares its incrementally updated database with a fresh database for
each state. Protocol compares the long-lived server with a fresh server opened
from the same current disk and overlays after every action. It uses actual
didOpen/didChange/didClose/didSave and watched
file notifications, a percent-encoded Unicode path, UTF-16 ranges, legacy and
versioned edits, and null versions for closed files. Unresolved states require
empty references/highlights and null prepare/rename. Deleting helper clears
its published diagnostics; opening the importer after deletion publishes the
missing-module codes, and reopening after recreation publishes an empty set.
The full workspace check also enforces the single `GlobalState` coordinator;
the protocol test driver uses the existing endpoint naming convention.

Remaining state cells, syntax-partition completeness and UX05/UX06 remain
before B04 acceptance.
Original B00-B03 snapshots stay fixed; macOS needs fresh evidence when used.
B16 remains deferred.
