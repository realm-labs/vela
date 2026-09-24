# Source-backed schema method parameters

B04.27 advances the 16 S5 positive/negative service/protocol reference,
highlight, prepare-rename and rename cells. The broad cells remain unreviewed
until the complete B04 gate.

The expanded schema-method fixture has 33 queries and ten ownership groups,
nine writable. Explicit source spans link `host::Counter.add`,
`third::Counter.add` and `host::Measure.add` to separate source functions with
same-named parameters. A closed consumer, a returned receiver and a closure
contribute named labels; the caller's same-spelled local and an ordinary source
method retain separate ownership. Unknown labels, metadata-only methods and
unknown receivers have no invented parameter target. Renaming a host parameter
to an existing unknown label on that method is rejected while other owners may
use that spelling.

Both layers repeat exact references, kinds, highlights, definitions, prepare
targets and complete edits under LF/CRLF. Each writable group is applied,
compared against whole-file expected text, parsed, requeried and restored.
Protocol also checks UTF-16, encoded paths, document versions, both edit forms
and schema ABI markers. The initial empty method-parameter reference set is
retained as the reproducer. The shared callable resolver now accepts a schema
method or trait-method source span only when it matches a HIR callable name span
and the terminal method name. Unrelated origin stubs still have no parameter
target.

An independent source-shape regression also points schema source spans directly
to a HIR impl method and a required trait method, checking the owner-specific
parameter lists and required-signature flag.

Remaining semantic/lifecycle partitions and UX05/UX06 still precede B04
acceptance. Existing B00-B03 snapshots stay fixed; macOS requires fresh
evidence when used. B16 remains deferred.
