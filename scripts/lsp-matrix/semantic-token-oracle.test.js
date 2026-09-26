"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const { tokenModel, decodeTokens, applyTokenDelta, within } = require("./semantic-token-oracle");
const { tokenResponses, tokenStreams } = require("./semantic-token-trace");
const legend = { tokenTypes: ["comment", "variable"], tokenModifiers: ["declaration"] };

test("editor token oracle pins Unicode LF CRLF shifts and typed versus Any members independently", () => {
  const lf = tokenModel(), crlf = tokenModel(true);
  assert.deepEqual(lf.states.map(s => s.tokens), crlf.states.map(s => s.tokens));
  assert(crlf.states.every(s => s.document.text.includes("\r\n")));
  assert.equal(lf.states[0].tokens.length, 43);
  assert.deepEqual(lf.states[0].tokens.filter(t => t.text === "value").map(t => t.type), ["field", "property", "variable"]);
  assert.deepEqual(lf.states[2].tokens.filter(t => t.text === "value").map(t => t.type), ["field", "variable", "variable"]);
  assert.equal(lf.states[1].tokens[2].line, 2);
  assert.equal(lf.states[0].tokens.find(t => t.text === "cell" && t.line === 2).start, 22);
});

test("token decoding rejects incomplete, out-of-bounds, overlapping and surrogate-splitting streams", () => {
  const source = "/* 中😀 */ score\r\nscore";
  const data = [0, 0, 9, 0, 0, 0, 10, 5, 1, 1, 1, 0, 5, 1, 0];
  assert.deepEqual(decodeTokens(data, legend, source).map(t => [t.text, t.modifiers]),
    [["/* 中😀 */", []], ["score", ["declaration"]], ["score", []]]);
  for (const bad of [data.slice(1), [0, 0, 16, 0, 0], [0, 5, 1, 0, 0],
    [0, 4, 1, 0, 0], [0, 0, 9, 0, 0, 0, 8, 1, 1, 0], [0, 0, 1, 2, 0],
    [0, 0, 1, 0, 2], [0, 0, 1.5, 0, 0]]) {
    assert.throws(() => decodeTokens(bad, legend, source));
  }
});

test("actual delta edits use original word offsets and cannot be substituted by full output", () => {
  const base = [0, 0, 9, 0, 0, 1, 0, 5, 1, 1];
  assert.deepEqual(applyTokenDelta(base, { edits: [
    { start: 5, deleteCount: 5 }, { start: 0, deleteCount: 1, data: [2] },
  ] }), [2, 0, 9, 0, 0]);
  assert.deepEqual(applyTokenDelta(base, { edits: [] }), base);
  assert.throws(() => applyTokenDelta(base, { data: base }), /actual delta/);
  for (const edits of [[{ start: 11, deleteCount: 0 }], [{ start: 0, deleteCount: -1 }],
    [{ start: 1, deleteCount: 4 }, { start: 3, deleteCount: 1 }]]) {
    assert.throws(() => applyTokenDelta(base, { edits }), /valid disjoint/);
  }
  const token = { line: 0, start: 3, length: 5 };
  assert.deepEqual(within([token], { start: { line: 0, character: 4 }, end: { line: 0, character: 7 } }), [token]);
  assert.deepEqual(within([token], { start: { line: 0, character: 8 }, end: { line: 0, character: 9 } }), []);
  assert.deepEqual(within([token], { start: { line: 0, character: 4 }, end: { line: 0, character: 4 } }), []);
});

const sent = (id, method = "full") => `[Trace - 03:44:26] Sending request 'textDocument/semanticTokens/${method} - (${id})'.\nParams: {"textDocument":{"uri":"file:///tokens.vela"}}\n\n\n`;
const response = (id, method = "full", result = { data: [], resultId: "a" }) =>
  `[Trace - 03:44:26] Received response 'textDocument/semanticTokens/${method} - (${id})' in 1ms.\nResult: ${JSON.stringify(result)}\n\n\n`;
test("passive token trace requires matched complete real responses and retains actual delta words", () => {
  assert.deepEqual(tokenResponses(response(1)), []);
  assert.deepEqual(tokenResponses(sent(1) + response(2)), []);
  assert.deepEqual(tokenResponses(sent(1) + response(1, "range")), []);
  assert.deepEqual(tokenResponses(sent(1) + response(1).slice(0, -1)), []);
  const delta = { edits: [{ start: 0, deleteCount: 5, data: [1, 0, 4, 1, 0] }], resultId: "b" };
  assert.deepEqual(tokenResponses((sent(3, "full/delta") + response(3, "full/delta", delta)).replaceAll("\n", "\r\n"))[0].result, delta);
  assert.throws(() => tokenResponses(sent(1) + response(1) + response(1)), /duplicate/);
  assert.throws(() => tokenResponses(sent(1) + response(1).replace('"data":[]', '"data":broken')), SyntaxError);
});

test("renderer token cache reconstruction requires the exact predecessor and excludes range results", () => {
  const full = { method: "textDocument/semanticTokens/full", params: {}, result: { resultId: "a", data: [0, 0, 9, 0, 0] } };
  const delta = { method: "textDocument/semanticTokens/full/delta", params: { previousResultId: "a" },
    result: { resultId: "b", edits: [{ start: 2, deleteCount: 1, data: [5] }] } };
  const noop = { ...delta, params: { previousResultId: "b" }, result: { resultId: "b", edits: [] } };
  assert.deepEqual(tokenStreams([full, delta, noop]).at(-1).applied, [0, 0, 5, 0, 0]);
  assert.throws(() => tokenStreams([delta]), /observed.*predecessor/);
  assert.throws(() => tokenStreams([{ ...full, method: "textDocument/semanticTokens/range" }, delta]), /observed.*predecessor/);
  assert.throws(() => tokenStreams([{ ...full, result: { data: [] } }]), /result ID/);
});
