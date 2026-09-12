# LSP Test Strategy And Executable Coverage Matrix

Status: the coverage inventory and audit gate are implemented. Full semantic,
stateful, and installed-editor acceptance is still open. The protocol document's
historical acceptance does not imply that every combination below has executable
proof. A bug report is a final source of regression cases, not the test plan.

For goal-driven implementation, follow the stable batches, commit gates, and
resume contract in [lsp-test-execution-plan.md](lsp-test-execution-plan.md).
P0-P5 below describe acceptance dimensions; they are not individual commit units.

The [VS Code interaction matrix](lsp-vscode-interaction-matrix.md) adds mandatory
user workflows and input/render evidence to P4. Its UX01-UX24 scenarios cover
widgets, keyboard/mouse routes, undo/recovery, remote hosts, upgrades and declared
extension coexistence. They are planned requirements, not current test coverage.

Current acceptance prioritizes complete coverage on one selected registered local
profile per run. Windows and macOS development machines share implementation
progress, with independent evidence and profile audit records; switching machines
does not reset the next task. Each strict gate requires fresh proof on its selected
profile for all accepted batches, and cannot combine bundles across profiles.
UX01-UX18 and UX21, all local semantic/state/range obligations, and P5
generated/scale checks remain required. B16's environments beyond the registered
local machines, additional editor versions,
remote hosts, upgrade/coexistence profiles, keyboard remaps and theme/zoom sweeps
are deferred follow-up work and do not block local acceptance. Preserve those
families separately; do not count them as verified or N/A. Existing CI remains
in place, but expanding environments or scheduling is outside this goal.

## Source Of Requirements

- [grammar.ebnf](grammar.ebnf) defines the documented grammar. The current
  inventory contains 133 productions; productions are not individual test cases.
- `vela_syntax`'s token, syntax-kind, and AST contracts describe additional
  implemented details such as bytes/numeric suffixes. Their reviewed fingerprints
  prevent a changed contract from silently retaining the old matrix.
- [lsp-protocol-test-matrix.md](lsp-protocol-test-matrix.md) defines 41 protocol
  rows, their S0-S14 applicability, and positive/negative contracts.
- The audit launches the actual `vela_lsp_server` binary and compares advertised
  capability keys. Existing lifecycle tests remain responsible for exact provider
  options, triggers, legends, and unsupported-request behavior.
- [catalog.json](../tests/lsp_matrix/catalog.json) and its feature/baseline files
  bind these requirements to coverage obligations and reviewed evidence.

Grammar names, contract fingerprints, protocol rows, feature applicability, and
live capabilities must agree. A changed grammar, AST/token contract, protocol
requirement, or capability forces matrix review in CI. Updating a fingerprint is
an acknowledgement of that review, not evidence that the new behavior was tested.
Parser changes that preserve all tracked contracts still require the author's
semantic review; a source inventory cannot infer every behavior change.

## Axes And Applicability

| Axis | Required partitions |
|---|---|
| Declarations and ownership | Public/private functions, parameters/defaults, locals/shadowing/captures, const, state/extern state, structs/fields, enums and unit/tuple/record variants, traits/default methods, inherent/trait impls, modules/import aliases, source-backed/schema-only/builtin ownership. |
| Expressions and control flow | Assignment/compound writes, nested blocks, if/match/guards, loop bindings, break/continue, closures/callbacks, constructor/shorthand fields, static paths versus member access, indexing, chained/returned receivers, unary/binary/range operators. |
| Type and callable facts | Primitives, tuples/unit, nested Array/Map/Set/Iterator/Option/Result contracts, source/schema types, Any/unknown, function/method/trait/stdlib/host calls, named/defaulted args, async/await, static Service and scoped-task paths. |
| Lexical and recovery | All literal/token families, interpolation, comments/shebang/trivia, incomplete item/member/call/type/pattern, malformed neighbor, empty file, cursor start/middle/end and EOF. |
| LSP feature | Every advertised method, resolve/delta/range variants, lifecycle/sync/watch/configuration, plus explicit rejection of unadvertised methods. |
| Document lifecycle | Disk-only target, opening importer/definition, unsaved edit, save-independent operation, close-to-disk, reopen, create/change/delete/rename dependency, schema replacement, stale versions, cancellation. |
| Position and client behavior | ASCII, Chinese, non-BMP characters, LF/CRLF, multiline prefix shifts, percent-encoded paths, spaces/percent signs, locally executable Windows drive-spelling fixtures, and minimal/editor-like client capabilities. |
| Deferred execution environments | Additional supported OS and minimum/current VS Code profiles, plus the interaction matrix's B16 variants. These do not gate current local acceptance. |

The current executable expansion contains 1327 obligations: applicable syntax
dimensions in both polarities at their owning test layers, selected document
states, encoding/URI environments, and installed-editor feature smoke checks.
This number counts requirements, not tests and not all possible Cartesian pairs.
The JSON report enumerates every obligation with a stable ID, for example:

```text
definition/syntax/S4/positive/service
definition/syntax/S4/negative/protocol
definition/states/dirty/protocol
definition/environments/utf16/protocol
definition/editor/smoke
```

S dimensions are broad acceptance groups. Before certifying one, cover every
applicable subcase in its protocol contract or split it into smaller obligations.
A single struct-field test cannot certify all source/schema fields, methods,
traits, constructors, and variants in S4. Evidence may link several exact test
identities to the same obligation. Negative tests must assert the chosen
null/empty/diagnostic/rejection policy, not merely that no exception occurred.

Unsupported features require negative proof. A genuinely inapplicable cell needs
a reviewed, concrete reason in `exemptions`; a known defect, inconvenient fixture,
or missing test is not N/A. The unused, conditional `workspace/configuration`
positive path is currently exempt; unsolicited response handling is still an
applicable negative requirement.

## Combination Policy

1. Cover every meaningful symbol/feature semantic partition, with positive and
   negative expectations. Source fields, trait methods, schema methods, and
   dynamic receivers cannot substitute for each other.
2. Exhaust the high-risk combinations: cross-file target × dirty buffer × UTF-16;
   rename/edit application × Unicode × multiple documents; schema reload ×
   returned receiver × dynamic boundary; cancellation × stale generation × edits;
   token full/delta × multiline edits × CRLF/non-BMP characters.
3. Use constrained pairwise generation for other independent factors, such as
   line ending, cursor offset, and disk/open state. Record the seed and selected
   tuples, and prove every valid required pair is represented. Pairwise generation
   is a planned next stage, not a currently implemented generator.
4. Run all current local scenarios on one recorded OS and exact supported VS Code
   version. Repeating them on minimum/current versions and other platforms belongs
   to deferred B16. Current CI runs Windows/Linux stable and Linux 1.90.0; its
   environment expansion is not a prerequisite for completing local coverage.

The test oracle must remain independent from the implementation under test.
Expected target/edit markers come from fixtures; never fill expected ranges by
asking the same provider whose ranges are being tested.

## Shared Fixture And Assertion Contract

Use small multi-file fixtures with named markers for declarations, references,
cursor positions, ranges, and edits. Strip markers before analysis and derive
byte/UTF-16 positions from the stripped source. Give scenarios stable IDs and
record the grammar/symbol partitions, feature, initial state, actions, and exact
oracle. The shared corpus under `tests/lsp_matrix/fixtures/` now feeds both Rust layers
and the installed VSIX suite. Its independent marker/edit/lifecycle self-tests
are implemented; expanding semantic coverage remains the feature batches’ work.

| Feature family | Required oracle |
|---|---|
| Definition/declaration/type definition | Exact target URI, start/end range, selected source text; distinguish the three methods; negative cases must not jump to an enclosing function. |
| Completion/resolve/signature/hover | Exact candidate inclusion/exclusion, ownership and rank where specified, insertion/replacement range, resolved docs, active parameter, rendered type, hover span. Apply completion edits to check the resulting source. |
| References/highlights/rename | Complete reference set, shadow separation, read/write classification, versioned nonoverlapping edits, collision/ownership rejection. Apply edits across files and query again. |
| Diagnostics/actions | Code, severity, exact range, related locations, appearance/clearing after edits. Applying a fix removes the intended diagnostic and preserves unrelated code. |
| Symbols/folding/selection/hierarchy | Symbol kind/parent, ranges and ancestry, exact incoming/outgoing edges, receiver ownership, exclusion of unknown targets. |
| Semantic tokens | Decode every token; assert type/modifiers, UTF-16 length, order, bounds, and source text. Applying deltas must produce the same stream as a fresh full request. |
| Formatting | Apply edits; assert exact text, trivia preservation, parseability for valid inputs, idempotence, and unchanged text outside the allowed range. |
| Inlay hints | Position, label/kind, parameter identity, requested-range containment, and suppression for explicit/dynamic/unknown facts. |
| Lifecycle/transport | Real initialization and synchronization order, disk loading, capability options, bounded response/termination, invalid-message rejection, and no stale publication. |

Queries must never execute Vela code, run the host application, or read live host
state. Schema fixtures are static metadata produced through the existing schema
contract. Preserve the language-service/protocol/editor ownership boundaries.

## Stateful And Metamorphic Tests

Add deterministic action sequences, not only fresh-workspace snapshots:

```text
load disk -> open importer -> query unopened definition
-> edit definition without saving -> query importer
-> introduce syntax error -> query unaffected symbol
-> repair -> close definition -> query restored disk
-> delete dependency -> verify stale locations disappear
```

At each stable step, compare incremental results with a freshly initialized
workspace containing the same disk files, overlays, schema, and configuration.
This catches stale caches and missing invalidation without needing a bug report.

Systematic transformations should preserve known relations:

- Prefixing lines shifts locations by the known line count.
- Inserting Chinese/emoji before a marker changes UTF-16 columns by the expected
  code-unit count, not UTF-8 byte count.
- LF/CRLF variants preserve logical line/column locations.
- Unrelated comments/declarations do not change the chosen symbol.
- Renaming a local and applying the returned edits preserves its reference set.
- Formatting twice produces no further change.
- Token delta application equals a fresh full-token response.
- Reopening or constructing a fresh workspace produces equivalent semantic facts.

All generated sequences need finite steps, per-request deadlines, fixed seeds,
and shrinking/minimization of failures. Performance checks belong in a separate
scale lane with explicit latency/memory budgets and recorded machine context.

## Commands And Evidence States

From the repository root:

```bash
# Validate inventory, discover compiled tests, and probe actual capabilities.
node scripts/lsp-matrix/run.js

# Also execute the language-service and protocol library tests.
node scripts/lsp-matrix/run.js --run

# Full current local workflow: installed VSIX plus the Rust matrix audit.
npm --prefix editors/vscode ci
npm --prefix editors/vscode test

# Reuse a specific editor result only if its source and binary hashes match.
node scripts/lsp-matrix/run.js --run --editor-results <run-directory>/results.json

# Acceptance gate; intentionally fails while required evidence remains open.
node scripts/lsp-matrix/run.js --run --strict --editor-results <run-directory>/results.json
```

Each audit retains its report and Cargo/Node logs under a distinct
`target/lsp-matrix/run-*` directory. `target/lsp-matrix/report.md` and `report.json`
are copies of the latest report. CI uploads all runs with editor/server failure logs. Evidence
states are:

- `unreviewed`: no complete reviewed proof linked. Candidate test names aid the
  audit; they are never counted as coverage.
- `mapped`: exact compiled test identities and an assertion are linked, but all
  of them have not passed in this audit.
- `verified`: all linked tests ran successfully in this audit. Supplied editor
  evidence must match current fixture/launcher/dependency inputs, server binary,
  and platform; old result files are not auto-discovered or trusted.
- `failed`: a linked test failed or was ignored. Ignored tests cannot prove a
  required behavior.
- `not_applicable`: an explicit reviewed reason excludes the requirement.

Ordinary CI rejects catalog drift, stale or wrong-layer references, invalid N/A
entries, and failing tests. It currently allows unreviewed cells so the audit can
land incrementally. `--strict` is the separate acceptance gate and requires every
applicable cell to be verified for the current execution profile. B19 requires
this full local gate and the added local interaction/generated/scale gates.
Cross-environment release certification later requires B16 evidence; local
acceptance does not claim that certification. Missing local tests or failures
cannot be deferred under the environment follow-up.

## Execution Plan And Exit Gates

| Phase | Work | Exit proof |
|---|---|---|
| P0 — inventory | Classify current grammar/protocols, expose obligations and candidate tests, add drift/evidence checks. | Implemented: reproducible report and checker self-tests; no claim of complete semantic coverage. |
| P1 — semantic audit | Review existing service/protocol assertions by syntax partition; add missing cases before changing product behavior. | Every applicable positive/negative semantic obligation links exact independent assertions; no broad cell certified by one incidental test. |
| P2 — ranges and edits | Run all range-bearing features through LF/CRLF and Unicode transformations, apply edits and decode tokens. | Every outgoing range/edit/token family has exact conversion and transformation proof. |
| P3 — lifecycle/schema | Stateful disk/overlay/dependency/config/schema/cancellation sequences with fresh-workspace oracle. | All state obligations verified; no stale results, guessed facts, or unbounded waits. |
| P4 — editor workflows | Shared scenarios for every advertised user-facing feature through installed VSIX. | Each editor obligation passes with current hashes on the recorded local profile. |
| P4 — user interaction | Execute UX01-UX18 and UX21 routes through the local workbench with exact resulting-state and rendering assertions. | Every mandatory local route has matching evidence at its required level; provider or command smoke cannot substitute for actual input. B16-owned families remain deferred. |
| P5 — generated/scale gates | Constrained pairwise generation, fixed-seed state machines, mutation checks, and dedicated scale budgets. | Recorded combination coverage, useful minimized failures, and strict acceptance without unexplained skips. |

Start P1/P2 with navigation, completion, references/rename, diagnostics/actions,
and semantic tokens. For the initial Unicode navigation proof, both definition
and declaration now run with LF and CRLF and with Chinese/emoji before the query
and target; one fixture is deliberately transformed rather than copied into
several unrelated regression tests.

Upstream contracts: [LSP 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
and [VS Code extension testing](https://code.visualstudio.com/api/working-with-extensions/testing-extension).
