"use strict";
const assert = require("node:assert/strict");
const test = require("node:test");
const { symbolResponses } = require("./document-symbol-trace");

test("empty outline evidence requires complete matched client pairs and preserves null/error distinctions", () => {
  const params = { textDocument: { uri: "file:///Chinese%20%25/imports.vela" } };
  const sent = id => `[Trace - 17:01:15] Sending request 'textDocument/documentSymbol - (${id})'.\nParams: ${JSON.stringify(params)}\n\n\n`;
  const response = (id, payload) => `[Trace - 17:01:15] Received response 'textDocument/documentSymbol - (${id})' in 2ms.\n${payload}\n\n\n`;
  const empty = response(4, "Result: []");
  assert.deepEqual(symbolResponses(sent(4) + empty), [{ id: "4", params, result: [] }]);
  assert.deepEqual(symbolResponses((sent(4) + empty).replaceAll("\n", "\r\n")), [{ id: "4", params, result: [] }]);
  assert.deepEqual(symbolResponses(empty), []);
  assert.deepEqual(symbolResponses(sent(3) + empty), []);
  assert.deepEqual(symbolResponses(sent(4) + empty.slice(0, -3)), []);
  assert.equal(symbolResponses(sent(4) + response(4, "No result returned."))[0].result, null);
  assert.throws(() => symbolResponses(sent(4) + response(4, "Error: missing provider")), /unrecognized/);
  assert.throws(() => symbolResponses(sent(4) + response(4, "Result: {broken}")), SyntaxError);
  assert.throws(() => symbolResponses(sent(4) + sent(4)), /duplicate.*request/);
  assert.throws(() => symbolResponses(sent(4) + empty + empty), /duplicate.*response/);
});
