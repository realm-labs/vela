"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-bodies.json");
const oracle = require("./selection-oracle");
const cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection body corpus owns statements closures callbacks all assignments and genuine point/empty vectors", () => {
  assert.equal(cases.length, 59); assert.equal(new Set(cases.map(c => c.id)).size, 59);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 180); assert.equal(queries.filter(q => q.chain.length).length, 168);
  assert.equal(queries.filter(q => !q.chain.length).length, 12);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 54);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const operator of ["set", "add", "subtract", "multiply", "divide", "remainder"])
    for (const prefix of ["assignment-", "lambda-body-assignment-", "callback-body-assignment-"])
      assert(cases.some(c => c.id === prefix + operator), prefix + operator);
  for (const id of ["typed-local-assignment-return", "uninitialized-local", "qualified-local-type", "builtin-local-type",
    "tuple-local-type", "attributed-local", "member-assignment", "index-assignment", "bare-return", "literal-return",
    "newline-statements", "nested-block-shadow", "deep-block-tail", "block-initializer", "empty-blocks", "if-statement",
    "if-initializer", "else-if-nesting", "match-guard-and-block", "match-initializer", "for-control", "for-key-value",
    "nested-for-if", "typed-lambda-capture", "expression-lambda", "zero-parameter-lambda", "nested-lambda", "callback-call",
    "named-callback-call", "trait-default-body", "inherent-method-body", "trait-impl-body", "same-named-function-bodies"])
    assert(cases.some(c => c.id === id), id);
  assert.deepEqual(cases.find(c => c.id === "deep-block-tail").queries[0].chain, ["value", "tail", "second", "first", "body", "item", "file"]);
  assert.deepEqual(cases.find(c => c.id === "callback-body-assignment-set").queries[1].chain,
    ["target", "assignment", "lambda", "args", "call", "statement", "body", "item", "file"]);
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
  }
});

test("selection body complete goldens pin UTF16 bytes duplicate vector order and separate helper ancestry", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 11);
    assert.deepEqual(flatten(utf16[0]), [[line, 65, line, 70], [line, 58, line, 71], [line, 17, line, 73], [line, 8, line, 73], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[0]), [[line, 69, line, 74], [line, 62, line, 75], [line, 21, line, 77], [line, 12, line, 77], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[1]), [[line, 23, line, 28], [line, 19, line, 41], [line, 17, line, 73], [line, 8, line, 73], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[1]), [[line, 27, line, 32], [line, 23, line, 45], [line, 21, line, 77], [line, 12, line, 77], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[6]), [[line, 51, line, 56], [line, 42, line, 56], [line, 42, line, 57], [line, 17, line, 73], [line, 8, line, 73], [0, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries)[0]), [[line, 29, line, 33], [line, 22, line, 34], [line, 20, line, 36], [line, 8, line, 36], [0, 0, line + 1, 0]]);
  }
});

test("selection body parents own full nested chains without phantom trivia empty or duplicate spans", () => {
  const before = (a, b) => a.line < b.line || (a.line === b.line && a.character <= b.character);
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const c of cases) {
    const doc = oracle.document(c.source, crlf, shifted), positions = oracle.positions(doc, c.queries);
    const actual = oracle.expected(doc, c.queries); assert.equal(actual.length, c.queries.length);
    for (const [index, result] of actual.entries()) {
      if (!c.queries[index].chain.length) {
        assert.deepEqual(result, { range: { start: positions[index], end: positions[index] } }); continue;
      }
      let child;
      for (let row = result; row; row = row.parent) {
        assert(before(row.range.start, positions[index]) && before(positions[index], row.range.end));
        assert.notDeepEqual(row.range.start, row.range.end);
        if (child) { assert(before(row.range.start, child.start) && before(child.end, row.range.end)); assert.notDeepEqual(row.range, child); }
        child = row.range;
      }
    }
  }
});
