# SF12: Final Test-Input Audit And Validation Limits

SF00–SF11 implementation is complete. SF12 closes the residual audit and records
actual validation, **not unconditional repository/editor acceptance**. The
[stage record](test-input-organization-acceptance.md) links all owner evidence.
No language/runtime feature, production dependency, artifact format or historical
editor capture was changed. A real-browser Playground smoke remains separately
scoped. Full aggregate Rust and installed-editor passes remain unproven below.

## Final Residual Audit

- 845 direct includes in changed Rust files resolve. Formatter discovery,
  package-tree materialization, CLI sidecars and site-manifest loading account for
  remaining consumers; no unused extracted fixture or missing source was found.
- All 218 matrix JSON trees resolve to their exact pre-extraction values. The 515
  references consume 198 distinct marked files; bytes, stripped source, markers,
  identities, schemas and independently authored oracles match.
- Parser 3/3, HIR 5/5, analysis 6/6 and compiler 8/8 source/golden pairs reconcile;
  formatter pairs and six CLI mappings also have real inventory/runner consumers.
  Existing root generic-hint and specialized VM/reload goldens remain separate.
- All 911 reviewed changed source/golden/manifest payloads have scoped LF checkout
  protection; no CR bytes were found in that selection. Existing explicit LF/CRLF
  generation remains intact, with no normalization in source readers.
- Four actual website sources are unchanged at 343/282/289/57 bytes, each without
  final LF; their real entry names and independently expected results are tested.
- Workload aggregate hashes reconcile 177 VM, seven Engine and one fuzz file.
  All 35 existing malformed fuzz corpus files remain byte-identical.
- Selected duplicate origins and retained-inline/generated/unit-data families are
  accounted for in owner reports. Short unrelated programs are not forced into a
  global fixture. The compiler/benchmark package-local decision is explicit.

The full baseline-to-final whitespace audit reports **34 warnings in 19 fixtures**:
analysis eight files/15 warnings, Engine reflection one/one, language-service
signature seven/14, native WASM reward one/two, VM reflection two/two. Every flagged
file matches its original evaluated bytes and extraction-ledger hash. These are
intentional trailing-space/blank-EOF preservation, not accidental Rust/doc whitespace.
Do not trim them or weaken source-byte checks to obtain a blanket clean Git audit.

### Issues Found And Corrected

Independent review found and fixed missing LF protection, one direct editor-guard
JSON reader, malformed Unicode reference-path parity, Lua preflight coercion of
fractions/numeric strings, invalid Node version probes, and empty/unknown runtime
selection. Raw Lua values now require an integer before comparison; timed conversion
is unchanged. Matrix reader negatives include replacement-character filename traps.

Site/query launchers now use bounded POSIX process-group ownership, hard termination,
output limits and finite cleanup; Windows is explicitly unsupported by these local
launchers. Tests cover ignored SIGTERM, descendants, successful-parent cleanup and
both output limits. A stale/empty native-site selection must fail instead of passing
zero tests. These checks do not claim containment of deliberately detached descendants.

The Linux `test-input-consumers` CI job now selects native site execution, emitted
payload checks, process/selection regressions, grammar/six queries and untimed
six-runtime workload preflight. Actual execution uses the existing tools, not a new
framework. YAML parses locally; the new remote CI job itself has not run here.

## Rust Validation

Initial final pass (logs under `target/final-rust-validation-sf11/`):

- Workspace all-target/all-feature locked Clippy, docs, examples tests/Clippy,
  default-feature benchmark compilation (34 executables), and fuzz-bin check passed.
- Formatting found one module-ordering error in shared fixture support; corrected.
- `cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=4`
  hit its **900-second bound** at macro UI. Completed harnesses recorded 3,200 passes,
  one unsafe-source audit failure and two existing ignores. No aggregate success.
- The unsafe audit found this run's copied proof trees under
  `examples/target/sf08-package-proof` and `fuzz/target/sf11-package`. Those exact
  generated copies were removed; the audit rule was not modified. Future disposable
  source topologies should stay under root `target/`, outside audited source roots.

Follow-up (logs and identity reconciliation under
`target/final-rust-validation-sf12/`) did **not** retry the aggregate with a longer
limit. Thirty bounded gates passed, each execution partition at most 300 seconds:

| Evidence | Result |
|---|---|
| Workspace formatting | Pass after module-order correction. |
| Workspace all-target/all-feature locked Clippy, `-D warnings` | Pass after final preflight/selector changes. |
| Unsafe-source audit | Pass in package-selected and workspace-built harnesses. |
| Macro UI, one test thread | Two runner tests pass in 10.6 seconds. Earlier aggregate stall cause is not established. |
| Remaining workspace-built unit/integration harnesses | All pass, including final VM preflight regressions. |
| Doctests | 54/54 pass in per-owner all-feature runs. |
| Actual-site ignored integration test, explicitly selected | Pass for all four advertised programs. |

Current all-feature discovery reconciles **4,592 unit/integration + 54 doctest
identities**: **4,644 uniquely passed, two existing ignored scale tests, zero failed
or missing identities**. This combines 3,200 earlier passes with 1,444 fresh passes;
it is not the sum of repeated owner runs. Ordinary Cargo selection also ignores
the repository-only site case, which was explicitly executed separately.

Remaining unit/integration partitions used exact binaries produced by workspace
all-feature discovery, preserving feature unification. Doctests ran per package;
their dependency feature unification can differ. No aggregate exit-zero workspace
run is claimed. Default lanes were exercised per owner, not freshly reconciled into
a final aggregate default run. Full aggregate commands remain required in CI.

## Repository Consumer Validation

Logs and exact command arrays: `target/sf11-final-validation/summary.json` plus
follow-up logs in the same directory. All below passed with finite bounds:

| Gate | Fresh result |
|---|---|
| Matrix/VS Code Node guards | 340 tests pass, zero skipped/failed. |
| Final site/query process and exact-selection guards | 21 tests pass. |
| Site docs | Placeholder audit and seven tests pass. |
| Site syntax | 36 scope and 36 token checks plus removed-keyword probes pass. |
| Site build | 145 pages; zero Astro errors/warnings/hints; both emitted payloads match. Vite retains its large-chunk warning. |
| Native actual-site lane | Exactly one ignored test discovered/executed, four actual sources verified. |
| Six shipped query files | Seven tests pass under installed CLI 0.27.1 and existing pinned 0.25.10. |
| Grammar | Three corpus cases, three parse fixtures and three checkout-guard tests pass. |
| Package validators | VS Code, Zed and grammar dry-run package checks pass. |
| WASM | Locked wasm32 release check passes. |
| Workload preflight | All 22 workloads, six runtime modes, 0/1/2 iterations pass; no normal measurement or performance claim. |
| Fuzz | Build and 128 bounded smoke iterations per target pass; not a coverage-guided campaign. |

Package-file inventories include selected inputs. Several owner reports also prove
isolated package-list source topologies. These are not registry archive builds:
existing versionless path dependencies prevent archive creation. Some owners only
claim package-list/repository reachability, as documented; do not promote that to
independent published-package acceptance or add production dependencies to hide it.

## Installed Editor: 26/27, Not Accepted As Complete

Fresh VS Code 1.137.0 on macOS arm64 ran from an isolated server build. The full
installed command timed out at **900 seconds**, after 24 base checks, one workspace-
symbol check and one selection-syntax check passed. Zero recorded failures, one
incomplete lifecycle check. A separately selected lifecycle run timed out at
**300 seconds** without a final result; it was not retried with a longer bound.

The existing merger confirms unique checks and identical current provenance:

```text
inputsSha256  861c12acd6be30ae218cf99d28556cfb48969a7912418cbe5a3705a8d0aa77d5
serverSha256  039c44bc383403bf6be840eb520d765c2ef9f5347517f4ef4aa4f4d6654671ac
platform      darwin
VS Code       1.137.0
```

Raw results remain under `editors/vscode/test-results/run-9CvBfC/`; partial lifecycle
traces are under `l2/`. Reconciliation is
`target/sf11-final-validation/followup-installed-reconciliation.json`. No original
capture was overwritten or relabelled. The post-editor Rust audit was not reached
by the timed-out aggregate (separate Rust evidence is described above).

Read-only diagnosis of the lifecycle trace found active progress, not a stalled
source reader: three complete variants, 342 observations each, then the fourth
variant's `open-main` phase at termination. All 2,304 selection requests received
responses (maximum recorded server latency 6 ms). The unchanged scenario prescribes
107 quiet waits of 750 ms per variant: **321 seconds across four variants before
other overhead**. Historical different-provenance runs took about 399 seconds;
they do not prove the unexecuted current remainder. The lifecycle source/runner is
unchanged and contains no extracted references. A subsequent authorized run needs
the existing 600-second suite bound plus startup/result-writing allowance; shortening
readiness waits or changing assertions to fit 300 seconds is not a fix.

The shared dev LSP binary changed during broad validation; stdio integration builds
can rebuild `CARGO_BIN_EXE_vela_lsp_server`. It was not restored or claimed preserved.
Installed target, bundled and installed payload hashes match the isolated server
above; their completed partitions remain internally consistent. The follow-up
preserved all checked fingerprints.

`observer.vsix` is still absent: physical native Input/Render was not run. Strict,
Windows and complete installed-editor acceptance remain open. No real-browser smoke
ran. These limits do not undo the source organization, but prevent a blanket all-
gates-green claim.
