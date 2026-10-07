"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-brackets.json");
const oracle = require("./selection-oracle"), cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("selection bracket recovery covers final operands in every statement context without certifying all S9", () => {
  assert.equal(cases.length, 90); assert.equal(new Set(cases.map(c => c.id)).size, 90);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 265); assert.equal(queries.filter(q => q.chain.length).length, 171);
  assert.equal(queries.filter(q => !q.chain.length).length, 94);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 85);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.equal(cases.filter(c => c.parse === "damaged").length, 43);
  assert.equal(cases.filter(c => c.parse === "quiet").length, 47);
  for (const c of cases) assert(["quiet", "damaged"].includes(c.parse));
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const context of ["local", "return", "statement"]) for (const kind of ["array", "index"])
    for (const operand of ["field", "binary", "call", "array", "index", "map"]) for (const state of ["incomplete", "repaired"])
      assert(cases.some(c => c.id === [state, context, kind, "final", operand].join("-")));
  for (const kind of ["array", "index", "call", "map", "record"]) for (const state of ["incomplete", "repaired"])
    assert(cases.some(c => c.id === [state, kind, "final-field"].join("-")));
  for (const kind of ["array", "index"]) {
    const first = cases.find(c => c.id === "incomplete-" + kind + "-final-field");
    const again = cases.find(c => c.id === "redamaged-" + kind + "-final-field");
    assert.deepEqual({ ...again, id: first.id }, first);
  }
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
  }
});

test("selection bracket goldens pin intact final field parents UTF16 bytes newline ownership and helper", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const spans = ranges => ranges.map(([start, end]) => [line, start, line, end]).concat([[0, 0, line + 1, 0]]);
    const utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    assert.equal(utf16.length, 4); assert.deepEqual(utf16[0], utf16[2]);
    assert.deepEqual(flatten(utf16[0]), spans([[61, 66], [57, 66], [49, 66], [48, 67], [35, 67], [22, 68], [20, 70], [8, 70]]));
    assert.deepEqual(flatten(bytes[0]), spans([[65, 70], [61, 70], [53, 70], [52, 71], [39, 71], [26, 72], [24, 74], [12, 74]]));
    assert.deepEqual(flatten(utf16[1]), spans([[49, 54], [49, 66], [48, 67], [35, 67], [22, 68], [20, 70], [8, 70]]));
    assert.deepEqual(flatten(utf16[3]), [[line + 1, 0, line + 1, 0]]);
    const damaged = (id, protocol) => {
      const item = cases.find(c => c.id === id), source = oracle.document(item.source, crlf, shifted);
      return flatten(oracle.expected(source, item.queries, protocol)[0]);
    };
    const tail = columns => columns.map(column => [line, column, line + 1, 0]).concat([[0, 0, line + 1, 0]]);
    assert.deepEqual(damaged("incomplete-array-final-field", true),
      [[line, 41, line, 46], [line, 37, line, 46], [line, 35, line, 46], ...tail([22, 20, 8])]);
    assert.deepEqual(damaged("incomplete-array-final-field", false),
      [[line, 45, line, 50], [line, 41, line, 50], [line, 39, line, 50], ...tail([26, 24, 12])]);
    assert.deepEqual(damaged("incomplete-index-final-field", true),
      [[line, 45, line, 50], [line, 41, line, 50], [line, 35, line, 50], ...tail([22, 20, 8])]);
    assert.deepEqual(damaged("incomplete-index-final-field", false),
      [[line, 49, line, 54], [line, 45, line, 54], [line, 39, line, 54], ...tail([26, 24, 12])]);
    assert.deepEqual(damaged("repaired-array-final-field", true), spans([[41, 46], [37, 46], [35, 47], [22, 48], [20, 50], [8, 50]]));
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries)[0]),
      spans([[43, 48], [37, 48], [37, 52], [30, 53], [28, 55], [8, 55]]));
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries, false)[0]),
      spans([[47, 52], [41, 52], [41, 56], [34, 57], [32, 59], [12, 59]]));
    assert.equal(doc.text.includes("\r\n"), crlf);
  }
});

test("selection bracket parents contain complete children and preserve true points and empty cardinality", () => {
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
