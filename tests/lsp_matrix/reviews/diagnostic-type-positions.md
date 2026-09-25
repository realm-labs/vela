# S3 type-position diagnostics

B05.16 uses one shared fixture with valid and invalid type positions. The
valid file covers primitive hints, Array/Map/Set and view/mut variants,
Iterator, Option, Result, tuple/unit, source struct and trait, dynamic `Any`
and an unknown hint. Parameter, return, struct/enum field, trait method,
const and local annotations remain free of speculative diagnostics. The
invalid file independently exercises each one- or two-argument contract,
unsupported source-type arguments, scalar and tuple non-keyable Map/Set
arguments, and a one-element tuple type.

Service tests assert the complete ordered diagnostics, messages, severity,
source byte ranges, related labels and candidate/repair metadata under LF and
CRLF. The complete unsupported source-generic list has one exact removal
repair hint; every other error has none. Protocol tests assert the complete UTF-16 publication objects in
an encoded Unicode/space/percent workspace URI. The fixture's separate repair
oracle replaces every marked invalid span; the whole repaired source parses,
all diagnostics clear, and live and fresh analysis agree. Closing the dirty
protocol document restores the original disk errors; reopening the repaired
text clears them again.

The existing schema lifecycle service and protocol tests cover a host type in
type position through valid, missing, replaced, invalid and restored schemas.
They pin the retained independent Array error and suppress speculative host
field errors when schema facts are unavailable. Unknown source type facts in
the valid fixture likewise do not produce a guessed type error.
