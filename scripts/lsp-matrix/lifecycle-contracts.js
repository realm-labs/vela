"use strict";
// Independent fixture and literal lifecycle policy; never build expected facts
// from the client, process list, restored editor or failure text.
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-lifecycle.json");
function lifecycleModel() {
  const disk = parseMarkers(spec.files[spec.oracle.file]).text;
  return { spec, disk, dirty: spec.oracle.dirtyPrefix + disk };
}
function lifecycleContracts(requirements) {
  if (!requirements.some(r => r.scenario === "UX18")) return [];
  const m = lifecycleModel(), o = spec.oracle;
  const key = (id, binding) => ({ id, device: "keyboard", key: binding });
  const text = (id, value) => ({ id, device: "keyboard", text: value });
  const command = (id, value, extra = {}) => ({ id, device: "command", command: value, ...extra });
  const palette = (prefix, title) => [key(`${prefix}-open`, "Meta+Shift+P"), text(`${prefix}-name`, title), key(`${prefix}-accept`, "Enter")];
  const call = prefix => [key(`${prefix}-goto`, "Control+g"), text(`${prefix}-position`, "5:18"), key(`${prefix}-place`, "Enter")];
  const check = (id, level, expected) => ({ id, level, expected });
  const source = position => ({ file: o.file, text: m.dirty, dirty: true, languageId: "vela", disk: m.disk,
    selections: [{ anchor: position, active: position }] });
  const wire = { method: "textDocument/definition", request: { file: o.file, position: o.call },
    result: { file: o.file, range: o.definition, text: "resume" } };
  const view = message => ({ visible: true, channel: "Vela", message });
  const prepared = [key("dirty-home", "Meta+Home"), text("dirty-prefix", o.dirtyPrefix), ...call("baseline"), key("baseline-query", "F12"), ...call("pending")];
  const commonChecks = [check("dirty-source", "Input", source(o.call)), check("baseline-destination", "Input", source(o.definition.start)),
    check("baseline-wire", "Input", wire), check("pending-source", "Input", source(o.call))];
  const recovery = [...palette("recover", "Developer: Reload Window"), key("recover-editor", "Meta+1"),
    command("recovered-query", "vscode.executeDefinitionProvider"), key("recovered-native-query", "F12")];
  const recoveryChecks = [check("reloaded", "Command", { changedHost: true, sameInstalledExtension: true, servers: 1 }),
    check("restored-source", "Input", source(o.call)), check("recovered-command", "Command", [wire.result]),
    check("recovered-destination", "Input", source(o.definition.start)), check("recovered-wire", "Input", wire)];
  const contract = (route, actions, checks) => ({ id: `ux18-${route}`, fixture: spec.id, deadlineMs: 60000,
    requirements: ["command", "input", "render"].map(level => {
      const id = `vscode/UX18/${route}/${level}/local`, r = requirements.find(r => r.id === id);
      if (!r) throw Error(`missing lifecycle obligation ${id}`);
      return { id, contractHash: r.contractHash };
    }), actions, checks,
    artifacts: ["trace.json", `${route}-failure.png`, `${route}-failure.aria.txt`, `${route}-recovered.png`,
      `${route}-sessions.json`, "workbench.log", "lsp-trace.log", "extension-host.log", "server-trace.jsonl", "vela-output.log"] });
  return [
    contract("server-stop-reload", [...prepared, command("suspend-owned", "test.suspendOwnedServer"),
      command("pending-query", "vscode.executeDefinitionProvider"), command("stop-owned", "test.stopOwnedServer"),
      ...palette("failure", "Vela: Show Output"), ...recovery],
      [...commonChecks, check("suspended", "Command", { servers: 1, owned: true, suspended: true }),
        check("pending-request", "Command", { method: wire.method, request: wire.request, received: false }),
        check("bounded-failure", "Command", { result: [], within5000Ms: true }), check("visible-failure", "Render", view(o.closedMessage)),
        check("stopped-policy", "Command", { servers: [0, 0, 0], starts: [1, 1, 1], atLeast1000Ms: true }), ...recoveryChecks]),
    contract("invalid-server-path", [...prepared, command("configure-invalid", "test.configureServerPath", { value: "missing" }),
      ...palette("invalid", "Developer: Reload Window"), key("open-picker", "Meta+p"), text("file-name", o.file), key("open-file", "Enter"),
      ...palette("failure", "Vela: Show Output"), command("failed-query", "vscode.executeDefinitionProvider"),
      command("restore-config", "test.configureServerPath", { value: "default" }), ...recovery],
      [...commonChecks, check("invalid-config", "Command", { missing: true, insideWorkspace: true }),
        check("invalid-reloaded", "Command", { changedHost: true, sameInstalledExtension: true, servers: 0 }),
        check("visible-failure", "Render", { ...view(o.failedMessage), missingExecutable: true }),
        check("bounded-failure", "Command", { result: [], within5000Ms: true }),
        check("stopped-policy", "Command", { servers: [0, 0, 0], starts: [1, 1, 1], atLeast1000Ms: true }),
        check("default-config", "Command", { path: "", override: false }), ...recoveryChecks])
  ];
}
module.exports = { lifecycleModel, lifecycleContracts };
