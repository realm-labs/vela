# B06 S7 literal, operator and interpolation token review

The shared `semantic-token-literals` fixture independently authors complete
475-token positive and 112-token negative streams. Literal text, token kinds,
complete modifier lists and byte/UTF-16 coordinates come from markers rather
than a provider. Both layers consume the same source and closed dependency.

| Partition | Exact expectations |
| --- | --- |
| Numbers | Decimal, hexadecimal, binary and underscore-separated numbers; all eight integer suffixes; decimal/exponent floats and both float suffixes are number tokens including their entire spelling. Signs remain independent operators. |
| Literal text | Ordinary and multiline strings, Unicode and escaped characters, ASCII byte strings with hexadecimal/control escapes, booleans, unit, tuples, arrays, maps, record values and the standard set constructor retain exact lexical and symbol roles. Blank multiline slices emit no token. |
| Interpolation | Ordinary and multiline strings preserve original text chunks, escaped braces and Unicode escapes. Embedded braces and expressions have independent tokens with exact parameter/local/function/property ownership, arithmetic/logical operators, nested interpolation, literal strings, record braces and member accesses. A multiline expression preserves line comments and source coordinates. |
| Comments | String text resembling comments remains string text. Actual nested block comments and line comments inside interpolation remain comments. Non-BMP text before embedded tokens pins both byte and UTF-16 positions. |
| Operators | Simple/compound assignment, indexed writes, unary signs/negation, all arithmetic/comparison/logical operators and both ranges retain exact kinds and no symbol provenance. Arrow, dot, path separator and collection delimiters retain their established roles. |
| Negative boundaries | Unresolved value/callee names inside interpolation remain unresolved. Any members and unowned field bases retain conservative lexical variable tokens without guessed ownership, matching existing HIR field-base policy. Invalid numeric suffixes and string/byte escapes retain lexical classes independently of diagnostics; no source/schema facts are invented. Literal occurrences of real or missing names never become references. |

The shared drivers compare every complete stream under LF and CRLF, apply real
deltas, check unchanged IDs and fresh-analysis parity, and pin every line/token
range plus empty token-end ranges. Protocol uses encoded Unicode/space/percent
URIs, a negative unsaved overlay and close restoration of the original full
stream and result ID.

The production fix expands only lexer-owned interpolation expression spans.
An iterative work list rebases nested expression tokens to original source
coordinates and emits string chunks and interpolation delimiters separately.
Each nested expression is contained in its parent literal, so work is finite
and no new recursive traversal is introduced. Literal chunks continue to shield
comment-like text; expression comments are collected through the normal scanner.
All existing semantic classification then uses the expanded exact spans.
This changes read-only tooling, not language syntax or runtime execution.

Only the twelve S7 positive/negative full/delta/range service/protocol obligations
use this evidence. Remaining B06 cells need their own proof, macOS evidence stays
independent and B16 stays deferred.
