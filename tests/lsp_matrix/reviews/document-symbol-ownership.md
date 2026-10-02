# Document symbol member and import ownership review

B10.3 closes S4/S8 positive and negative at service/protocol layers and protocol
unresolved/dynamic states. Dirty/close, dependency and schema lifecycle states,
installed document-symbol interaction and the other B10 features remain open.

The independently authored eight-file fixture pins 25 roots and 46 complete
nodes. Whole ordered trees specify names, kinds, optional details, declaration
and member extents, actual name/header selections, ancestry and canonical source
identities. It includes public/private structs and fields, all enum variant
shapes and fields, required/default trait methods, inherent/trait overrides,
and explicit impl declarations against imported source and host aliases.
The existing declaration and recovery oracles supply the wider valid/malformed
member partition, rather than treating one new field as sufficient S4 proof.

Plain/aliased function, const, VM/extern state, type, trait and variant imports,
file and directory/deep module aliases, qualified constructors and uses, and
stdlib imports never add outline declarations. An imports-only file is empty.
Private and missing imports, conflicting aliases, unknown constructors/calls,
typed Any members and unresolved receivers cannot borrow same-spelled members
or invent types. Actual private declarations remain in their own source tree.
Local/module/schema collisions retain their source names, kinds and details.

Loaded metadata includes host and metadata-only types, source-name collisions,
fields, methods, trait methods, functions and unit/tuple/record variants.
Explicit setup assertions pin fact-category counts and nonempty bound source
locations; this is not an absent-schema test. Schema return/field facts differ
from the source definitions. Source spans deliberately point at different
actual declaration names or variant shapes: the origins outline retains the
real struct/enum/trait/function and never substitutes a schema name, kind,
signature, field or variant. Metadata-only locations stay absent. Schema JSON
and missing source requests have exact empty results.

Service compares two separately assembled databases to authored trees, repeating
every complete response three times. Protocol checks actual disk requests and
didOpen requests, repeating each complete JSON-RPC response three times.
Every file is checked under LF and CRLF. The marked Chinese/non-BMP prefix has
literal `Widget` name goldens: line 0, UTF-16 21..27 and bytes 25..31. Encoded
private roots include Chinese, spaces and percent signs. Setup verifies all
eight configured sources are discovered. Expectations never come from results.

The first authored impl root omitted the established `impl ` prefix. It was
corrected against B10.1's existing name/identity contract, while all member,
range and exclusion expectations remained fixed. The first protocol setup
omitted required package metadata from its manifest; the valid fixture now
contains it and asserts source discovery before binding metadata. These are
oracle/setup corrections, not production defects; their failed logs are retained.

Acceptance requires focused whole-tree tests, relevant all-target Clippy and
formatting, Node matrix checks, fresh complete Windows VSIX/native evidence and
the strict prior-batch gate on one frozen source. Preserve accepted snapshots,
prior children, native contracts and the independent historical macOS audit.
B16 stays deferred; macOS needs fresh local evidence when that profile is used.
