"use strict";
const { outlineModel } = require("./outline-contracts");
const spec = require("../../tests/lsp_matrix/fixtures/input-document-picker.json");

// Pinned SDK flattens authored trees, sorts source ranges, and uses immediate
// parent names (not detail strings) as descriptions. Never read provider/DOM.
function documentPickerModel() {
  const source = outlineModel(), remap = f => f.replace("scripts/outline_", "scripts/document_picker_");
  const files = Object.fromEntries(Object.entries(source.files).map(([f,t]) => [remap(f),t]));
  const cases = source.cases.map(c => ({ ...c, file: remap(c.file), nodes: c.nodes.toSorted((a,b) =>
    a.range.start.line-b.range.start.line || a.range.start.character-b.range.start.character ||
    b.range.end.line-a.range.end.line || b.range.end.character-a.range.end.character) }));
  return { spec, files, cases };
}
function documentPickerContracts(requirements) {
  if (!requirements.some(r => r.id.startsWith("vscode/UX11/"))) return [];
  const m = documentPickerModel(), actions = [], checks = [];
  const key = (id,key) => actions.push({ id, device: "keyboard", key });
  const check = (id,level,expected) => checks.push({ id,level,expected });
  const row = n => ({ name: n.name, description: n.parent ?? "", icon: n.icon });
  for (const c of m.cases) {
    check(`${c.id}-source`, "Input", { file: c.file, text: c.text, dirty: false });
    key(`${c.id}-open`, "Meta+Shift+o");
    check(`${c.id}-request`, "Input", { method: "textDocument/documentSymbol", completed: true });
    check(`${c.id}-query`, "Render", { value: "@" });
    if (!c.nodes.length) {
      check(`${c.id}-empty`, "Render", { name: spec.oracle.emptyText, description: "", icon: "", disabled: false });
      key(`${c.id}-empty-accept`, "Enter");
      check(`${c.id}-empty-retained`, "Input", { file: c.file, text: c.text, dirty: false, selections: [{ anchor: { line: 0, character: 0 }, active: { line: 0, character: 0 } }], pickerVisible: true });
      key(`${c.id}-dismiss`, "Escape");
    } else {
      check(`${c.id}-separator`, "Render", { text: spec.oracle.separator.replace("{count}", c.nodes.length) });
      key(`${c.id}-home`, "Control+Home");
      for (const [index,n] of c.nodes.entries()) {
        const id = `${c.id}-${index}`;
        if (index) key(`${id}-next`, "ArrowDown");
        check(`${id}-row`, "Render", row(n));
        if (index % 5 === 0) actions.push({ id: `${id}-accept`, device: "pointer", selector: "document symbol picker focused row", clickCount: 1 });
        else key(`${id}-accept`, "Enter");
        check(`${id}-destination`, "Input", { file: c.file, text: c.text, dirty: false,
          selections: [{ anchor: n.selection.start, active: n.selection.start }] });
        // Reopening uses the SDK's source-range containing-position selection.
        key(`${id}-reopen`, "Meta+Shift+o");
        check(`${id}-restored-row`, "Render", row(n));
      }
      key(`${c.id}-end`, "Control+End");
      check(`${c.id}-tail`, "Render", row(c.nodes.at(-1)));
      key(`${c.id}-past-tail`, "ArrowDown");
      check(`${c.id}-wrapped-head`, "Render", row(c.nodes[0]));
      key(`${c.id}-select-query`, "Meta+a");
      actions.push({ id: `${c.id}-no-match-query`, device: "keyboard", text: "@" + spec.oracle.noMatchQuery });
      check(`${c.id}-no-match`, "Render", { name: spec.oracle.noMatchText, description: "", icon: "", disabled: false });
      key(`${c.id}-no-match-accept`, "Enter");
      check(`${c.id}-no-match-retained`, "Input", { file: c.file, text: c.text, dirty: false,
        selections: [{ anchor: c.nodes.at(-1).selection.start, active: c.nodes.at(-1).selection.start }], pickerVisible: true });
      key(`${c.id}-dismiss`, "Escape");
    }
    check(`${c.id}-final-source`, "Input", { file: c.file, text: c.text, dirty: false });
  }
  check("disk-inputs", "Input", m.files);
  return [{ id: "ux11-document-picker", fixture: spec.id, deadlineMs: 240000,
    requirements: ["input","render"].map(level => {
      const id = `vscode/UX11/document-picker/${level}/local`, r = requirements.find(r => r.id === id);
      if (!r) throw Error(`missing document picker obligation ${id}`);
      return { id, contractHash: r.contractHash };
    }), actions, checks,
    artifacts: ["trace.json", "ux11-document-picker-final.png", "ux11-document-picker-final.aria.txt", "ux11-document-picker-observations.json",
      ...m.cases.flatMap(c => [`ux11-document-picker-${c.id}.png`, `ux11-document-picker-${c.id}.aria.txt`]),
      ...m.cases.filter(c => c.nodes.length).flatMap(c => [`ux11-document-picker-${c.id}-no-match.png`, `ux11-document-picker-${c.id}-no-match.aria.txt`]),
      "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"] }];
}
module.exports = { documentPickerModel, documentPickerContracts };
