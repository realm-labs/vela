# B06 S9 semantic-token recovery review

The shared `semantic-token-recovery` fixture independently authors a complete
76-token valid stream and 25 damaged streams containing 1706 tokens in total.
Markers supply exact source text, byte/UTF-16 coordinates, kinds and complete
modifier sets. A closed defining file supplies functions, structs and an enum;
the malformed importer never executes script or host code.

| Partition | Exact expectations |
| --- | --- |
| Valid neighbors | Imports, declarations, parameters, source fields, source calls and builtin methods retain their exact ownership beside malformed syntax. Each repair restores the complete valid stream and clears its diagnostics. |
| Member and call recovery | Empty member names, empty named argument values, open calls and nested open calls preserve typed receivers, known callees, known argument labels and delimiters. Missing names and qualified callees remain unresolved; diagnostic suggestions do not turn a misspelled method into a builtin method token. Unknown source fields retain lexical member classification under the existing conservative diagnostic precision policy. |
| Types and declarations | Invalid type arity, unknown hints and an open `Array<` keep lexical type roles without invented arguments. Partial signatures retain real parameter spans. Nameless struct/enum/trait owners report parser errors and cannot lend field or member ownership to their orphaned syntax. Removing the previous `Local` owner removes its type/field provenance from the still-valid neighboring function. |
| Owners, records and patterns | Open inherent/trait methods, an open source record and a partial enum pattern retain independently known owner/member/variant facts. A pattern name with no bound HIR local stays lexical; the fixture does not invent a binding from an unfinished arm. |
| Literal and trivia recovery | Unterminated strings, interpolations, byte strings, chars and multiline strings retain opaque lexical envelopes with exact line slices and no symbol provenance. Comment-looking contents stay literal text. A real unfinished block comment stays a comment. Lexer/parser errors remain available rather than being suppressed by token recovery. |
| Empty source and stale facts | Replacing all source with an empty document returns an empty full stream and empty range, without cached declarations or tokens. Actual deltas applied to prior data reproduce every complete current stream. |

Both layers run LF and CRLF, alternate each damaged document with the complete
valid source, compare every state with a fresh workspace, and check unchanged
deltas, every line/token range, token-end empty ranges and the zero range at
document start. Protocol overlays use encoded Unicode, space and percent URIs,
monotonically increasing versions and final close-to-disk restoration. It checks
actual `publishDiagnostics` notifications, including the exact independent range
and complete `first`, `find`, `last` candidate set for `frist`.
Every checked publication must also be free of diagnostic projection errors.
The CRLF literal reproducer exposed a valid lexer endpoint immediately after
`\r`; diagnostic coordinates now map that boundary to the visible line end while
the Unicode projection regression continues rejecting invalid byte columns.
Edit projections remain strict because normalizing their boundaries would change
which source bytes the edit affects.
The expanded suite exposed the five-minute orchestration limit under local load.
Full Rust audit commands now have ten minutes and their editor wrapper has a
bounded twenty-five minutes; individual assertion/performance budgets are intact.

The production fixes retain bounded CST parameter/field/variant nodes when a
closing delimiter is missing and diagnose missing type-owner names. Semantic
lexical recovery accepts only an Unknown lossless span with a matching literal
diagnostic and opener. It indexes diagnosed envelopes and keeps the normal token
path ordered without an additional sort. Unfinished interpolation envelopes stay
opaque until the lexer supplies an actual complete interpolation structure.
These changes preserve errors and do not make malformed programs executable.

Only twelve S9 full/delta/range positive/negative service/protocol obligations
use this evidence. Other B06 cells remain separate; macOS requires independent
fresh evidence and B16 remains deferred.
