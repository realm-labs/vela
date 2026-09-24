# Reference and rename syntax recovery

B04.31 advances the recovery protocol state cells for references, highlight,
prepare-rename and rename (four IDs). These broad state cells remain unreviewed
until their other syntax and interaction cases are complete.

The independent five-file fixture runs seven explicit actions: open importer,
open definition, damage a trailing declaration, repair it, damage a preceding
declaration, repair it again and redamage the trailing declaration. A closed
qualified consumer and a separate same-named decoy stay in the workspace.
The damaged trailing type position contains the same word as the imported
function but must never acquire its references, highlights or rename target.

At every state, service and protocol repeat complete references with and
without the declaration, both document highlights, exact definition and
prepare targets, and complete multi-file rename edits. Service compares
incrementally updated and fresh databases. Protocol applies real didOpen and
didChange notifications, checks `E_PARSE` publication and clearing, and
compares long-lived and fresh servers. Both layers use LF and CRLF variants
with Chinese and non-BMP prefixes. Protocol asserts UTF-16 ranges, encoded
Unicode paths, legacy and versioned edits, and null versions for the closed
consumer and definition-independent decoy.

Remaining syntax partitions, edit-application combinations and UX05/UX06
remain before B04 acceptance. B00-B03 snapshots stay fixed; macOS needs fresh
evidence when used. B16 remains deferred.
