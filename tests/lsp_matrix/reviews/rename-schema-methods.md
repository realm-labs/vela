# Schema method ownership and applied edits

B04.13 targets the 32 S4/S5 positive/negative service/protocol partitions of
references, highlight, prepare-rename and rename. Requirement IDs follow
`<feature>/syntax/<S4|S5>/<positive|negative>/<service|protocol>`. Broad cells remain
unreviewed pending remaining callable, semantic and lifecycle partitions.

The independent four-file fixture separates source-backed schema methods on two
same-named host types, a schema trait method, a source method and metadata-only
methods. It covers aliases, a returned receiver, closure and closed consumer;
unknown receivers/methods and literals must remain empty. Renaming must reject an
existing method or capture of an unresolved call on the same owner, while another
owner can use that spelling independently.

Both coordinate drivers check exact sets/kinds, highlights, definitions, prepare
and full edit plans; apply writable groups, compare entire files, parse, explicitly
regenerate schema metadata, requery and restore. Metadata-only methods retain
references without rename. Protocol additionally checks LF/CRLF, UTF-16, encoded
paths, both edit forms, document versions and schema ABI confirmation annotations.

The matrix exposed unresolved-call capture and inconsistent source impl method
symbols between reference and rename. Rename now checks receiver ownership before
accepting a proposed method name and uses the canonical impl/trait symbol helper.
Validation and fresh Windows editor/native artifacts are recorded in the B04.13
checkpoint and its commit trailers. Schema functions, variants and parameter
ownership remain separate pending partitions.
