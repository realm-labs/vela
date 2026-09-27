# Hover S3 type positions

Scope: `hover/syntax/S3/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::type_matrix_tests::hover_type_matrix_preserves_complete_scoped_hints_and_degraded_boundaries`
and `tests::hover::type_matrix::hover_type_matrix_preserves_complete_scoped_hints_and_degraded_boundaries`.

`hover-s3` independently authors 245 queries, including 74 parameter declaration,
parameter use and type-token triples. 236 queries return a complete authored
hover; nine return explicit null. Every supported token is queried at its start
and one character inside; null positions run once. LF/CRLF and present/absent
physical schema variants yield 1,924 positions per layer plus repeated queries.
Expected text and byte/UTF-16 token spans come from authored markers and results,
never hover output or the production line index. Service checks complete labels,
kinds, details, docs, ranges and canonical source/schema/builtin/local identities.
Protocol checks the whole Markdown hover and range, including explicit JSON null,
through real initialization, document opening and typed request dispatch.

| Partition | Exact independent expectations |
|---|---|
| Primitives and builtin types | All 15 primitives, including unit parameter facts, plus Any and Range. Lowercase primitive type tokens now have builtin identities. Unit/tuple punctuation has an explicit null policy while their parameters retain complete types. |
| Containers and callable hints | Bare and parameterized Array/Map/Set families and their View/Mut forms, Iterator, Option/Result, tuples, nested containers, Function and Closure. Mutation modes follow the existing fixed ArrayMut and growable MapMut/SetMut contract; extra mode arguments are unsupported and remain unknown. Iterator's public hover label is deliberately erased to `Iterator`; this review does not infer hidden item identity from that label. |
| Source type ownership | Qualified struct/enum/trait hints, individual and namespace aliases, nested imported types and exact docs. An unrelated same-short-name Row/Readable and conflicting registry facts cannot supply metadata. Unimported/private types, private aliases and source function/const names used as types degrade to Any without an identity, while parameter facts remain unknown. |
| Schema type ownership | Qualified host and enum types, traits, schema aliases, nested containers and inner leaves retain exact canonical names/docs. Schema-only metadata needs no source span. When the physical schema is absent, schema tokens degrade to Any with no docs/identity, parameters become unknown, and container shapes preserve unknown leaves. Source/builtin facts remain unchanged. |
| Compiler type facts | Literal `task::Error` retains the existing compiler-owned type fact and builtin identity; no task value, alias capability or language extension is introduced. |
| Hint locations and inner leaves | Const, state, extern state, struct/enum fields, trait/method/function parameters and returns, let and lambda annotations. Inner source, String and host leaves have their own precise ranges and complete facts rather than borrowing the outer container. |
| Unknown boundaries | Missing capital/lowercase hints, obsolete int/Future spellings and unsupported source/host type arguments have exact degraded results. Any/unknown member accesses, including type/builtin-looking names, and lower/uppercase unresolved expressions are null. A type context cannot borrow a value declaration, and a member cannot borrow unrelated global metadata. |

The first independently authored primitive-token case reproduced a null `bool`
hover. The matrix also exposed missing schema facts at parameter declarations
and global fallback after dynamic member queries. The correction uses the same
CST-to-HIR hint lowering as declaration metadata, the existing scoped analysis
converter and the current schema-aware fact cache. Type hints own unresolved
results before value lookup. Unknown members stop at the receiver boundary.
The old global short-name and uppercase-name guesses are removed.

Configured roots use Chinese, spaces and percent signs with encoded file URIs.
The source declaration and schema files remain unopened. Protocol asserts all
physical fixture inputs are unchanged. This review does not close other hover
syntax/state obligations or UX10 installed input/render routes. Strict acceptance
still requires fresh complete evidence from one registered Windows/macOS profile.
