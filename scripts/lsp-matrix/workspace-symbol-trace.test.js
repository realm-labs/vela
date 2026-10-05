"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { workspaceSymbolResponses } = require("./workspace-symbol-trace");

test("workspace symbol wire evidence requires a complete fresh paired request including Unicode whitespace and real empty results", () => {
  const params = { query: " \t\n中😀" };
  const sent = id => `[Trace - 17:01:15] Sending request 'workspace/symbol - (${id})'.\nParams: ${JSON.stringify(params)}\n\n\n`;
  const response = (id, payload) => `[Trace - 17:01:15] Received response 'workspace/symbol - (${id})' in 2ms.\n${payload}\n\n\n`;
  const empty = response(4, "Result: []");
  assert.deepEqual(workspaceSymbolResponses(sent(4) + empty), [{ id: "4", params, result: [] }]);
  assert.deepEqual(workspaceSymbolResponses((sent(4) + empty).replaceAll("\n", "\r\n")), [{ id: "4", params, result: [] }]);
  assert.deepEqual(workspaceSymbolResponses(empty), []);
  assert.deepEqual(workspaceSymbolResponses(sent(3) + empty), []);
  assert.deepEqual(workspaceSymbolResponses(sent(4) + empty.slice(0, -3)), []);
  assert.equal(workspaceSymbolResponses(sent(4) + response(4, "No result returned."))[0].result, null);
  assert.throws(() => workspaceSymbolResponses(sent(4) + response(4, "Error: missing provider")), /unrecognized/);
  assert.throws(() => workspaceSymbolResponses(sent(4) + response(4, "Result: {broken}")), SyntaxError);
  assert.throws(() => workspaceSymbolResponses(sent(4) + sent(4)), /duplicate.*request/);
  assert.throws(() => workspaceSymbolResponses(sent(4) + empty + empty), /duplicate.*response/);
});
