"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const { nodeResults, assessInfrastructure } = require("./infrastructure");

test("TAP evidence distinguishes success, failure, ignored and duplicate tests", () => {
  assert.deepEqual([...nodeResults("ok 1 - good\nnot ok 2 - broken\nok 3 - skipped # SKIP reason\nnot ok 4 - todo # TODO later\n")],
    [["good", "ok"], ["broken", "FAILED"], ["skipped", "ignored"], ["todo", "ignored"]]);
  assert.throws(() => nodeResults("ok 1 - same\nok 2 - same\n"), /duplicate/);
  assert.throws(() => nodeResults("no test records"), /no infrastructure tests/);
});

test("gate evidence requires exact executed tests and cannot use candidate names", () => {
  const requirements = [{ id: "batch/B00/proof", layer: "gate" }];
  const evidence = [{ requirement: "batch/B00/proof", assertion: "exact gate behavior", tests: [{ layer: "gate", name: "proof" }] }];
  const run = { available: ["proof"], results: new Map() };
  assert.equal(assessInfrastructure(requirements, [], run)[0].status, "unreviewed");
  assert.equal(assessInfrastructure(requirements, evidence, run)[0].status, "mapped");
  run.results.set("proof", "ignored");
  assert.equal(assessInfrastructure(requirements, evidence, run)[0].status, "failed");
  run.results.set("proof", "ok");
  assert.equal(assessInfrastructure(requirements, evidence, run)[0].status, "verified");
  evidence[0].tests[0].name = "prefix*";
  assert.throws(() => assessInfrastructure(requirements, evidence, run), /stale test reference/);
});

test("gate evidence rejects wrong layers, duplicate mappings and unknown obligations", () => {
  const requirements = [{ id: "batch/B00/proof", layer: "gate" }];
  const proof = { requirement: "batch/B00/proof", assertion: "exact behavior", tests: [{ layer: "gate", name: "proof" }] };
  const run = { available: ["proof"], results: new Map([["proof", "ok"]]) };
  assert.throws(() => assessInfrastructure(requirements, [proof, proof], run), /invalid infrastructure evidence/);
  proof.tests[0].layer = "editor";
  assert.throws(() => assessInfrastructure(requirements, [proof], run), /invalid infrastructure evidence/);
  proof.tests[0].layer = "gate";
  proof.requirement = "unknown";
  assert.throws(() => assessInfrastructure(requirements, [proof], run), /invalid infrastructure evidence/);
});

test("shared infrastructure proof requires executed Rust and installed editor assertions", () => {
  const requirements = [{ id: "batch/B01/shared-fixtures", layer: "gate" }];
  const proof = { requirement: requirements[0].id, assertion: "shared fixture crosses actual consumers",
    tests: ["gate", "service", "protocol", "editor"].map((layer) => ({layer,name:`${layer}-test`})) };
  const run = {available:["gate-test"],results:new Map([["gate-test","ok"]])};
  const other = {available:{},results:{}};
  for (const layer of ["service","protocol","editor"]) {
    other.available[layer] = [`${layer}-test`]; other.results[layer] = new Map([[`${layer}-test`,"ok"]]);
  }
  assert.equal(assessInfrastructure(requirements,[proof],run,other)[0].status,"verified");
  other.results.editor.clear();
  assert.equal(assessInfrastructure(requirements,[proof],run,other)[0].status,"mapped");
  other.results.protocol.set("protocol-test","ignored");
  assert.equal(assessInfrastructure(requirements,[proof],run,other)[0].status,"failed");
  other.available.service = [];
  assert.throws(() => assessInfrastructure(requirements,[proof],run,other),/stale test reference/);
});
