"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { installationModel, installationContracts } = require("./installation-contracts");
const { localContracts } = require("./local-contracts");
const { parseMarkers } = require("./fixtures");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
test("UX01 requires actual native and command openings with independent positive and negative facts", () => {
  const m = installationModel(), contracts = installationContracts(requirements), o = m.spec.oracle;
  assert.deepEqual(contracts.flatMap(c => c.requirements.map(r => r.id)).sort(), requirements.filter(r => r.scenario === "UX01").map(r => r.id).sort());
  assert.equal(contracts.length, 2);
  assert.equal(m.disk[o.velaFile], m.disk[o.unrelatedFile], "same Vela-shaped bytes must remain plaintext in .txt");
  const marked = parseMarkers(m.spec.files[o.velaFile]);
  assert.deepEqual({ line: marked.markers.call.start.line, character: marked.markers.call.start.character }, o.call);
  assert.deepEqual({ start: { line: marked.markers.definition.start.line, character: marked.markers.definition.start.character },
    end: { line: marked.markers.definition.end.line, character: marked.markers.definition.end.character } }, o.definition);
  const positive = contracts[1];
  assert.deepEqual(positive.checks.find(c => c.id === "native-wire").expected, positive.checks.find(c => c.id === "command-wire").expected);
  assert(contracts.every(c => c.actions.some(a => a.device === "command" && a.command === "vscode.open") && c.actions.some(a => a.device === "keyboard" && a.key === "F12")));
});
test("UX01 opening uses registered native Windows/macOS bindings and preserves command actions", () => {
  const fixture = require("../../tests/lsp_matrix/fixtures/input-driver.json");
  for (const [platform, modifier] of [["win32", "Control"], ["darwin", "Meta"]]) {
    const contracts = localContracts(requirements, fixture, platform).filter(c => c.id.startsWith("ux01-"));
    assert.equal(contracts.length, 2);
    for (const c of contracts) {
      assert.equal(c.actions[0].key, modifier + "+p");
      assert.equal(c.actions.find(a => a.id === "goto-line").key, "Control+g");
      assert.equal(c.actions.find(a => a.id === "command-open").command, "vscode.open");
    }
  }
});
