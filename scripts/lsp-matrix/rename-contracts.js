"use strict";
const { parseMarkers, applyEdits } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-rename.json");

function renameModel() {
  const documents = Object.fromEntries(Object.entries(spec.files).map(([file, source]) => [file, parseMarkers(source)]));
  const original = Object.fromEntries(Object.entries(documents).map(([file, document]) => [file, document.text]));
  const renamed = Object.fromEntries(Object.entries(documents).map(([file, document]) => [
    file,
    applyEdits(document.text, spec.oracle.sites.filter((site) => site.file === file).map((site) => ({
      range: document.markers[site.marker], newText: spec.oracle.rename,
    }))),
  ]));
  const cursor = documents[spec.oracle.openFile].markers.cursor.start;
  return { spec, original, renamed, cursor: { line: cursor.line, character: cursor.character + 2 } };
}

function renameContracts(requirements) {
  if (!requirements.some((item) => item.id.startsWith("vscode/UX05/"))) return [];
  const id = "ux05-rename-confirm", model = renameModel();
  const refs = ["input", "render"].map((level) => {
    const name = `vscode/UX05/rename-confirm/${level}/local`;
    const requirement = requirements.find((item) => item.id === name);
    if (!requirement) throw Error(`missing rename obligation: ${name}`);
    return { id: name, contractHash: requirement.contractHash };
  });
  const key = (id, key) => ({ id, device: "keyboard", key });
  const check = (id, level, expected) => ({ id, level, expected });
  const source = (files, openDirty, disk = files, otherDirty = false) => ({
    openText: files[model.spec.oracle.openFile],
    openDirty,
    openDisk: disk[model.spec.oracle.openFile],
    originText: files["scripts/rename_origin.vela"],
    originDirty: otherDirty,
    originDisk: disk["scripts/rename_origin.vela"],
    closedText: files[model.spec.oracle.closedFile],
    closedDirty: otherDirty,
    closedDisk: disk[model.spec.oracle.closedFile],
  });
  return [{
    id, fixture: spec.id, deadlineMs: 60000, requirements: refs,
    actions: [key("open-rename", "F2"), key("select-name", "Meta+a"),
      { id: "type-name", device: "keyboard", text: spec.oracle.rename },
      key("confirm-rename", "Enter"), key("undo-rename", "Meta+z"),
      key("redo-rename", "Meta+Shift+z")],
    checks: [
      check("origin", "Input", source(model.original, false)),
      check("unopened-targets", "Input", { origin: true, closed: true }),
      check("editor-focus", "Input", { focused: true }),
      check("rename-widget", "Render", { visible: true, value: spec.oracle.source }),
      check("rename-focus", "Input", { focused: true }),
      check("typed-name", "Render", { visible: true, value: spec.oracle.rename }),
      check("widget-hidden", "Render", { visible: false }),
      check("renamed-workspace", "Input", source(model.renamed, false)),
      check("rename-confirmation", "Render", { text: "Successfully renamed 'grant' to 'award'. Summary: Made 4 text edits in 3 files" }),
      check("undo-workspace", "Input", source(model.original, true, model.renamed, true)),
      check("redo-workspace", "Input", source(model.renamed, false)),
      check("final-diagnostics", "Input", { open: [], origin: [], closed: [] }),
    ],
    artifacts: ["trace.json", `${id}-open.png`, `${id}-open.aria.txt`, `${id}.png`, `${id}.aria.txt`,
      ...["origin", "renamed-workspace", "undo-workspace", "redo-workspace"].map((state) => `${id}-${state}.json`),
      `${id}-diagnostics.json`,
      "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"],
  }];
}
module.exports = { renameModel, renameContracts };
