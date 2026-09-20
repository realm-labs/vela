# Local shorthand rename slice

B04.04 advances the positive service/protocol cells for rename S4 constructors
and S6 patterns. Those broad cells remain unreviewed; this does not close B04 or
certify field-target, schema, named-argument or native rename workflows.

Sixteen independent before/after fixtures cover parameter/local constructor
shorthand, captured and repeated constructors, explicit field values, enum record
constructors, record/enum patterns nested in tuple lets, guarded match patterns,
explicit pattern bindings, for patterns, a pattern binding reused in a constructor,
no-op renames, unrelated shadows, nested constructors and a new local name that
already exists as a different field label. Each runs with LF/CRLF and Chinese plus
non-BMP text before queried and edited identifiers.

The oracle spells out every original edit span, replacement string and complete
resulting source. At every target-owned site, both layers assert prepareRename
range/placeholder and the full edit plan. Both independently apply actual edits,
compare the entire file, parse it, check the exact local declaration target, and
compare complete reference sets with and without declarations. Protocol checks
both workspace edit forms, UTF-16 coordinates and the open document version,
sends didChange, then restores and checks the original document again.

The initial reproduction replaced shorthand `value` with `renamed_value`, which
also changed the field label. Rename now indexes canonical HIR shorthand labels
and expands only affected local sites to `value: renamed_value`, including pattern
binding declarations. Explicit labels, unrelated shadows and no-ops retain their
original form. Field metadata and the local variable remain separate identities.
No grammar, runtime binding or host behavior changes.

Fixture authoring also inspected the current CST let boundary: a left side that
starts directly with an identifier is not emitted as a record pattern. The
positive let fixtures use existing nested tuple-pattern structure; direct nominal
let patterns are not certified here. Match and for records use their existing
pattern nodes.

B04.05 continues named-argument and field-target edit ownership, followed by the
remaining semantic/lifecycle and UX05/UX06 gates. Original B00-B03 snapshots remain
fixed and get separate fresh current-profile regression evidence. macOS needs its
own fresh run when used; B16 remains deferred.
