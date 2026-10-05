"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const path = require("node:path");
const { mergeEditorResults, editorTests } = require("./editor-results");
const result = (name, passed = true) => ({ version: 1, vscodeVersion: "1.137.0",
  provenance: { inputsSha256: "source", serverSha256: "server", platform: "win32" }, results: [{ name, passed }] });

test("isolated editor suites merge all checks and retain failures with identical pinned provenance", () => {
  const first = result("existing"), second = result("workspace", false);
  const old = structuredClone([first, second]);
  assert.deepEqual(mergeEditorResults(first, second).results, [{ name: "existing", passed: true }, { name: "workspace", passed: false }]);
  assert.deepEqual([first, second], old);
});
test("isolated editor suites reject duplicate checks or different source binary platform and VSCode versions", () => {
  assert.throws(() => mergeEditorResults(result("same"), result("same")), /duplicate/);
  for (const key of ["inputsSha256", "serverSha256", "platform"]) {
    const second = result("workspace"); second.provenance[key] = "other";
    assert.throws(() => mergeEditorResults(result("existing"), second), /same source/);
  }
  const second = result("workspace"); second.vscodeVersion = "other";
  assert.throws(() => mergeEditorResults(result("existing"), second), /same pinned editor/);
  const missing = result("workspace"); missing.provenance = null;
  assert.throws(() => mergeEditorResults(result("existing"), missing), /provenance present/);
});
test("matrix editor discovery retains original checks plus folding and isolated global workspace checks", () => {
  const names = editorTests(path.resolve(__dirname, "../.."));
  assert.equal(names.length, 25);
  assert.equal(names.filter(name => name.startsWith("folding provider ")).length, 1);
  assert.equal(names.filter(name => name.startsWith("workspace symbol provider ")).length, 1);
  assert.equal(names.filter(name => name.startsWith("document symbol provider ")).length, 3);
  assert(names.includes("installed VSIX activates on opening a Vela file"));
});
