# Local rename scope and capture slice

B04.03 advances the four positive/negative `rename/syntax/S2` service/protocol
cells. They remain unreviewed until the other syntax and edit-ownership partitions
are covered; this slice does not certify B04 or the native rename UI.

The shared fixture specifies 35 allow/reject cases, run on LF and CRLF with
Chinese/non-BMP text before queries. It covers same-scope conflicts, sibling and
nested blocks, distinct functions, lambda parameters and captures, closure
creation before inner declarations, loops and iterables, match arms and guards,
tuple bindings, parameter defaults (including later parameters), no-op renames,
and a longer replacement identifier. Capture negatives include source imports,
stdlib calls, unresolved names/member receivers, and qualified path heads. Bare
Map keys and qualified terminal names are positive non-lookup boundaries.

Both drivers independently specify every target declaration and use marker and
probe the exact declaration target of target-owned and unrelated references.
They compare complete reference sets with and without declarations, exact
prepareRename ranges/placeholders, and allow/reject policy at every target site.
The service applies every successful result and compares the entire changed
file. The protocol compares both complete workspace edit forms, including the
open document version and UTF-16 ranges, applies the edits, sends didChange,
rechecks owners/reference sets, restores the original document and checks again.
Changed sources parse successfully. No unrelated file may receive an edit.

The first reproduction showed a valid sibling-scope rename rejected by the old
whole-BindingMap name check. The new check uses actual HIR scope ancestry to
compare lookup after substituting the proposed local name. It follows a closure
to its creation scope and a parameter default to its owning parameter scope.
Let bindings become visible after their initializer; for and match bindings are
visible through their actual HIR scopes. Same-scope conflicts retain the existing
rejection contract. A no-op keeps the current binding graph unchanged.

Additional receiver cases exposed missing capture checks for unresolved field
and method receivers, which the binder does not place in its ordinary unresolved
reference list. The check now includes HIR field receivers while excluding literal
Map keys and recorded task/service capabilities. Scope traversal is bounded by
the indexed scope count; there is no source reparse, runtime execution or mutation.

Remaining B04 work includes shorthand/pattern and named-argument edit ownership,
other source/member/schema partitions, lifecycle coverage and UX05/UX06. Broad S2
cells and the overall feature counts are unchanged. Original B00-B03 acceptance
snapshots remain fixed; fresh current-profile regression evidence is separate.
macOS needs its own fresh run when used, and B16 remains deferred.
