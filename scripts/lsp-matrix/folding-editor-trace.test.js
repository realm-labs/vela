"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const { foldingResponses } = require("./folding-editor-trace");

test("folding fresh client pairs reject partial stale duplicate null and error evidence", () => {
  const params = { textDocument: { uri: "file:///Chinese%20%25/fold.vela" } };
  const sent = id => `[Trace - 17:01:15] Sending request 'textDocument/foldingRange - (${id})'.\nParams: ${JSON.stringify(params)}\n\n\n`;
  const response = (id, payload) => `[Trace - 17:01:15] Received response 'textDocument/foldingRange - (${id})' in 2ms.\n${payload}\n\n\n`;
  const empty = response(4, "Result: []");
  assert.deepEqual(foldingResponses(sent(4) + empty), [{ id: "4", params, result: [] }]);
  assert.deepEqual(foldingResponses((sent(4) + empty).replaceAll("\n", "\r\n")), [{ id: "4", params, result: [] }]);
  assert.deepEqual(foldingResponses(empty), []);
  assert.deepEqual(foldingResponses(sent(3) + empty), []);
  assert.deepEqual(foldingResponses(sent(4) + empty.slice(0, -3)), []);
  assert.deepEqual(foldingResponses((sent(4) + empty).replaceAll("textDocument/foldingRange", "textDocument/selectionRange")), []);
  assert.equal(foldingResponses(sent(4) + response(4, "No result returned."))[0].result, null);
  const rows = [{ startLine: 0, startCharacter: 8, endLine: 1, endCharacter: 37, kind: "imports" }];
  assert.deepEqual(foldingResponses(sent(4) + response(4, "Result: " + JSON.stringify(rows)))[0].result, rows);
  assert.throws(() => foldingResponses(sent(4) + response(4, "Error: failed provider")), /unrecognized/);
  assert.throws(() => foldingResponses(sent(4) + response(4, "Result: {broken}")), SyntaxError);
  assert.throws(() => foldingResponses(sent(4) + sent(4)), /duplicate.*request/);
  assert.throws(() => foldingResponses(sent(4) + empty + empty), /duplicate.*response/);
});
