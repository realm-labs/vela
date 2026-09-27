# Hover S10 ownership

Scope: four `hover/syntax/S10/{positive,negative}/{service,protocol}` obligations.
New exact tests are
`hover::ownership_matrix_tests::hover_ownership_matrix_preserves_exact_owners_and_shadowed_facts`
and `tests::hover::ownership::hover_ownership_matrix_preserves_exact_owners_and_shadowed_facts`.

The independently authored `hover-s10` has 120 queries in 27 physical inputs:
103 complete hovers and 17 explicit nulls with schema. LF/CRLF, present/physically
absent schema and start/interior positions produce 862 positions per layer, each
repeated. Absent schema changes 21 expectations: metadata disappears, host
parameters retain unknown, unresolved hint leaves retain Any without identity,
and a short registry type's absence exposes the existing short stdlib fallback.
There are 32 null queries without schema. Service checks whole metadata and exact
local/source/schema/builtin identity; protocol checks whole Markdown and UTF-16
ranges or explicit JSON null through real initialize/open/request dispatch.
Optional definition assertions add 90 positions per layer plus repeats. Source
coordinates come from markers, not a provider. Physical inputs remain unchanged.

| Ownership partition | Independent assertions |
|---|---|
| Local and parameter | Source function names collide with outer/inner lets, typed/Any/unknown parameters, typed lambda parameters, loop and tuple-variant bindings. Calls before a let and in its initializer retain the source function; later uses and captures retain the exact visible declaration. Leaving an inner scope restores the outer binding. Function copies and typed lambdas retain local Function facts and exact physical identities. Colliding schema types cannot supply local docs or replace unknown facts. |
| Source declaration | Source functions, struct/enum/trait types, imported and namespace aliases, and current-module functions retain their concrete owner, signature and docs. Qualified calls survive short local shadows. An unrelated same-named source function has a different signature/docs. Private functions cannot borrow an exact same-named schema function. Accepted S1/S8 matrices additionally certify constants and VM/extern state ownership. |
| Source member | Source fields, constructor labels, inherent methods, default trait methods and returned receivers retain source identity/docs despite conflicting host types and members. A schema-only member of the exact source type cannot extend that source type. Accepted S4/S5 matrices additionally cover required/override trait methods, writes, shorthand labels, async methods and ambiguous receiver sets. |
| Source variant | Unit, tuple and record constructors retain source variant identity, declared field-name display and docs despite same-named host variants. Missing variants and non-enum qualified constructors stay null. Accepted S4/S6/S8 matrices add patterns, fields, imported variants and alias ownership. |
| Schema/host fact | Functions, types, traits, fields, methods, trait methods, variants and modules retain registered identities, complete docs and available callable metadata. A registered source-backed function retains Schema identity while its definition points to the independently marked physical declaration; removing schema removes both. Metadata-only members cannot fabricate a source target. Accepted S8 additionally checks source-backed type/trait/module spans. |
| Standard library | Exact `math::max` and namespace ownership survives conflicting registry functions/modules, including a source-backed decoy. Aliases keep the canonical builtin function. Short local shadows retain their own facts while qualified standard calls remain available. Standard functions/modules have no source jump. Accepted S5 supplies static task/Service paths and async/Option/collection call contracts. |
| Builtin | Primitive/container hints and specialized Array/String methods retain builtin identity, exact type/method facts and null source definitions despite registry type decoys. Keyword/boolean tokens stay null. A complete Array hint retains its known element fact. Accepted S3/S7 cover other builtin hints, literals and operators. |
| Module | Source and registered namespaces and their aliases retain the intended owner. Source module identity includes package identity independently of its displayed label. A local shadow can be queried as a local but cannot serve as a namespace or lend a qualified terminal an owner. Standard namespaces retain their builtin contract. |
| Dynamic/unknown | Any/unknown locals and parameters retain their own exact lexical facts; suffixes cannot borrow fields or methods. Known source/schema Any-return callables keep their metadata while downstream members stay null. Missing bare/qualified names, variants and private paths stay null rather than borrowing a registered short spelling. |

The fixture exposed registry `math::max` overriding the standard hover. After
lexical/source ownership is checked, exact standard functions/namespaces now win
static metadata lookup. Shared SymbolTarget lookup and hover projection use the
same order, so a registered source span cannot create a false standard-library
jump. Short registry names retain precedence over short-name stdlib fallback;
actual source declarations, source modules, local shadows and visibility checks
remain authoritative. Querying a registered bare `max` type explicitly checks
this distinction. No grammar, runtime or host-state access is added.

Oracle details follow existing primary contracts: lambdas use pipe syntax;
source modules use `PackageId + ModulePath` identity; source variant display
lists declared field names while field/parameter hovers own their types; complete
container hints preserve known elements; absent schema leaves become Any in hint
metadata and unknown in parameter facts. No expected value comes from a provider.

This review does not close hover lifecycle/S11, installed hover smoke or full
UX10 input/render routes. Windows/macOS acceptance requires independent fresh
evidence on one registered profile and cannot combine bundles.
