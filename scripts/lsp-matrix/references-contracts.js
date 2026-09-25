"use strict";
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-references.json");

function referencesModel() {
  const documents = Object.fromEntries(Object.entries(spec.files).map(([file, source]) => [file, parseMarkers(source)]));
  const point = ({ line, character }) => ({ line, character });
  const sites = spec.oracle.referenceSites.map(({ file, marker }) => ({
    file, range: { start: point(documents[file].markers[marker].start), end: point(documents[file].markers[marker].end) }, text: "grant",
  })).sort((a, b) => a.file.localeCompare(b.file) || a.range.start.line - b.range.start.line ||
    a.range.start.character - b.range.start.character);
  const call = documents[spec.oracle.openFile].markers.call.start;
  return { spec, documents, sites, cursor: { line: call.line, character: call.character + 1 } };
}

function referencesContracts(requirements) {
  if (!requirements.some((item) => item.id.startsWith("vscode/UX06/"))) return [];
  const model = referencesModel(), id = "ux06-references-select";
  const refs = ["input", "render"].map((level) => {
    const name = `vscode/UX06/references-select/${level}/local`;
    const requirement = requirements.find((item) => item.id === name);
    if (!requirement) throw Error(`missing references obligation: ${name}`);
    return { id: name, contractHash: requirement.contractHash };
  });
  const groups = [...new Set(model.sites.map((site) => site.file))];
  const groupActions = (file, index) => [
    { id: `select-group-${index}-${file}`, device: "pointer", target: file, clickCount: 1 },
    { id: `expand-group-${index}-${file}`, device: "keyboard", key: "ArrowRight" },
  ];
  const actions = model.sites.flatMap((site, index) => [
    { id: `open-references-${index}`, device: "keyboard", key: "Shift+F12" },
    ...(index === 0 ? groups : [site.file]).flatMap((file) => groupActions(file, index)),
    { id: `select-reference-${index}`, device: "pointer", target: site, clickCount: 2 },
  ]);
  const checks = [
    { id: "origin", level: "Input", expected: {
      file: spec.oracle.openFile, text: model.documents[spec.oracle.openFile].text,
      dirty: false, selections: [{ anchor: model.cursor, active: model.cursor }],
    } },
    { id: "unopened-targets", level: "Input", expected: { origin: true, closed: true } },
    { id: "panel", level: "Render", expected: { visible: true, count: model.sites.length } },
    { id: "file-counts", level: "Render", expected: Object.fromEntries(groups.map((file) => [file,
      model.sites.filter((site) => site.file === file).length])) },
    { id: "reference-set", level: "Render", expected: model.sites },
    ...model.sites.map((site, index) => ({ id: `destination-${index}`, level: "Input", expected: {
      file: site.file, text: model.documents[site.file].text, dirty: false,
      selections: [{ anchor: site.range.start, active: site.range.end }],
    } })),
  ];
  return [{ id, fixture: spec.id, deadlineMs: 60000, requirements: refs, actions, checks,
    artifacts: ["trace.json", `${id}.png`, `${id}.aria.txt`, `${id}-rows.json`,
      "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"] }];
}

module.exports = { referencesModel, referencesContracts };
