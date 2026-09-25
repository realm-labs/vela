"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { loadInventory } = require("./inventory");
const { applyEdits } = require("./fixtures");
const { quickFixModel, quickFixContracts } = require("./quick-fix-contracts");
const { localContracts } = require("./local-contracts");
const driver = require("../../tests/lsp_matrix/fixtures/input-driver.json");

const requirements = loadInventory(path.resolve(__dirname, "../..")).executionRequirements;

test("UX08 shortcut fixes only the marked method and restores it by undo", () => {
  const model = quickFixModel();
  assert.equal(applyEdits(model.disk.text, [{ range: model.target.range, newText: "first" }]), model.applied.text);
  assert.equal(model.target.code, "analysis::unknown_method");
  assert.equal(model.unrelated.code, "analysis::unknown_method");
  assert.notDeepEqual(model.target.range, model.unrelated.range);
  const contracts = quickFixContracts(requirements);
  assert.deepEqual(contracts.map((item) => item.id), [
    "ux08-shortcut-fix", "ux08-lightbulb-fix", "ux08-dismiss", "ux08-no-fix",
  ]);
  for (const item of contracts) {
    assert.deepEqual(item.requirements.map((requirement) => requirement.id), [
      `vscode/UX08/${item.id.slice(5)}/input/local`,
      `vscode/UX08/${item.id.slice(5)}/render/local`,
    ]);
  }
  const [contract] = contracts;
  assert.deepEqual(contract.requirements.map((item) => item.id), [
    "vscode/UX08/shortcut-fix/input/local", "vscode/UX08/shortcut-fix/render/local",
  ]);
  assert.deepEqual(contract.actions.map((item) => item.id), ["open-quick-fix", "choose-fix", "focus-editor", "undo-fix"]);
  assert.deepEqual(contract.checks.find((item) => item.id === "visible-fixes").expected.rows,
    ["Quick Fix", ...model.spec.oracle.actionTitles]);
});

test("UX08 shortcut and undo use each registered platform's native key binding", () => {
  for (const [platform, open, undo] of [
    ["darwin", "Meta+.", "Meta+z"], ["win32", "Control+.", "Control+z"],
  ]) {
    const contract = localContracts(requirements, driver, platform).find((item) => item.id === "ux08-shortcut-fix");
    assert.equal(contract.actions.find((item) => item.id === "open-quick-fix").key, open);
    assert.equal(contract.actions.find((item) => item.id === "undo-fix").key, undo);
    assert.equal(contract.actions.find((item) => item.id === "focus-editor").key,
      platform === "win32" ? "Control+1" : "Meta+1");
  }
});
