"use strict";

const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-diagnostics.json");
const file = "scripts/diagnostics.vela";

function model() {
  const disk = parseMarkers(spec.files[file]);
  const typed = parseMarkers(spec.oracle.typed);
  const point = ({ line, character }) => ({ line, character });
  const range = (marker, document) => ({
    start: point(document.markers[marker].start),
    end: point(document.markers[marker].end),
  });
  const diagnostic = (marker, document) => ({
    code: spec.oracle[marker].code,
    message: spec.oracle[marker].message,
    severity: 0,
    range: range(marker, document),
    related: 0,
  });
  return {
    spec, file, disk, typed,
    cursor: point(disk.markers.cursor.start),
    afterType: point(typed.markers.target.end),
    target: diagnostic("target", typed),
    unrelated: diagnostic("unrelated", typed),
    valid: range("valid", disk),
  };
}

function diagnosticContracts(requirements) {
  if (!requirements.some((item) => item.id.startsWith("vscode/UX07/"))) return [];
  const m = model();
  const refs = (route) => ["input", "render"].map((level) => {
    const id = `vscode/UX07/${route}/${level}/local`;
    const requirement = requirements.find((item) => item.id === id);
    if (!requirement) throw Error(`missing diagnostic obligation ${id}`);
    return { id, contractHash: requirement.contractHash };
  });
  const key = (id, value) => ({ id, device: "keyboard", key: value });
  const check = (id, level, expected) => ({ id, level, expected });
  const state = (text, dirty, position) => ({
    file, text, dirty, disk: m.disk.text,
    selections: [{ anchor: position, active: position }],
  });
  const baseline = check("origin", "Input", state(m.disk.text, false, m.cursor));
  const typed = check("typed-source", "Input", state(m.typed.text, true, m.afterType));
  const typedDiagnostics = check("typed-diagnostics", "Input", [m.target, m.unrelated]);
  const location = (item) => `Ln ${item.range.start.line + 1}, Col ${item.range.start.character + 1}`;
  const panel = check("problems-panel", "Render", {
    visible: true,
    target: { code: m.target.code, message: m.target.message, severity: "error", location: location(m.target) },
    unrelated: { code: m.unrelated.code, message: m.unrelated.message, severity: "error", location: location(m.unrelated) },
  });
  const selected = check("selected-problem", "Input", {
    file, text: m.typed.text, dirty: true,
    selections: [{ anchor: m.target.range.start, active: m.target.range.end }],
  });
  const decoration = check("target-decoration", "Render", {
    marker: "target", text: "@", aligned: true, visible: true,
  });
  const sharedActions = [
    { id: "type-error", device: "keyboard", text: spec.oracle.typedText },
    key("open-problems", "Meta+Shift+M"),
    { id: "select-problem", device: "pointer", target: m.target.message, clickCount: 2 },
  ];
  const artifacts = (id) => [
    "trace.json", `${id}.png`, `${id}.aria.txt`, `${id}-observations.json`,
    "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl",
  ];
  return [
    {
      id: "ux07-problems-navigate", fixture: spec.id, deadlineMs: 45000,
      requirements: refs("problems-navigate"), actions: sharedActions,
      checks: [baseline, typed, typedDiagnostics, panel, selected, decoration],
      artifacts: artifacts("ux07-problems-navigate"),
    },
    {
      id: "ux07-repair-unsaved", fixture: spec.id, deadlineMs: 45000,
      requirements: refs("repair-unsaved"),
      actions: [...sharedActions, key("focus-editor", "Meta+1"), key("repair-error", "Backspace")],
      checks: [baseline, typed, typedDiagnostics, panel, selected, decoration,
        check("repair-focus", "Input", { focused: true }),
        check("repaired-source", "Input", {
          file, text: m.disk.text, dirty: true, disk: m.disk.text,
        }),
        check("remaining-diagnostics", "Input", [m.unrelated]),
        check("remaining-panel", "Render", { target: false, unrelated: true }),
        check("cleared-target-decoration", "Render", { marker: "target", visible: false, unrelatedVisible: true }),
      ],
      artifacts: artifacts("ux07-repair-unsaved"),
    },
    {
      id: "ux07-valid-location", fixture: spec.id, deadlineMs: 45000,
      requirements: refs("valid-location"),
      actions: [
        { id: "type-error", device: "keyboard", text: spec.oracle.typedText },
        key("open-problems", "Meta+Shift+M"),
        { id: "select-valid", device: "pointer", target: "valid", clickCount: 1 },
      ],
      checks: [baseline, typed, typedDiagnostics, panel,
        check("valid-caret", "Input", { file, position: m.valid.start }),
        check("valid-source", "Input", state(m.typed.text, true, m.valid.start)),
        check("valid-decoration", "Render", { marker: "valid", visible: false }),
        check("unchanged-diagnostics", "Input", [m.target, m.unrelated]),
      ],
      artifacts: artifacts("ux07-valid-location"),
    },
  ];
}

module.exports = { diagnosticModel: model, diagnosticContracts };
