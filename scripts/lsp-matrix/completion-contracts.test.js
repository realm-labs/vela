"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { completionModel, completionContracts } = require("./completion-contracts");
const { localContracts } = require("./local-contracts");
const { loadInventory } = require("./inventory");
const { proofStatus } = require("./local-evidence");
const requirements = loadInventory(path.resolve(__dirname, "../..")).executionRequirements;

test("completion contracts require all six UX04 obligations and cannot infer dismissal from acceptance", () => {
  const contracts = completionContracts(requirements);
  assert.deepEqual(contracts.flatMap((item) => item.requirements.map((item) => item.id)).sort(),
    requirements.filter((item) => item.id.startsWith("vscode/UX04/")).map((item) => item.id).sort());
  const proved = contracts[0].requirements.map((item) => ({ ...item, status: "verified" }));
  for (const requirement of requirements.filter((item) => item.id.startsWith("vscode/UX04/"))) {
    assert.equal(proofStatus(requirement, [], proved), requirement.id.includes("/accept-enter/") ? "verified" : "unreviewed");
  }
  assert.throws(() => completionContracts(requirements.filter((item) => item.id !== "vscode/UX04/accept-tab/render/local")), /missing completion obligation/);
});

test("completion replacement covers the suffix and Unicode caret; undo restores the typed prefix", () => {
  for (const route of ["accept-enter", "accept-tab", "dismiss-escape"]) {
    const model = completionModel(route);
    assert.match(model.origin.text, /中😀.*ux04::coWrong/);
    assert.match(model.typed.text, /ux04::comWrong/);
    assert.match(model.accepted.text, /ux04::completion_beta\(\)/);
    assert.equal(model.accepted.text.slice(0, model.accepted.selections[0].active.character).endsWith("completion_beta("), true);
    assert.equal(model.origin.text.slice(0, model.cursor.character).endsWith("ux04::co"), true);
  }
  for (const platform of ["darwin", "win32"]) {
    const contracts = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform);
    const enter = contracts.find((item) => item.id === "ux04-accept-enter");
    assert.equal(enter.actions.find((item) => item.id === "undo-completion").key, platform === "win32" ? "Control+z" : "Meta+z");
    assert.deepEqual(enter.checks.find((item) => item.id === "undo-source").expected, completionModel("accept-enter").typed);
    assert.ok(enter.actions.every((item) => item.device === "keyboard"));
    const dismiss = contracts.find((item) => item.id === "ux04-dismiss-escape");
    assert.equal(dismiss.actions.find((item) => item.id === "finish-completion").key, "Escape");
    assert.deepEqual(dismiss.checks.find((item) => item.id === "final-source").expected, completionModel("dismiss-escape").typed);
  }
});
