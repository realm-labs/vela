"use strict";
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/input-workspace-trust.json");
const point = p => ({ line: p.line, character: p.character });
const span = m => ({ start: point(m.start), end: point(m.end) });
function workspaceTrustModel(platform = "darwin", crlf = false) {
  const parse = text => parseMarkers(crlf ? text.replaceAll("\n", "\r\n") : text), o = spec.oracle;
  return { spec, o, disk: file => parse(spec.files[file]), dirty: parse(o.dirtyPrefix + spec.files[o.caller]),
    configured: "bin/vela_lsp_server" + (platform === "win32" ? ".exe" : ""), point, span };
}
function workspaceTrustContracts(requirements, platform = "darwin") {
  if (!requirements.some(r => r.id.startsWith("vscode/UX21/"))) return [];
  const m = workspaceTrustModel(platform), o = m.o, p = point(m.dirty.markers.call.start), target = m.disk(o.target);
  const key = (id, key) => ({ id, device: "keyboard", key });
  const text = (id, text) => ({ id, device: "keyboard", text });
  const pointer = (id, selector) => ({ id, device: "pointer", selector, button: "left" });
  const picker = (id, file) => [key(id + "-open", "Meta+p"), { id: id + "-path", device: "keyboard", path: file }, pointer(id + "-accept", "Quick Open file " + file)];
  const palette = (id, label) => [key(id + "-open", "Meta+Shift+P"), text(id + "-name", label), pointer(id + "-accept", "Command palette " + label)];
  const go = id => [key(id + "-goto", "Control+g"), text(id + "-position", "4:15"), key(id + "-place", "Enter")];
  const source = { file: o.caller, text: m.dirty.text, disk: m.disk(o.caller).text, dirty: true, selections: [{ anchor: p, active: p }] };
  const policy = { supported: false, description: o.policyDescription };
  const location = { file: o.target, range: span(target.markers.decl), text: "trusted_value" };
  const routes = [
    { route: "untrusted-open", actions: [pointer("decline", "Do not trust startup folder"), ...picker("caller", o.caller), key("dirty-home", "Meta+Home"), text("dirty-prefix", o.dirtyPrefix),
      ...go("untrusted"), key("untrusted-native", "F12"), ...palette("installed", "Extensions: Show Installed Extensions"),
      pointer("installed-search", "Extensions search"), key("installed-select", "Meta+a"), text("installed-query", "@installed Vela"),
      pointer("installed-vela", "Installed Vela extension"), ...palette("manage", "Workspaces: Manage Workspace Trust")],
      checks: [
        { id: "policy", level: "Input", expected: policy },
        { id: "untrusted-state", level: "Input", expected: { trusted: false, active: false, serverStarted: false, serverTraceExists: false } },
        { id: "untrusted-source", level: "Input", expected: source },
        { id: "untrusted-caret", level: "Input", expected: source },
        { id: "untrusted-provider", level: "Input", expected: [] },
        { id: "untrusted-banner", level: "Render", expected: { visible: true, text: o.banner } },
        { id: "untrusted-extension", level: "Render", expected: { visible: true, description: o.policyDescription } },
        { id: "untrusted-trust-editor", level: "Render", expected: { heading: o.untrustedHeading, trustButton: "Trust", visible: true } },
      ] },
    { route: "grant-trust", actions: [pointer("grant", "Workspace Trust grant button"), ...picker("permitted-caller", o.caller), ...go("permitted"), key("permitted-native", "F12"),
      ...palette("trusted-manage", "Workspaces: Manage Workspace Trust")], checks: [
        { id: "trusted-state", level: "Input", expected: { trusted: true, active: true, serverStarted: true, serverTraceExists: true } },
        { id: "trusted-owned-server", level: "Input", expected: { sessions: 1, configuredBinaryMatchesInstalled: true, launchedConfiguredCommand: true } },
        { id: "trusted-source", level: "Input", expected: source },
        { id: "trusted-provider", level: "Input", expected: [location] },
        { id: "trusted-wire", level: "Input", expected: { method: "textDocument/definition", request: { file: o.caller, position: p }, result: location } },
        { id: "trusted-destination", level: "Input", expected: { file: o.target, text: target.text, disk: target.text, dirty: false,
          selections: [{ anchor: location.range.start, active: location.range.start }] } },
        { id: "trusted-dirty-caller", level: "Input", expected: { file: o.caller, text: m.dirty.text, disk: m.disk(o.caller).text, dirty: true } },
        { id: "trusted-diagnostics", level: "Input", expected: [{ uri: o.caller, diagnostics: [] }, { uri: o.target, diagnostics: [] }] },
        { id: "trusted-banner", level: "Render", expected: { visible: false } },
        { id: "trusted-trust-editor", level: "Render", expected: { heading: o.trustedHeading, visible: true } },
      ] },
  ];
  return routes.map(({ route, actions, checks }) => {
    const id = "ux21-" + route;
    return { id, fixture: spec.id, deadlineMs: 90000, requirements: ["input", "render"].map(level => {
      const r = requirements.find(r => r.id === `vscode/UX21/${route}/${level}/local`); if (!r) throw Error("missing trust obligation " + route + "/" + level);
      return { id: r.id, contractHash: r.contractHash };
    }), actions, checks, artifacts: ["trace.json", "trust/workbench.log", id + ".png", id + ".aria.txt", "trust/sessions.json", "trust/setup.json",
      "trust/中文 % trust workspace/" + m.configured,
      ...(route === "untrusted-open" ? [id + "-extension.png", id + "-extension.aria.txt"] : []),
      ...(route === "grant-trust" ? ["trust/lsp-trace.log", "trust/vela-output.log", "trust/server-trace.jsonl"] : [])] };
  });
}
module.exports = { workspaceTrustModel, workspaceTrustContracts };
