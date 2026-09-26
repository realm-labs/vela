# B06 S3 type-position token review

The shared `semantic-token-type-positions` fixture hand-marks all 398 positive
and 195 negative tokens with independent text, type and complete modifier
expectations. Both layers use the same static schema JSON and source files;
neither expected stream nor coordinates come from a production provider.

| Type-position partition | Exact expectations |
| --- | --- |
| Primitive and erased hints | bool, char, all signed/unsigned integer widths, both floats, String/Bytes, Any, Function/Closure and Range carry builtin type and defaultLibrary. Names Range/Closure used as local variables retain variable/source classification. Removed or unsupported int/float/string/bytes spellings gain no builtin flags. |
| Containers | Array/Map/Set/Iterator/Option/Result, nested Map of Array of Result, Set and Iterator of source types, and Array/Map/Set shared/exclusive view hints pin every base, argument and delimiter. Builtin bases keep their identity even at invalid arity; this does not admit source-language generics. |
| Source types and traits | Local struct, imported public struct/enum/trait and qualified source references carry type/source. Qualified prefixes are namespace tokens without ownership modifiers. An unopened dependency supplies the imported types. Unimported names and a private qualified type remain type tokens with no provenance. |
| Schema facts | HostThing, host::External and HostTrait carry type/host/schema. Exact source ownership wins over same-named schema metadata, including qualified source paths. A private source owner blocks schema fallback even when a matching schema entry exists. Wrong qualified paths cannot borrow a schema terminal name. A function occupying a type name blocks schema fallback. |
| Missing and dynamic facts | Missing, unimported, private, wrong-path and unknown-alias hints retain exact conservative type tokens. Any remains builtin and does not gain source/schema flags. Unsupported source type arguments and Option arity do not invent type definitions or owner modifiers for arguments. |
| Nested positions | Function parameters/returns, locals, unit/tuples, typed lambda parameters, lambda locals, required trait-signature default bodies and inherent method positions preserve precise independent token spans and classifications. |

The drivers repeat LF/CRLF full streams, apply actual deltas, compare with fresh
analysis and require unchanged queries to be deterministic. Every line and
individual token range is pinned against the oracle, with an empty query at
each token end. Encoded Unicode/space/percent workspace URIs, unsaved negative
overlays and close-to-disk restoration retain correct UTF-16 positions and IDs.

Production now collects exact type path spans from the syntax AST once per
document. This also visits annotations inside lambdas and required-signature
defaults. Provenance expands imports in the requesting module, respects source
kind/visibility and consults only exact unowned schema paths. Range, Closure and
collection-view builtin classifications no longer fall through to ordinary types.
No script execution, host access, type structure or runtime semantics change.

The twelve S3 positive/negative full/delta/range service/protocol cells use these
assertions. Remaining syntax, lifecycle, editor and generated/scale obligations
require their own evidence; macOS execution remains independent and B16 deferred.
