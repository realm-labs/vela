"use strict";
// Complete authored state transitions, independent of installed providers.
const { parseMarkers } = require("./fixtures");
const { workspaceRootsModel } = require("./workspace-roots-contracts");
const spec = require("../../tests/lsp_matrix/fixtures/input-invalid-config-schema.json");
const oracle = require("../../tests/lsp_matrix/fixtures/workspace-configuration.json").oracle;
const point = p => ({ line: p.line, character: p.character });
const span = m => ({ start: point(m.start), end: point(m.end) });
function invalidConfigSchemaModel(platform = "darwin", crlf = false) {
  const parse = text => parseMarkers(crlf ? text.replaceAll("\n", "\r\n") : text);
  const roots = workspaceRootsModel(platform, crlf), o = spec.oracle;
  const workspace = phase => JSON.stringify({ folders: [roots.o.right, roots.o.left].map(path => ({ path })),
    settings: { ...roots.settings, "vela.host.schema": phase.schema, ...(phase.roots === undefined ? {} : { "vela.workspace.roots": phase.roots }) } });
  return { spec, o, roots, workspace, disk: parse(spec.files[o.file]), dirty: parse(o.dirtyPrefix + spec.files[o.file]),
    invalidManifest: parse(o.invalidManifest), validManifest: parse(o.validManifest), point, span };
}
function invalidConfigSchemaContracts(requirements, platform = "darwin") {
  if (!requirements.some(r => r.id.startsWith("vscode/UX17/invalid-config-schema/"))) return [];
  const m = invalidConfigSchemaModel(platform), o = m.o, ro = m.roots.o, actions = [], checks = [], seen = new Set();
  const key = (id, key) => ({ id, device: "keyboard", key });
  const text = (id, text) => ({ id, device: "keyboard", text });
  const pointer = (id, selector, button = "left") => ({ id, device: "pointer", selector, button });
  const check = (id, level, expected) => checks.push({ id, level, expected });
  const palette = (id, label) => [key(id + "-open", "Meta+Shift+P"), text(id + "-name", label), pointer(id + "-accept", "Command palette " + label)];
  const picker = (id, file) => [key(id + "-open", "Meta+p"), { id: id + "-name", device: "keyboard", path: file }, pointer(id + "-accept", "Quick Open file " + file)];
  const selection = p => [{ anchor: p, active: p }];
  const editorWrite = (id, file, content, open = true) => {
    if (open) actions.push(...picker(id + "-editor", file));
    actions.push(key(id + "-select", "Meta+a"), key(id + "-clear", "Backspace"), text(id + "-replace", content), key(id + "-save", "Meta+s"));
    check(id + "-selected", "Input", { file, allSelected: true });
    check(id + "-empty", "Input", { file, text: "", selections: selection({ line: 0, character: 0 }) });
    check(id + "-inserted", "Input", { file, text: content, dirty: true });
    check(id + "-saved", "Input", { file, text: content, disk: content, dirty: false });
  };
  const explorer = id => palette(id + "-explorer", "View: Show Explorer");
  const create = (id, file) => {
    actions.push(...explorer(id), key(id + "-home", "Home"), pointer(id + "-folder", "Explorer folder left"), pointer(id + "-new", "Explorer New File"),
      text(id + "-filename", file.split("/").at(-1)), key(id + "-create", "Enter"));
    check(id + "-created", "Input", { file, text: "", disk: "", dirty: false });
  };
  const remove = (id, file) => {
    actions.push(...picker(id + "-reveal", file), ...palette(id + "-focus", "File: Reveal Active File in Explorer View"),
      pointer(id + "-context", "Explorer file " + file.split("/").at(-1), "right"), pointer(id + "-menu", "Explorer Delete"), pointer(id + "-confirm", "Confirm file deletion"));
    check(id + "-confirmation", "Input", { role: "dialog", visible: true, name: `Info Are you sure you want to delete '${file.split("/").at(-1)}'?`,
      button: platform === "win32" ? "Move to Recycle Bin" : "Move to Trash" });
    check(id + "-deleted", "Input", { file, disk: null });
  };
  const zero = { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } };
  const generic = (code, message, severity = 1) => ({ code, message, severity, source: "vela", range: zero, data: { labels: [], candidates: [], repairHints: [] } });
  const unknownField = schema => {
    const unknown = oracle.fieldErrors[schema]; if (!unknown) return [];
    const field = unknown.field, range = span(m.dirty.markers[field]), names = unknown.candidates;
    return [{ code: "analysis::unknown_field", message: `unknown field \`${field}\` for \`HostCell\``, severity: 1, source: "vela", range,
      data: { labels: ["unknown member access", `did you mean \`${names[0]}\`?`, "similar candidates: " + names.join(", ")].map(message => ({ uri: o.file, range, message })),
        candidates: names.map(replacement => ({ replacement })), repairHints: [] } }];
  };
  const moduleError = module => {
    const range = span(m.roots.dirty(ro.leftCaller).markers["import-" + module]);
    return { code: "hir::unresolved_module", message: "unresolved module `" + module + "`", severity: 1, source: "vela", range,
      data: { labels: [{ uri: ro.leftCaller, range, message: ro.missingModule.label }], candidates: [], repairHints: [] } };
  };
  // Prefix insertion keeps native indentation behavior out of the authored
  // fixture body and establishes the same dirty authority as the schema proof.
  actions.push(...picker("probe", o.file), key("probe-home", "Meta+Home"), text("probe-replace", o.dirtyPrefix));
  check("probe-before", "Input", { file: o.file, text: m.disk.text, disk: m.disk.text, dirty: false, selections: selection({ line: 0, character: 0 }) });
  check("probe-inserted", "Input", { file: o.file, text: m.dirty.text, disk: m.disk.text, dirty: true });
  for (const phase of o.phases) {
    const id = phase.id, schemaFile = phase.schema ? ro.left + "/" + phase.schema : null;
    const state = phase.unavailable ? oracle.schemaStates.find(s => s.path === "schema-missing.json") : oracle.schemaStates.find(s => s.path === phase.schema);
    const error = state.error?.replace("{SCHEMA}", "<workspace>/" + schemaFile);
    switch (phase.operation) {
      case "settings": editorWrite(id, ro.workspace, m.workspace(phase)); break;
      case "delete-schema": remove(id, schemaFile); break;
      case "create-schema": create(id, schemaFile); editorWrite(id, schemaFile, m.roots.disk(schemaFile).text, false); break;
      case "create-manifest": create(id, o.manifest); break;
      case "invalid-manifest": editorWrite(id, o.manifest, m.invalidManifest.text); break;
      case "repair-manifest": editorWrite(id, o.manifest, m.validManifest.text); break;
      case "delete-manifest": remove(id, o.manifest); break;
      default: throw Error("unknown invalid configuration operation");
    }
    if (schemaFile) seen.add(schemaFile);
    // A valid empty manifest has no metadata diagnostic owner yet. Owners enter
    // the retained clear set when they actually acquire an authored error.
    if (phase.operation === "invalid-manifest") seen.add(o.manifest);
    check(id + "-workspace-json", "Input", { file: ro.workspace, document: JSON.parse(m.workspace(phase.operation === "settings" ? phase : o.phases[5])) });
    check(id + "-folders", "Input", { folders: [ro.right, ro.left], settingsRoots: phase.roots ?? null });
    check(id + "-client", "Input", { serverFolder: ro.right, started: true });
    const sourceFiles = [o.file, ro.leftCaller, ro.leftTarget, ro.rightTarget, ro.rightCaller].sort();
    const publications = sourceFiles.map(uri => ({ uri, diagnostics: error ? [generic("schema::unavailable", error, 2)] : uri === o.file ? unknownField(phase.schema)
      : uri === ro.leftCaller && phase.scratch ? [moduleError("alpha"), moduleError("beta")] : uri === ro.leftCaller && phase.package ? [moduleError("beta")] : [] }));
    for (const uri of [...seen].sort()) {
      const diagnostics = uri === schemaFile && error ? [generic("schema::diagnostic", error)] : uri === o.manifest && phase.operation === "invalid-manifest" ? [generic("project::diagnostic",
        `source.roots must be an array of strings at bytes ${m.invalidManifest.markers.bad.start.byte}..${m.invalidManifest.markers.bad.end.byte}`)] : [];
      publications.push({ uri, diagnostics });
    }
    check(id + "-diagnostics", "Input", publications.sort((a, b) => a.uri.localeCompare(b.uri, "en")));
    if (id === "rejected") check(id + "-rejection", "Input", { type: 1, message: "invalid didChangeConfiguration settings: invalid type: integer `7`, expected a sequence" });
    // Hover receipts include the complete wire result and independent visible widget.
    actions.push(...picker(id + "-caller", o.file));
    for (const field of ["value", "gone", "rank"]) {
      const query = id + "-" + field, p = point(m.dirty.markers[field].start), type = state.fields[field];
      const result = type === null ? null : { contents: { kind: "markdown", value: `\`\`\`vela\nHostCell.${field}\n\`\`\`\n\n_field_: ${type}` }, range: span(m.dirty.markers[field]) };
      actions.push(key(query + "-goto", "Control+g"), text(query + "-position", `${p.line + 1}:${p.character + 1}`), key(query + "-place", "Enter"),
        { id: query + "-command", device: "command", command: "vscode.executeHoverProvider", field }, key(query + "-dismiss-before", "Escape"),
        key(query + "-chord", "Meta+k"), key(query + "-native", "Meta+i"), key(query + "-dismiss", "Escape"));
      check(query + "-source", "Input", { file: o.file, text: m.dirty.text, disk: m.disk.text, dirty: true, selections: selection(p) });
      check(query + "-command", "Command", result ? [{ contents: [{ value: result.contents.value }], range: result.range }] : []);
      check(query + "-wire", "Input", { method: "textDocument/hover", request: { file: o.file, position: p }, result });
      check(query + "-widget", "Input", result ? { visible: true, label: "HostCell." + field, paragraphs: ["field: " + type], diagnostics: [] }
        : oracle.fieldErrors[phase.schema] && !error ? { visible: true, label: "", paragraphs: [], diagnostics: [`unknown field \`${field}\` for \`HostCell\`vela(analysis::unknown_field)`] } : { visible: false });
    }
  }
  check("final-overlays", "Input", [ro.leftTarget, ro.leftCaller, ro.rightCaller, o.file].map(file => ({ file,
    text: file === o.file ? m.dirty.text : m.roots.dirty(file).text, disk: file === o.file ? m.disk.text : m.roots.disk(file).text, dirty: true })));
  const id = "ux17-invalid-config-schema";
  return [{ id, fixture: spec.id, deadlineMs: 120000, requirements: ["command", "input"].map(level => {
    const r = requirements.find(r => r.id === `vscode/UX17/invalid-config-schema/${level}/local`);
    if (!r) throw Error("missing invalid configuration/schema obligation " + level);
    return { id: r.id, contractHash: r.contractHash };
  }), actions, checks, artifacts: ["trace.json", id + ".png", id + ".aria.txt", "workbench.log", "lsp-trace.log", "server-trace.jsonl", id + "-sessions.json"] }];
}
module.exports = { invalidConfigSchemaModel, invalidConfigSchemaContracts };
