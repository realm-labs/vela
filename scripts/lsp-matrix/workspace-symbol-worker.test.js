"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const worker = require("../../tests/lsp_matrix/fixtures/workspace-symbol-worker.json");
const lifecycle = require("../../tests/lsp_matrix/fixtures/workspace-symbol-lifecycle.json");

test("workspace worker contracts pin typed IDs ignored versions malformed cancels and bounded retry errors", () => {
  assert.equal(worker.version, 1); assert.equal(worker.id, "workspace-symbol-worker");
  assert.deepEqual(worker.ids, [-2147483648, 0, 2147483647, "cancel 中😀", "20", ""]);
  assert.equal(new Set(worker.ids.map(JSON.stringify)).size, 6);
  assert.notEqual(JSON.stringify(20), JSON.stringify("20"));
  assert.deepEqual(worker.ignoredVersions, [-2147483648, -1, 0, 1]);
  assert.deepEqual(worker.ignoredCancelParams, [{}, { id: null }, { id: true }, { id: [] }, { id: {} }, { id: 1.5 }, { id: 2147483648 }, { id: -2147483649 }]);
  assert.equal(worker.retryLimit, 1);
  assert.deepEqual(worker.errors, {
    cancelled: { code: -32800, message: "request was cancelled before processing" },
    stale: { code: -32801, message: "request result is stale because the document was modified" }
  });
});

test("workspace worker queries reuse complete authored lifecycle sets across every source and schema mutation", () => {
  assert.deepEqual(worker.queries, ["", "host::Box", "not_a_workspace_symbol_94"]);
  assert.equal(lifecycle.oracle.phases.length, 19);
  for (const phase of lifecycle.oracle.phases) {
    for (const query of worker.queries) {
      const expected = phase.workspace.queries.find(row => row.query === query);
      assert(expected);
      assert(expected.symbols.every(id => phase.workspace.symbols.some(row => row.id === id)));
      if (query === "") assert.deepEqual(expected.symbols, phase.workspace.symbols.map(row => row.id));
      if (query === "host::Box") assert.equal(expected.symbols.length, phase.schema.mode === "valid" ? 2 : 0);
      if (query === "not_a_workspace_symbol_94") assert.deepEqual(expected.symbols, []);
    }
    if (phase.id !== "disk") assert(phase.actions.length > 0 || phase.schemaAction !== null, phase.id);
  }
  assert.deepEqual(lifecycle.oracle.phases[1].open, { "scripts/main.vela": "main-base" });
  assert.deepEqual(lifecycle.oracle.phases[2].open, { "scripts/main.vela": "main-dirty" });
});
