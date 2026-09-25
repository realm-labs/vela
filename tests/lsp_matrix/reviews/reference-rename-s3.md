# Reference and rename S3 type positions

B04.50 reviews the sixteen S3 positive and negative service/protocol syntax
cells for references, document highlights, prepare rename and rename. The
shared imported-type fixture now marks `Secret` inside `Array`, `Option`,
`Result` and `Map` hints. It also marks the container names, `i64`, `String`,
`Any` and an unknown type separately. Service and protocol query every marker
under LF/CRLF, with Chinese and non-BMP prefixes before type positions.

The four nested `Secret` uses join its private source declaration, direct
parameter and return hints in one exact reference set. Highlights remain in
the queried document; prepare targets use the token range and placeholder;
complete rename edits are applied to source, parsed, queried again and then
restored. Container names, primitive hints, `Any` and unknown types return no
references or highlights and have no prepare or rename target. This proves
that traversing a generic-like builtin wrapper reaches the nested source type
without making the wrapper or an unrelated builtin editable.

The original imported-type matrix covers public struct and enum hints,
constructor positions, direct/aliased/qualified imports, trait hints and impl
headers, private source types and inaccessible/missing imports. Its public
types remain read-only while private source types are editable. The separate
schema-type import matrix covers source-backed host class/enum/trait facts,
their aliases and qualified hints, plus a metadata-only type with no source
edit. Both matrices assert exact owner identities and complete service byte
ranges or protocol UTF-16 locations, highlights, prepare ranges and workspace
edits; protocol checks open/closed file versions and encoded workspace URIs.

This closes S3 for these four methods only. Other B04 syntax and local editor
interaction gates remain open. B00-B03 accepted snapshots stay fixed; macOS
needs independent current evidence when used, and B16 remains deferred.
