# Reference and rename syntax recovery

B04.31 established the syntax-recovery fixture. B04.44 completes its four
protocol state cells for references, highlight, prepare-rename and rename.
Syntax partitions and editor interactions remain separate obligations.

The independent five-file fixture runs eleven explicit actions: open importer
and definition, damage and repair trailing and preceding declarations, then
repair and exercise incomplete member and call contexts. A closed qualified
consumer and a separate same-named decoy stay in the workspace. The damaged
trailing type position contains the same word as the imported function but
must never acquire its references, highlights or rename target. The incomplete
member receiver and call target remain unresolved too.

At every state, service and protocol repeat complete references with and
without the declaration, both document highlights, exact definition and
prepare targets, and complete multi-file rename edits. Malformed same-named
type positions have no definition, references, highlights or rename target.
Service compares
incrementally updated and fresh databases. Protocol applies real didOpen and
didChange notifications, checks `E_PARSE` publication and clearing, and
compares long-lived and fresh servers. Both layers use LF and CRLF variants
with Chinese and non-BMP prefixes. Protocol asserts UTF-16 ranges, encoded
Unicode paths, legacy and versioned edits, and null versions for the closed
consumer and definition-independent decoy.

Remaining syntax partitions, edit-application combinations and UX05/UX06
remain before B04 acceptance. B00-B03 snapshots stay fixed; macOS needs fresh
evidence when used. B16 remains deferred.
