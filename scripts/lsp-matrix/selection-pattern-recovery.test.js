"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-pattern-recovery.json");
const oracle = require("./selection-oracle"), cases = spec.oracle.cases;
const flatten = row => {
  const result = [];
  for (; row; row = row.parent) {
    const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return result;
};

test("recovered final tuple pattern owns a source declaration in the complete semantic-token fixture", () => {
  const fixture = require("../../tests/lsp_matrix/fixtures/semantic-token-recovery.json");
  const damaged = fixture.oracle.negative.cases;
  assert.equal(fixture.oracle.positive.length, 76);
  assert.equal(damaged.length, 25);
  assert.equal(damaged.reduce((n, c) => n + c.tokens.length, 0), 1706);
  const c = damaged.find(c => c.id === "pattern-eof");
  assert.deepEqual(c.tokens.at(-1), { marker: "pattern-eof-4-32", text: "bound",
    type: "variable", modifiers: ["declaration", "source"] });
  assert.equal(c.tokens.length, 77);
  assert.deepEqual(c.diagnostics, { phase: "pattern-eof", parseErrors: true });
  for (const crlf of [false, true]) {
    const doc = oracle.document(c.source, crlf);
    const span = doc.markers["pattern-eof-4-32"];
    assert.equal(doc.text.slice(0, - (crlf ? 2 : 1)).endsWith("One ( bound"), true);
    assert.deepEqual(oracle.point(doc, span.start), { line: 4, character: 123 });
    assert.deepEqual(oracle.point(doc, span.end), { line: 4, character: 128 });
    assert.deepEqual(oracle.point(doc, span.start, false), { line: 4, character: 127 });
    assert.deepEqual(oracle.point(doc, span.end, false), { line: 4, character: 132 });
  }
});

test("selection tuple pattern recovery covers complete final patterns without certifying all S9", () => {
  assert.equal(cases.length, 83); assert.equal(new Set(cases.map(c => c.id)).size, 83);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 321); assert.equal(queries.filter(q => q.chain.length).length, 234);
  assert.equal(queries.filter(q => !q.chain.length).length, 87);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 78);
  assert.equal(cases.filter(c => c.queries.length && c.queries.every(q => !q.chain.length)).length, 4);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.equal(cases.filter(c => c.parse === "damaged").length, 40);
  assert.equal(cases.filter(c => c.parse === "quiet").length, 43);
  for (const c of cases) assert(["quiet", "damaged"].includes(c.parse));
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  for (const context of ["local", "return", "statement"]) for (const kind of ["variant", "tuple"])
    for (const operand of ["binding", "wildcard", "literal", "path", "tuple", "record"]) for (const state of ["incomplete", "repaired"])
      assert(cases.some(c => c.id === [state, context, kind, "final", operand].join("-")));
  for (const kind of ["variant", "tuple"]) {
    const first = cases.find(c => c.id === "incomplete-" + kind + "-final-binding");
    const again = cases.find(c => c.id === "redamaged-" + kind + "-final-binding");
    assert.deepEqual({ ...again, id: first.id }, first);
  }
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) { assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]); }
    if (c.id.startsWith("incomplete-local-") || c.id.startsWith("incomplete-return-") || c.id.startsWith("incomplete-variant-") || c.id.startsWith("incomplete-tuple-") || c.id.startsWith("redamaged-")) {
      assert.deepEqual(doc.markers.pattern, doc.markers.arm);
      for (const q of c.queries) assert(!q.chain.includes("arm"));
    }
    if (c.id.includes("-statement-")) for (const q of c.queries) assert(!q.chain.includes("statement"));
  }
});

test("selection pattern recovery goldens pin tuple binding owners UTF16 bytes deduplication and direct match geometry", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const spans = ranges => ranges.map(([start, end]) => [line, start, line, end]).concat([[0, 0, line + 1, 0]]);
    const utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    assert.equal(utf16.length, 4); assert.deepEqual(utf16[0], utf16[2]);
    assert.deepEqual(flatten(utf16[0]), spans([[67, 71], [47, 72], [47, 77], [45, 80], [35, 80], [22, 81], [20, 83], [8, 83]]));
    assert.deepEqual(flatten(bytes[0]), spans([[71, 75], [51, 76], [51, 81], [49, 84], [39, 84], [26, 85], [24, 87], [12, 87]]));
    assert.deepEqual(flatten(utf16[1]), spans([[61, 65], [47, 72], [47, 77], [45, 80], [35, 80], [22, 81], [20, 83], [8, 83]]));
    assert.deepEqual(flatten(utf16[3]), [[line + 1, 0, line + 1, 0]]);
    const selected = (id, protocol) => {
      const item = cases.find(c => c.id === id), source = oracle.document(item.source, crlf, shifted);
      return flatten(oracle.expected(source, item.queries, protocol)[0]);
    };
    const tail = columns => columns.map(column => [line, column, line + 1, 0]).concat([[0, 0, line + 1, 0]]);
    assert.deepEqual(selected("incomplete-variant-final-binding", true),
      [[line, 67, line, 71], [line, 47, line, 71], [line, 45, line, 71], [line, 35, line, 71], ...tail([22, 20, 8])]);
    assert.deepEqual(selected("incomplete-variant-final-binding", false),
      [[line, 71, line, 75], [line, 51, line, 75], [line, 49, line, 75], [line, 39, line, 75], ...tail([26, 24, 12])]);
    assert.deepEqual(selected("incomplete-tuple-final-binding", true),
      [[line, 54, line, 58], [line, 47, line, 58], [line, 45, line, 58], [line, 35, line, 58], ...tail([22, 20, 8])]);
    assert.deepEqual(selected("repaired-tuple-final-binding", true), spans([[54, 58], [47, 59], [47, 64], [45, 67], [35, 67], [22, 68], [20, 70], [8, 70]]));
    assert.deepEqual(selected("incomplete-statement-variant-final-binding", true),
      [[line, 54, line, 58], [line, 34, line, 58], ...tail([34, 32, 22, 20, 8])]);
    assert.deepEqual(selected("repaired-statement-variant-final-binding", true), spans([[54, 58], [34, 59], [34, 64], [32, 67], [22, 67], [20, 70], [8, 70]]));
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries)[0]),
      spans([[43, 48], [37, 48], [37, 52], [30, 53], [28, 55], [8, 55]]));
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries, false)[0]),
      spans([[47, 52], [41, 52], [41, 56], [34, 57], [32, 59], [12, 59]]));
    assert.equal(doc.text.includes("\r\n"), crlf);
  }
});

test("selection pattern recovery chains contain whole children and preserve exact points empty vectors and order", () => {
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
