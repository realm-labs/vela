"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { workspaceRootsModel, workspaceRootsContracts } = require("./workspace-roots-contracts");
const { localContracts } = require("./local-contracts");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
test("UX17 roots settings owns exactly its two routes and requires five complete state transitions", () => {
  const [c] = workspaceRootsContracts(requirements);
  assert.deepEqual(c.requirements.map(r => r.id), ["vscode/UX17/roots-settings/command/local", "vscode/UX17/roots-settings/input/local"]);
  assert.equal(c.checks.filter(k => k.level === "Command").length, 15);
  assert.equal(c.checks.filter(k => k.id.endsWith("-wire")).length, 15);
  assert.deepEqual(c.checks.filter(k => k.id.endsWith("-folders")).map(k => k.id), ["both-folders", "selected-folders", "restored-folders", "removed-folders", "readded-folders"]);
});
test("roots fixture pins independent Unicode LF CRLF targets and dirty callers", () => {
  for (const crlf of [false, true]) {
    const m = workspaceRootsModel("darwin", crlf), o = m.o;
    assert.deepEqual(m.span(m.disk(o.leftTarget).markers.decl), { start: { line: 0, character: 17 }, end: { line: 0, character: 22 } });
    assert.deepEqual(m.span(m.dirty(o.leftTarget).markers.decl), { start: { line: 1, character: 17 }, end: { line: 1, character: 22 } });
    assert.deepEqual(m.span(m.disk(o.rightTarget).markers.decl), { start: { line: 2, character: 7 }, end: { line: 2, character: 11 } });
    assert.deepEqual(m.point(m.dirty(o.leftCaller).markers.alpha.start), { line: 3, character: 28 });
    assert.deepEqual(m.point(m.dirty(o.leftCaller).markers.beta.start), { line: 3, character: 37 });
    assert.deepEqual(m.point(m.dirty(o.rightCaller).markers.call.start), { line: 2, character: 28 });
    assert.equal(m.dirty(o.leftCaller).text.includes("\r\n"), crlf);
    for (const [file, marker] of [[o.leftCaller, "alpha"], [o.leftCaller, "beta"], [o.rightCaller, "call"]]) {
      const source = m.dirty(file), p = source.markers[marker].start;
      const prefix = source.text.split(/\r?\n/)[p.line].slice(0, p.character);
      assert(prefix.includes("中😀"));
      const visibleColumn = 1 + [...prefix].reduce((width, ch) => width + (ch === "中" || ch === "😀" ? 2 : 1), 0);
      assert.equal(visibleColumn, o.inputColumns[marker]);
    }
    assert(m.dirty(o.leftTarget).text.includes("return 33")); assert(m.disk(o.leftTarget).text.includes("return 1"));
  }
});
test("selected roots and folder removal reject both scratch imports while right remains current", () => {
  const [c] = workspaceRootsContracts(requirements), expected = id => c.checks.find(k => k.id === id).expected;
  for (const phase of ["both", "restored", "readded"]) {
    assert.equal(expected(phase + "-alpha-wire").result.text, "alpha");
    assert.equal(expected(phase + "-alpha-wire").result.range.start.line, 1);
    assert.equal(expected(phase + "-beta-wire").result.text, "beta");
    assert.equal(expected(phase + "-beta-wire").result.range.start.line, 2);
    assert.deepEqual(expected(phase + "-alpha-destination").selections, [{ anchor: { line: 1, character: 17 }, active: { line: 1, character: 17 } }]);
    assert(expected(phase + "-diagnostics").every(p => p.diagnostics.length === 0));
  }
  for (const phase of ["selected", "removed"]) {
    for (const name of ["alpha", "beta"]) {
      assert.deepEqual(expected(phase + "-" + name + "-command"), []);
      assert.equal(expected(phase + "-" + name + "-wire").result, null);
      assert.deepEqual(expected(phase + "-" + name + "-destination"), expected(phase + "-" + name + "-source"));
    }
    const publications = expected(phase + "-diagnostics"); assert.equal(publications.length, 3);
    assert.deepEqual(publications[0].diagnostics.map(d => d.message), ["unresolved module `alpha`", "unresolved module `beta`"]);
    assert.deepEqual(publications[0].diagnostics.map(d => d.range), [
      { start: { line: 1, character: 0 }, end: { line: 1, character: 17 } },
      { start: { line: 2, character: 0 }, end: { line: 2, character: 15 } }]);
    for (const d of publications[0].diagnostics) assert.deepEqual(d.data, { labels: [{ uri: publications[0].uri, range: d.range, message: "no similar modules found" }], candidates: [], repairHints: [] });
    assert.equal(expected(phase + "-right-wire").result.file, "ux17-roots/right/beta.vela");
    assert(publications.slice(1).every(p => p.diagnostics.length === 0));
  }
});
test("root changes are physical native dialogs and exact full workspace replacements on both platforms", () => {
  for (const platform of ["darwin", "win32"]) {
    const c = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform).find(c => c.id === "ux17-roots-settings");
    const m = workspaceRootsModel(platform), modifier = platform === "win32" ? "Control" : "Meta";
    assert.equal(c.actions.find(a => a.id === "both-select").key, modifier + "+a");
    assert.equal(c.actions.find(a => a.id === "both-clear").key, "Backspace");
    assert.equal(c.actions.find(a => a.id === "both-save").key, modifier + "+s");
    assert(c.actions.filter(a => a.device === "command").every(a => a.command === "vscode.executeDefinitionProvider"));
    for (const phase of ["both", "selected", "restored"]) {
      const content = c.actions.find(a => a.id === phase + "-replace").text;
      const parsed = JSON.parse(content); assert.equal(parsed.settings["editor.fontFamily"], m.settings["editor.fontFamily"]);
      assert.equal(Object.hasOwn(parsed.settings, "window.dialogStyle"), false);
      assert.equal(parsed.settings["files.eol"], "\n");
      assert.deepEqual(parsed.folders, [{ path: m.o.left }, { path: m.o.right }]);
      assert.deepEqual(parsed.settings["vela.workspace.roots"] ?? null, phase === "selected" ? ["../right"] : null);
      const inserted = c.checks.find(k => k.id === phase + "-inserted").expected;
      assert.equal(inserted.text, content); assert.equal(inserted.selections[0].active.character, content.length);
      assert.equal(c.checks.find(k => k.id === phase + "-saved").expected.disk, content);
    }
    assert.deepEqual(c.actions.filter(a => ["workspace-path", "save-workspace-path", "readd-path"].includes(a.id)).map(a => a.path), [m.o.left, m.o.workspace, m.o.left]);
    assert(c.actions.filter(a => a.id.endsWith("-caller-name")).every(a => a.path === m.o.leftCaller || a.path === m.o.rightCaller));
    assert.equal(c.actions.find(a => a.id === "both-alpha-position").text, "4:30");
    assert.equal(c.actions.find(a => a.id === "both-beta-position").text, "4:39");
    assert.equal(c.actions.find(a => a.id === "both-right-position").text, "3:30");
  }
});
