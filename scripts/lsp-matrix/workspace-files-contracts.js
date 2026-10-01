"use strict";
// Authored file membership, Unicode spans, unsaved callers and recovery facts.
// No observation or provider output participates in these expectations.
const path = require("node:path");
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-workspace-files.json");
const point = ({ line, character }) => ({ line, character });
const range = marker => ({ start: point(marker.start), end: point(marker.end) });
function workspaceFileModel(route, crlf = false) {
  const o = spec.oracle[route];
  const parse = text => parseMarkers(crlf ? text.replaceAll("\n", "\r\n") : text);
  const disk = parse(spec.files[o.caller]), dirty = parse(spec.oracle.dirtyPrefix + spec.files[o.caller]);
  const dependency = parse(o.source ?? spec.files[o.dependency]);
  const renamed = route === "rename" ? parse(spec.oracle.dirtyPrefix + spec.files[o.caller].replace(o.module, o.renamedModule)) : dirty;
  return { spec, o, disk, dirty, dependency, renamed, call: point(dirty.markers.call.start), target: range(dependency.markers.decl) };
}
function workspaceFileContracts(requirements) {
  if (!requirements.some(r => /^vscode\/UX17\/explorer-/.test(r.id))) return [];
  const key = (id, value) => ({ id, device: "keyboard", key: value });
  const text = (id, value) => ({ id, device: "keyboard", text: value });
  const pointer = (id, selector, button = "left") => ({ id, device: "pointer", selector, button });
  const palette = (prefix, title) => [key(prefix + "-open", "Meta+Shift+P"), text(prefix + "-name", title), key(prefix + "-accept", "Enter")];
  const picker = (prefix, file) => [key(prefix + "-open", "Meta+p"), text(prefix + "-name", file), pointer(prefix + "-accept", "Quick Open file " + file)];
  // Go to Line/Column uses visible columns: the preceding 中 occupies two.
  // LSP expectations below continue to use the independent UTF-16 marker.
  const go = (prefix, p) => [key(prefix + "-goto", "Control+g"), text(prefix + "-position", p.line === 2 ? spec.oracle.callInput : `${p.line + 1}:${p.character + 1}`), key(prefix + "-place", "Enter")];
  const query = prefix => [{ id: prefix + "-command", device: "command", command: "vscode.executeDefinitionProvider" }, key(prefix + "-native", "F12")];
  const check = (id, level, expected) => ({ id, level, expected });
  return ["create", "rename", "delete"].map(route => {
    const m = workspaceFileModel(route), o = m.o, file = o.caller;
    const source = document => ({ file, text: document.text, dirty: true, disk: m.disk.text, languageId: "vela",
      selections: [{ anchor: point(document.markers.call.start), active: point(document.markers.call.start) }] });
    const location = target => target ? { file: target, range: m.target, text: o.symbol } : null;
    const diagnostic = document => ({ uri: file, diagnostics: [{ code: spec.oracle.missingCode, message: `unresolved module \`${o.module}\``,
      severity: 1, source: "vela", range: range(document.markers.import), data: {
        labels: [{ uri: file, range: range(document.markers.import), message: spec.oracle.missingLabel }], candidates: [], repairHints: [] } }] });
    const facts = (prefix, document, target, missing) => [check(prefix + "-source", "Input", source(document)),
      check(prefix + "-diagnostics", "Input", missing ? diagnostic(document) : { uri: file, diagnostics: [] }),
      check(prefix + "-command", "Command", target ? [location(target)] : []),
      check(prefix + "-wire", "Input", { method: "textDocument/definition", request: { file, position: point(document.markers.call.start) }, result: location(target) }),
      check(prefix + "-destination", "Input", target ? { file: target, text: m.dependency.text, dirty: false, disk: m.dependency.text,
        languageId: "vela", selections: [{ anchor: m.target.start, active: m.target.start }] } : source(document))];
    const returnCaller = prefix => [...picker(prefix, file), ...go(prefix, m.call)];
    const explorer = prefix => palette(prefix, "View: Show Explorer");
    const create = prefix => [...explorer(prefix + "-explorer"), key(prefix + "-folder-home", "Home"), pointer(prefix + "-folder", "Explorer folder scripts"),
      pointer(prefix + "-new", "Explorer New File"), text(prefix + "-filename", path.posix.basename(o.dependency)), key(prefix + "-create", "Enter"),
      text(prefix + "-content", m.dependency.text), key(prefix + "-save", "Meta+s"), ...returnCaller(prefix + "-caller")];
    let actions = [key("dirty-home", "Meta+Home"), text("dirty-prefix", spec.oracle.dirtyPrefix), ...go("baseline", m.call), ...query("baseline")];
    let checks = facts("baseline", m.dirty, route === "create" ? null : o.dependency, route === "create");
    if (route === "create") {
      actions.push(...create("create"), ...query("current")); checks.push(...facts("current", m.dirty, o.dependency, false));
    } else if (route === "rename") {
      actions.push(...explorer("rename-explorer"), pointer("rename-file", "Explorer file " + path.posix.basename(o.dependency)), key("rename-start", "F2"),
        key("rename-select", "Meta+a"), text("rename-filename", path.posix.basename(o.renamed)), key("rename-accept", "Enter"),
        ...returnCaller("missing-caller"), ...query("missing"), ...returnCaller("rewrite-caller"), ...go("rewrite-import", { line: 1, character: 4 }),
        key("rewrite-select", "Meta+Shift+ArrowRight"), text("rewrite-text", `${o.renamedModule}::${o.symbol};`), ...go("current", m.call), ...query("current"));
      checks.push(...facts("missing", m.dirty, null, true), ...facts("current", m.renamed, o.renamed, false),
        check("disk-membership", "Input", { old: null, current: m.dependency.text }));
    } else {
      actions.push(...explorer("delete-explorer"), pointer("delete-file", "Explorer file " + path.posix.basename(o.dependency), "right"),
        pointer("delete-menu", "Explorer Delete"), pointer("delete-confirm", "Confirm file deletion"), ...returnCaller("missing-caller"), ...query("missing"),
        ...create("restore"), ...query("current"));
      checks.push(check("confirmation", "Input", { role: "dialog", visible: true,
        name: `Info Are you sure you want to delete '${path.posix.basename(o.dependency)}'?`, button: "Move to Trash" }),
        ...facts("missing", m.dirty, null, true), check("deleted-disk", "Input", { file: o.dependency, text: null }),
        ...facts("current", m.dirty, o.dependency, false));
    }
    const id = "ux17-explorer-" + route;
    return { id, fixture: spec.id, deadlineMs: 60000, requirements: ["command", "input"].map(level => {
      const r = requirements.find(r => r.id === `vscode/UX17/explorer-${route}/${level}/local`);
      if (!r) throw Error("missing workspace file obligation " + id + "/" + level);
      return { id: r.id, contractHash: r.contractHash };
    }), actions, checks, artifacts: ["trace.json", id + ".png", id + ".aria.txt", "workbench.log", "lsp-trace.log", "server-trace.jsonl",
      ...(route === "delete" ? [id + "-confirmation.png", id + "-confirmation.aria.txt"] : [])] };
  });
}
module.exports = { workspaceFileModel, workspaceFileContracts };
