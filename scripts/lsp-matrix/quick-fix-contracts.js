"use strict";

const { parseMarkers, applyEdits } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/diagnostic-action-method-typo.json");
const file = "scripts/game/main.vela";

function quickFixModel() {
  const disk = parseMarkers(spec.files[file]);
  const applied = parseMarkers(spec.oracle.applied);
  const range = (name, document = disk) => ({
    start: { line: document.markers[name].start.line, character: document.markers[name].start.character },
    end: { line: document.markers[name].end.line, character: document.markers[name].end.character },
  });
  const diagnostic = (name, document = disk) => {
    const oracle = spec.oracle.diagnostics.find((item) => item.marker === name);
    return { code: oracle.code, message: oracle.message, severity: 0, range: range(name, document), related: 0 };
  };
  const target = diagnostic("fix");
  if (!disk.text.startsWith("fn main(")) throw Error("quick-fix no-repair location changed");
  if (applyEdits(disk.text, [{ range: target.range, newText: spec.oracle.action.newText }]) !== applied.text)
    throw Error("quick-fix oracle does not match one exact replacement");
  return { spec, file, disk, applied, target, unrelated: diagnostic("unrelated"),
    cursor: target.range.start, noFixCursor: { line: 0, character: 3 } };
}

function quickFixContracts(requirements) {
  if (!requirements.some((item) => item.id.startsWith("vscode/UX08/"))) return [];
  const m = quickFixModel();
  const requirementsFor = (route) => ["input", "render"].map((level) => {
    const id = `vscode/UX08/${route}/${level}/local`;
    const requirement = requirements.find((item) => item.id === id);
    if (!requirement) throw Error(`missing quick-fix obligation ${id}`);
    return { id, contractHash: requirement.contractHash };
  });
  const check = (id, level, expected) => ({ id, level, expected });
  const state = (text, dirty) => ({ file, text, dirty, disk: m.disk.text });
  const initial = [
    check("origin", "Input", state(m.disk.text, false)),
    check("initial-diagnostics", "Render", [m.target, m.unrelated]),
  ];
  const menu = [
    check("visible-fixes", "Render", { visible: true, rows: ["Quick Fix", ...spec.oracle.actionTitles] }),
    check("selected-fix", "Render", { label: spec.oracle.action.title, focused: true }),
  ];
  const fixed = [
    check("applied-source", "Input", state(m.applied.text, true)),
    check("after-fix-diagnostics", "Render", [m.unrelated]),
    check("undo-focus", "Input", { focused: true }),
    check("undo-source", "Input", state(m.disk.text, false)),
    check("undo-diagnostics", "Render", [m.target, m.unrelated]),
  ];
  const artifacts = (id) => ["trace.json", `${id}-open.png`, `${id}-open.aria.txt`,
    `${id}-final.png`, "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"];
  return [{
    id: "ux08-shortcut-fix", fixture: spec.id, deadlineMs: 45000,
    requirements: requirementsFor("shortcut-fix"),
    actions: [
      { id: "open-quick-fix", device: "keyboard", key: "Meta+." },
      { id: "choose-fix", device: "keyboard", key: "Enter", label: spec.oracle.action.title },
      { id: "focus-editor", device: "keyboard", key: "Meta+1" },
      { id: "undo-fix", device: "keyboard", key: "Meta+z" },
    ],
    checks: [...initial, ...menu, ...fixed],
    artifacts: artifacts("ux08-shortcut-fix"),
  }, {
    id: "ux08-lightbulb-fix", fixture: spec.id, deadlineMs: 45000,
    requirements: requirementsFor("lightbulb-fix"),
    actions: [
      { id: "open-lightbulb", device: "pointer", selector: ".lightBulbWidget", clickCount: 1 },
      { id: "choose-fix", device: "keyboard", key: "Enter", label: spec.oracle.action.title },
      { id: "focus-editor", device: "keyboard", key: "Meta+1" },
      { id: "undo-fix", device: "keyboard", key: "Meta+z" },
    ],
    checks: [...initial, check("visible-lightbulb", "Render", { visible: true }), ...menu, ...fixed],
    artifacts: artifacts("ux08-lightbulb-fix"),
  }, {
    id: "ux08-dismiss", fixture: spec.id, deadlineMs: 45000,
    requirements: requirementsFor("dismiss"),
    actions: [
      { id: "open-quick-fix", device: "keyboard", key: "Meta+." },
      { id: "dismiss-fixes", device: "keyboard", key: "Escape" },
    ],
    checks: [...initial, ...menu,
      check("dismissed-menu", "Render", { visible: false }),
      check("unchanged-source", "Input", state(m.disk.text, false)),
      check("unchanged-diagnostics", "Render", [m.target, m.unrelated])],
    artifacts: artifacts("ux08-dismiss"),
  }, {
    id: "ux08-no-fix", fixture: spec.id, deadlineMs: 45000,
    requirements: requirementsFor("no-fix"),
    actions: [{ id: "open-quick-fix", device: "keyboard", key: "Meta+." }],
    checks: [...initial, check("no-fix-cursor", "Input", { position: m.noFixCursor }),
      check("no-fix-menu", "Render", { visible: false, unsafeActions: [] }),
      check("unchanged-source", "Input", state(m.disk.text, false)),
      check("unchanged-diagnostics", "Render", [m.target, m.unrelated])],
    artifacts: artifacts("ux08-no-fix"),
  }];
}

module.exports = { quickFixModel, quickFixContracts };
