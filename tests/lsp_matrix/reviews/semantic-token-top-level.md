# B06 S1 top-level declaration token review

The independent `semantic-token-top-level` fixture manually marks every token
and specifies its text, type and complete modifier list. Neither expected
classifications nor expected streams come from the lexer, service or protocol
provider. The shared oracle derives coordinates only from these marker spans.

The positive source covers `pub`, `use`, `const`, contextual `state`, `extern
state`, functions, parameters and defaults, struct fields, unit/tuple/record enum
variants, required and default trait methods, inherent impls and trait impls.
Source constants referenced by defaults in required signatures, default methods,
impl methods and functions carry `readonly` and `source`. Required signatures
have parameter declarations even without executable method bodies. Reserved
`self` remains a keyword; a function and parameters named contextual `state`
retain function/parameter classifications. Impl header terminals are type uses,
not definitions. `self.x` uses the field of its resolved impl target.

The negative source contains duplicate parameters, struct fields, enum variants
and variant fields, required trait methods and impl methods. These syntax sites
retain their exact declaration classification; diagnostics do not remove nearby
tokens. An unresolved import terminal carries only the unresolved reference
classification. Abstract trait receivers, unknown impl targets and an explicitly
`Any` receiver must not acquire the similarly named concrete `Bad.count` field.
Unknown impl type terminals carry no source/schema modifier. A valid function
after the duplicate declarations remains classified, including its `state`
parameter declaration and use.

Both service and protocol tests run LF and CRLF and check the complete positive,
negative and repaired streams. They apply the actual delta to the preceding
stream, check the result ID and compare incremental analysis with a fresh
database/server. Protocol queries use an encoded Unicode/space/percent workspace
URI and close the unsaved negative overlay to restore the original disk stream
and ID. Service repair restores the same original snapshot. Repeats require
deterministic full streams and empty deltas.

Range queries check each entire declaration line, each individual token and an
empty range at every token end against subsets of the independent oracle. No
extra neighbor, declaration flag, guessed owner or CRLF byte may enter a result.
The twelve mapped S1 positive/negative full/delta/range service/protocol cells
all use these assertions; other syntax/state/owner cells remain unreviewed until
their own evidence is added.

The reproducers exposed three missing joins in semantic token classification:
method signature parameter sites, method/default body binding owners, and impl
header type paths. Production uses signature name spans, the canonical HIR body
binding lookup and syntax AST header spans with visible source/schema resolution.
The impl receiver fallback applies only to the actual first `self` parameter of
a resolved impl target, and does not replace an authoritative `Any` fact. These
are read-only tooling projections and do not execute scripts or change language
or runtime semantics.
