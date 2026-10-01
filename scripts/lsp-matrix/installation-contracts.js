"use strict";
// Authored startup oracle: no provider result is used to build expectations.
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-installation.json");
function installationModel() {
  const disk = Object.fromEntries(Object.entries(spec.files).map(([file, source]) => [file, parseMarkers(source).text]));
  const state = (file, languageId, position) => ({ file, text: disk[file], languageId, dirty: false,
    selections: [{ anchor: position, active: position }] });
  return { spec, disk, state };
}
function installationContracts(requirements) {
  if (!requirements.some(r => r.id.startsWith("vscode/UX01/"))) return [];
  const m = installationModel(), o = spec.oracle;
  const key = (id, binding) => ({ id, device: "keyboard", key: binding });
  const text = (id, value) => ({ id, device: "keyboard", text: value });
  const open = file => [key("open-picker", "Meta+p"), text("file-name", file), key("open-file", "Enter"),
    key("goto-line", "Control+g"), text("call-position", "4:18"), key("place-cursor", "Enter")];
  const commandOpen = file => ({ id: "command-open", device: "command", command: "vscode.open", file, position: o.call });
  const check = (id, level, expected) => ({ id, level, expected });
  const contract = (route, actions, checks) => ({ id: `ux01-${route}`, fixture: spec.id, deadlineMs: 45000,
    requirements: ["input", "command"].map(level => {
      const id = `vscode/UX01/${route}/${level}/local`, r = requirements.find(r => r.id === id);
      if (!r) throw Error(`missing installation obligation ${id}`);
      return { id, contractHash: r.contractHash };
    }), actions, checks: [...checks, check("disk-inputs", "Input", m.disk)],
    artifacts: ["trace.json", `ux01-${route}.png`, "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl"] });
  const inactive = { active: false, bundled: true, started: false };
  const destination = m.state(o.velaFile, "vela", o.definition.start);
  const wire = { method: "textDocument/definition", request: { file: o.velaFile, position: o.call },
    result: { file: o.velaFile, range: o.definition, text: "launch" } };
  return [
    contract("unrelated-file", [...open(o.unrelatedFile), key("native-query", "F12"),
      commandOpen(o.unrelatedFile), { id: "command-query", device: "command", command: "vscode.executeDefinitionProvider", file: o.unrelatedFile, position: o.call }],
      [check("native-source", "Input", m.state(o.unrelatedFile, o.unrelatedLanguage, o.call)),
        check("native-unchanged", "Input", m.state(o.unrelatedFile, o.unrelatedLanguage, o.call)),
        check("native-inactive", "Input", inactive), check("command-source", "Command", m.state(o.unrelatedFile, o.unrelatedLanguage, o.call)),
        check("command-empty", "Command", []), check("command-inactive", "Command", inactive)]),
    contract("install-open", [...open(o.velaFile), key("native-query", "F12"), commandOpen(o.velaFile),
      { id: "command-query", device: "command", command: "editor.action.revealDefinition" }],
      [check("native-source", "Input", m.state(o.velaFile, "vela", o.call)),
        check("activated", "Input", { active: true, bundled: true, started: true }),
        check("native-destination", "Input", destination), check("native-wire", "Input", wire),
        check("command-source", "Command", m.state(o.velaFile, "vela", o.call)),
        check("command-destination", "Command", destination), check("command-wire", "Command", wire)])
  ];
}
module.exports = { installationModel, installationContracts };
