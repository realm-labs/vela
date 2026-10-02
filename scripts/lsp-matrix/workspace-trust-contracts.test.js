"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { workspaceTrustModel, workspaceTrustContracts } = require("./workspace-trust-contracts");
const { localContracts } = require("./local-contracts");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
const expected = (c, id) => c.checks.find(k => k.id === id).expected;
test("UX21 owns exactly two input and two rendered trust obligations", () => {
  const [untrusted, granted] = workspaceTrustContracts(requirements);
  assert.deepEqual([untrusted, granted].flatMap(c => c.requirements.map(r => r.id)), ["vscode/UX21/untrusted-open/input/local", "vscode/UX21/untrusted-open/render/local", "vscode/UX21/grant-trust/input/local", "vscode/UX21/grant-trust/render/local"]);
  assert(untrusted.checks.some(c => c.level === "Render" && c.expected.heading === "You are in Restricted Mode"));
  assert(granted.checks.some(c => c.level === "Render" && c.expected.heading === "You trust this folder"));
  assert.deepEqual(expected(untrusted, "untrusted-state"), { trusted: false, active: false, serverStarted: false, serverTraceExists: false });
  assert.deepEqual(expected(granted, "trusted-state"), { trusted: true, active: true, serverStarted: true, serverTraceExists: true });
  assert.deepEqual(expected(untrusted, "untrusted-provider"), []);
});
test("Unicode LF CRLF trust fixtures independently pin dirty caller and target ranges", () => {
  for (const crlf of [false, true]) {
    const m = workspaceTrustModel("darwin", crlf);
    assert.deepEqual(m.span(m.dirty.markers.call), { start: { line: 3, character: 14 }, end: { line: 3, character: 27 } });
    assert.deepEqual(m.span(m.disk(m.o.target).markers.decl), { start: { line: 1, character: 7 }, end: { line: 1, character: 20 } });
    assert.equal(m.dirty.text.includes("\r\n"), crlf); assert(m.dirty.text.includes("中😀"));
    assert(m.disk(m.o.target).text.includes("初😀")); assert.notEqual(m.dirty.text, m.disk(m.o.caller).text);
  }
});
test("trust transition preserves exact unsaved authority and restores full native wire/caret facts", () => {
  const [negative, positive] = workspaceTrustContracts(requirements);
  assert.deepEqual(expected(negative, "untrusted-source"), expected(negative, "untrusted-caret"));
  assert.deepEqual(expected(negative, "untrusted-source"), expected(positive, "trusted-source"));
  assert.deepEqual(expected(positive, "trusted-wire"), { method: "textDocument/definition", request: { file: "trust_caller.vela", position: { line: 3, character: 14 } },
    result: { file: "trust_target.vela", range: { start: { line: 1, character: 7 }, end: { line: 1, character: 20 } }, text: "trusted_value" } });
  assert.deepEqual(expected(positive, "trusted-provider"), [expected(positive, "trusted-wire").result]);
  assert.deepEqual(expected(positive, "trusted-destination").selections, [{ anchor: { line: 1, character: 7 }, active: { line: 1, character: 7 } }]);
  assert.deepEqual(expected(positive, "trusted-diagnostics"), [{ uri: "trust_caller.vela", diagnostics: [] }, { uri: "trust_target.vela", diagnostics: [] }]);
});
test("both registered platforms use real trust pointer input and independently authored rendered states", () => {
  for (const platform of ["win32", "darwin"]) {
    const contracts = localContracts(requirements, require("../../tests/lsp_matrix/fixtures/input-driver.json"), platform).filter(c => c.id.startsWith("ux21-"));
    assert.equal(contracts[0].actions.find(a => a.id === "dirty-home").key, (platform === "win32" ? "Control" : "Meta") + "+Home");
    assert.deepEqual(contracts[0].actions[0], { id: "decline", device: "pointer", selector: "Do not trust startup folder", button: "left" });
    assert.deepEqual(contracts[1].actions[0], { id: "grant", device: "pointer", selector: "Workspace Trust grant button", button: "left" });
    assert.deepEqual(expected(contracts[0], "untrusted-extension"), { visible: true, description: "Vela starts a native language server and requires a trusted workspace." });
    assert.equal(expected(contracts[0], "untrusted-trust-editor").trustButton, "Trust");
    assert(contracts.every(c => c.actions.every(a => a.device !== "command")));
    assert.equal(contracts[0].actions.find(a => a.id === "manage-name").text, "Workspaces: Manage Workspace Trust");
    assert.equal(contracts[1].actions.find(a => a.id === "permitted-position").text, "4:15");
    assert(contracts[0].artifacts.includes("trust/中文 % trust workspace/bin/vela_lsp_server" + (platform === "win32" ? ".exe" : "")));
    assert.equal(contracts[0].deadlineMs + contracts[1].deadlineMs, 180000);
  }
});
test("shipped native launcher trust policy differs from private read-only observer support", () => {
  const manifest = require("../../editors/vscode/package.json"), observer = require("../../editors/vscode/test/driver/package.json");
  assert.deepEqual(manifest.capabilities.untrustedWorkspaces, { supported: false, description: "Vela starts a native language server and requires a trusted workspace." });
  assert.equal(observer.capabilities.untrustedWorkspaces.supported, true);
  assert.equal(manifest.main, "./extension.js");
  assert.equal(workspaceTrustModel("win32").configured, "bin/vela_lsp_server.exe");
  assert.equal(workspaceTrustModel("darwin").configured, "bin/vela_lsp_server");
});
