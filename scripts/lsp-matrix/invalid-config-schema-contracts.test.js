"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { invalidConfigSchemaModel, invalidConfigSchemaContracts } = require("./invalid-config-schema-contracts");
const { localContracts } = require("./local-contracts");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
const contract = () => invalidConfigSchemaContracts(requirements)[0];
const expected = (c, id) => { const check = c.checks.find(c => c.id === id); assert(check, id); return check.expected; };

test("invalid configuration proof owns only both UX17 routes and all twelve recovery states", () => {
  const c = contract();
  assert.deepEqual(c.requirements.map(r => r.id), ["vscode/UX17/invalid-config-schema/command/local", "vscode/UX17/invalid-config-schema/input/local"]);
  assert.deepEqual(c.checks.filter(k => k.id.endsWith("-folders")).map(k => k.id), ["baseline", "rejected", "missing", "invalid", "switched", "repaired", "deleted", "recreated", "empty", "badmanifest", "package", "fallback"].map(id => id + "-folders"));
  assert.equal(c.checks.filter(k => k.level === "Command").length, 36);
  assert.equal(c.checks.filter(k => k.id.endsWith("-wire")).length, 36);
  assert(c.actions.filter(a => a.device === "command").every(a => a.command === "vscode.executeHoverProvider"));
  assert.deepEqual(expected(c, "rejected-diagnostics"), expected(c, "baseline-diagnostics"));
  assert.deepEqual(expected(c, "rejected-rejection"), { type: 1, message: "invalid didChangeConfiguration settings: invalid type: integer `7`, expected a sequence" });
  assert.deepEqual(expected(c, "rejected-folders"), { folders: ["ux17-roots/right", "ux17-roots/left"], settingsRoots: 7 });
});

test("independent Unicode LF CRLF source and TOML coordinates pin byte and UTF16 units", () => {
  for (const crlf of [false, true]) {
    const m = invalidConfigSchemaModel("darwin", crlf);
    assert.deepEqual(["value", "gone", "rank"].map(field => m.span(m.dirty.markers[field])), [
      { start: { line: 3, character: 7 }, end: { line: 3, character: 12 } },
      { start: { line: 3, character: 19 }, end: { line: 3, character: 23 } },
      { start: { line: 3, character: 30 }, end: { line: 3, character: 34 } }]);
    assert.deepEqual(m.invalidManifest.markers.bad, { start: { byte: crlf ? 27 : 25, line: 2, character: 6 }, end: { byte: crlf ? 28 : 26, line: 2, character: 7 } });
    assert.equal(m.dirty.text.includes("\r\n"), crlf);
    assert(!m.dirty.text.includes("use "));
    assert(m.dirty.text.includes("中😀"));
  }
});

test("schema switches pin complete hover values, nil results and visible diagnostic widgets", () => {
  const c = contract(), range = { start: { line: 3, character: 7 }, end: { line: 3, character: 12 } };
  assert.deepEqual(expected(c, "baseline-value-wire"), { method: "textDocument/hover", request: { file: "ux17-roots/left/invalid_probe.vela", position: range.start },
    result: { contents: { kind: "markdown", value: "```vela\nHostCell.value\n```\n\n_field_: i64" }, range } });
  assert.deepEqual(expected(c, "switched-value-command"), [{ contents: [{ value: "```vela\nHostCell.value\n```\n\n_field_: String" }], range }]);
  assert.deepEqual(expected(c, "baseline-rank-widget"), { visible: true, label: "", paragraphs: [], diagnostics: ["unknown field `rank` for `HostCell`vela(analysis::unknown_field)"] });
  assert.deepEqual(expected(c, "switched-gone-widget"), { visible: true, label: "", paragraphs: [], diagnostics: ["unknown field `gone` for `HostCell`vela(analysis::unknown_field)"] });
  for (const phase of ["missing", "invalid", "deleted", "empty", "badmanifest"]) for (const field of ["value", "gone", "rank"]) {
    assert.equal(expected(c, phase + "-" + field + "-wire").result, null);
    assert.deepEqual(expected(c, phase + "-" + field + "-command"), []);
    assert.deepEqual(expected(c, phase + "-" + field + "-widget"), { visible: false });
  }
  for (const phase of ["repaired", "recreated", "package", "fallback"]) assert.deepEqual(expected(c, phase + "-value-command"), expected(c, "baseline-value-command"));
});

test("metadata ownership and full diagnostic clearing distinguish schema failures from manifest failures", () => {
  const c = contract(), p = "ux17-roots/left/", zero = { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } };
  for (const [phase, filename, tail] of [["missing", "schema-missing.json", "is unavailable; host facts degrade to Any"],
    ["invalid", "schema-invalid.json", "is invalid: unsupported schema artifact format version 99; expected 1; host facts degrade to Any"],
    ["deleted", "schema-one.json", "is unavailable; host facts degrade to Any"]]) {
    const rows = expected(c, phase + "-diagnostics"), message = "host schema `<workspace>/" + p + filename + "` " + tail;
    assert.equal(rows.filter(r => r.uri.endsWith(".vela")).length, 5);
    for (const row of rows.filter(r => r.uri.endsWith(".vela"))) assert.deepEqual(row.diagnostics, [{ code: "schema::unavailable", message, severity: 2, source: "vela", range: zero,
      data: { labels: [], candidates: [], repairHints: [] } }]);
    assert.deepEqual(rows.find(r => r.uri === p + filename).diagnostics, [{ code: "schema::diagnostic", message, severity: 1, source: "vela", range: zero,
      data: { labels: [], candidates: [], repairHints: [] } }]);
    assert(rows.filter(r => r.uri.endsWith(".json") && r.uri !== p + filename).every(r => r.diagnostics.length === 0));
  }
  assert.deepEqual(expected(c, "badmanifest-diagnostics").find(r => r.uri.endsWith("vela.toml")).diagnostics,
    [{ code: "project::diagnostic", message: "source.roots must be an array of strings at bytes 25..26", severity: 1, source: "vela", range: zero,
      data: { labels: [], candidates: [], repairHints: [] } }]);
  assert(!expected(c, "empty-diagnostics").some(r => r.uri.endsWith("vela.toml")));
  for (const phase of ["empty", "badmanifest"]) assert.deepEqual(expected(c, phase + "-diagnostics").find(r => r.uri.endsWith("cross.vela")).diagnostics.map(d => d.message),
    ["unresolved module `alpha`", "unresolved module `beta`"]);
  assert(expected(c, "fallback-diagnostics").filter(r => !r.uri.endsWith("invalid_probe.vela")).every(r => r.diagnostics.length === 0));
  const packageRows = expected(c, "package-diagnostics");
  assert.deepEqual(packageRows.find(r => r.uri.endsWith("cross.vela")).diagnostics, [{ code: "hir::unresolved_module", message: "unresolved module `beta`", severity: 1, source: "vela",
    range: { start: { line: 2, character: 0 }, end: { line: 2, character: 15 } }, data: { labels: [{ uri: p + "cross.vela", range: { start: { line: 2, character: 0 }, end: { line: 2, character: 15 } }, message: "no similar modules found" }], candidates: [], repairHints: [] } }]);
  assert(packageRows.filter(r => r.uri.startsWith("ux17-roots/right/")).every(r => r.diagnostics.length === 0));
});

test("native writes pin full JSON and confirmation while preserving all four dirty overlays", () => {
  for (const platform of ["darwin", "win32"]) {
    const c = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform).find(c => c.id === "ux17-invalid-config-schema");
    const modifier = platform === "win32" ? "Control" : "Meta", m = invalidConfigSchemaModel(platform);
    assert.equal(c.actions.find(a => a.id === "rejected-select").key, modifier + "+a");
    for (const phase of m.o.phases.filter(p => p.operation === "settings")) {
      const content = c.actions.find(a => a.id === phase.id + "-replace").text, doc = JSON.parse(content);
      assert.equal(doc.settings["editor.fontFamily"], m.roots.settings["editor.fontFamily"]);
      assert.equal(doc.settings["vela.host.schema"], phase.schema);
      assert.deepEqual(doc.settings["vela.workspace.roots"] ?? null, phase.roots ?? null);
      assert.equal(doc.settings["files.eol"], "\n");
      assert.equal(expected(c, phase.id + "-saved").disk, content);
    }
    assert.equal(expected(c, "deleted-confirmation").button, platform === "win32" ? "Move to Recycle Bin" : "Move to Trash");
    assert.equal(expected(c, "fallback-confirmation").name, "Info Are you sure you want to delete 'vela.toml'?");
    assert.deepEqual(expected(c, "empty-created"), { file: m.o.manifest, text: "", disk: "", dirty: false });
    assert.equal(expected(c, "final-overlays").length, 4);
    assert(expected(c, "final-overlays").every(d => d.dirty && d.text !== d.disk));
  }
});

test("schema error path normalization preserves complete error bodies and external paths", () => {
  const { ownedMessage } = require("../../editors/vscode/test/input/invalid-config-schema");
  assert.equal(ownedMessage("host schema `f:/Owned 中😀/schema.json` is invalid: version 99", "F:\\Owned 中😀", "win32"), "host schema `<workspace>/schema.json` is invalid: version 99");
  assert.equal(ownedMessage("/owned/workspace-else/schema.json 25..26", "/owned/workspace", "darwin"), "/owned/workspace-else/schema.json 25..26");
  assert.equal(ownedMessage("/Owned/SCHEMA/file.json", "/owned/schema", "darwin"), "/Owned/SCHEMA/file.json");
});
