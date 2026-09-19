# Completion and resolve: S10 review

Scope: the eight `completion[-resolve]/syntax/S10/{positive,negative}/{service,protocol}`
obligations and `completion-resolve/states/dynamic/protocol`. Every ownership
partition in the S10 baseline has explicit proof below. The catalog binds exact
compiled tests at their owning layers; an executed audit determines whether
these reviewed mappings pass.

| Partition | Shared fixture(s) | Assertions |
|---|---|---|
| Local and parameter | `completion-resolve-identities` | Let, parameter, nested shadow, capture, typed lambda parameter, Any parameter and unresolved local value retain exact kind/detail and applied local declaration ranges. They have no symbol or lazy payload and cannot borrow colliding schema docs. Actual protocol resolve preserves the whole candidate. |
| Source declaration | `completion-type-ownership`, `completion-package-callables`, `completion-import-aliases` | Exact struct/enum/trait/function/constant identities, aliases and qualified/dependency ownership; complete candidate sets and applied definitions/signatures; private, missing, duplicate and shadowed imports cannot resolve to unrelated facts. Source documentation remains empty. |
| Source member | `completion-package-members`, `completion-members` | Fields and inherent/trait/default methods retain owner-specific details, parameters/returns and applied targets; wrong-owner and erased receiver candidates remain absent. Source identities do not borrow schema docs. |
| Source variant | `completion-enum-aliases` | Unit, tuple and record variants in value/pattern contexts, imported and namespace aliases, and constructor fields retain source ownership and exact applied targets. Private, missing, duplicate, non-enum and shadowed owners produce explicit empty sets. |
| Schema and host fact | `completion-type-ownership`, `completion-members`, `completion-enum-aliases`, `completion-schema-callable-lifecycle` | Exact schema identities and lazy type/member/variant/callable documentation; source precedence, receiver boundaries and schema-only null source definitions. Changed/removed schema facts cannot leave stale documentation on retained resolve payloads. |
| Standard library fact | `completion-builtin-callable-resolve`, `completion-import-aliases` | Canonical and aliased standard functions/methods keep builtin identity, exact signature and insertion. No source definition or borrowed colliding schema docs; dynamic/unresolved/missing members remain empty. |
| Builtin fact | `completion-builtin-types`, `completion-resolve-identities` | Public builtin type spellings keep exact builtin identity and empty documentation; erased boundaries do not gain members. Boolean values use label insertion, have no explicit text edit or lazy identity, and resolve unchanged. Their fixture applies the label at its marked word range and checks parsing; this is not an explicit-edit or editor-acceptance claim. |
| Module | `completion-type-ownership`, `completion-import-aliases` | Source, schema and builtin namespace aliases retain module identity and exact insertion; applying the namespace selects the intended member owner. Initial items are lightweight and actual resolve preserves the whole item because these fixture modules have no documentation. Unknown/private/duplicate namespace boundaries are explicit. |
| Dynamic Any and unresolved | `completion-resolve-identities`, `completion-members`, `completion-builtin-callable-resolve` | Known source/schema Any-return callables keep their own signature and resolve policy, while downstream member sets are empty. An independent signature query proves each callable is known before the empty-member assertion. Dynamic parameters and unknown receivers cannot acquire guessed members or schema docs. |

The new identity fixture has fifteen queries: eleven candidate queries and four
empty member queries. Both service and protocol repeat them with LF and CRLF;
Chinese and a non-BMP character precede each completion on its line. Candidate
sets, details, identities, byte/UTF-16 edits and applied source are independently
specified. Local declarations have marked targets, builtin values have no source
target, and known callables have exact post-edit signatures. Source and schema
functions returning Any are also queried directly, so the dynamic resolve proof
does not rely solely on an empty downstream list.

The import-alias driver now checks lightweight items, absent local payloads,
empty documentation for the fixture's actual owned candidates, and whole-item
protocol resolve preservation. It covers source/schema/builtin module candidates
as well as imported declarations. The general ownership driver resolves every
actual candidate, including candidates without a payload. Separate protocol tests
reject malformed payloads and preserve all supplied fields for unknown well-formed
source/schema/builtin/local identities.

Only schema identities currently supply lazy documentation. Empty source and
stdlib documentation is the supported contract, not proof of future documentation
rendering. This review does not certify the remaining syntax groups, UX04 editor
interaction, B14 expansion, generated combinations or scale gates. B03 stays open.
