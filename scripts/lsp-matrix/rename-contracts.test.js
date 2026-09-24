"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { renameModel, renameContracts } = require("./rename-contracts");
const { localContracts } = require("./local-contracts");
const { loadInventory } = require("./inventory");
const { proofStatus } = require("./local-evidence");
const { offsetAt } = require("./fixtures");
const requirements = loadInventory(path.resolve(__dirname, "../..")).executionRequirements;

test("UX05 confirmation keeps separate input and render proof and leaves negative routes pending", () => {
  const contracts = renameContracts(requirements);
  assert.deepEqual(contracts.flatMap((item) => item.requirements.map((item) => item.id)).sort(), [
    "vscode/UX05/rename-confirm/input/local", "vscode/UX05/rename-confirm/render/local",
  ]);
  const proved = contracts[0].requirements.map((item) => ({ ...item, status: "verified" }));
  for (const requirement of requirements.filter((item) => item.id.startsWith("vscode/UX05/"))) {
    assert.equal(proofStatus(requirement, [], proved), requirement.id.includes("/rename-confirm/") ? "verified" : "unreviewed");
  }
  assert.throws(() => renameContracts(requirements.filter((item) => item.id !== "vscode/UX05/rename-confirm/render/local")), /missing rename obligation/);
});

test("UX05 rename oracle changes exact cross-file sites but preserves the shadow", () => {
  const model = renameModel();
  assert.match(model.original["scripts/rename_open.vela"], /grant\(1\).*let grant = 3; grant/s);
  assert.match(model.renamed["scripts/rename_open.vela"], /award\(1\).*let grant = 3; grant/s);
  assert.match(model.renamed["scripts/rename_origin.vela"], /pub fn award\(/);
  assert.match(model.renamed["scripts/rename_closed.vela"], /rename_origin::award\(2\)/);
  assert.equal(model.original["scripts/rename_open.vela"].slice(0, offsetAt(model.original["scripts/rename_open.vela"], model.cursor)).endsWith("gr"), true);
  for (const platform of ["darwin", "win32"]) {
    const contract = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform)
      .find((item) => item.id === "ux05-rename-confirm");
    assert.equal(contract.actions[0].key, "F2");
    assert.equal(contract.actions.find((item) => item.id === "undo-rename").key, platform === "win32" ? "Control+z" : "Meta+z");
    assert.equal(contract.actions.find((item) => item.id === "redo-rename").key, platform === "win32" ? "Control+y" : "Meta+Shift+z");
    assert.equal(contract.checks.find((item) => item.id === "unopened-targets").level, "Input");
    assert.equal(contract.checks.find((item) => item.id === "rename-widget").level, "Render");
  }
});
