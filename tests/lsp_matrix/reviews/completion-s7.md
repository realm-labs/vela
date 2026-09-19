# Completion: S7 review

Scope: `completion/syntax/S7/{positive,negative}/{service,protocol}`. Exact test
identities are bound in the catalog; a fresh audit establishes verification.

| Partition | Shared evidence | Assertions |
|---|---|---|
| Literal values | `completion-literal-operators` | Integer bases, signed/unsigned and float suffixes, strings/multiline strings, bytes, chars, booleans, unit, tuples, arrays, maps and set construction retain exact local candidate types and declaration targets. Public spellings follow the primitive contract; display details retain the existing parenthesized container notation. |
| Operators and operands | `completion-literal-operators` | Unary negation/not, all arithmetic, equality/identity/comparison, logical operators and both ranges retain exact result details. Operands, array/tuple elements, Map values, index expressions and interpolation expressions retain local candidates and applied reference ownership. Precedence and parentheses are explicit. |
| Indexing and member boundaries | `completion-literal-operators`, `completion-authoring-surface` | Array/Map/string/bytes indexing preserves result types. Literal strings/arrays/maps expose owned builtin methods with exact post-edit signatures and no colliding schema docs. Scalar, tuple, unit, dynamic and unresolved members never fall back to matching locals. Existing empty-dot inventories assert complete builtin receiver sets, including bytes and sets. |
| Literal content | `completion-literal-operators` | Strings, bytes, chars, multiline strings, raw interpolation chunks, escaped braces and unterminated literal tokens have exact empty candidate sets. A prefix immediately before an interpolation brace is still text; code inside braces remains eligible. An empty unfinished string also suppresses code candidates. |
| Map keys | `completion-literal-operators` | Source/schema hints, source and namespace aliases, schema aliases and type/value name collisions select the owned enum candidates. Used keys are excluded while the edited key remains eligible. Mid-token edits replace the full identifier. Untyped/nested/non-enum/private owners and quoted keys have exact exclusions; source Color excludes a colliding schema Counterfeit variant. Resolve uses only owned metadata. |
| Map values and recovery | `completion-literal-operators` | The value after a Map colon is an expression, including a qualified constant and an unfinished Map at EOF. A genuine nested type annotation remains a type position. The completed EOF edit parses and navigates to the marked local declaration. |
| Records and constructors | `completion-members`, `completion-enum-aliases` | Source/schema records, field labels/shorthand, payload variants and chains retain exact candidate sets, constructor edits, detail/resolve and applied owners. Used, unknown, private and non-enum owners have explicit exclusions. |

The new fixture has 113 queries, including 33 exact empty sets. Both layers run
LF/CRLF with Unicode before cursors and independently check complete candidates,
kind/detail, identity/resolve, byte/UTF-16 edits, parsed applied source and definition
results. Service queries repeat; protocol applies actual didChange and restores
the original query. Existing matrices complement these literal/operator partitions;
no generated-combination or runtime execution coverage is inferred.

Map-key completion identity and applied-key navigation are distinct. The existing
HIR binder converts bare Map key paths to logical string keys; MIR lowers those
keys directly. A suggested enum name therefore retains its source/schema metadata,
while definition on the applied bare key is explicitly null. The fixture does
not invent a variant reference or certify a runtime Map<Enum, V> conversion.

The fixes use CST ownership: Map values cannot inherit the type role of their
colon, and literal-content gating uses the prefix token so raw interpolation
boundaries remain text. Malformed literal tokens are recognized by the lexer's
Unknown token and literal opener; closed interpolation expressions use their own
CST tokens. Unclosed maps retain their last value through shared braced-content
bounds. A direct parser test proves lossless Map recovery and excludes ordinary
blocks. Map-key suggestions reuse the existing enum path owner resolver, expand
type-hint imports without value shadowing, exclude other entries and carry exact
identifier edits. No grammar productions or runtime semantics are added.

B03 still needs completion-resolve editor smoke and UX04 native Input/Render
evidence. This review does not certify those routes, scale, generated partitions
or fresh macOS execution. B16 stays deferred.
