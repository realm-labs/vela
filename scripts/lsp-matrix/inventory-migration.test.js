"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const model = require("./model");
const execution = require("./execution");
const checkpointModel = require("./checkpoint");
const { loadInventory } = require("./inventory");

const root = path.resolve(__dirname, "../..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const json = (file) => JSON.parse(read(`tests/lsp_matrix/${file}`));
const hash = (value) => model.digest(JSON.stringify(value));
const review = json("reviews/checkpoint-e-inventory.json");

test("E inventory keeps every original ID and owner with exact reviewed contract migrations", () => {
  const { catalog, manifest, requirements, executionRequirements: current } = loadInventory(root);
  const baseline = json("execution-baseline.json");
  assert.equal(model.digest(read("tests/lsp_matrix/execution-baseline.json")), review.preserved.executionBaselineSha256);
  assert.equal(requirements.length, 1327);
  assert.equal(current.length, 1477);
  assert.deepEqual(current.map((item) => item.id).sort(), baseline.requirements.map((item) => item.id).sort());
  const originals = new Map(baseline.requirements.map((item) => [item.id, item]));
  const replacements = new Map(current.map((item) => [item.id, item]));
  const changed = current.filter((item) => originals.get(item.id).contractHash !== item.contractHash);
  assert.equal(changed.length, 224);
  assert.equal(current.length - changed.length, review.preserved.unchangedExecutionContracts);
  for (const item of current) assert.equal(item.owner, originals.get(item.id).owner, item.id);
  assert.equal(hash(manifest.batches), review.preserved.batchOwnershipSha256);
  assert.deepEqual(manifest.migrations.map((item) => ({ id: item.id, count: item.from.length })),
    review.migrations.map(({ id, count }) => ({ id, count })));
  const migrated = new Set();
  for (const migration of manifest.migrations) {
    assert.ok(migration.reason && migration.preservedBehavior);
    assert.deepEqual(migration.from.map((item) => item.id), migration.to.map((item) => item.id));
    for (const from of migration.from) {
      assert.deepEqual(from, originals.get(from.id));
      assert.ok(!migrated.has(from.id), from.id);
      migrated.add(from.id);
    }
    for (const to of migration.to) {
      const { id, owner, contractHash } = replacements.get(to.id);
      assert.deepEqual(to, { id, owner, contractHash });
    }
  }
  assert.ok(changed.every((item) => migrated.has(item.id)));
  const withoutReview = structuredClone(manifest.migrations);
  withoutReview.shift();
  assert.throws(() => execution.validateBaseline(current, baseline, withoutReview), /explicit migration/);
  const forged = structuredClone(manifest.migrations);
  forged[0].from[0].contractHash = "0".repeat(64);
  assert.throws(() => execution.validateBaseline(current, baseline, forged), /invalid migration source/);
  const target = structuredClone(manifest.migrations);
  target[0].to[0].contractHash = "0".repeat(64);
  assert.throws(() => execution.validateBaseline(current, baseline, target), /invalid migration target/);
  for (const [file, digest] of Object.entries(review.preserved.featureFiles)) {
    assert.equal(model.digest(read(`tests/lsp_matrix/${file}`)), digest, file);
  }
  assert.equal(hash(catalog.exemptions), review.preserved.exemptionsSha256);
  assert.equal(model.digest(read("tests/lsp_matrix/interactions.json")), review.preserved.interactionsSha256);
});

test("E protocol and grammar retirement record original definitions hashes and retained rejection obligations", () => {
  const { catalog, requirements } = loadInventory(root);
  assert.deepEqual(catalog.grammarRules, model.grammarRules(read("docs/grammar.ebnf")));
  assert.equal(catalog.grammarRules.length, 131);
  assert.deepEqual(catalog.grammarGroups.flatMap((group) => group.productions).sort(), catalog.grammarRules);
  assert.deepEqual(review.retiredGrammar.map((item) => item.id), ["grammar/await_suffix", "grammar/extern_state_decl"]);
  for (const item of review.retiredGrammar) {
    const { id, group, syntax, definition } = item;
    assert.equal(hash({ id, group, syntax, definition }), item.contractHash);
    assert.ok(!catalog.grammarRules.includes(id.slice("grammar/".length)));
    assert.ok(item.reason);
    for (const id of item.retainedNegativeRequirements) {
      assert.ok(requirements.some((requirement) => requirement.id === id && requirement.polarity === "negative"), id);
    }
  }
  for (const item of review.protocolChanges) {
    assert.equal(hash(item.from.contract), item.from.contractHash);
    assert.equal(hash(item.to.contract), item.to.contractHash);
    assert.deepEqual(catalog.protocolBaseline[item.row], item.to.contract);
    assert.match(item.from.contract.positive, /host-path type facts/);
    assert.match(item.to.contract.positive, /script member type facts/);
    assert.deepEqual(item.from.contract.syntax, item.to.contract.syntax);
    assert.equal(item.from.contract.negative, item.to.contract.negative);
  }
  assert.deepEqual(Object.keys(catalog.syntax), ["S1", "S3"]);
  for (const item of review.syntaxChanges) {
    assert.equal(hash(item.from.contract), item.from.contractHash);
    assert.equal(hash(item.to.contract), item.to.contractHash);
    assert.deepEqual(catalog.syntax[item.id], item.to.contract);
  }
  for (const source of review.sourceChanges) {
    assert.equal(model.digest(read(source.file)), source.to);
    assert.equal(catalog.syntaxSources[source.file], source.to);
    assert.notEqual(source.from, source.to);
  }
});

test("E retains historical assertions and test links but stale proof cannot certify changed contracts", () => {
  const { catalog, requirements } = loadInventory(root);
  const prior = catalog.evidence.map((item) => {
    const copy = { ...item };
    delete copy.pendingReview;
    return copy;
  });
  assert.equal(hash(prior), review.preserved.catalogEvidenceSha256);
  const pending = catalog.evidence.filter((item) => item.pendingReview);
  assert.equal(pending.length, 158);
  assert.deepEqual(pending.map((item) => item.requirement).sort(), [...review.pendingEvidence].sort());
  const results = {};
  for (const proof of pending) {
    for (const test of proof.tests) {
      (results[test.layer] ??= new Map()).set(test.name, "ok");
    }
  }
  const scoped = requirements.filter((item) => review.pendingEvidence.includes(item.id));
  const assessed = model.assess(scoped, pending, {}, results);
  assert.ok(assessed.every((item) => item.status === "unreviewed" && item.historicalEvidence));
  assert.equal(assessed.length, pending.length);
  for (const retired of review.retiredTestReferences) {
    assert.ok(pending.some((item) => item.tests.some((test) => `${test.layer}:${test.name}` === retired.id)));
    assert.ok(!catalog.evidence.some((item) => item.tests.some((test) => test.name === retired.replacementCandidate)));
  }
});

test("E reopens only affected accepted batches and preserves unrelated acceptance and native snapshots", () => {
  const { manifest, executionRequirements } = loadInventory(root);
  const checkpoint = json("checkpoint.json");
  checkpointModel.validateCheckpoint(checkpoint, manifest, executionRequirements);
  assert.deepEqual(checkpoint.acceptedBatches.map((item) => item.id), ["B00", "B01", "B09"]);
  assert.deepEqual(checkpoint.reopenedBatches.map((item) => item.id), review.reopenedBatches);
  assert.equal(checkpoint.nextTask, "B02");
  assert.equal(checkpoint.activeChild, null);
  assert.equal(checkpoint.remainingRequirements.length, 1423);
  assert.equal(review.previousCheckpoint.activeChild, "B10.45");
  const snapshots = [...checkpoint.acceptedBatches, ...checkpoint.reopenedBatches].sort((a, b) => a.id.localeCompare(b.id));
  assert.deepEqual(snapshots.map((item) => item.id), review.previousCheckpoint.acceptedIds);
  for (const item of snapshots) {
    const original = { ...item };
    delete original.reason;
    assert.equal(hash(original), review.previousCheckpoint.acceptanceSha256[item.id], item.id);
  }
  for (const field of ["profileAudits", "completedChildren", "artifacts", "openIssues"]) {
    assert.equal(hash(checkpoint[field]), review.previousCheckpoint[`${field}Sha256`], field);
  }
  assert.throws(() => checkpointModel.gate(executionRequirements,
    executionRequirements.map((item) => ({ ...item, status: "unreviewed" })), checkpoint, "B02", false), /unverified obligations/);
});
