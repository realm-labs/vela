"use strict";
// Authored source/ownership facts. Providers supply observations only.
const { parseMarkers, offsetAt } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-workspace-roots.json");
const point = p => ({ line: p.line, character: p.character });
const span = m => ({ start: point(m.start), end: point(m.end) });
function workspaceRootsModel(platform = "darwin", crlf = false) {
  const parse = text => parseMarkers(crlf ? text.replaceAll("\n", "\r\n") : text);
  const o = spec.oracle, disk = file => parse(spec.files[file]);
  const dirty = file => parse(file === o.leftTarget ? o.overlay : o.dirtyPrefix + spec.files[file]);
  const profile = require(`../../editors/vscode/test/input/profiles/${platform === "win32" ? "win32-x64" : "darwin-arm64"}.json`);
  const { "window.dialogStyle": ignored, ...settings } = profile.settings;
  const workspace = (folders, roots) => JSON.stringify({ folders: folders.map(path => ({ path })),
    settings: { ...settings, ...(roots ? { "vela.workspace.roots": roots } : {}) } });
  return { spec, o, disk, dirty, settings, workspace, point, span };
}
function workspaceRootsContracts(requirements, platform = "darwin") {
  if (!requirements.some(r => r.id.startsWith("vscode/UX17/roots-settings/"))) return [];
  const m = workspaceRootsModel(platform), o = m.o;
  const actions = [], checks = [];
  const key = (id, key) => ({ id, device: "keyboard", key });
  const text = (id, text) => ({ id, device: "keyboard", text });
  const pointer = (id, selector) => ({ id, device: "pointer", selector, button: "left" });
  const check = (id, level, expected) => checks.push({ id, level, expected });
  const palette = (id, label) => [key(id + "-open", "Meta+Shift+P"), text(id + "-name", label), pointer(id + "-accept", "Command palette " + label)];
  const picker = (id, file) => [key(id + "-open", "Meta+p"), { id: id + "-name", device: "keyboard", path: file }, pointer(id + "-accept", "Quick Open file " + file)];
  const add = (id, folder) => [...palette(id, "Workspaces: Add Folder to Workspace..."), key(id + "-path-select", "Meta+a"),
    { id: id + "-path", device: "keyboard", path: folder, directory: true }, pointer(id + "-confirm", "Folder dialog Add")];
  actions.push(...add("workspace", o.left), ...palette("save-workspace", "Workspaces: Save Workspace As..."),
    key("save-workspace-path-select", "Meta+a"), { id: "save-workspace-path", device: "keyboard", path: o.workspace },
    pointer("save-workspace-confirm", "Workspace dialog OK"));
  check("workspace-created", "Input", { file: o.workspace, exists: true, contained: true });
  const write = (id, folders, roots) => {
    const content = m.workspace(folders, roots);
    actions.push(...palette(id + "-settings", "Preferences: Open Workspace Settings (JSON)"), key(id + "-select", "Meta+a"),
      key(id + "-clear", "Backspace"), text(id + "-replace", content), key(id + "-save", "Meta+s"));
    check(id + "-selected", "Input", { file: o.workspace, allSelected: true });
    check(id + "-empty", "Input", { file: o.workspace, text: "", dirty: true, selections: [{ anchor: { line: 0, character: 0 }, active: { line: 0, character: 0 } }] });
    check(id + "-inserted", "Input", { file: o.workspace, text: content, dirty: true, selections: [{ anchor: { line: 0, character: content.length }, active: { line: 0, character: content.length } }] });
    check(id + "-saved", "Input", { file: o.workspace, text: content, disk: content, dirty: false });
  };
  write("both", [o.left, o.right]);
  for (const [name, file] of [["target", o.leftTarget], ["left", o.leftCaller], ["right", o.rightCaller]]) {
    actions.push(...picker("dirty-" + name, file), key("dirty-" + name + "-home", "Meta+Home"), key("dirty-" + name + "-select", "Meta+a"),
      key("dirty-" + name + "-clear", "Backspace"), text("dirty-" + name + "-replace", m.dirty(file).text));
    check("dirty-" + name + "-selected", "Input", { file, allSelected: true });
    check("dirty-" + name + "-empty", "Input", { file, text: "", dirty: true, selections: [{ anchor: { line: 0, character: 0 }, active: { line: 0, character: 0 } }] });
    check("dirty-" + name + "-inserted", "Input", { file, text: m.dirty(file).text, disk: m.disk(file).text, dirty: true });
  }
  const missing = (file, marker, module) => {
    const range = span(m.dirty(file).markers[marker]);
    return { code: "hir::unresolved_module", message: "unresolved module `" + module + "`", severity: 1, source: "vela", range,
      data: { labels: [{ uri: file, range,
        message: o.missingModule.label }], candidates: [], repairHints: [] } };
  };
  const query = (phase, name, file, marker, target, targetDoc) => {
    const id = phase + "-" + name, p = point(m.dirty(file).markers[marker].start);
    const result = target ? { file: target, range: span(targetDoc.markers.decl), text: targetDoc.text.slice(offsetAt(targetDoc.text, targetDoc.markers.decl.start), offsetAt(targetDoc.text, targetDoc.markers.decl.end)) } : null;
    // Go-to-Line uses visible columns, including the full-width Han glyph.
    // Keep independently authored input columns separate from UTF-16 LSP facts.
    actions.push(...picker(id + "-caller", file), key(id + "-goto", "Control+g"), text(id + "-position", `${p.line + 1}:${o.inputColumns[marker]}`), key(id + "-place", "Enter"),
      { id: id + "-command", device: "command", command: "vscode.executeDefinitionProvider", file, marker }, key(id + "-native", "F12"), key(id + "-dismiss", "Escape"));
    check(id + "-command", "Command", result ? [result] : []);
    check(id + "-wire", "Input", { method: "textDocument/definition", request: { file, position: p }, result });
    check(id + "-source", "Input", { file, text: m.dirty(file).text, disk: m.disk(file).text, dirty: true, selections: [{ anchor: p, active: p }] });
    check(id + "-destination", "Input", result ? { file: target, text: targetDoc.text, disk: m.disk(target).text, dirty: target === o.leftTarget,
      selections: [{ anchor: result.range.start, active: result.range.start }] } : { file, text: m.dirty(file).text, disk: m.disk(file).text, dirty: true,
      selections: [{ anchor: p, active: p }] });
    if (!result) check(id + "-empty", "Input", { text: `No definition found for '${marker}'`, visible: true });
  };
  const phase = (id, folders, selected) => {
    check(id + "-folders", "Input", { folders, settingsRoots: selected ?? null });
    check(id + "-client", "Input", { serverFolder: folders[0], started: true });
    check(id + "-workspace-json", "Input", { file: o.workspace, document: JSON.parse(m.workspace(folders, selected)) });
    const known = selected === undefined && folders.includes(o.left);
    check(id + "-diagnostics", "Input", [{ uri: o.leftCaller, diagnostics: known ? [] : [missing(o.leftCaller, "import-alpha", "alpha"), missing(o.leftCaller, "import-beta", "beta")] },
      { uri: o.leftTarget, diagnostics: [] }, { uri: o.rightCaller, diagnostics: [] }]);
    query(id, "alpha", o.leftCaller, "alpha", known ? o.leftTarget : null, m.dirty(o.leftTarget));
    query(id, "beta", o.leftCaller, "beta", known ? o.rightTarget : null, m.disk(o.rightTarget));
    query(id, "right", o.rightCaller, "call", o.rightTarget, m.disk(o.rightTarget));
  };
  phase("both", [o.left, o.right]);
  write("selected", [o.left, o.right], ["../right"]); phase("selected", [o.left, o.right], ["../right"]);
  write("restored", [o.left, o.right]); phase("restored", [o.left, o.right]);
  actions.push(...palette("remove", "Workspaces: Remove Folder from Workspace..."), pointer("remove-left", "Workspace folder left"));
  phase("removed", [o.right]);
  actions.push(...add("readd", o.left)); phase("readded", [o.right, o.left]);
  const id = "ux17-roots-settings";
  return [{ id, fixture: spec.id, deadlineMs: 120000, requirements: ["command", "input"].map(level => {
    const r = requirements.find(r => r.id === `vscode/UX17/roots-settings/${level}/local`);
    if (!r) throw Error("missing roots/settings obligation " + level);
    return { id: r.id, contractHash: r.contractHash };
  }), actions, checks, artifacts: ["trace.json", id + ".png", id + ".aria.txt", "workbench.log", "lsp-trace.log", "server-trace.jsonl", id + "-sessions.json"] }];
}
module.exports = { workspaceRootsModel, workspaceRootsContracts };
