"use strict";
const assert = require("node:assert/strict");
const { parseMarkers } = require("./fixtures");
const spec = require("../../tests/lsp_matrix/fixtures/semantic-token-editor.json");

function tokenModel(crlf = false) {
  const states = spec.oracle.states.map((state) => {
    const document = parseMarkers(state.source.replaceAll("\n", crlf ? "\r\n" : "\n"));
    const tokens = state.tokens.map(({ marker, text, type, modifiers }) => {
      const { start, end } = document.markers[marker];
      assert.equal(start.line, end.line, `multiline oracle token ${marker}`);
      assert.equal(Buffer.from(document.text).subarray(start.byte, end.byte).toString(), text);
      return { line: start.line, start: start.character, length: end.character - start.character,
        text, type, modifiers };
    });
    return { id: state.id, document, tokens };
  });
  return { file: spec.oracle.file, states };
}

function decodeTokens(data, legend, source) {
  assert(data && typeof data[Symbol.iterator] === "function", "token data is iterable");
  const words = Array.from(data), lines = source.split(/\r?\n/), tokens = [];
  assert.equal(words.length % 5, 0, "complete token tuples");
  assert(words.every((word) => Number.isInteger(word) && word >= 0 && word <= 0xffffffff), "u32 token data");
  let line = 0, start = 0, previousEnd = 0;
  for (let i = 0; i < words.length; i += 5) {
    const [deltaLine, deltaStart, length, typeIndex, bits] = words.slice(i, i + 5);
    line += deltaLine;
    start = deltaLine ? deltaStart : start + deltaStart;
    if (deltaLine) previousEnd = 0;
    assert(length > 0 && line < lines.length && start >= previousEnd && start + length <= lines[line].length,
      `sorted non-overlapping in-bounds token ${i / 5}`);
    for (const offset of [start, start + length]) {
      const code = lines[line].charCodeAt(offset);
      assert(!(code >= 0xdc00 && code <= 0xdfff), "UTF-16 boundary cannot split a surrogate pair");
    }
    assert(typeIndex < legend.tokenTypes.length, "known token type");
    assert(bits < 2 ** legend.tokenModifiers.length, "known modifier bits");
    const modifiers = legend.tokenModifiers.filter((_, index) => bits & 2 ** index);
    tokens.push({ line, start, length, text: lines[line].slice(start, start + length),
      type: legend.tokenTypes[typeIndex], modifiers });
    previousEnd = start + length;
  }
  return tokens;
}

function applyTokenDelta(previous, result) {
  assert(Array.isArray(result.edits), "an actual delta response is required");
  const words = Array.from(previous), edits = [...result.edits].sort((a, b) => a.start - b.start);
  let previousEnd = 0;
  for (const edit of edits) {
    assert(Number.isInteger(edit.start) && Number.isInteger(edit.deleteCount) && edit.start >= previousEnd &&
      edit.deleteCount >= 0 && edit.start + edit.deleteCount <= words.length, "valid disjoint delta edits");
    assert(!edit.data || Array.isArray(edit.data), "delta replacement words");
    previousEnd = edit.start + edit.deleteCount;
  }
  for (const edit of edits.reverse()) words.splice(edit.start, edit.deleteCount, ...(edit.data ?? []));
  return words;
}

function within(tokens, range) {
  const before = (a, b) => a.line < b.line || a.line === b.line && a.character < b.character;
  if (!before(range.start, range.end)) return [];
  return tokens.filter((token) => {
    const start = { line: token.line, character: token.start };
    const end = { line: token.line, character: token.start + token.length };
    return before(start, range.end) && before(range.start, end);
  });
}

module.exports = { tokenModel, decodeTokens, applyTokenDelta, within };
