# B06 S5 call and argument token review

The shared `semantic-token-calls` fixture independently marks complete
577-token positive and 455-token negative streams. Both layers load the same
closed dependency and static schema. Every token's literal text, kind, complete
modifiers and byte/UTF-16 coordinates are authored independently of providers.

| Call partition | Exact expectations |
| --- | --- |
| Source functions | Bare and qualified calls, defaulted/reordered arguments, nested calls, function references, callback arguments, local callable aliases, parenthesized callbacks and typed lambdas retain their own function/parameter/local roles. Qualified module prefixes are namespaces. |
| Source methods | Inherent, enum and trait methods, default trait bodies, actual struct/enum impl `self` calls, Host script extensions, source-return and method-return chains pin exact method and named-parameter ownership. Trait self is a trait receiver and never a guessed record. |
| Schema functions and methods | Explicit signature metadata owns host function, host method and schema trait labels. Returned Host method chains keep host/schema flags. Legacy metadata without parameter names retains callable ownership but cannot invent named labels. |
| Stdlib calls | `math::max` and `Array.push` use defaultLibrary for callees and their declared named labels. An exact same-path registry function cannot override the builtin contract. |
| Named/default positions | Caller values remain their own source locals, distinct from same-spelled labels. Omitted defaults, reordered names, duplicate labels and a missing argument value retain static parameter ownership where known; unknown labels carry no ownership. Tuple variant labels use payload property/source metadata independently of argument-validity diagnostics. |
| Negative boundaries | Missing/Any/erased callback calls, source/schema Any-return chains, private and wrong-path callees, missing values and unknown tuple labels cannot borrow facts. A local callable shadows a source function; a non-callable source const shadows an exact schema function. Private source ownership blocks an exact same-path schema fallback. |

The drivers check LF/CRLF complete streams, apply actual deltas and compare
fresh analysis, repeat unchanged requests, and pin every line/token/empty-end
range. Protocol uses encoded Unicode/space/percent URIs, unsaved negative
overlays and close-to-disk restoration of the original stream and result ID.
Active-parameter indices belong to signature help; these token cells assert
the lexical and static roles at those argument positions.

Production projects exact call/label syntax spans through the existing scoped
callable resolver. It removes the old raw-path registry fallback. An implicit
self fallback requires the actual first self binding and its impl/trait
signature; known Any facts remain authoritative. Shared callable matching now
includes exact Host script extensions and enum owners, with source kind,
visibility and import checks. Record-label collection avoids navigating every
named call because the call projection owns tuple labels directly. Runtime,
host state access and language semantics remain unchanged.

Only the twelve S5 positive/negative full/delta/range service/protocol cells use
this evidence. Remaining B06 cells need their own proof; macOS evidence is
independent and B16 remains deferred.
