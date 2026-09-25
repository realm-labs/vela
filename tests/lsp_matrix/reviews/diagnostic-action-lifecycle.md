# Diagnostic and quick-fix overlay lifecycle

B05.2 verifies eight protocol environment and state cells for diagnostics and
code actions. The B05.1 method-typo fixture is materialized in an isolated root
whose Chinese, spaces and percent sign require an encoded file URI. LF and CRLF
run the same sequence. Marker positions independently determine expected UTF-16
ranges, and the diagnostic oracle fixes the complete code, message and severity
set. Every action check requires the exact three candidate titles, kinds, URI,
single edit, replacement, range and document version in both workspace-edit
forms.

The client opens the disk text at version 1, repeats an identical request, and
sends the same text at version 2. Diagnostics must be identical and actions must
use version 2. A version-3 unsaved edit lengthens the Unicode comment before the
typo, moving its range. Both diagnostics and all quick fixes must follow the
overlay; repeated requests must agree. Applying the selected fix must match the
complete expected dirty source, leaving only the unrelated diagnostic and no
action at the repaired location. Disk is never written.

Closing the overlay must republish the two original disk diagnostics and expose
the original quick fixes with a null closed-document version. Reopening at
version 5 restores the same exact disk diagnostics and edits with the new client
version. The URI must remain encoded in every publication and edit. These cells
do not certify the remaining B05 syntax partitions, dependency/schema states,
editor smoke or native UX07/UX08 routes. B00-B04 accepted snapshots remain
fixed; macOS requires independent current evidence when used, and B16 remains
deferred.
