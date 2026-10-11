"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { outlineModel, outlineContracts } = require("./outline-contracts");
const { localContracts } = require("./local-contracts");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;

test("Outline covers complete authored declaration and ownership trees in both line endings", () => {
  const m = outlineModel();
  assert.equal(m.cases.length, 18);
  assert.equal(new Set(m.cases.map(c => c.file)).size, 18);
  for (const form of ["lf", "crlf"]) {
    const cases = m.cases.filter(c => c.form === form);
    assert.deepEqual(cases.map(c => [c.roots, c.nodes.length]), [[19,33],[10,22],[5,9],[0,0],[1,1],[2,3],[1,1],[4,7],[2,3]]);
    assert.equal(cases.reduce((n,c) => n + c.nodes.length, 0), 79);
    assert.equal(cases.reduce((n,c) => n + c.roots, 0), 44);
    assert.equal(cases.filter(c => !c.nodes.length).length, 1);
    for (const c of cases) {
      assert.doesNotMatch(c.text, /\[\[/);
      if (form === "crlf") assert.doesNotMatch(c.text, /(?<!\r)\n/);
      else assert.doesNotMatch(c.text, /\r/);
      for (const n of c.nodes) {
        const nameLine = c.text.split(/\r?\n/)[n.selection.start.line];
        assert.equal(nameLine.slice(n.selection.start.character, n.selection.end.character), n.name,
          "authored selection selects the full literal identifier, with UTF-16 coordinates");
      }
    }
  }
  const first = m.cases[0].nodes[0];
  assert.deepEqual(first.range, { start: {line:1,character:0}, end: {line:1,character:25} });
  assert.deepEqual(first.selection, { start: {line:1,character:10}, end: {line:1,character:15} });
  assert.equal(first.icon, "symbol-constant");
  assert.match(m.cases[0].text, /中😀/);
  assert.deepEqual(m.cases.slice(9).map(c => c.nodes), m.cases.slice(0,9).map(c => c.nodes));
});
test("Outline contracts require every authored row, every click, full range and empty completion", () => {
  const [c] = outlineContracts(requirements), m = outlineModel();
  assert.deepEqual(c.requirements.map(r => r.id), ["vscode/UX11/outline/input/local", "vscode/UX11/outline/render/local"]);
  assert.equal(c.checks.filter(c => c.id.endsWith("-row")).length, 158);
  assert.equal(c.actions.filter(a => a.id.endsWith("-click") && a.clickCount === 1).length, 158);
  assert.equal(c.checks.filter(c => c.id.endsWith("-tail")).length, 16);
  assert.equal(c.checks.filter(c => c.id.endsWith("-whole-range")).length, 16);
  assert.equal(c.checks.filter(c => c.id.endsWith("-keyboard-focus")).length, 16);
  assert.equal(c.checks.filter(c => c.id.endsWith("-final-source")).length, 18);
  assert.equal(c.checks.filter(c => c.id.endsWith("-empty-request")).length, 2);
  for (const list of [c.actions, c.checks]) assert.equal(new Set(list.map(x => x.id)).size, list.length);
  for (const item of m.cases.filter(c => c.nodes.length)) {
    const last = item.nodes.at(-1);
    assert.notDeepEqual(last.range, last.selection, "double-click selects the complete declaration instead of its identifier");
  }
  assert.equal(c.deadlineMs, 120000);
});
test("Outline uses each platform's physical palette binding and leaves prior routes intact", () => {
  const fixture = require("../../tests/lsp_matrix/fixtures/input-driver.json");
  for (const [platform, modifier] of [["win32", "Control"], ["darwin", "Meta"]]) {
    const contracts = localContracts(requirements, fixture, platform), c = contracts.find(c => c.id === "ux11-outline");
    assert.equal(contracts.length, 52);
    assert.equal(c.id, "ux11-outline");
    assert(c.actions.filter(a => a.id.endsWith("-query")).every(a => a.text === "Explorer: Focus on Outline View"));
    assert(c.actions.filter(a => a.id.endsWith("-palette")).every(a => a.key === modifier + "+Shift+P"));
    assert(c.actions.filter(a => a.device === "pointer").every(a => a.selector === "Outline focused treeitem label"));
  }
});
