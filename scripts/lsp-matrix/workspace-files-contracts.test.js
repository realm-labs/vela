"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { workspaceFileModel, workspaceFileContracts } = require("./workspace-files-contracts");
const { localContracts } = require("./local-contracts");
const { offsetAt } = require("./fixtures");
const fs = require("node:fs"), os = require("node:os");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
test("UX17 Explorer routes cover exactly six owned input and command obligations", () => {
  const contracts = workspaceFileContracts(requirements);
  assert.deepEqual(contracts.flatMap(c => c.requirements.map(r => r.id)).sort(),
    requirements.filter(r => /^vscode\/UX17\/explorer-/.test(r.id)).map(r => r.id).sort());
  assert.equal(contracts.length, 3);
  assert(contracts.every(c => c.checks.some(k => k.level === "Command") && c.checks.some(k => k.level === "Input")));
});
test("UX17 keeps literal UTF-16 source spans separate from native visible columns in LF and CRLF", () => {
  for (const crlf of [false, true]) for (const [route, end] of [["create", 24], ["rename", 26], ["delete", 24]]) {
    const m = workspaceFileModel(route, crlf);
    assert.deepEqual(m.call, { line: 2, character: 33 });
    assert.deepEqual(m.target, { start: { line: 0, character: 17 }, end: { line: 0, character: end } });
    assert.equal(m.dirty.text.slice(offsetAt(m.dirty.text, m.call), offsetAt(m.dirty.text, m.call) + m.o.symbol.length), m.o.symbol);
    assert.equal(m.spec.oracle.callInput, "3:35");
    assert.equal(m.dirty.markers.import.start.line, 1);
    assert.equal(m.disk.markers.import.start.line, 0);
  }
});
test("UX17 requires native Explorer create rename and confirmed delete with no bridge mutations", () => {
  const cs = workspaceFileContracts(requirements);
  assert(cs[0].actions.some(a => a.selector === "Explorer New File"));
  assert(cs[1].actions.some(a => a.id === "rename-start" && a.key === "F2"));
  assert(cs[2].actions.some(a => a.selector === "Explorer Delete"));
  assert(cs[2].actions.some(a => a.selector === "Confirm file deletion"));
  for (const c of cs) {
    assert(c.actions.some(a => a.device === "pointer"));
    assert(c.actions.filter(a => a.device === "command").every(a => a.command === "vscode.executeDefinitionProvider"));
  }
});
test("UX17 pins complete missing diagnostics null native targets and repair clears", () => {
  for (const c of workspaceFileContracts(requirements)) {
    const missing = c.id.endsWith("create") ? "baseline" : "missing";
    const diag = c.checks.find(k => k.id === missing + "-diagnostics").expected;
    assert.equal(diag.diagnostics.length, 1);
    assert.equal(diag.diagnostics[0].code, "hir::unresolved_module");
    assert.equal(diag.diagnostics[0].data.labels[0].message, "no similar modules found");
    assert.deepEqual(diag.diagnostics[0].range.start, { line: 1, character: 0 });
    assert.deepEqual(diag.diagnostics[0].data.candidates, []);
    assert.deepEqual(diag.diagnostics[0].data.repairHints, []);
    assert.equal(c.checks.find(k => k.id === missing + "-wire").expected.result, null);
    assert.deepEqual(c.checks.find(k => k.id === missing + "-command").expected, []);
    assert.deepEqual(c.checks.find(k => k.id === "current-diagnostics").expected.diagnostics, []);
    assert.equal(c.checks.find(k => k.id === "current-source").expected.dirty, true);
    assert(c.checks.find(k => k.id === "current-wire").expected.result.file.startsWith("scripts/"));
  }
});
test("UX17 binds native save and import selection independently on Windows and macOS", () => {
  for (const [platform, modifier] of [["win32", "Control"], ["darwin", "Meta"]]) {
    const cs = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform).filter(c => c.id.startsWith("ux17-"));
    assert.equal(cs[0].actions.find(a => a.id === "create-save").key, modifier + "+s");
    assert.equal(cs[1].actions.find(a => a.id === "rewrite-select").key, platform === "win32" ? "Shift+End" : "Meta+Shift+ArrowRight");
    assert.equal(cs[0].actions.find(a => a.id === "baseline-position").text, "3:35");
    assert.deepEqual(cs[2].checks.find(c => c.id === "confirmation").expected, { role: "dialog", visible: true,
      name: "Info Are you sure you want to delete 'indigo_lighthouse.vela'?", button: platform === "win32" ? "Move to Recycle Bin" : "Move to Trash" });
  }
});
test("UX17 pins real workbench confirmations on both profiles without disabling file deletion prompts", () => {
  for (const platform of ["win32-x64", "darwin-arm64"]) {
    const profile = require(`../../editors/vscode/test/input/profiles/${platform}.json`);
    assert.equal(profile.settings["window.dialogStyle"], "custom");
    assert.equal(profile.settings["explorer.confirmDelete"], true);
  }
});
test("UX17 application dialog preference is confined to the private user profile", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "vela-dialog-profile-"));
  try {
    const profile = require("../../editors/vscode/test/input/profiles/win32-x64.json");
    const workspace = path.join(directory, "workspace"), user = path.join(directory, "user-data");
    require("../../editors/vscode/test/input/profile-settings").writeProfileSettings(workspace, user, profile);
    const read = f => JSON.parse(fs.readFileSync(f));
    assert.deepEqual(read(path.join(user, "User/settings.json")), { "window.dialogStyle": "custom" });
    const expected = { ...profile.settings }; delete expected["window.dialogStyle"];
    assert.deepEqual(read(path.join(workspace, ".vscode/settings.json")), expected);
    assert.equal(read(path.join(workspace, ".vscode/settings.json"))["explorer.confirmDelete"], true);
  } finally { fs.rmSync(directory, { recursive: true, force: true }); }
});
