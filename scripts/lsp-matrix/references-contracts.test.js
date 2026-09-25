"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const { loadInventory } = require("./inventory");
const { referencesModel, referencesContracts } = require("./references-contracts");
const { localContracts } = require("./local-contracts");
const requirements = loadInventory(path.resolve(__dirname, "../..")).executionRequirements;

test("UX06 routes own independent input and render obligations", () => {
  const contracts = referencesContracts(requirements);
  assert.deepEqual(contracts.map((item) => item.id), [
    "ux06-references-select", "ux06-highlights-caret", "ux06-shadow-exclusion",
  ]);
  assert.deepEqual(contracts.flatMap((item) => item.requirements.map((item) => item.id)).sort(),
    requirements.filter((item) => item.id.startsWith("vscode/UX06/"))
      .map((item) => item.id).sort());
  assert.throws(() => referencesContracts(requirements.filter((item) =>
    item.id !== "vscode/UX06/references-select/render/local")), /missing references obligation/);
});

test("UX06 reference oracle lists exact cross-file sites and platform input", () => {
  const model = referencesModel();
  assert.deepEqual(model.sites.map((site) => site.file), [
    "scripts/refs_closed.vela", "scripts/refs_open.vela",
    "scripts/refs_open.vela", "scripts/refs_origin.vela",
  ]);
  assert.ok(model.sites.every((site) => site.text === "grant"));
  assert.match(model.documents[model.spec.oracle.openFile].text, /let score = 1; score \+= 1; score;/);
  assert.deepEqual(model.localHighlights.map((item) => item.style),
    ["wordHighlightText", "wordHighlightStrong", "wordHighlight"]);
  assert.deepEqual(model.shadowHighlights.map((item) => item.style),
    ["wordHighlightText", "wordHighlight"]);
  const contracts = referencesContracts(requirements);
  const shadow = contracts.find((item) => item.id === "ux06-shadow-exclusion");
  assert.deepEqual(shadow.checks.find((item) => item.id === "shadow-read-decorations").expected
    .map((item) => item.marker), ["shadow-decl", "shadow-read"]);
  assert.deepEqual(shadow.checks.find((item) => item.id === "local-write-decorations").expected
    .map((item) => item.marker), ["local-decl", "local-write", "local-read"]);
  for (const platform of ["win32", "darwin"]) {
    const contract = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform)
      .find((item) => item.id === "ux06-references-select");
    assert.equal(contract.actions[0].key, "Shift+F12");
    assert.equal(contract.actions.filter((item) => item.id.startsWith("select-reference-")).length, 4);
    assert.equal(contract.checks.find((item) => item.id === "reference-set").level, "Render");
  }
});
