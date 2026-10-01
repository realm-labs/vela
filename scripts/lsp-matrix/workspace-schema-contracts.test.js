"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { workspaceSchemaModel, workspaceSchemaContracts } = require("./workspace-schema-contracts");
const { localContracts } = require("./local-contracts");
const { hoverResponses } = require("./hover-trace");
const { offsetAt } = require("./fixtures");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
test("UX17 schema replacement owns precisely its two obligations without closing roots or invalid inputs", () => {
  const [c] = workspaceSchemaContracts(requirements);
  assert.deepEqual(c.requirements.map(r => r.id), ["vscode/UX17/schema-replace/command/local", "vscode/UX17/schema-replace/input/local"]);
  assert.equal(c.checks.filter(k => k.level === "Command").length, 9);
  assert.equal(c.checks.filter(k => k.id.endsWith("-wire")).length, 9);
});
test("schema fixture pins Unicode LF and CRLF disk and dirty spans independently", () => {
  for (const crlf of [false, true]) {
    const m = workspaceSchemaModel(crlf);
    assert(m.disk.text.startsWith("// 中😀"));
    for (const [name, start, end] of [["value", 7, 12], ["gone", 19, 23], ["rank", 30, 34]]) {
      assert.deepEqual(m.dirty.markers[name].start.line, 3);
      assert.deepEqual([m.dirty.markers[name].start.character, m.dirty.markers[name].end.character], [start, end]);
      assert.equal(m.disk.markers[name].start.line, 2);
      assert.equal(m.dirty.text.slice(offsetAt(m.dirty.text, m.dirty.markers[name].start), offsetAt(m.dirty.text, m.dirty.markers[name].end)), name);
    }
    assert.equal(m.dirty.text.includes("\r\n"), crlf);
  }
});
test("schema goldens require new types, removed fields, new fields, complete diagnostics and exact restoration", () => {
  const [c] = workspaceSchemaContracts(requirements), expected = id => c.checks.find(k => k.id === id).expected;
  assert.equal(expected("baseline-value-wire").result.contents.value, "```vela\nWorkbenchCell.value\n```\n\n_field_: i64");
  assert.equal(expected("current-value-wire").result.contents.value, "```vela\nWorkbenchCell.value\n```\n\n_field_: String");
  assert.equal(expected("current-gone-wire").result, null);
  assert.deepEqual(expected("current-gone-command"), []);
  assert.deepEqual(expected("current-gone-widget"), { visible: true, label: "", paragraphs: [], diagnostics: ["unknown field `gone` for `WorkbenchCell`vela(analysis::unknown_field)"] });
  assert.deepEqual(expected("current-value-widget"), { visible: true, label: "WorkbenchCell.value", paragraphs: ["field: String"], diagnostics: [] });
  assert.equal(expected("baseline-rank-wire").result, null);
  assert.equal(expected("current-rank-wire").result.contents.value, "```vela\nWorkbenchCell.rank\n```\n\n_field_: bool");
  assert.deepEqual(expected("restored-value-wire"), expected("baseline-value-wire"));
  for (const [state, field, names] of [["baseline", "rank", ["gone", "value"]], ["current", "gone", ["rank", "value"]], ["restored", "rank", ["gone", "value"]]]) {
    const p = expected(state + "-diagnostics");
    assert.equal(p.uri, "scripts/schema_observer.vela"); assert.equal(p.diagnostics.length, 1);
    const d = p.diagnostics[0]; assert.equal(d.code, "analysis::unknown_field");
    assert.equal(d.message, `unknown field \`${field}\` for \`WorkbenchCell\``);
    assert.equal(d.severity, 1); assert.equal(d.source, "vela");
    assert.deepEqual(d.data.labels.map(l => l.message), ["unknown member access", `did you mean \`${names[0]}\`?`, "similar candidates: " + names.join(", ")]);
    assert.deepEqual(d.data.candidates, names.map(replacement => ({ replacement }))); assert.deepEqual(d.data.repairHints, []);
    assert.deepEqual(expected(state + "-metadata-diagnostics"), { uri: "ux04-schema.json", diagnostics: [] });
  }
  assert.equal(expected("final-source").dirty, true);
  assert.deepEqual(expected("final-disk"), expected("baseline-disk"));
});
test("schema replacement uses real editor inputs and binds both platforms independently", () => {
  const m = workspaceSchemaModel();
  assert.deepEqual(JSON.parse(m.original).facts.fields.map(f => [f.name, f.fact.name]), [["value", "i64"], ["gone", "bool"]]);
  assert.deepEqual(JSON.parse(m.replacement).facts.fields.map(f => [f.name, f.fact.name]), [["value", "string"], ["rank", "bool"]]);
  const completion = JSON.parse(require("../../tests/lsp_matrix/fixtures/input-completion.json").files["ux04-schema.json"]);
  assert.deepEqual(JSON.parse(m.original).facts.functions, completion.facts.functions);
  assert.deepEqual(JSON.parse(m.replacement).facts.functions, completion.facts.functions);
  for (const [platform, modifier] of [["win32", "Control"], ["darwin", "Meta"]]) {
    const cs = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform);
    const c = cs.find(c => c.id === "ux17-schema-replace");
    assert.equal(c.actions.find(a => a.id === "current-save").key, modifier + "+s");
    assert.equal(c.actions.find(a => a.id === "current-select").key, modifier + "+a");
    assert.equal(c.actions.find(a => a.id === "current-clear").key, "Backspace");
    assert.deepEqual(c.checks.find(k => k.id === "current-empty").expected.selections, [{ anchor: { line: 0, character: 0 }, active: { line: 0, character: 0 } }]);
    assert.equal(c.checks.find(k => k.id === "current-inserted").expected.text, m.replacement);
    assert.equal(c.actions.find(a => a.id === "current-value-chord").key, modifier + "+k");
    assert.equal(c.actions.find(a => a.id === "current-value-native").key, modifier + "+i");
    assert.equal(c.actions.find(a => a.id === "current-replace").text, m.replacement);
    assert.equal(c.actions.find(a => a.id === "restored-replace").text, m.original);
    assert(c.actions.filter(a => a.device === "command").every(a => a.command === "vscode.executeHoverProvider"));
  }
});
const params = { textDocument: { uri: "file:///workspace/%E4%B8%AD.vela" }, position: { line: 3, character: 19 } };
const sent = id => `Sending request 'textDocument/hover - (${id})'.\nParams: ${JSON.stringify(params)}\n\n\n`;
const received = (id, payload) => `Received response 'textDocument/hover - (${id})' in 4ms.\n${payload}\n\n\n`;
test("native hover trace preserves the complete authored result and wire null", () => {
  const result = workspaceSchemaContracts(requirements)[0].checks.find(k => k.id === "baseline-value-wire").expected.result;
  const rows = hoverResponses(sent(21) + received(21, "Result: " + JSON.stringify(result)) + sent(22) + received(22, "No result returned."));
  assert.deepEqual(rows, [{ method: "textDocument/hover", id: "21", params, result }, { method: "textDocument/hover", id: "22", params, result: null }]);
});
test("native hover trace rejects unmatched truncated and unrecognized records", () => {
  assert.deepEqual(hoverResponses(received(21, "No result returned.")), []);
  assert.deepEqual(hoverResponses(sent(21) + received(21, "No result returned.").slice(0, -2)), []);
  assert.throws(() => hoverResponses(sent(21) + received(21, "Something else")), /unrecognized/);
});
