# Source function import boundaries

B04.32 advances the S8 positive and negative service/protocol paths for
references, highlight, prepare-rename and rename. The broad S8 cells remain
unreviewed until all module/import partitions have an explicit audit.

The independent eight-file fixture has 17 queries. A public function is called
through a direct import, qualified module path, explicit function alias and
module alias, including a closed consumer. Both Rust layers compare the complete
reference and highlight sets, prepare targets and versioned edit sets, apply
the rename across files, parse the result, requery and restore. Unicode prefixes
exercise byte and UTF-16 coordinates under LF and CRLF. An explicit function
alias keeps its spelling while its import terminal changes. Private cross-module
imports, stdlib-only imports and unresolved imports have no source owner.

The initial fixture exposed two missing source ownership paths. A module alias
callee had an `Import` binding for the module name, so declaration references
and rename skipped its terminal function. A source function import alias could
appear in the reference set but could not itself query that set or initiate the
same rename. Resolution now expands the imported module from the recorded HIR
path and checks visible source function ownership; import alias target lookup
accepts either the alias span or the underlying terminal. The remaining S8
partitions and UX05/UX06 remain open.
