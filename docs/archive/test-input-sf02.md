# SF02: Syntax, Renderer And Parser UI Evidence

## Scope And Result

Implemented against SF01 commit `bdc23b480`, following the SF02 rows in
`docs/test-input-census-runtime.md` and UI-P in `docs/test-organization-plan.md`.
Ownership: `crates/vela_common`, `crates/vela_syntax`, and this evidence file only.
No Engine, shared documentation, Cargo manifest, dependency, production feature,
framework, schema, bless mode or commit changes. Other workers' changes are not
part of this report.

SF02's selected source organization and UI-P work is complete. Formatter
reparse/idempotence is covered here; before/after executable equivalence is an
explicit Engine SF06 handoff below, not claimed by parser success. Common was
reviewed and deliberately left unchanged. Existing test target/module ownership,
SourceIds, logical filenames, typed assertions and recovery/EOL generators remain.

## Complete Family Review And Dispositions

Paths in this section are relative to `crates/vela_syntax` unless stated otherwise.

| Census family | Completed disposition and concrete rationale |
|---|---|
| Common `src/diagnostic_render.rs` | Retain all three short source documents inline: primary/related-label offsets, multiline highlights and gutter alignment are clearest beside their manufactured diagnostics. The fourth test has no source document. All four renderer tests remain unchanged; they do not claim real parser or host-write acceptance. |
| Common `src/{lib,capability,interop_type,primitive}.rs` | Retain unit data: IDs, spans, permission bits, scalar tags, type/identity conversions and diagnostic builders. No substantial Vela source to extract; production error messages are not test input. |
| Parser signature/schema/trait/block/postfix/container/match/loop catalogs | Extracted 13 programs to `tests/fixtures/parser/`: `function_signature`, `struct_fields`, `enum_variants`, `trait_impl_methods`, `block_statements`, `statement_expressions`, `postfix_expressions`, `containers_lambdas`, `match_patterns`, `unseparated_match_arms`, `for_patterns`, `use_const_state`, `contextual_state`. Retained every original item/node/accessor/order/range assertion in `src/parse/tests.rs`; these are grammar demonstrations, not necessarily semantically executable programs. `use_const_state` intentionally still contains removed extern-state syntax and its rejection assertion. |
| `src/ast/expr_tests.rs` | Extracted `ast_expr_variants`, `ast_binary_operators`, `ast_update_operators`, `ast_path_delimiters`. These are multi-operator/node catalogs with long ordered observations. Retained the small block/map contrast, parentheses, direct-operand precedence, unary pair, named-call arguments, lambda argument, self/field/tuple/index access, two-entry map, shorthand record, two-arm match, missing guard and if-as-binary-operand examples inline: each tests one accessor distinction, and moving its short wrapper would separate the relevant spelling from the assertion. |
| `src/ast/items_tests.rs` | Extracted `item_visibility`, `declaration_names`, `type_hint_delimiters`, `default_values`, `impl_headers`. They compare multiple declarations, header forms, or nested delimiters. Retained the one-function unit/tuple hint example inline (three closely related parameter hints). All `SyntaxTreeBuilder` token-ownership tests remain unit data: replacing their hand-built nodes with parsing would change the tested boundary. |
| `src/ast/syntax/statement_tests.rs` | Extracted `statement_variants`, `for_patterns`, `statement_tokens`, `let_path_patterns`. Retained multiline postfix/infix/condition probes and the missing-operand-versus-body-block comparison inline: exact line boundaries or the adjacent damaged/valid operand contrast are the observation, not a reusable program. Retained the single tuple-binding probe and builder tokens. The iterable/tail Cartesian product remains generated, not expanded into files. |
| `src/ast/attributes.rs`, `patterns.rs`, `syntax.rs` | Extracted the whole-document nested attribute argument example (`attribute_arguments`) and six-way pattern-kind catalog (`pattern_variants`). Retained the four-arm path/binding/boolean/wildcard contrast and single record-payload pattern inline: these are compact classification/accessor controls. Retained literal-pattern spelling/semantic-value tables, including radix/suffix/byte escapes, beside their exact typed values. `syntax.rs` contains small hand-built node/token controls, not a parser program corpus. |
| Parser removed async/extern/borrow/release, legacy state, generic/null and malformed rows; `parse/tests/item_boundaries.rs` | Narrow rejection/control tables, malformed Unicode seed and default-block/neighbor boundary probe stay inline. The existing missing-function-name source is shared with UI-P via `include_str!`, retaining its original SourceId 12 and recovery assertions. New UI-P coverage strengthens diagnostics independently of these existing checks. |
| CST recovery files `{bracket,paren,pattern,expression,declaration,method,map}_recovery_tests.rs`, `parameter_default_tests.rs`, `cst_expr/records.rs` | Retain generated LF/CRLF, Unicode prefix, context, operand and repair/redamage products. Retain fixed short method/map/default/record fragments alongside their delimiters/neighbor observations. Their constructors intentionally vary source positions and owner boundaries; fixed files would obscure or reduce that coverage. |
| AST `{call_recovery_tests,expr_lambda_tests,expr_path_tests}.rs`; validation `recovery/{tests,required_child_tests}.rs` | Retain bounded operators/parameter counts/callback prefixes/newlines/required-child matrices and local valid/damaged controls. Small array-receiver lambda, zero-argument lambda and qualified-path controls remain inline; no substantial catalog was hidden in these helpers. |
| `ast/expr/literals.rs`, token/kind/node tests | Retain spelling/decoding unit data, including byte escapes, interpolation braces, ordered decoded text, multiline strings and missing expression structure. The literal catalog's ordering directly maps to typed radix/suffix/value assertions; externalizing it would obscure precisely the escape distinction being tested. Token/kind/tree unit-data tests remain unchanged. |
| Formatter | SF01's two extracted input/expected pairs and inventory remain unchanged. Arithmetic, use-newline, else, comments, shebang, hints and removed-async microcases remain inline. Added an explicit four-case retained-valid root conformance subset, not a broad glob over negative inputs or dependency modules. The existing container example still calls undefined `score`; it was not repaired or advertised as executable. |
| Existing generic-hint diagnostic fixture | Reused the existing root `tests/fixtures/diagnostics/generic_type_hint.{vela,expected}` without copying or modifying it. The original named integration test remains and now checks the entire typed diagnostic, label and repair as well as rendered output. |

The narrower retain-inline choices above refine the census's family-level
suggestions; they do not exempt whole owners or leave its large named starting
examples untouched. There is no zero-inline or fixture-count target. No
near-matching parser/AST inputs were merged. All 28 grammar fixtures have one
owning direct Rust consumer; the missing-name UI input has both the existing
parser-unit consumer and the new integration consumer.

## Evaluated Byte And Assertion Proof

29 existing literals were extracted: 13 parser catalogs, 15 AST catalogs and the
57-byte missing-name source. A one-off Rust program evaluated the original raw or
cooked literals and compared bytes directly to each fixture before replacement.
A second Rust `assert_eq!(original_literal.as_bytes(), include_bytes!(...))` pass
against `bdc23b480` verified all 29 final files. No dedenting, trimming, line-ending
normalization or EOF insertion was used. In particular, leading newlines in
`default_values`/`attribute_arguments` and the cooked continuation stripping in
`let_path_patterns` remain exactly as Rust evaluated them.

For all six extraction-bearing Rust files, reconstructing the baseline with only
literal-to-include replacements and scoped rustfmt produced exactly the final
file: all old assertion expressions, test names, SourceIds and control flow are
unchanged. Temporary Rust evaluators were removed. The following SHA-256 prefixes
are review references, not substitutes for the direct byte comparisons. Paths
are under `crates/vela_syntax/tests/fixtures`; all end in `.vela`.

| Fixture | Bytes | SHA-256 prefix |
|---|---:|---|
| `parser/use_const_state` | 164 | `b93cd2a5d6ac66e3` |
| `parser/contextual_state` | 244 | `876d7f8c4144b995` |
| `parser/function_signature` | 122 | `2c832af96ea1beb3` |
| `parser/struct_fields` | 117 | `f901639b810c5527` |
| `parser/enum_variants` | 148 | `480bee403c7b0018` |
| `parser/trait_impl_methods` | 301 | `25f501088b61a30c` |
| `parser/block_statements` | 337 | `478861c815387762` |
| `parser/statement_expressions` | 194 | `5db3116ee624ba6c` |
| `parser/postfix_expressions` | 105 | `8246b87978de178f` |
| `parser/containers_lambdas` | 287 | `50996ede468b014f` |
| `parser/match_patterns` | 259 | `96d7ec7e4f8af53c` |
| `parser/unseparated_match_arms` | 165 | `a35d213bf7cf2ccf` |
| `parser/for_patterns` | 158 | `022701b05d9ad106` |
| `ast/ast_expr_variants` | 611 | `7bbeded7dbe8a150` |
| `ast/ast_binary_operators` | 587 | `fb2472e30157b953` |
| `ast/ast_update_operators` | 116 | `b648631c2437c41c` |
| `ast/ast_path_delimiters` | 405 | `869d2712f0fc019b` |
| `ast/item_visibility` | 239 | `1ad535fb5a8ca227` |
| `ast/declaration_names` | 388 | `4cfd7d124b81cc36` |
| `ast/type_hint_delimiters` | 132 | `57209374f77469a4` |
| `ast/default_values` | 234 | `2393fbb54f38fe2f` |
| `ast/impl_headers` | 142 | `f3a3d3cc309b9bc7` |
| `ast/statement_variants` | 278 | `670aa235a7889e46` |
| `ast/for_patterns` | 166 | `6d75df92f1f0f5af` |
| `ast/statement_tokens` | 243 | `d4e604a83ad974c6` |
| `ast/let_path_patterns` | 215 | `11e7df4c3c2e12d3` |
| `ast/attribute_arguments` | 143 | `0584dcfffb16666c` |
| `ast/pattern_variants` | 233 | `81154eb5434e206b` |
| `ui/parser/missing_function_name` | 57 | `cb67eb65fc85b385` |

`tests/fixtures/.gitattributes` scopes LF rules to owner-local `.vela` and
`.expected` files. `git check-attr text eol` verified `text=set`, `eol=lf` on all
34 new source/golden files; byte reads found no CR characters. Existing SF01
formatting-specific rules remain in place.
Generated CRLF/mixed-byte inputs were not rewritten or made subject to fixture
normalization. Shared root inputs remain unchanged and are repository-test
inputs, not new production package dependencies. No isolated published-package
validation is claimed.

## UI-P Contract And Discovery

Target: `cargo test -p vela_syntax --test diagnostic_fixtures --locked`.

- Three `rstest #[files("tests/fixtures/ui/parser/*.vela")]` cases pair each
  source with its same-stem `.expected`; there is no Cartesian product.
- `missing_function_name`: one `E_PARSE`, span `3..4`, plus the original unit
  recovery observation of two function items.
- `recovered_declaration`: one `E_PARSE`, span `0..21`. Independent recovery
  checks retain const name/type, absent initializer, and the healthy following
  function/body.
- `removed_async`: exactly two `syntax::removed_async` diagnostics in order,
  spans `4..9` and `28..33` (declaration and await).
- The existing named generic-hint case retains one diagnostic at `17..18`, its
  exact related label, and the `17..22` remove-type-arguments repair.
- Typed `Diagnostic` equality checks count, order, severity, code, message,
  primary span, labels, candidates and repairs. Every actual primary/label/repair
  span must have SourceId 1, ordered in-bounds endpoints and UTF-8 boundaries
  before rendering. Lossless source reconstruction is also checked.
- New goldens compare the complete rendered stream byte-for-byte, with exactly
  one final LF. Only the existing root generic-hint helper retains its prior
  explicit CRLF/`trim_end()` policy; that policy was not spread to the new lane.
- Four named valid controls repair the missing name/const initializer/async
  syntax/unsupported generic hint. They require zero diagnostics, lossless text,
  exact function/const names and counts, present bodies and present const initializers.
- Inventory rejects empty discovery, missing/orphan pairs, unexpected entries
  and sources without typed expectations. The explicitly named shared root pair
  is checked separately; unrelated owners' root goldens are not globbed.

`build.rs` now watches `tests/fixtures/ui/parser` as well as SF01 formatting.
Actual `--list` results, without cleaning: **3 -> 4 -> 3**. The temporary
`sf02_discovery_probe.{vela,expected}` pair created the fourth identity
`parser_ui_diagnostics::path_4_tests_fixtures_ui_parser_sf02_discovery_probe_vela`;
removal restored exactly the original three identities. The probe was also
rejected by inventory because it had no typed expectation. Both files are gone.

Final target has **10 tests**: three discovered error cases, the existing root
case, one inventory, four named valid controls and one recovery test. Original
unit-test identities were retained; four formatter identities were added.

## Actual Negative Controls (Restored)

Each row below was executed, not inferred from code. Each selected test failed
with exit 101 and exactly one failed test, then its input was restored from the
saved original bytes. No intentionally failing tests or corrupt fixtures remain.

| Mutation | Actual failure |
|---|---|
| Change missing-name golden messages to `wrong expected message` | Full-output equality failed (`complete parser diagnostic output`). |
| Change typed missing-name expected span from `3..4` to `3..5` | Full typed diagnostic equality failed; count/message remained the same. |
| Append `fn extra() { value.await; }` to recovered-declaration input | Diagnostic count failed, actual 2 versus expected 1. |
| Remove missing-name `.expected` | Selected discovered test failed reading the golden; inventory independently failed missing/orphan set equality. |
| Add `sf02_orphan.expected` without source | Inventory failed missing/orphan set equality. |
| Add `sf02_unexpected.txt` | Inventory rejected unexpected extension. |
| Temporarily remove all six UI files | Just-built test binary's inventory failed `empty parser corpus`. Used the binary directly so rstest's no-files compile error could not mask the runtime empty assertion. |
| Add the discovery probe pair without typed registration | Inventory failed `no typed diagnostic expectations`. |

Cargo-driven controls used the real `diagnostic_fixtures` target, exact nonempty
test selections and 180-second per-command bounds. The empty-directory control
used the just-built `diagnostic_fixtures-8399a28fa2c53ad9` test binary with
`parser_ui_fixture_inventory --exact`. A temporary diagnostic inspection test
was removed; no runner/generator/bless tool was retained.

## Retained-Valid Formatter Subset / SF06 Handoff

`formatting::tests::formatting_retained_valid_conformance` explicitly includes
these existing root sources, without copying or changing them:

| Root fixture | SF06 entry / existing VM typed oracle |
|---|---|
| `tests/fixtures/conformance/assignments/local_operators.vela` | `main()` -> `I64(20)` |
| `tests/fixtures/conformance/assignments/index_reads.vela` | `main()` -> `I64(10)` |
| `tests/fixtures/conformance/control_flow/break_continue.vela` | `main()` -> `I64(8)` |
| `tests/fixtures/conformance/control_flow/block_tail.vela` | `terminated()` -> Unit; `nonterminated()` -> `I64(9)`; `defaulted()` -> Unit |

This stage checks original parse success/losslessness, formatting diagnostics,
formatted reparse success/losslessness, second-format diagnostics and exact
idempotence. Existing VM oracle bodies were inspected, not rerun or edited.
SF06 must compile/run original and formatted versions through Engine and preserve
these exact value tags and entries. Parser success is not execution proof. Do
not use the undefined-`score` container formatting fixture or negative primitive
conformance files as substitutes.

## Validation Results

All listed Cargo invocations were bounded (180 seconds individually, or 240
seconds for grouped owner test/Clippy runs). Mutation/discovery orchestration had
finite outer bounds and 180-second subprocess limits. Cargo lock contention was
observed (including roughly 17 seconds in one probe), but **no command timed out**.
These are scoped owner gates, not workspace/browser/editor or performance claims.

| Command / phase | Actual result |
|---|---|
| Baseline `cargo test -p vela_common -p vela_syntax --locked` | 53 Common + 129 Syntax unit + 1 diagnostic fixture passed; zero failures/ignored; both doc-test targets had 0 tests. |
| Post-extraction `cargo test -p vela_syntax --lib --test diagnostic_fixtures --locked` | 129 unit + 10 integration passed before adding formatter subset. |
| `cargo test -p vela_syntax --lib formatting_retained_valid_conformance --locked` | 4 passed, 129 filtered, zero failed. |
| Final `cargo test -p vela_common -p vela_syntax --locked` | 53 Common + 133 Syntax unit + 10 diagnostic fixtures passed; zero failures/ignored; 0 doc tests. |
| Final `cargo test -p vela_common -p vela_syntax --all-features --locked` | Same 53 + 133 + 10 passed; Common's affected `serde` feature included; zero failures/ignored; 0 doc tests. |
| `cargo clippy -p vela_common -p vela_syntax --all-targets --locked --no-deps -- -D warnings` | Passed. |
| Same scoped Clippy command with `--all-features` | Passed. |
| Scoped `rustfmt --edition 2024 --check` | Passed for the nine changed Rust files below; no workspace formatting. |
| `git diff --check -- crates/vela_common crates/vela_syntax` | Passed. |
| Final byte/assertion/fixture-consumer audit | 29 exact evaluated-byte matches; six extraction modules differ only by literal/include substitution plus formatting; all 28 grammar fixtures have one direct owner consumer. |

Scoped Rust formatting files: `build.rs`, `src/formatting.rs`,
`src/parse/tests.rs`, `src/ast/{attributes,expr_tests,items_tests,patterns}.rs`,
`src/ast/syntax/statement_tests.rs`, and `tests/diagnostic_fixtures.rs`, all under
`crates/vela_syntax`. Common has no edits. Shared status/validation documents
remain parent-owned; this file supplies the actual commands and handoff rather
than editing those files. Parent owns commits.
