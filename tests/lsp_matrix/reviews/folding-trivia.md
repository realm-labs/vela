# Folding trivia partition review

B10.21 covers folding S12 positive/negative service and protocol cells.
The independent marked fixture has 38 cases, 28 nonempty sets, 60 fold spans
and 10 genuinely empty sets. Expected spans come from authored boundaries;
neither folding providers nor production coordinate helpers generate them.

| Partition | Authored cases and expected behavior |
| --- | --- |
| Leading and trailing trivia | Line, documentation, nested block and trailing comments cannot fabricate declarations or imports. |
| Internal trivia | Blank lines, tabs, header/type comments, attributes and comments inside blocks, branches, arrays, records and lambdas preserve their full enclosing spans. |
| Literal decoys | Plain and formatted multiline strings retain literal spans while comment-like text remains literal content. |
| Member boundaries | Impl method bodies remain separate from their enclosing impl; required trait signatures, struct fields and enum variants remain enclosed by their declaration. |
| Imports | Consecutive imports retain one imports group across intervening comments and blank lines; a real declaration separates groups. |
| Shebang | Only the file-start shebang is trivia. Unicode shift lines are inserted after it so the same syntax partition is retained. |
| Exact empties | Standalone comments, shebang and whitespace; single-line items/members; comment decoys in a constant string; separated single imports; standalone unclosed comment. |
| Lexical damage | A valid function before an unterminated block comment retains exactly its item/body folds. Both damaged cases pin one E_LEX_BLOCK_COMMENT error, message, severity, source ownership and full error span. Close restoration clears parse diagnostics. |

The first case literally pins item column 8 UTF-16 versus 12 bytes and body
column 35 versus 39 bytes, both ending at line 4 column 1. Error spans are
independently pinned from line 3 to 5 and line 0 to 2, all at column 0.
LF/CRLF and two Unicode shift lines exercise all 152 corpus forms.
The bounded actual stdio probe preserves real diagnostics receipts and all
112 positive comparisons and 40 empty sets, including exact UTF-16 error bounds.

Service and protocol compare complete main/helper/missing sets three times
on disk, every dirty case, close restoration, fresh analysis and every earlier
immutable source/parse/query snapshot. Queries leave generation, parse count,
project and HIR rebuild counts unchanged. Both source records and absent schema
are checked. Protocol adds encoded private physical roots, unchanged disk bytes,
typed full folding response envelopes and actual open/change/close diagnostic
publication and clearing. Independent Node checks pin membership, counts,
literal geometry, shebang shifts and region/import ownership.

Existing folding tests and acceptance evidence remain in place. This adds no
language/runtime or folding behavior. S9 recovery, installed folding smoke,
selection and UX11/UX12 are separate open partitions. B10 remains open; platform
progress is shared while fresh Windows/macOS evidence stays independent.
