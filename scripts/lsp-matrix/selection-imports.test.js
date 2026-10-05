"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-imports.json");
const oracle = require("./selection-imports-oracle");
const { cases } = spec.oracle;
const flatten = row => {
  const rows = []; for (let current = row; current; current = current.parent) {
    const r = current.range; rows.push([r.start.line, r.start.character, r.end.line, r.end.character]);
  }
  return rows;
};

test("selection imports pin complete authored aliases reexports dedup boundaries point and empty vectors", () => {
  assert.equal(cases.length, 16); assert.equal(new Set(cases.map(c => c.id)).size, 16);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 52); assert.equal(queries.filter(q => q.chain.length).length, 42);
  assert.equal(queries.filter(q => !q.chain.length).length, 10);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.equal(cases.filter(c => c.queries.some(q => q.chain.length)).length, 12);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  assert.deepEqual(cases.find(c => c.id === "single-segment-dedup").queries[0].chain, ["name", "item", "file"]);
  assert.deepEqual(cases[0].queries.find(q => q.position === "alias-end").chain, ["semi", "item", "file"]);
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const q of c.queries) {
      assert(doc.markers[q.position]); assert(new Set(q.chain).size === q.chain.length);
      for (const name of q.chain) assert(doc.markers[name]);
    }
  }
});

test("selection complete UTF16 byte goldens retain duplicate vector order and whole shifted file extents", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    const c = cases[0], doc = oracle.document(c.source, crlf, shifted), line = shifted ? 2 : 0;
    const positions = oracle.positions(doc, c.queries), utf16 = oracle.expected(doc, c.queries), bytes = oracle.expected(doc, c.queries, false);
    assert.deepEqual(positions[0], { line, character: 32 }); assert.deepEqual(positions[1], { line, character: 22 });
    assert.deepEqual(positions[0], positions[2]); assert.deepEqual(utf16[0], utf16[2]); assert.equal(utf16.length, 13);
    assert.deepEqual(flatten(utf16[0]), [[line, 29, line, 34], [line, 8, line, 35], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[0]), [[line, 33, line, 38], [line, 12, line, 39], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(utf16[1]), [[line, 20, line, 25], [line, 12, line, 25], [line, 8, line, 35], [0, 0, line + 1, 0]]);
    assert.deepEqual(flatten(bytes[1]), [[line, 24, line, 29], [line, 16, line, 29], [line, 12, line, 39], [0, 0, line + 1, 0]]);
    assert.equal(doc.markers.file.start.byte, 0); assert.equal(doc.markers.file.end.byte, Buffer.byteLength(doc.text));
    assert.equal(doc.text.includes("\r\n"), crlf);
    const helper = oracle.document(spec.files["scripts/helper.vela"], crlf, shifted);
    assert.deepEqual(flatten(oracle.expected(helper, spec.oracle.helperQueries)[0]),
      [[line, 12, line, 18], [line, 12, line, 26], [line, 8, line, 27], [0, 0, line + 1, 0]]);
  }
});

test("selection ancestry owns every input position without phantom comment or EOF parents", () => {
  const before = (a, b) => a.line < b.line || (a.line === b.line && a.character <= b.character);
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const c of cases) {
    const doc = oracle.document(c.source, crlf, shifted), positions = oracle.positions(doc, c.queries);
    for (const [index, result] of oracle.expected(doc, c.queries).entries()) {
      const q = c.queries[index];
      if (!q.chain.length) {
        assert.deepEqual(result, { range: { start: positions[index], end: positions[index] } }); continue;
      }
      let child;
      for (let row = result; row; row = row.parent) {
        assert(before(row.range.start, positions[index]) && before(positions[index], row.range.end));
        assert.notDeepEqual(row.range.start, row.range.end);
        if (child) {
          assert(before(row.range.start, child.start) && before(child.end, row.range.end));
          assert.notDeepEqual(row.range, child);
        }
        child = row.range;
      }
    }
    if (c.id === "semicolonless-import") {
      assert.equal(doc.markers.item.end.line, (shifted ? 2 : 0) + 1);
      assert.equal(doc.markers.item.end.character, 0);
    }
    if (c.id === "semicolon-at-eof") assert.deepEqual(c.queries[1].chain, ["semi", "item", "file"]);
  }
});
