# Selective Test-Input Organization Execution Record

This records SF00–SF12 execution of the [plan](../test-organization-plan.md),
not completion of the earlier TO pass. Commits are stage-sized. Source relocation,
intentional added coverage and existing proof are distinguished. No runtime feature
or portable format change is authorized. Pending stages are not accepted.

## Stage Status

| Stage | State | Evidence |
|---|---|---|
| SF00 | Complete | Census and baseline inspection below. |
| SF01 | Complete | Byte-preserving formatter/reload extraction and discovery proof below. |
| SF02 | Complete | [Syntax/UI-P evidence](test-input-sf02.md): 29 preserved inputs, parser goldens, typed/recovery controls and formatter subset. |
| SF03 | Complete | [Semantic/package evidence](test-input-sf03.md): 75 extracted inputs, 11 UI-S cases and fresh package trees; 329 tests per feature lane. Archive packaging remains limited by existing versionless path dependencies. |
| SF04 | Complete | [MIR/compiler evidence](test-input-sf04.md): 48 source inputs, 12 MIR expectations, eight UI-S cases; 650 default / 666 all-feature tests. |
| SF05–SF12 | Pending | No acceptance claimed. |

## SF00 — Census And Ownership

The census covers [runtime/compiler](../test-input-census-runtime.md),
[tooling/matrix](../test-input-census-tooling.md), and
[public/repository consumers](../test-input-census-consumers.md).
It records actual family/helper/target ownership, source routes and dispositions,
selected duplicate origins, generators, typed data and specialized corpora.
Inventory counts are lexical/file signals, not evaluated-program/test counts.
Concrete extraction choices require byte/assertion reconciliation in owning stages.
No integration-target migration or global mutable setup is selected.

Baseline proof: relevant runtime/compiler/public-entry test bodies inspected;
tooling ran 24 focused Node tests, two shared-fixture tests in each Rust library,
and all 10 stdio transport tests successfully. Commands and exact filters are in
the tooling census. No full Rust suite or editor acceptance is claimed at SF00.
Local census links and whitespace checks passed. Existing planning edits are
included in this authorized plan/census checkpoint, preserving TO history.

The query census corrects an earlier count: six shipped query files comprise four
behavioral queries plus highlights and syntax-region overrides; SF11 accounts for
all six. Real-browser Playground smoke remains separately scoped, not an SF gate.

## SF01 — Representative Extraction And Inline Retention

Extracted two formatter input/expected pairs under the syntax crate and the
retained-closure layout v1/v2 under root reload fixtures. Compiled temporary Rust
assertions compared evaluated original literals against all six files before
removal: 269/381, 302/346 and 240/303 bytes respectively, unchanged afterward.
All three Engine string rows, short formatter rows and tiny removed-state reload
versions remain inline for locality/escape clarity. Lifecycle assertions remain
unchanged (old/new results 17/131, exact generations, scalar plans and budget 1).

Formatter pairs use ordinary rstest file discovery and an owner-local inventory.
A minimal directory build watch was necessary: without it a new pair was absent
from discovery; with it `--list` changed 2 → 3 → 2 without a clean build. Missing
and orphan pairs failed as intended. Temporary inputs/proof binaries were removed.
Scoped attributes preserve LF; authored absence/presence of terminal LF is retained.

Validation (all Cargo commands locked, bounded; no timeout):

- `cargo test -p vela_syntax --lib formatting::tests`: baseline 14, final 15 pass.
- `cargo test -p vela_syntax --lib`: 129 pass, default and all features.
- `cargo test -p vela_engine --lib tests::strings::executes_string_literals`:
  3 pass, baseline and final; final default/all features.
- `cargo test -p vela_engine --test integration hot_reload::`: 7 pass,
  default/all features; initial retained-only baseline 3 pass.
- Scoped Syntax/Engine Clippy, all features, `-D warnings`; scoped rustfmt,
  whitespace and Git attributes checks pass.

Two formatter cases now have glob-generated identities; remaining named declaration
rows retain labels with shifted ordinal suffixes. No other target was moved.

## Cross-stage Review Notes

Independent review expanded includes and compared original token content: 83 of
91 changed Rust files in SF02–SF05 differed only in fixture substitution/formatting;
the remaining eight contained reviewed additive tests, wiring or package-copy setup.
It rechecked SF02/SF03/SF05 extraction ledgers and ran 52 focused UI tests plus
workspace formatting successfully. Root set-combination LF protection was added.

The SF03 staged whitespace audit additionally reports intentionally preserved
trailing whitespace/blank EOF in eight analysis fixtures. These bytes came from
original evaluated literals; they are not normalized to silence Git. Earlier
scoped whitespace checks covered changed tracked files, not all new fixture bytes.
Final acceptance must distinguish these byte-preservation exceptions from accidental
whitespace in Rust or docs.
