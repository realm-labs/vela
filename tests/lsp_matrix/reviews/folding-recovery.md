# Folding recovery partition review

B10.22 covers folding S9 positive/negative service/protocol and protocol
recovery state. The marked fixture independently defines 54 whole sets:
49 positives, 264 fold spans and five genuinely empty sets. Forty-three cases
require parse errors; eleven retain valid or accepted quiet recovery policy.
No provider output generates expected folding ranges.

| Partition | Independent expected behavior |
| --- | --- |
| Declarations | Missing const/state/extern names, types, values; missing function names, parameter names/defaults, return types and bodies retain healthy neighboring function folds. |
| Members | Missing struct/enum/trait/impl owners, field names/defaults, enum payload defaults and required/inherent method defaults preserve enclosing recovered declaration and available body spans. |
| Quiet expressions | Unfinished members, named arguments, constructors, patterns and initializers retain their complete enclosing function/body set without introducing parse errors. Dynamic/unresolved receivers stay independent. |
| EOF owners | Unclosed function, array, map, record, block, lambda, parameter list/default array, struct, enum, tuple variant, trait and impl retain exact available multiline spans. |
| Unknown expression shape | An unclosed parenthesized list and a lexically rejected multiline string retain the primary CST's PathExpr recovery shape, independently checked over their complete authored byte extent. They add no guessed tuple/literal fold. |
| Imports | A consecutive group ending in an unfinished use path remains one imports fold under the existing quiet completion policy. |
| Exact empties | Empty/trivia files, stray semicolon, single-line partial function and partial const own no folds. Their error policy is explicit. |

The initial 43-case actual stdio probe retains all 172 raw comparisons. Four
LF/CRLF/shift lambda cases expose an inner statement semicolon trimmed as an
outer terminator. A direct parser test first fails on the same missing byte,
then verifies let, return and expression contexts through damage/repair/redamage.
The fix trims a terminator only at the expression's root delimiter depth.
All initial 172 comparisons pass after the fix.

Expansion adds declaration/default, import, tuple/map, string and header cases.
A bodyless multiline function steals a later healthy function's body and
suppresses its required diagnostic. Direct red tests pin separate full
declarations and the exact missing-body diagnostic. A focused header boundary
module stops before following declaration starters and recognizes contextual
state declarations without treating state return types or function names as
declarations. Existing unclosed-parameter body recovery stays in place.
Direct tests include attributes, pub/async functions, types, imports, const,
state and extern state; contextual state, Option<state> and qualified hints
remain valid. This changes malformed CST ownership, not executable syntax.

The expanded first draft guessed tuple/literal folds for two unknown CST shapes
and an error for a quiet unfinished use. Primary classification/lexer/use-path
code established the existing policy; reviewed expectations pin PathExpr
ownership and quiet use explicitly. Original source text, raw responses and
failed draft expectations remain as artifacts. These are not production
defect claims, and no existing accepted assertion was weakened.

Service and protocol run every case through damage -> clean repair -> same
damage, for LF/CRLF and two Unicode shift lines: 162 transitions per form.
Every transition compares complete main/helper/missing sets three times,
fresh state and all earlier immutable source/parse/query snapshots. Two source
records, absent schema, source-owned bounded parse spans and unchanged query
generation/parse/project/HIR counters are checked. Full current/fresh parse
metadata agrees. Authored unknown expression kinds are checked from cached CST.
Protocol adds real open/change/close diagnostic publication and clearing,
typed whole response envelopes, encoded owned physical roots and unchanged
disk bytes. Close restores the exact healthy disk set and clears errors.

Literal goldens pin the first neighbor's item column 8 UTF-16 versus 12 bytes,
body 20 versus 24, both ending at line 2 column 1. Node tests independently
pin all memberships/counts, EOF semicolon ownership, malformed CST extents,
quiet policy, missing-body neighbors and empty sets.

Validation includes the direct red/green parser tests, full syntax checks,
focused folding service/protocol and architecture checks, relevant all-target
Clippy/fmt, Node inventory, actual stdio, package, fresh installed VSIX/native
regression and strict accepted-batch audit. Prior accepted snapshots, catalog
evidence and native contracts stay fixed. B10 remains open for installed
folding, selection and UX11/UX12. macOS evidence remains independent; B16 is
deferred.
