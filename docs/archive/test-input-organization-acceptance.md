# Selective Test-Input Organization Execution Record

This records SF00–SF12 execution of the [plan](../test-organization-plan.md),
not completion of the earlier TO pass. Commits are stage-sized. Source relocation,
intentional added coverage and existing proof are distinguished. No runtime feature
or portable format change is authorized. Pending stages are not accepted.

## Stage Status

| Stage | State | Evidence |
|---|---|---|
| SF00 | Complete | Census and baseline inspection below. |
| SF01–SF12 | Pending | No acceptance claimed. |

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
