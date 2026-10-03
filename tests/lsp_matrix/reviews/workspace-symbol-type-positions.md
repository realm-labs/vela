# Workspace symbol type and ownership review

B10.8 closes S3/S10 positive/negative service/protocol requirements. S10 also
retains the complete member/import ownership tests and independently authored
source/schema corpus from B10.7. Recovery, lifecycle and installed/native picker
proof remain open.

The new corpus contains three source files and 78 complete authored rows:
three files, three modules, three model declarations, 47 typed functions,
untyped and locally binding functions, const/state/extern state and 17 schema
rows. Each row pins canonical Source/Schema identity, kind, optional detail and
container, full source extent or URI-only vela-schema location. Same-named
models::Row Class and Struct entries remain distinct.

The 47 hint partitions cover unit/bool/char/all signed and unsigned numeric
scalars/floats/String/Bytes, erased Any/Array/Map/Set/Iterator/Range/Function/Closure,
tuple, typed containers, Option/Result, nested combinations, six collection-view
contracts, aliased/qualified source/schema structs, enums and traits, and missing
direct/nested named types. Source declarations retain their authored hint text
and complete signatures; metadata uses explicit structured Any/unknown and
container/function facts. Missing names never invent a type declaration.

Eighty literal ordered query result sets compare complete output, individual
signatures, collisions, const/state/extern details, schema unknown/Any/nested
unknown facts and imports-only file/module ownership. Parameters, local names,
closure parameters/captures, constructors, source members, aliases, stdlib and
builtin uses do not become global workspace declarations. Querying i64/Any/value
can return only the explicitly declared matching function/metadata field, never
a manufactured primitive, parameter or dynamic owner. Existing B10.7 whole
results supply the source-member/source-variant/host/stdlib ownership partitions.

Every query repeats three times with LF/CRLF and a known two-line Unicode shift.
Service compares complete output with fresh immutable databases and unchanged
source/schema sets; protocol compares full typed JSON-RPC envelopes in encoded
private Chinese/space/percent roots and verifies physical source/schema bytes.
Literal first-function golden is line 2 (4 after shift), UTF-16 column 8 versus
byte 12. Independent Node checks pin corpus partitions, query memberships and
whole-file/range goldens. No query executes a script or live host state.

Acceptance uses fresh frozen Windows VSIX23/native50, Node infrastructure,
focused Rust tests, fmt, relevant all-target Clippy, automatic full audit and
strict B09 refresh. This checkpoint explicitly uses installed Rust1.97.0 for
all final processes, consistent with previous acceptance and the macOS1.97
baseline. The machine default changed to Rust1.99; its initial baseline passes
with an existing deprecated fetch_update warning and does not certify Rust1.99
Clippy compatibility. No machine-wide toolchain default is changed. Preserve
all50 native contracts, prior acceptance/completed records and the independent
macOS audit; macOS needs fresh evidence when used and B16 stays deferred.
