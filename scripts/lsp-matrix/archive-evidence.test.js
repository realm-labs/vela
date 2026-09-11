"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { comparePackageEntries } = require("./archive-evidence");
const { hash } = require("./local-evidence");
function setup() {
  const current = {
    "package.json": Buffer.from('{"name":"vela","version":"1"}'),
    "extension.js": Buffer.from("candidate"),
    "language-configuration.json": Buffer.from("{}"),
    "syntaxes/vela.tmLanguage.json": Buffer.from("grammar"),
    "server/vela_lsp_server": Buffer.from("binary"),
  };
  const entries = Object.entries(current).map(([name, bytes]) => ({
    name,
    bytes,
  }));
  const installed = {
    ...current,
    "package.json": Buffer.from(
      '{"name":"vela","version":"1","__metadata":{"id":"installation"}}',
    ),
  };
  return {
    current,
    entries,
    installed,
    server: hash(current["server/vela_lsp_server"]),
  };
}
test("VSIX proof matches installed payload and candidate while allowing installer metadata", () => {
  const { current, entries, installed, server } = setup();
  comparePackageEntries(
    entries,
    (name) => current[name],
    (name) => installed[name],
    server,
  );
});
test("VSIX proof rejects stale or substituted extension manifest grammar and server bytes", () => {
  for (const name of [
    "package.json",
    "extension.js",
    "syntaxes/vela.tmLanguage.json",
    "server/vela_lsp_server",
  ]) {
    const { current, entries, installed, server } = setup();
    const changed =
      name === "package.json"
        ? Buffer.from('{"name":"old"}')
        : Buffer.from("old");
    entries.find((entry) => entry.name === name).bytes = changed;
    installed[name] = changed;
    assert.throws(() =>
      comparePackageEntries(
        entries,
        (key) => current[key],
        (key) => installed[key],
        server,
      ),
    );
  }
});
test("VSIX proof rejects missing duplicate escaping and changed installed payloads", () => {
  for (const mutate of [
    (s) => s.entries.shift(),
    (s) => s.entries.push(s.entries[0]),
    (s) => (s.entries[0].name = "../file"),
    (s) => (s.installed["extension.js"] = Buffer.from("changed")),
  ]) {
    const s = setup();
    mutate(s);
    assert.throws(() =>
      comparePackageEntries(
        s.entries,
        (key) => s.current[key],
        (key) => s.installed[key],
        s.server,
      ),
    );
  }
});
