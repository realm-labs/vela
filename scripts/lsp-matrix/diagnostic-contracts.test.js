"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { loadInventory } = require("./inventory");
const { parseMarkers, applyEdits } = require("./fixtures");
const { diagnosticModel, diagnosticContracts } = require("./diagnostic-contracts");
const { localContracts } = require("./local-contracts");
const driver = require("../../tests/lsp_matrix/fixtures/input-driver.json");

const root = path.resolve(__dirname, "../..");
const requirements = loadInventory(root).executionRequirements;

test("UX07 fixture and contracts fix exactly the typed syntax error", () => {
  const model = diagnosticModel();
  const expected = applyEdits(model.disk.text, [{
    range: { start: model.cursor, end: model.cursor }, newText: model.spec.oracle.typedText,
  }]);
  assert.equal(model.typed.text, expected);
  assert.equal(parseMarkers(model.spec.oracle.typed).markers.target.start.character, model.cursor.character);
  assert.equal(model.target.code, "E_LEX_CHAR");
  assert.equal(model.unrelated.code, "analysis::unknown_method");
  assert.notDeepEqual(model.target.range, model.valid);
  assert.equal(applyEdits(model.typed.text, [{ range: model.target.range, newText: "" }]), model.disk.text);
  const contracts = diagnosticContracts(requirements);
  assert.deepEqual(contracts.map((item) => item.id), [
    "ux07-problems-navigate", "ux07-repair-unsaved", "ux07-valid-location",
  ]);
  for (const contract of contracts) {
    assert.deepEqual(contract.requirements.map((item) => item.id), [
      `vscode/UX07/${contract.id.slice(5)}/input/local`,
      `vscode/UX07/${contract.id.slice(5)}/render/local`,
    ]);
    assert.ok(contract.checks.some((item) => item.level === "Render"));
    assert.ok(contract.actions.some((item) => item.id === "type-error" && item.device === "keyboard"));
  }
});

test("UX07 keeps native shortcuts distinct across registered platforms", () => {
  for (const [platform, openProblems, focusEditor] of [
    ["darwin", "Meta+Shift+M", "Meta+1"],
    ["win32", "Control+Shift+M", "Control+1"],
  ]) {
    const contracts = localContracts(requirements, driver, platform).filter((item) => item.id.startsWith("ux07-"));
    assert.equal(contracts.length, 3);
    for (const contract of contracts) {
      assert.equal(contract.actions.find((item) => item.id === "open-problems").key, openProblems);
    }
    assert.equal(contracts.find((item) => item.id === "ux07-repair-unsaved")
      .actions.find((item) => item.id === "focus-editor").key, focusEditor);
  }
});
