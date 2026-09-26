# B06 S6 control-flow and pattern token review

The shared `semantic-token-control` fixture independently marks complete
603-token positive and 305-token negative streams. Every literal token, kind,
complete modifier list and byte/UTF-16 coordinate comes from authored markers,
not a provider. Both layers load the same closed dependency and static schema.

| Partition | Exact expectations |
| --- | --- |
| Control flow | Statement and value `if`, `else if`, `else`, `match`, guards, nested blocks, `for`/`in`, `break`, `continue` and `return` carry controlFlow only on their keywords. Keywords and wildcard `_` acquire no symbol provenance. |
| Pattern ownership | Source and schema unit/tuple/record variants, qualified dependency variants, record labels, shorthand bindings, nested tuple/record patterns and plain binding arms retain exact owners. Source enum metadata wins over exact schema collisions. Immutable Option/Result variants have defaultLibrary ownership ahead of exact schema collisions; unknown builtin variants and wrong namespaces cannot borrow it. |
| Binding scopes | Iterables see the outer scope; loop bindings are visible only in the body. Match payload bindings are visible in their own guard and arm, not siblings or following statements. Pair/tuple/record loop bindings, let destructuring, nested shadowing and lambda captures pin the same-spelled binding's kind and provenance. |
| Expressions | All assignment/compound operators, unary/binary/comparison/logical/range operators, indexing, arrays, maps, tuple values, branch values and literal patterns retain exact lexical roles independently of value-domain or control-flow legality. |
| Negative boundaries | Unknown, wrong-path and private pattern owners are unresolved at their exact path segments; private source ownership blocks an exact schema fallback. Unknown variants and field labels gain no provenance, while their payload bindings retain local roles. Out-of-scope loop/arm/branch names remain unresolved, and Any member access gains no field ownership. |

The shared drivers run LF/CRLF full streams, apply actual deltas, compare fresh
analysis, repeat unchanged queries, and pin every line/token/empty-end range.
Protocol uses encoded Unicode/space/percent URIs, an unsaved negative overlay
and close-to-disk restoration of the complete original stream and result ID.

The CST fix preserves compound identifier-starting let patterns and limits a
binding's type annotation to the text before its initializer. A typed lambda's
parameter colon is not the outer let's annotation. A dedicated syntax regression
checks nested record/qualified tuple patterns, ordinary typed names, lambdas
and bindings without initializers under semicolon and newline termination.
Token paths use the record/tuple AST's owner tokens, including unqualified
record owners, and explicitly mark unowned pattern paths to prevent later
schema variant fallback. Builtin enum owners/variants use the immutable standard
library manifest with exact paths rather than a registry or terminal-name guess.
Schema collisions cannot manufacture record variants or fields on builtin enums.
All work remains static analysis; no host state or
source execution participates in proof.

Only the twelve S6 positive/negative full/delta/range service/protocol cells use
this evidence. Other B06 cells require their own proof; macOS evidence remains
independent and B16 stays deferred.
