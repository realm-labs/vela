"use strict";
const path = require("node:path");
const { document } = require("./document-symbol-oracle");
const spec = require("../../tests/lsp_matrix/fixtures/input-outline.json");

// Authored declarations and ownership trees, never provider or DOM output.
// The pinned SDK renders name/detail and SymbolKind icons. Single click/Enter
// collapses selectionRange to its start; double click selects the whole range.
function outlineModel() {
  const files = {}, cases = [];
  for (const form of spec.forms) for (const source of spec.sources) {
    const fixture = require(path.join(__dirname, "../../tests/lsp_matrix/fixtures", source.fixture));
    const remap = file => `scripts/outline_${form}_${source.namespace}/${file.replace(/^scripts\//, "")}`;
    for (const [file, text] of Object.entries(fixture.files).filter(([file]) => file.endsWith(".vela")))
      files[remap(file)] = document(text, form === "crlf").text;
    const trees = fixture.oracle.trees ?? { [fixture.oracle.file]: fixture.oracle.symbols };
    for (const [file, rows] of Object.entries(trees)) {
      const doc = document(fixture.files[file], form === "crlf");
      const flat = (rows, parent = null, level = 1) => rows.flatMap(row => {
        const marker = name => {
          const m = doc.markers[name];
          return { start: { line: m.start.line, character: m.start.character }, end: { line: m.end.line, character: m.end.character } };
        };
        if (!spec.oracle.icons[row.kind]) throw Error(`unreviewed Outline kind ${row.kind}`);
        return [{ id: row.id, parent, level, name: row.name, detail: row.detail ?? "",
          icon: spec.oracle.icons[row.kind], range: marker(row.range), selection: marker(row.selection),
          expandable: row.children.length !== 0 }, ...flat(row.children, row.name, level + 1)];
      });
      cases.push({ id: `${form}-${source.namespace}-${file.replaceAll(/[/.]/g, "-")}`, file: remap(file), form,
        text: doc.text, roots: rows.length, nodes: flat(rows) });
    }
  }
  return { spec, files, cases };
}
function outlineContracts(requirements) {
  if (!requirements.some(r => r.id.startsWith("vscode/UX11/"))) return [];
  const m = outlineModel(), actions = [], checks = [];
  const key = (id, binding) => actions.push({ id, device: "keyboard", key: binding });
  const command = id => {
    key(`${id}-palette`, "Meta+Shift+P");
    actions.push({ id: `${id}-query`, device: "keyboard", text: spec.oracle.paletteLabel });
    key(`${id}-invoke`, "Enter");
  };
  const check = (id, level, expected) => checks.push({ id, level, expected });
  for (const c of m.cases) {
    command(`${c.id}-focus`);
    check(`${c.id}-source`, "Input", { file: c.file, text: c.text, dirty: false });
    if (!c.nodes.length) {
      check(`${c.id}-empty-request`, "Input", { method: "textDocument/documentSymbol", completed: true });
      check(`${c.id}-empty`, "Render", { text: spec.oracle.emptyText.replace("{file}", path.posix.basename(c.file)), rows: [] });
      check(`${c.id}-final-source`, "Input", { file: c.file, text: c.text, dirty: false });
      continue;
    }
    key(`${c.id}-home`, "Home");
    for (const [index, node] of c.nodes.entries()) {
      const id = `${c.id}-${index}`;
      if (index) key(`${id}-next`, "ArrowDown");
      if (node.expandable) {
        check(`${id}-initial-parent`, "Render", { name: node.name, detail: node.detail, icon: node.icon,
          level: node.level, parent: node.parent, expanded: true });
        key(`${id}-collapse`, "ArrowLeft");
        check(`${id}-collapsed-parent`, "Render", { name: node.name, detail: node.detail, icon: node.icon,
          level: node.level, parent: node.parent, expanded: false });
        key(`${id}-expand`, "ArrowRight");
      }
      check(`${id}-row`, "Render", { name: node.name, detail: node.detail, icon: node.icon,
        level: node.level, parent: node.parent, expanded: node.expandable ? true : null });
      actions.push({ id: `${id}-click`, device: "pointer", selector: "Outline focused treeitem label", clickCount: 1 });
      check(`${id}-selection`, "Input", { file: c.file, selections: [{ anchor: node.selection.start, active: node.selection.start }] });
    }
    // Enter and double click have different reviewed SDK selection semantics.
    const last = c.nodes.at(-1);
    key(`${c.id}-end`, "End");
    check(`${c.id}-tail`, "Render", { name: last.name, detail: last.detail, icon: last.icon, level: last.level,
      parent: last.parent, expanded: last.expandable ? true : null });
    key(`${c.id}-enter`, "Enter");
    check(`${c.id}-keyboard-focus`, "Input", { focused: true });
    check(`${c.id}-keyboard-selection`, "Input", { file: c.file, selections: [{ anchor: last.selection.start, active: last.selection.start }] });
    command(`${c.id}-refocus`);
    actions.push({ id: `${c.id}-double-click`, device: "pointer", selector: "Outline focused treeitem label", clickCount: 2 });
    check(`${c.id}-whole-range`, "Input", { file: c.file, selections: [{ anchor: last.range.start, active: last.range.end }] });
    check(`${c.id}-final-source`, "Input", { file: c.file, text: c.text, dirty: false });
  }
  check("disk-inputs", "Input", m.files);
  return [{ id: "ux11-outline", fixture: spec.id, deadlineMs: 120000,
    requirements: ["input", "render"].map(level => {
      const id = `vscode/UX11/outline/${level}/local`, r = requirements.find(r => r.id === id);
      if (!r) throw Error(`missing Outline obligation ${id}`);
      return { id, contractHash: r.contractHash };
    }), actions, checks,
    artifacts: ["trace.json", "ux11-outline-final.png", "ux11-outline-final.aria.txt", "ux11-outline-observations.json",
      ...m.cases.flatMap(c => [`ux11-outline-${c.id}.png`, `ux11-outline-${c.id}.aria.txt`]),
      "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"] }];
}
module.exports = { outlineModel, outlineContracts };
