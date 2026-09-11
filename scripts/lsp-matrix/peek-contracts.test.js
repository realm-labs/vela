"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { peekContracts } = require("./peek-contracts");
const { loadInventory } = require("./inventory");
const { proofStatus } = require("./local-evidence");
const requirements = loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
test("Peek contracts retain all eight local input and render obligations including both unknown routes", () => {
  const contracts = peekContracts(requirements);
  assert.equal(contracts.length, 4);
  assert.deepEqual(contracts.flatMap((item) => item.requirements.map((item) => item.id)).sort(),
    requirements.filter((item) => item.id.startsWith("vscode/UX03/")).map((item) => item.id).sort());
  for (const contract of contracts) {
    assert.equal(new Set(contract.actions.map((action) => action.id)).size, contract.actions.length,
      "route setup must not mutate action arrays shared by later routes");
    assert.equal(contract.actions.filter((action) => action.id === "clear-prior-message").length, 1);
    assert.deepEqual([...new Set(contract.checks.map((check) => check.level))].sort(), ["Input", "Render"]);
    assert.ok(contract.actions.some((action) => action.device === "pointer"));
    assert.ok(contract.checks.some((check) => check.id.includes("wire")));
  }
  const unknown = contracts.find((item) => item.id === "ux03-unknown-target");
  assert.ok(unknown.actions.some((item) => item.id === "modifier-click"));
  assert.ok(unknown.actions.some((item) => item.id === "peek-definition"));
  assert.equal(unknown.checks.find((item) => item.id === "modifier-wire").expected.result, null);
  assert.equal(unknown.checks.find((item) => item.id === "peek-wire").expected.result, null);
  assert.throws(() => peekContracts(requirements.filter((item) => item.id !== "vscode/UX03/peek-follow/render/local")), /missing Peek obligation/);
});
test("scoped modifier proof cannot certify missing native Peek routes", () => {
  const contracts = peekContracts(requirements);
  const proved = contracts[0].requirements.map((item) => ({ ...item, status: "verified" }));
  for (const requirement of requirements.filter((item) => item.id.startsWith("vscode/UX03/"))) {
    assert.equal(proofStatus(requirement, [], proved), requirement.id.includes("/modifier-click/") ? "verified" : "unreviewed");
  }
  const rendered = contracts[0].checks.find((check) => check.id === "source-line").expected.text;
  assert.equal(rendered, '    let text: String = "中😀"; make();', "pointer geometry must include the visible inlay");
});
test("Peek title following selects the complete target range while modifier navigation places the caret", () => {
  const contracts = peekContracts(requirements);
  const follow = contracts.find((item) => item.id === "ux03-peek-follow");
  assert.deepEqual(follow.actions.find((item) => item.id === "follow-target"), {
    id: "follow-target", device: "pointer", target: "peek-target-title", button: "left", clickCount: 1,
  });
  const destination = follow.checks.find((item) => item.id === "destination").expected;
  const range = follow.checks.find((item) => item.id === "wire-definition").expected.result.range;
  assert.deepEqual(destination.selections, [{ anchor: range.start, active: range.end }]);
  assert.equal(range.end.character - range.start.character, 4);
  assert.deepEqual(contracts[0].checks.find((item) => item.id === "destination").expected.selections,
    [{ anchor: range.start, active: range.start }]);
});
