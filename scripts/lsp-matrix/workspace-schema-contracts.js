"use strict";
// Authored static metadata and complete responses. Never use a provider to
// construct these expectations or to perform the accepted artifact replacement.
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-workspace-schema.json");
const point = p => ({ line: p.line, character: p.character });
const span = marker => ({ start: point(marker.start), end: point(marker.end) });
function workspaceSchemaModel(crlf = false) {
  const parse = source => parseMarkers(crlf ? source.replaceAll("\n", "\r\n") : source);
  // The existing independent completion artifact owns this installed window.
  // Keep all its facts verbatim; only our distinct type/fields are replaced.
  const completion = JSON.parse(require("../../tests/lsp_matrix/fixtures/input-completion.json").files[spec.oracle.schema]);
  const artifact = source => JSON.stringify({ ...completion, facts: { ...completion.facts, ...JSON.parse(source).facts } });
  return { spec, o: spec.oracle, disk: parse(spec.files[spec.oracle.file]),
    dirty: parse(spec.oracle.dirtyPrefix + spec.files[spec.oracle.file]),
    original: artifact(spec.files[spec.oracle.seed]), replacement: artifact(spec.oracle.replacement) };
}
function workspaceSchemaContracts(requirements) {
  if (!requirements.some(r => r.id.startsWith("vscode/UX17/schema-replace/"))) return [];
  const m = workspaceSchemaModel(), o = m.o;
  const key = (id, value) => ({ id, device: "keyboard", key: value });
  const text = (id, value) => ({ id, device: "keyboard", text: value });
  const check = (id, level, expected) => ({ id, level, expected });
  const picker = (prefix, file) => [key(prefix + "-open", "Meta+p"), text(prefix + "-name", file),
    { id: prefix + "-accept", device: "pointer", selector: "Quick Open file " + file, button: "left" }];
  const source = field => ({ file: o.file, text: m.dirty.text, dirty: true, disk: m.disk.text, languageId: "vela",
    selections: [{ anchor: point(m.dirty.markers[field].start), active: point(m.dirty.markers[field].start) }] });
  const hover = (state, field) => state.fields[field] === null ? null : {
    contents: { kind: "markdown", value: "```vela\nWorkbenchCell." + field + "\n```\n\n_field_: " + state.fields[field] },
    range: span(m.dirty.markers[field]) };
  const diagnostics = state => {
    const field = state.unknown, range = span(m.dirty.markers[field]), names = state.candidates;
    return { uri: o.file, diagnostics: [{ code: "analysis::unknown_field", message: `unknown field \`${field}\` for \`WorkbenchCell\``,
      severity: 1, source: "vela", range, data: {
        labels: ["unknown member access", `did you mean \`${names[0]}\`?`, "similar candidates: " + names.join(", ")]
          .map(message => ({ uri: o.file, range, message })),
        candidates: names.map(replacement => ({ replacement })), repairHints: [] } }] };
  };
  const actions = [key("dirty-home", "Meta+Home"), text("dirty-prefix", o.dirtyPrefix)], checks = [];
  for (const state of o.states) {
    const prefix = state.id, artifact = prefix === "current" ? m.replacement : m.original;
    if (prefix !== "baseline") {
      const previous = prefix === "current" ? m.original : m.replacement;
      const editor = (text, dirty, anchor, active) => ({ file: o.schema, text, dirty, disk: previous, languageId: "json", selections: [{ anchor, active }] });
      const home = { line: 0, character: 0 }, end = text => ({ line: 0, character: text.length });
      actions.push(...picker(prefix + "-artifact", o.schema), key(prefix + "-select", "Meta+a"), key(prefix + "-clear", "Backspace"),
        text(prefix + "-replace", artifact), key(prefix + "-save", "Meta+s"), ...picker(prefix + "-caller", o.file));
      checks.push(check(prefix + "-selection", "Input", editor(previous, false, home, end(previous))),
        check(prefix + "-empty", "Input", editor("", true, home, home)),
        check(prefix + "-inserted", "Input", editor(artifact, true, end(artifact), end(artifact))));
    }
    checks.push(check(prefix + "-disk", "Input", { file: o.schema, text: artifact }),
      check(prefix + "-diagnostics", "Input", diagnostics(state)), check(prefix + "-metadata-diagnostics", "Input", { uri: o.schema, diagnostics: [] }));
    for (const field of ["value", "gone", "rank"]) {
      const id = prefix + "-" + field, p = m.dirty.markers[field].start, result = hover(state, field);
      actions.push(key(id + "-goto", "Control+g"), text(id + "-position", `${p.line + 1}:${p.character + 1}`), key(id + "-place", "Enter"),
        { id: id + "-command", device: "command", command: "vscode.executeHoverProvider", field },
        key(id + "-dismiss-before", "Escape"), key(id + "-chord", "Meta+k"), key(id + "-native", "Meta+i"), key(id + "-dismiss", "Escape"));
      checks.push(check(id + "-source", "Input", source(field)),
        check(id + "-command", "Command", result ? [{ contents: [{ value: result.contents.value }], range: result.range }] : []),
        check(id + "-wire", "Input", { method: "textDocument/hover", request: { file: o.file, position: point(p) }, result }),
        check(id + "-widget", "Input", result ? { visible: true, label: "WorkbenchCell." + field, paragraphs: ["field: " + state.fields[field]], diagnostics: [] }
          : { visible: true, label: "", paragraphs: [], diagnostics: [`unknown field \`${field}\` for \`WorkbenchCell\`vela(analysis::unknown_field)`] }));
    }
  }
  checks.push(check("final-source", "Input", source("rank")), check("final-disk", "Input", { file: o.schema, text: m.original }));
  const id = "ux17-schema-replace";
  return [{ id, fixture: spec.id, deadlineMs: 60000, requirements: ["command", "input"].map(level => {
    const r = requirements.find(r => r.id === `vscode/UX17/schema-replace/${level}/local`);
    if (!r) throw Error("missing schema replacement obligation " + level);
    return { id: r.id, contractHash: r.contractHash };
  }), actions, checks, artifacts: ["trace.json", id + ".png", id + ".aria.txt", "workbench.log", "lsp-trace.log", "server-trace.jsonl",
    ...o.states.map(s => id + "-" + s.id + ".png")] }];
}
module.exports = { workspaceSchemaModel, workspaceSchemaContracts };
