# Hover S4 members and constructors

Scope: `hover/syntax/S4/{positive,negative}/{service,protocol}` only. Exact tests:
`hover::member_matrix_tests::hover_member_matrix_preserves_constructor_labels_and_scoped_owners`
and `tests::hover::members::hover_member_matrix_preserves_constructor_labels_and_scoped_owners`.

`hover-s4` independently authors 158 queries: 114 complete hovers and 44 explicit
null results with schema present. Positive tokens run at their start and
interior, negative tokens at their start. LF/CRLF and present/physically absent
schema variants yield 1,034 positions per layer, each repeated. Registered
members, types and variants become null without schema; source and lexical
owners remain available. Expected metadata and Markdown are authored; markers
independently supply byte/UTF-16 ranges and exact local declaration locations.
Service asserts the complete label, kind, detail, docs, range and source/schema/
local identity. Protocol asserts complete Markdown/ranges or explicit JSON null
through real initialize/open/request dispatch.

| Partition | Independent expectations |
|---|---|
| Source fields | Same-named cross-module Row types keep distinct i64/String fields and docs. Alias receivers, nested and returned receivers, Any-valued fields, plain writes and five compound writes retain their exact owner. A missing source field cannot borrow registry metadata. |
| Source methods | Struct and enum inherent methods, async methods, trait interface/default/override methods, typed and returned receivers and chained calls retain their existing signatures, docs and identities. Competing same-named traits resolve in the impl's module. An inherent method wins over a same-named trait override declared earlier. Trait impl identities preserve the existing `Readable for Row.read` contract. Known enum methods remain available when the variant is unknown. |
| Receiver alternatives | Different source field/method owners, source/schema mixtures and incomplete source/Any origins return null. Two complete source receivers sharing one trait default return its complete common metadata and identity. Registry method decoys cannot replace an actual source method's symbol. |
| Source constructors | Aliased, qualified and namespace-aliased structs, a foreign same-named struct and an empty struct retain type docs. Explicit labels retain field metadata; shorthand/value tokens retain lexical binding facts and exact declaration locations. Bare constructors use the HIR declaration namespace even beside a same-named value; locally shadowed qualified heads have no constructor/field hover. |
| Source variants | Unit, tuple and record variants retain owner, docs and existing shape text. Named tuple arguments and explicit record labels retain field types and identities; shorthand/value tokens keep local identity. Sibling record variants retain distinct fields, and known record-variant dot reads/writes retain variant ownership. Record-pattern labels retain static owner metadata; ordinary struct-pattern bindings retain unknown facts under the existing compiler contract. |
| Schema members | Host fields retain exact writable/reflection/permission metadata and docs on reads and writes, including a read-only field in a write position. Nested and returned/chained receivers, async and Any-returning methods, registered trait methods, method permissions and existing unknown-effect metadata are complete. Queries never execute these writes or calls. |
| Schema constructors and variants | Registered host constructors/labels, shorthand locals, unit/tuple/record variant uses, record-variant fields and dot writes retain schema identities and docs. The artifact supplies variant facts and record-field metadata; unavailable tuple/record shape text is not invented. Methods remain available without a known variant, while variant fields remain null in that state. Same-named foreign variants retain their own schema owner. |
| Negative boundaries | Missing, dynamic/unknown, primitive and Any-returned members; missing labels/variants; inaccessible private structs/enums; invalid source unit/tuple record labels and tuple dot fields; methods used as field labels; ambiguous unqualified variants; and shadowed qualified constructor heads return null. Source/private/wrong-form and missing schema-variant labels cannot borrow conflicting registry fields, methods or variants. |

The authored matrix exposed short-name field/impl collisions, absent trait override
and enum method hovers, borrowed schema identities, guessed receiver alternatives,
constructor labels returning the enclosing type, absent enum record-member facts,
private label leakage and missing variants returning their enum owner. Repairs
retain complete scoped owners, source metadata identity and whole-candidate
agreement; use exact CST label tokens with static source/schema metadata; respect
visibility and variant form; and validate enum terminals through their actual HIR
binding. Constructor and member handling stay in focused modules and use existing
cached facts. No interface body or language/runtime semantics is added.

Two authored expectations were aligned with independent existing contracts:
`SyntaxBindingLowerer::resolve_constructor_path` uses the declaration namespace
for a bare record constructor; compiler placement uses `NeverMatchesRecord` /
`UnsupportedRecordPattern` for ordinary struct patterns, and semantic pattern
facts preserve unknown bindings. This batch does not add struct destructuring.
The shared drivers preserve all previously accepted exact test identities.
Protocol uses encoded isolated roots, immutable physical inputs, physically absent
schema and repeated requests. Query prefixes are ASCII; accepted coordinate
evidence owns same-line non-BMP proof. No other hover syntax/state group or full
UX10 native input/render acceptance is claimed. Each Windows/macOS gate needs
independent fresh evidence from one registered profile.
