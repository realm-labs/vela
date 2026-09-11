"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const cp = require("./checkpoint");

function fixture() {
  const requirements = cp.localOrder.map((owner) => ({ id: `batch/${owner}/proof`, owner, contractHash: owner }));
  const manifest = { version: 1 };
  const profile = { platform: "darwin", arch: "arm64", vscodeVersion: "1.137.0" };
  const checkpoint = { version: 1, scope: "local", manifestVersion: 1, profile,
    deferredBatches: ["B16"], acceptedBatches: [], reopenedBatches: [], completedChildren: [],
    activeChild: "B00.3", nextTask: "B00.3", remainingRequirements: requirements.map((item) => item.id).sort(),
    artifacts: [], openIssues: [] };
  const validation = { source: { revision: "a".repeat(40), treeSha256: "b".repeat(64) }, profile,
    commands: [{ command: "node scripts/lsp-matrix/run.js --run --batch B00", exitCode: 0 }] };
  const assessed = requirements.map((item) => ({ ...item, status: "verified" }));
  return { requirements, manifest, checkpoint, validation, assessed };
}

function accept(f, id = "B00") {
  f.checkpoint = cp.acceptBatch(f.checkpoint, f.manifest, f.requirements, f.assessed, id, f.validation, false);
  return f.checkpoint;
}

test("scoped acceptance permits pending future batches but requires selected proof", () => {
  const f = fixture();
  f.assessed[1].status = "unreviewed";
  assert.deepEqual(cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false), ["B00"]);
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "all", false), /unverified/);
  f.assessed[0].status = "mapped";
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false), /unverified/);
});

test("accepted batches must pass again with current evidence", () => {
  const f = fixture();
  accept(f);
  for (const status of ["unreviewed", "mapped", "failed"]) {
    f.assessed[0].status = status;
    assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B01", false), /unverified|executed test failed/);
  }
  f.assessed.shift();
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B01", false), /unverified/);
});

test("any executed failure blocks acceptance even outside selected scope", () => {
  const f = fixture();
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", true), /executed test failed/);
  f.assessed.at(-1).status = "failed";
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false), /executed test failed/);
});

test("final acceptance cannot silently drop unresolved baseline issues", () => {
  const f = fixture();
  f.checkpoint.openIssues.push({ id: "baseline-failure", summary: "unresolved failure" });
  cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false);
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "all", false), /unresolved checkpoint issues/);
});

test("missing editor results and deferred batch selection cannot pass", () => {
  const f = fixture();
  f.requirements.push({ id: "vscode/UX01/open/input/local", owner: "B00", layer: "interaction", contractHash: "input" });
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false), /unverified/);
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B16", false), /deferred batch/);
});

test("stale contracts and N/A cannot substitute for current gate or interaction proof", () => {
  const f = fixture();
  f.assessed[0].contractHash = "stale";
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false), /unverified/);
  f.assessed[0].contractHash = f.requirements[0].contractHash;
  f.assessed[0].status = "not_applicable";
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false), /unverified/);
});

test("checkpoint acceptance advances exact remaining scope and retains provenance", () => {
  const f = fixture();
  const checkpoint = accept(f);
  assert.equal(checkpoint.nextTask, "B01");
  assert.equal(checkpoint.activeChild, null);
  assert.equal(checkpoint.remainingRequirements.length, f.requirements.length - 1);
  assert.deepEqual(checkpoint.acceptedBatches[0].validation, f.validation);
  cp.validateCheckpoint(checkpoint, f.manifest, f.requirements);
});

test("forged order, duplicate acceptance and stale remaining IDs fail", () => {
  const f = fixture();
  assert.throws(() => accept(f, "B02"), /first incomplete/);
  accept(f);
  assert.throws(() => accept(f), /first incomplete/);
  f.checkpoint.remainingRequirements.pop();
  assert.throws(() => cp.validateCheckpoint(f.checkpoint, f.manifest, f.requirements), /remaining requirement IDs/);
  const g = fixture();
  accept(g);
  g.checkpoint.acceptedBatches.push(structuredClone(g.checkpoint.acceptedBatches[0]));
  assert.throws(() => cp.validateCheckpoint(g.checkpoint, g.manifest, g.requirements), /duplicate checkpoint/);
});

test("new, removed or changed accepted scope requires explicit reopening", () => {
  for (const edit of [
    (f) => f.requirements.push({ id: "new", owner: "B00", contractHash: "new" }),
    (f) => f.requirements.shift(),
    (f) => { f.requirements[0].contractHash = "changed"; }
  ]) {
    const f = fixture();
    accept(f);
    edit(f);
    assert.throws(() => cp.validateCheckpoint(f.checkpoint, f.manifest, f.requirements), /accepted scope changed/);
  }
});

test("reopening records the prior acceptance and keeps later regression gates", () => {
  const f = fixture();
  accept(f);
  accept(f, "B01");
  f.requirements[0].contractHash = "reviewed migration";
  assert.throws(() => cp.reopenBatch(f.checkpoint, f.requirements, "B00", ""), /reason/);
  f.checkpoint = cp.reopenBatch(f.checkpoint, f.requirements, "B00", "expanded contract");
  cp.validateCheckpoint(f.checkpoint, f.manifest, f.requirements);
  assert.equal(f.checkpoint.nextTask, "B00");
  assert.equal(f.checkpoint.reopenedBatches[0].requirements[0].contractHash, "B00");
  f.assessed[1].status = "mapped";
  assert.throws(() => cp.gate(f.requirements, f.assessed, f.checkpoint, "B00", false), /unverified/);
});

test("checkpoint rejects missing source identity, failed commands and changed profiles", () => {
  for (const edit of [
    (f) => { f.validation.source.treeSha256 = ""; },
    (f) => { f.validation.commands[0].exitCode = 1; },
    (f) => { f.validation.commands = []; }
  ]) {
    const f = fixture();
    edit(f);
    assert.throws(() => accept(f), /invalid accepted batch validation/);
  }
  const f = fixture();
  accept(f);
  f.checkpoint.profile = { ...f.checkpoint.profile, vscodeVersion: "1.90.0" };
  assert.throws(() => cp.validateCheckpoint(f.checkpoint, f.manifest, f.requirements), /changed local profile/);
});

test("checkpoint rejects wrong active child and machine-specific artifact paths", () => {
  const f = fixture();
  f.checkpoint.activeChild = "B16.1";
  assert.throws(() => cp.validateCheckpoint(f.checkpoint, f.manifest, f.requirements), /active child/);
  const g = fixture();
  g.checkpoint.artifacts = ["/tmp/results.json"];
  assert.throws(() => cp.validateCheckpoint(g.checkpoint, g.manifest, g.requirements), /repository-relative/);
});

test("local gates pin the execution profile while ordinary existing CI stays portable", () => {
  const { checkpoint } = fixture();
  const other = { platform: "linux", arch: "x64", vscodeVersion: "1.90.0" };
  cp.validateExecutionProfile(checkpoint.profile, other, false);
  assert.throws(() => cp.validateExecutionProfile(checkpoint.profile, other, true), /local execution profile/);
  assert.throws(() => cp.validateExecutionProfile(checkpoint.profile,
    { ...checkpoint.profile, vscodeVersion: "1.90.0" }, true), /local execution profile/);
  cp.validateExecutionProfile(checkpoint.profile, checkpoint.profile, true);
});

test("repository checkpoint retains every obligation until its whole batch is accepted", () => {
  const fs = require("node:fs");
  const path = require("node:path");
  const root = path.resolve(__dirname, "../..");
  const { loadInventory } = require("./inventory");
  const { manifest, executionRequirements } = loadInventory(root);
  const checkpoint = JSON.parse(fs.readFileSync(path.join(root, "tests/lsp_matrix/checkpoint.json"), "utf8"));
  cp.validateCheckpoint(checkpoint, manifest, executionRequirements);
  if (checkpoint.remainingRequirements.length) {
    const incomplete = structuredClone(checkpoint);
    incomplete.remainingRequirements.pop();
    assert.throws(() => cp.validateCheckpoint(incomplete, manifest, executionRequirements), /remaining requirement IDs are stale/);
  }
});
