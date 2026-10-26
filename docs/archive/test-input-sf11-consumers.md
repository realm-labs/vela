# SF11 — Site And Editor Consumer Slice

## Scope And Result

Implemented the site/editor slice of [the plan](../test-organization-plan.md) and
[consumer census](../test-input-census-consumers.md), not the benchmark/fuzz slice
or all of SF11. Writes are confined to `site`, the Tree-sitter package test
commands, a Zed test script, a new WASM integration test, and this report. No
runtime code, shared status/design docs, SF10 matrix/VS Code test readers,
production queries/grammars, dependencies, or commits were changed. Existing
SF08 edits to `vela_playground_wasm/src/lib.rs` and its local reward fixture were
preserved, not authored by this slice. Other owners' working-tree edits remain.

**Passed:** exact actual-site native compile/run coverage, emitted site payload
byte/entry equality, current retained-positive parsing and capture/range tests
for all six shipped queries, affected site/native/wasm32/package-text gates.
**Not claimed:** a distributable Cargo archive, standalone repository-query
execution from the grammar npm package, installed editor/native Input/Render
acceptance, or actual browser Playground execution.

## Actual Site Inputs: One Authored Origin

- `site/src/data/playground/*.vela` owns the four source byte sequences.
- `site/src/data/playground-examples.json` owns titles, entries and source-file
  selection. `playground-examples.ts` imports that table and the raw `.vela`
  files through Vite's eager raw glob, preserving the existing
  `PLAYGROUND_EXAMPLES` interface used by `Playground.astro`.
- `crates/vela_playground_wasm/tests/site_examples.rs` reads the **same** JSON
  records and source files at runtime, calling existing `compile_script` and
  `run_script` exports. It passes each advertised entry to `run_script`; it does
  not use a similar Rust source or hardcoded execution entry in its place.
- Rust owns independent expected titles, source inventory, entry assertions,
  results and error properties. Every compile response is exactly successful
  with `{"status":"compiled"}` and no diagnostics. Success run responses are
  checked completely. The runtime-error row must compile, then return false
  `ok`, null value and exactly one error with `vm::division_by_zero`; the reported
  source span must slice exactly `before / 0` from the actual site input.
- The runner `site/scripts/check-playground-native.mjs`, exposed as
  `npm --prefix site run test:playground`, supplies `VELA_SITE_DATA_DIR` explicitly
  and runs the named ignored repository integration test with a 180-second child
  deadline, 4 MiB per-stream output cap and two-second process-group cleanup bound
  (see the review correction below). The ignore is a visible package boundary, not a conditional successful
  skip: explicitly running the test without the variable fails. Ordinary native
  tests and production builds do not require site files.
- `site/scripts/check-playground-build.mjs` runs at the end of the existing site
  build. It decodes the actual English/Chinese `data-examples` payloads and
  compares full records against the authored files/table. It also checks exactly
  four records and source-file inventory equality. This proves the raw-import
  path and emitted component payload, not browser JS, WASM glue or interaction.

There is no external `include_str!`, symlink, fallback literal, second editable
source, new product API or dependency. Runtime Rust reads are strict UTF-8;
Node's new payload reader uses fatal UTF-8 decoding and preserves BOM content.
No source trimming/reindentation/newline insertion was performed.

### Exact-Byte Proof

Before replacing the TS literals, compared each extracted file with its original
literal bytes and each JSON title/entry with the original row. These literals
have no escapes/interpolation requiring evaluation. All four comparisons passed.
Repeated the comparison after implementation against
`git --no-pager show HEAD:site/src/data/playground-examples.ts`; all still matched.
The four sources start at the first declaration and have **no final newline**.

| File under `site/src/data/playground` | Bytes | Entry | Independent run oracle | SHA-256 |
|---|---:|---|---|---|
| `level_reward.vela` | 343 | `main` | JSON 40 | `aad3f912e650070cad5118f7e9190aadbc5a42deff7a30ccab87e33d52fbd559` |
| `collections.vela` | 282 | `main` | JSON 37 | `cf1ddf006ee6ec3ac3757f0f3873a41909e30972d233b9838a02a7d7499b1170` |
| `methods.vela` | 289 | `main` | JSON 50 | `bf8ee8be43181e65697cafc9c582bc1d6b0fbb0a7ef10c7bf51453c9b462001d` |
| `runtime_error.vela` | 57 | `main` | Runtime division-by-zero, not compile failure | `89733d4ceb6c6bf5268345cb36e3a3efa38d4024d9d6f35a98f3350f8fee41a8` |

The independent SF08 native reward fixture remains **585 bytes**, SHA-256
`e481b155b36c0ec63dd1acf3b47ba11251cc5ba03b218edf19276cf36e9d5ed3`.
It retains `base: int`/`multiplier: int`, leading newline, indentation and trailing
spaces. The actual-site source retains `i64`; the two are intentionally distinct.
Scoped `playground/.gitattributes` declares `*.vela text eol=lf` and
`git check-attr text eol -- site/src/data/playground/*.vela` confirmed both
attributes on all four files. This does not insert an EOF newline.

## Editor And Highlighting Dispositions

`editors/zed/scripts/check-queries.test.js` uses existing Node tests and a direct
installed Tree-sitter CLI, not a query catalog/framework or another editor
matrix. Each CLI subprocess has a 30-second deadline, a 4 MiB per-stream output
limit and a two-second process-group cleanup bound (review correction below). Tests
invoke the **shipped** `.scm` files against the existing
`site/src/syntax/fixtures/complete.vela`; there are no copied query patterns.
The test parses match/capture coordinates, checks valid nonempty byte ranges,
and rejects unparsed capture output. Assertions are authored source-coordinate
expectations, not regenerated query snapshots.

| Test/production query | Actual assertions |
|---|---|
| Retained-positive parse | Existing `complete.vela`, `examples/src/bin/native_function/main.vela`, and `examples/src/bin/container_type_hints/main.vela` all have a `source_file` tree with no `ERROR` or `MISSING`. No rejection/example glob is used. |
| `brackets.scm` | Every match pairs ordered open/close ranges and matching delimiter text; exact struct braces, array brackets and trait-parameter parentheses are asserted within the same match. |
| `indents.scm` | Every end lies within its containing region and shares its ending coordinate; exact function block, array and parameter-list ranges/end tokens are paired. |
| `outline.scm` | Exactly nine item/name pairs, with exact ranges for Player, Stats, Reward, trait Tickable, impl Tickable, tick, START_LEVEL, current_player and award. Uses do not become extra declarations. |
| `textobjects.scm` | Exactly 15 captures: seven paired class/function outer and body ranges plus the line comment. Inside regions deliberately include braces, matching the existing query contract rather than inventing stripped-body semantics. |
| `highlights.scm` | Exact type, property, builtin type, function, parameter, state constant/keyword, suffixed number, assignment operator, interpolated string, method and comment ranges/text. Existing package-validator precedence/mapping checks remain independent. |
| `overrides.scm` | Exactly 12 complete regions: line comment plus all eleven ordinary/interpolated/character string regions, including quotes and escape spelling. Distinct from highlight captures. |

All seven tests pass. Baseline direct query invocations also succeeded, with
68 bracket / 58 indent / 18 outline / 15 textobject / 359 highlight / 12 override
captures; counts alone were not treated as acceptance. Coverage is meaningful
selected-fixture proof, not every possible syntax/node variant or Zed's live UI.

The Tree-sitter `parse:fixtures` npm script now uses the correct `../../` paths
and the three explicit retained-positive inputs. `test:queries` invokes the
seven Node checks above. Generated grammar/parser outputs and all production
queries are unchanged; regeneration has an exit-zero diff check.

Retained without extraction:

- Shiki `complete.vela` plus `complete.expected-scopes.json`: already file-backed;
  all 36 required scopes and 36 independent token checks remain unchanged.
- The short inline removed-keyword probe remains next to its negative assertions;
  extracting three lines would add indirection without useful reuse.
- Site prose/placeholder audits, regex/unit strings and both independent
  TextMate grammars remain independent, not a merged syntax corpus.
- Tree-sitter `test/corpus/state.txt` remains the specialized three source/tree
  cases (ordinary state, removed extern-state recovery, malformed-state recovery).
- Zed checkout ownership tests retain their local Git sentinels/cleanup and all
  three cases. No generated checkout was reset, cleaned or replaced.
- Shared highlighting showcase/consistency mappings remain at their existing
  paths. Package validators are audits, not semantic execution of the showcase.
- Zed launcher unit data remain unchanged. VS Code tests and matrix readers are
  SF10-owned; only the existing read-only VS Code package validator was run.

## Validation Commands And Results

These are the initial slice validation results. All commands had finite tool
deadlines; none timed out. The original synchronous launchers' configured Node
timeouts did not prove hard-bounded process-tree cleanup; the review correction
below supersedes that aspect of the original claim. Commands below use
repository-root cwd unless explicitly labeled. A bound on a combined command
applies to the entire sequence, not each constituent. No workspace formatting
was run.

| Exact command | Bounds used | Baseline / post result |
|---|---|---|
| `npm --prefix site run test:docs` | 180s baseline site sequence; 60s post audit sequence | Both: placeholder audit + 7 tests pass. |
| `npm --prefix site run test:syntax` | Same sequences | Both: 36 scopes, 36 tokens, removed-keyword probes pass. |
| `npm --prefix site run build` | 180s | Both: Astro check 0 errors/warnings/hints, 145 pages. Post: both emitted payload equality checks pass. Existing Vite large-chunk warning remains. |
| `cargo test -p vela_playground_wasm --locked` | 180s | Baseline 14 pass. Post 14 pass, new repository test explicitly ignored; zero doctests. |
| `cargo test -p vela_playground_wasm --locked --all-features` | 180s | Post 14 pass, one explicit ignore. No features/dependencies added. |
| `npm --prefix site run test:playground` | 180s tool/child | Post one nonempty integration test passes all four exact sources through compile and run. |
| `cargo test -p vela_playground_wasm --test site_examples --locked -- --list` | 180s sequence | Exactly `advertised_site_examples_compile_and_run: test`, 1 test, 0 benchmarks. |
| `cargo check -p vela_playground_wasm --target wasm32-unknown-unknown --release --locked` | 180s | Baseline/post pass. One final run waited for concurrent build locks and finished in 2m06s; it did not time out. |
| `cargo clippy -p vela_playground_wasm --all-targets --all-features --locked -- -D warnings` | 180s | Final pass. Initial new-test `unwrap_used` errors were corrected to descriptive `expect` calls. |
| `rustfmt --edition 2024 --check crates/vela_playground_wasm/tests/site_examples.rs` | 180s sequence | Final pass after formatting only this new file with `rustfmt --edition 2024 crates/vela_playground_wasm/tests/site_examples.rs`. Initial formatting checks reported local indentation/wrapping differences; no workspace fmt. |
| `node --test editors/zed/scripts/check-grammar-checkout.test.js` | 60s audit sequence | Baseline/post 3 pass. |
| `node editors/zed/scripts/validate-package.js` | 60s audit sequence | Baseline/post pass; checkout metadata/launcher audit, not installed-package proof. |
| `node editors/vscode/scripts/validate-package.js` | 60s audit sequence | Baseline/post pass; no VS Code file changes. |
| `cargo test --manifest-path editors/zed/Cargo.toml --lib --locked` | 180s | 3 launcher unit tests pass. Existing lockfile dependencies downloaded, no dependency changes. |
| `node --test editors/zed/scripts/check-queries.test.js` | 240s initial; 180s npm sequence post | 7 pass. Every subprocess additionally bounded at 30s. |
| `cargo package -p vela_playground_wasm --list --allow-dirty --offline` | 60s baseline; 180s post sequence | Baseline 7 entries, post 8, including new `tests/site_examples.rs` and existing package-local reward fixture. |
| `cargo package -p vela_playground_wasm --allow-dirty --offline --no-verify` | 60s baseline; 180s post sequence | Both exit 101: existing `vela_bytecode` path dependency has no version requirement. No archive pass. |

Grammar cwd `editors/tree-sitter-vela`, baseline and post:

```sh
npx --yes tree-sitter-cli@0.25.10 generate
npx --yes tree-sitter-cli@0.25.10 test
npx --yes tree-sitter-cli@0.25.10 parse ../../site/src/syntax/fixtures/complete.vela ../../examples/src/bin/native_function/main.vela ../../examples/src/bin/container_type_hints/main.vela
git --no-pager diff --exit-code -- src
```

All passed within 90s/180s sequences: three corpus cases, three positive parses,
unchanged generated output. Baseline also invoked each of the six queries using
`npx --yes tree-sitter-cli@0.25.10 query ../zed/languages/vela/<name>.scm ../../site/src/syntax/fixtures/complete.vela`
for `brackets`, `indents`, `outline`, `textobjects`, `highlights`, `overrides`;
30s per invocation within a 180s sequence. Post npm entrypoints were exercised:

```sh
npx --yes --package tree-sitter-cli@0.25.10 npm run parse:fixtures
npm run test:queries
npm pack --dry-run --json
```

The first two passed in the 180s grammar sequence; the package dry run passed in
30s with 10 files, including grammar, generated parser/headers and corpus. CLI
warnings about unconfigured global parser directories and unnecessary grammar
conflicts remain; execution uses the current grammar cwd and all parse/query
assertions passed. The npm package does **not** include the repository site,
examples or sibling Zed scripts: `parse:fixtures` and `test:queries` are explicitly
checkout integration commands, not independently packaged tests. No installed
extension was built/launched. The Zed package validator still expects this
checkout's absolute repository path; its pass is not portable installation proof.

## Source-Package Reachability And Limits

The new Rust test has only package-local compile-time dependencies. A temporary
source topology was materialized at
`crates/vela_playground_wasm/target/sf11-package-proof` using the WASM owner's
actual `cargo package --list` source files/original manifest, the workspace
manifest/lock/config, and current local dependency/test source topology. All
2,743 copied source/config files were byte-compared; no symlinks or `site`
directory were present. This was an ephemeral validation snapshot, not a new
source origin. A fresh isolated target directory forced a new native build.

```sh
cargo test --manifest-path crates/vela_playground_wasm/target/sf11-package-proof/Cargo.toml -p vela_playground_wasm --locked --offline
```

Passed within 300s (44.88s build): **14 native tests, one explicit repository-test
ignore, zero doctests**. Then explicitly running the ignored test without its
input environment failed as intended with exit 101 and
`explicit site/src/data path required` (30s bound). The isolated binary also
passed against the **original, uncopied** site sources (60s tool bound), with
`VELA_SITE_DATA_DIR` set to the resolved location of the original repository's
`site/src/data` directory (the machine-specific environment value is omitted):

```sh
cargo test --manifest-path crates/vela_playground_wasm/target/sf11-package-proof/Cargo.toml -p vela_playground_wasm --test site_examples --locked --offline -- --ignored --nocapture
```

One test passed, all four exact sources/entries exercised. The temporary topology
was removed afterward. This proves source reachability and explicit external
repository-test inputs with the current path-dependency topology; it is **not**
a registry `.crate` build. Actual Cargo packaging remains blocked by the existing
unversioned path dependency. No manifest/version policy was changed to conceal it.

## Handoff

The actual-site native linkage and selected current grammar/query obligations
are implemented. The new repository lane must be run explicitly with
`npm --prefix site run test:playground`; ordinary Cargo success alone does not
prove it. Existing site build now owns emitted-source equality. Parent owns any
shared progress/acceptance update and commit. Benchmark/fuzz SF11 and SF10 remain
outside this slice. Real browser Compile/Run, generated glue/assets and interaction
smoke remain separately scoped and untested here.

## Review Correction — Hard-Bounded Launcher Cleanup

The review correctly identified that `execFileSync`'s default timeout signal is
SIGTERM and does not bound a SIGTERM-ignoring executable or clean its launcher
descendants. The original passing command results above stand, but they did
**not** establish process-tree timeout safety.

Replaced both synchronous launchers with small owner-local built-in Node
`spawn` helpers: `site/scripts/playground-process.mjs` and
`editors/zed/scripts/query-process.js`. They are private to these checks and their
regression probes, not a shared framework. No dependency, source program,
query/capture oracle, Rust test, runtime code or other owner's files changed.

- Spawn directly with `shell: false`, ignored stdin, piped stdout/stderr, and a
  new POSIX session/process group (`detached: true`). The native command still
  invokes the same Cargo target/filter with the same explicit site-data path.
- Native execution deadline: **180,000 ms**. Each query/parse/version command:
  **30,000 ms**. Both cap retained output at **4 MiB independently per stream**.
  Capture is buffered and displayed after completion; original interleaving/live
  progress is not preserved. Overflow is a failure, not truncated success.
- Deadline, output overflow, spawn/stream errors, nonzero exit **and successful
  launcher exit** initiate group cleanup. Send **SIGKILL to the negative group
  ID**, not SIGTERM to only the launcher. This also handles descendants retaining
  inherited pipes after the direct parent exits.
- Wait for Node's direct-child/pipe `close` and poll the process group until
  `kill(-pgid, 0)` reports `ESRCH`. The direct child is reaped by Node; orphaned
  descendants are reaped by the OS. The runner verifies group disappearance,
  rather than pretending Node can `waitpid` arbitrary grandchildren.
- Cleanup has its own **2,000 ms** bound. A still-present group or missing
  child/pipe closure becomes an explicit `cleanup incomplete` error; streams are
  destroyed and the child handle is unreferenced so the helper does not wait
  indefinitely. No cleanup failure is reported as success. These are application
  deadlines, not a guarantee against kernel-uninterruptible processes or a
  stalled Node event loop.
- Observed macOS `EPERM` during transient orphan-group reaping is treated as
  **still pending**, never as disappearance. Only eventual `ESRCH` completes
  cleanup; otherwise the cleanup deadline fails. The first probe run exposed
  two false failures from preserving that transient observation as a permanent
  error; corrected without weakening the disappearance assertion.

### Direct CLI Selection

`check-queries.test.js` no longer launches `npx` or downloads packages. It uses
`VELA_TREE_SITTER_CLI` when supplied, otherwise the existing grammar package's
installed `node_modules/tree-sitter-cli/tree-sitter` native binary if present,
otherwise `tree-sitter` on PATH. A bounded `--version` invocation prints the
actual version. The former seven test identities and all range/capture assertions
remain; only subprocess calls became asynchronous. Missing executables fail
with installation/override guidance.

The installed `tree-sitter` executable selected from PATH was **0.27.1**.
The existing npm-cached **0.25.10** native executable was also run directly via
`VELA_TREE_SITTER_CLI`; both passed all seven checks. No grammar version/dependency
or generated parser file was changed to use the installed CLI.

### Platform Contract And Limits

Verified environment: **macOS / Darwin arm64, Node v22.20.0**. The POSIX code path
accepts macOS and Linux; **Linux was not exercised**. Windows and other platforms
are rejected **before spawn**. No Windows `taskkill /T /F` implementation or
Windows validation is claimed.

Containment covers the launched process group and ordinary inherited-group
compiler/test children. Deliberate `setsid`/detached daemon escape is outside
this helper's contract. OS reaping failures (including environments whose PID 1
does not reap orphan zombies) fail the cleanup bound. External SIGKILL/crash of
the supervising Node process cannot run its JavaScript cleanup. No claim of a
sandbox, Windows Job Object, or arbitrary daemon-tree containment is made.

### Finite Regression Probes And Fresh Results

Each owner has its own test file and `scripts/fixtures/process-probe.cjs`, with
no cross-owner runtime import. Both parent and descendant install SIGTERM
handlers; the parent sends real SIGTERM to each and waits for acknowledgements
before flooding output or exiting. Each process has an independent **eight-second
exit watchdog**, so even a broken supervisor cannot leave an indefinite probe.
Tests use a **one-second command deadline**, **4 KiB per-stream caps**, a
six-second Node test timeout, and require termination in under four seconds.
They assert `ESRCH` for the parent PID, descendant PID and negative group ID.

Each owner runs eight tests:

1. Exact stdout/stderr (including NUL/Unicode), success status and rejected exit 7.
2. Bounded missing-executable spawn failure.
3. Invalid deadlines/output limits rejected before spawning.
4. Deadline kills/reaps the SIGTERM-ignoring parent and descendant with SIGKILL.
5. Stdout overflow kills/reaps the same tree and respects both capture caps.
6. Stderr overflow kills/reaps the same tree and respects both capture caps.
7. Successful parent exit still kills/reaps a descendant holding output pipes.
8. Successful parent exit still kills/reaps a descendant with no inherited output
   pipes (closure alone must not be mistaken for tree cleanup).

Fresh commands from repository-root cwd:

| Exact command | Tool bound | Result |
|---|---:|---|
| `npm --prefix site run test:playground` | 210s initial combined sequence; 195s final | Pass: same one Rust test, all four exact source lengths/entries and compile/run expectations. |
| `npm --prefix editors/tree-sitter-vela run test:queries` | 210s initial combined sequence; 45s final regression/query sequence | Pass: seven checks, direct CLI 0.27.1. |
| `npm --prefix editors/tree-sitter-vela run test:queries`, with `VELA_TREE_SITTER_CLI` set to the installed 0.25.10 native executable | 180s | Pass: seven checks, direct CLI 0.25.10. The environment value identifies an existing executable, not a repository source or an automatic download. |
| `node --test site/scripts/playground-process.test.mjs editors/zed/scripts/query-process.test.js` | 30s | Initial 14 pass / 2 transient-EPERM false failures; after correction **16 pass**, zero skipped. |
| Same regression command repeated three times, then the query npm command | 45s total | All three repetitions **16/16**, followed by **7/7** queries. Deadline probes finished around 1.02s; overflow/early-exit probes under 0.1s, well before watchdogs. |

Final checks also passed: `npm --prefix site run test:docs` (30s tool sequence;
placeholder audit plus seven tests); `node --check` on both launchers, both local
process helpers, both regression test files and both probe fixtures; scoped
`git --no-pager diff --check -- site editors/zed docs/archive/test-input-sf11-consumers.md`;
and `git --no-pager diff --exit-code -- editors/tree-sitter-vela/src editors/zed/languages`.
Report links resolved and all four actual-site SHA-256 values above were rechecked
unchanged. These checks did not run workspace formatting or touch other scopes.

No real command timed out. Expected deadline/overflow rejection happened only
inside the negative probes. The prior package-publication and browser limitations
are unchanged; no fresh full site build/wasm32 or installed-editor claim is made
for this launcher-only correction.

## Residual Audit — Reject Zero-Test Success

The audit reproduced an exit-zero libtest result for a stale exact test filter:
`running 0 tests` with `0 passed` and `1 filtered out`. Checking the Cargo exit
status alone was insufficient. The existing site launcher now validates both
phases for this one named target, without adding a runner framework:

1. Run discovery with the same `--ignored --exact
   advertised_site_examples_compile_and_run --format pretty --color never`
   selection and `--list`. Require exactly the named `: test` row followed by
   `1 test, 0 benchmarks`; empty, stale-name, extra or duplicate discovery fails.
   The ignored-only listing also rejects loss of the Rust `#[ignore]` contract.
2. Execute the same selection with `--show-output`. Require exactly one
   `running 1 test` header, exactly one named `... ok` result row, and exactly one
   successful summary with **1 passed / 0 failed / 0 ignored / 0 measured /
   0 filtered out**. Exit-zero empty execution, wrong names, duplicate rows or
   summaries, and ignored tests all fail. `--show-output` preserves all four
   actual-site logs without interleaving them with the named result row.

`assertSiteDiscovery` and `assertSiteExecution` live in the existing
`site/scripts/check-playground-native.mjs`; importing them for tests does not
launch Cargo. Discovery does not execute the test body. The actual ignored Rust
integration test and explicit `VELA_SITE_DATA_DIR` injection remain unchanged.
Both phases use the existing process-group runner, each with a 180-second command
deadline, 4 MiB per-stream cap and two-second cleanup bound; this is a finite
two-phase check, not a new claim of a single 180-second whole-script deadline.

Added five tests in `site/scripts/check-playground-native.test.mjs`: positive
selection/execution; rejected empty/stale/duplicate discovery; rejected zero,
ignored, wrong-name and repeated execution; real libtest exit-zero stale-filter
listing/execution; and real ignored-only discovery of an existing nonignored
library test. The last case models removal of `#[ignore]` without editing the
actual-site Rust test. Live negative Cargo calls have 30-second deadlines and
the Node cases have 65-second timeouts. Pure output samples keep independent
expected names, counts and failure cases.

Fresh validation on the same macOS arm64 / Node v22.20.0 environment:

| Exact command | Tool bound | Result |
|---|---:|---|
| `npm --prefix site run test:playground` | 195s combined script/regression sequence | Pass: discovery exactly 1 test / 0 benchmarks, execution exactly 1 passed with zero ignored/filtered-out; all four original source lengths/entries and compile/run expectations pass. |
| `node --test site/scripts/check-playground-native.test.mjs site/scripts/playground-process.test.mjs editors/zed/scripts/query-process.test.js` | Same 195s sequence | **21 passed, 0 failed, 0 skipped**: five selection/execution tests plus sixteen prior process-tree regressions. The real stale-filter calls exited 0 but were rejected by both guards. |

The initial reproduction used `cargo test -p vela_playground_wasm --locked
--test site_examples -- --ignored --exact sf11_stale_test_name --list --format
pretty` and the same selection with `--format pretty --color never` instead of
`--list`: both returned 0 with zero selected tests. The nonignored control used
`cargo test -p vela_playground_wasm --locked --lib -- --ignored --exact
tests::run_script_reports_runtime_error --list --format pretty` and also listed
zero tests. These ran in a finite 180-second baseline sequence, not as passing
actual-site acceptance.

Machine-specific absolute paths were removed from this report. Temporary target
and manifest paths remain repository-relative; environment values for the
original site-data directory and installed CLI are described by purpose rather
than a user's home/cache path. No CI, shared docs, runtime/Rust test, editor query,
process helper, dependency or commit changes belong to this residual correction.
