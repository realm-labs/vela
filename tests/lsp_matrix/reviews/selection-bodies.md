# Selection function and method bodies

B10.26 closes only S2 positive/negative service/protocol. Other selection syntax,
recovery, installed selection and native UX11/UX12 remain open.

Fifty-nine independently marked cases author 180 inputs: 168 complete ancestor
chains and twelve points. Fifty-four documents have positive chains, four have
only points and one has an actual empty query vector. LF/CRLF and two Unicode
prefix lines give 236 whole response vectors and 720 query positions.

Cover locals and explicit primitive/qualified/tuple/allowed builtin hints,
uninitialized/attributed declarations, simple and all five compound assignments,
member/index writes, bare/value returns and semicolons, newline termination,
nested shadows, tail statements and block initializers. If/else-if branches,
match guards/arms, for key/value bindings and break/continue retain exact parents.
Typed, zero-parameter, nested and captured lambdas, positional/named callbacks
and trait-default/inherent/trait-impl bodies have independently authored chains.
All six assignment operators occur in local lambda expression bodies and
positional callbacks; zero-parameter assignment bodies are also represented.
Same-spelled neighboring functions cannot borrow ranges. Comments, whitespace,
empty files and EOF after newline retain only their requested zero-width point.

The first return value spans UTF16 65..70 versus byte 69..74, under return 58..71,
body 17..73, item 8..73 and full file starting at zero. Local binding 23..28
versus 27..32 belongs to statement 19..41; assignment 42..56 belongs to expression
statement 42..57. The independent helper return value is 29..33 under return
22..34, body 20..36 and item 8..36. Duplicate/reordered queries preserve vector
order and every recursive parent. Same-span token/expression/argument nodes
deduplicate; tail ExprStmt and MatchArm trailing trivia remain real ancestors.

Primary `find_statement_term_end` and `find_match_arm_expression_end` own trailing
spaces until their closing delimiter. `arg_list` skips leading argument trivia.
Initial markers omitted those spans or included an argument's leading space;
correct markers and chains against these routines, preserve identical source
bytes and the original failed draft. An initial typed-lambda draft also invented
a lambda parameter default outside `grammar.ebnf::lambda_param`; retain that
draft and use a supported typed parameter instead. No new grammar is introduced.

Valid expression-body assignments expose two actual CST defects: root assignment
classification steals the lambda's outer node; scanning for any argument `=`
steals a positional callback's body as a named-argument value. The reviewed
pre-fix 236-vector corpus fails in 56 whole vectors and 168 individual positions.
Two direct AST tests and both formal service/protocol tests fail before repair.
Classify a leading `|`/`||` lambda before inspecting its body operators; a named
argument requires the initial IDENT followed by `=`. Recursive body parsing keeps
the assignment operands/operator under that lambda and preserves real labels.
Existing syntax/AST/import/declaration tests remain active. After repair all 236
vectors/720 positions match the same independent expected responses and bytes.

Both drivers repeat complete main/helper/missing/empty vectors three times through
disk, every dirty variant and close restoration. Current and independently fresh
analysis, and every retained immutable source/CST/query snapshot, must match.
Exact source records, quiet parse, absent schema, generation and parse/project/HIR
counters stay fixed on queries. Protocol checks complete typed result/error
envelopes, real initialize/open/change/close, encoded owned physical roots,
unchanged disk and invalid half-character/outside/mixed-vector atomic errors.

Direct AST checks also pin lambda parameters, assignment operands/operator,
callback labels and independent fallback arguments across all six operators.
Node tests pin complete membership, UTF16/byte/helper goldens, dedup, containment
and point/empty cardinality. A test insertion accidentally entered an existing
raw string and selected zero tests; restore the original source exactly, move
the new tests outside it and retain that draft/zero-test log. Genuine failing
one-test runs are recorded separately. Preserve all accepted contracts and
independent platform audits; B16 remains deferred.

Two validation invocations also used incorrect targets: the package check named
a nonexistent script, and the architecture filter selected zero tests in the
service crate. Preserve both logs; the existing package validator and the server
crate's three architecture tests pass. Neither draft counts as acceptance proof.
