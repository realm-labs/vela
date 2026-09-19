# Completion and resolve: S5 review

Scope: the eight `completion[-resolve]/syntax/S5/{positive,negative}/{service,protocol}`
obligations. The S5 contract includes source functions/methods, stdlib
functions/methods, schema functions/methods, named/defaulted arguments, active
parameter tracking, and dynamic/unresolved calls. All of these partitions have
explicit proof below; candidate test-name prefixes are not used as evidence.

`catalog.json` lists each exact compiled test identity at both owning layers.
Fixture names below identify independent source/edit/metadata oracles consumed
by those tests. This review records assertion scope; only an executed audit can
mark its catalog mappings verified.

| Partition | Shared fixture(s) | Positive and negative assertions |
|---|---|---|
| Source functions and imported/package ownership | `completion-package-callables`, `callable-imports` | Complete candidate sets, source ownership, explicit edits, post-edit signature and exact declaration; aliases, current module and dependency qualification; private, unreachable transitive, missing/ambiguous imports and shadowing cannot borrow another callable's parameters. |
| Source inherent/trait/default methods | `completion-package-members`, `completion-members` | Owner-specific parameter/return facts, trait/default methods and aliases; apply call edits and query signature/definition; other owners, missing members, dynamic and erased return receivers have exact empty or remaining sets. |
| Schema functions and methods | `completion-schema-callable-lifecycle`, `completion-members` | Exact candidates, lazy documentation and edits; sync/async replacement, changed parameters/return owner and removal/restoration; retained resolve payloads cannot return removed docs. Source precedence and schema-only null definitions remain explicit. |
| Standard functions and methods | `completion-builtin-callable-resolve`, `completion-stdlib-arguments` | Canonical/aliased functions, namespace aliases and String/Array/Map/Option/Result methods; exact builtin identity, complete candidates, edits, applied signatures and null source definitions. Actual builtin items retain no lazy docs, even with colliding schema method documentation. Dynamic, unresolved and missing members produce no guessed candidate. |
| Named/defaulted parameters | `completion-named-arguments`, `completion-stdlib-arguments`, `completion-service-arguments`, `completion-callable-hints` | Complete available parameter and expression sets, named-role detail, default marker and independent edit; occupied positional/named slots, reordered names, existing equals, unknown targets/owners, views/nested hints and registered native parameter prefixes. Apply/requery keeps the current name editable without duplicating equals. Every actual parameter item lacks docs/payload; repeated protocol resolve preserves the whole item and parse/HIR counters. |
| Active parameter and expected argument facts | `call-parameter-mapping`, `callable-imports` | Explicit signature, semantic parameter index, expected name/type and unchanged syntactic ordinal; named/reordered/unknown/duplicate slots and overflow. Protocol also inserts a non-BMP prefix and verifies the same active slot. |
| Argument expressions and callable values | `completion-call-expressions`, `completion-callback-contracts` | Complete choices and distinct name/value/call insertion; local shadow/captures, nested calls/blocks/lambdas, incomplete close, future names, source/schema callbacks, local callable and dynamic/missing calls. Source/builtin resolve payloads preserve the supported empty-doc result. |
| Returned receiver and origin boundaries | `completion-return-flow` | Root/dependency function, method, trait, await, block and local-lambda return chains; exact candidate/return owners and applied targets. Unknown trait owners, Any returns and terminated blocks do not invent members or schema documentation. |

The standard callable fixture adds eleven cases (eight positive, three empty),
each repeated with LF and CRLF. Unicode precedes each completion on its line.
Builtin symbol identities and signatures use the existing semantic type display
(`Array(i64)`, for example); source hints retain their documented angle brackets.
The static numeric function fact remains `i64 | f64`, rather than being widened
to the raw registry manifest's placeholder `any`.

Resolve policy is deliberately explicit. Currently only schema identities
provide lazy documentation. Source and stdlib callables still expose semantic
details and resolvable identities, but their documentation result is empty.
This review does not introduce source-doc or stdlib-doc rendering. Parameters
carry no lazy identity. At the protocol boundary, malformed payload tests assert
InvalidRequest and a successful subsequent request; unknown well-formed symbols
preserve the supplied item, including all edits and labels.

Shared drivers assert service byte offsets and protocol UTF-16 positions against
fixture markers, apply returned edits, and check the resulting source. Schema
lifecycle queries compare current state with fresh analysis. B03 remains open
for its other syntax/state/editor obligations; this review does not certify
S1/S2/S6-S11/S13/S14, whole-feature editor behavior, B14 partition expansion,
generated combinations, or scale budgets.
