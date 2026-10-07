"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-trivia.json");
const oracle = require("./selection-trivia-oracle"), cases = spec.oracle.cases;
const get = id => { const c = cases.find(c => c.id === id); assert(c, id); return c; };
const flatten = row => { const a = []; for (; row; row = row.parent) {
  const r = row.range; a.push([r.start.line, r.start.character, r.end.line, r.end.character]);
} return a; };

test("selection trivia pins the complete S12 partition and whole lexical/CST ownership inventory", () => {
  assert.equal(cases.length, 63); assert.equal(new Set(cases.map(c => c.id)).size, 63);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 458); assert.equal(queries.filter(q => q.chain.length).length, 155);
  assert.equal(queries.filter(q => !q.chain.length).length, 303);
  assert.equal(cases.filter(c => c.parse === "quiet").length, 56);
  assert.equal(cases.filter(c => c.parse === "damaged").length, 5);
  assert.equal(cases.filter(c => c.parse === "lexical/E_LEX_BLOCK_COMMENT").length, 2);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  for (const id of ["shebang-field", "shebang-only-eof", "shebang-only-newline", "leading-blank-indented-item",
    "significant-final-brace-eof", "blank-line-group", "tab-indentation", "mixed-indentation", "nested-block-field",
    "leading-line-doc", "leading-block-doc", "leading-multiline-block", "import-path-comments", "nested-type-trivia",
    "nested-struct", "nested-enum-record", "nested-enum-tuple", "nested-enum-tuple-raw", "nested-trait-required",
    "nested-trait-default", "nested-inherent-impl", "nested-trait-impl", "call-argument-trivia", "unclosed-body-trivia",
    "ordinary-string-comment-lookalikes", "bytes-comment-lookalikes", "char-whitespace-lookalike",
    "multiline-string-comment-lookalikes", "empty", "empty-vector", "boundary-space", "boundary-block", "boundary-line"]) get(id);
  for (const c of cases) {
    const doc = oracle.document(c.source);
    assert(c.nodes.some(n => n.kind === "SourceFile"));
    for (const q of c.queries) {
      assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]);
      if (!q.token) { assert.deepEqual(q.chain, []); continue; }
      assert(c.tokens.some(t => t.marker === q.token));
      assert.deepEqual(q.ancestors.at(-1), { marker: "file", kind: "SourceFile" });
      for (const a of q.ancestors) assert(c.nodes.some(n => n.marker === a.marker && n.kind === a.kind));
    }
  }
});

test("selection trivia independently pins full literal geometry, equal spans and significant EOF", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const protocol of [false, true]) {
    const offset = shifted ? 2 : 0, byteExtra = protocol ? 0 : 4;
    const first = (id, wanted) => {
      const c = get(id), doc = oracle.document(c.source, crlf, shifted);
      const q = c.queries.find(q => q.chain.length);
      assert.deepEqual(flatten(oracle.expected(doc, [q], protocol)[0]), wanted, id);
    };
    first("baseline-field-trivia", [[1+offset,13,1+offset,18],[1+offset,9,1+offset,18],
      [1+offset,2,1+offset,21],[offset,20+byteExtra,2+offset,3],
      [offset,8+byteExtra,2+offset,3],[0,0,3+offset,0]]);
    first("significant-final-brace-eof", [[offset,29+byteExtra,offset,30+byteExtra],
      [offset,17+byteExtra,offset,30+byteExtra],[offset,8+byteExtra,offset,30+byteExtra],
      [0,0,offset,30+byteExtra]]);
    const c = get("nested-enum-tuple"), doc = oracle.document(c.source, crlf, shifted);
    const q = c.queries.find(q => q.chain.length);
    assert(q.ancestors.some(a => a.kind === "TypeHint")); assert(!q.chain.includes("hint"));
    assert.deepEqual(doc.markers.hint, doc.markers.leaf);
    first("call-argument-trivia", [[1+offset,33+byteExtra,1+offset,38+byteExtra],[1+offset,29+byteExtra,1+offset,38+byteExtra],
      [1+offset,3,2+offset,0],[offset,34+byteExtra,2+offset,1],
      [offset,29+byteExtra,2+offset,1],[offset,22+byteExtra,2+offset,2],
      [offset,20+byteExtra,2+offset,4],[offset,8+byteExtra,2+offset,4],[0,0,3+offset,0]]);
  }
});

test("selection trivia preserves lexical CR ownership, shebang byte zero, trivia points and opaque literals", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const c of cases) {
    const doc = oracle.document(c.source, crlf, shifted);
    for (const t of c.tokens) {
      const span = oracle.lexical(doc, t, c.tokens); assert(span.end > span.start);
      if (t.kind === "LineComment") { assert(!span.text.includes("\n")); assert.equal(span.text.endsWith("\r"), crlf); }
      if (t.kind === "Shebang") {
        assert.equal(span.start, 0); assert(span.text.startsWith("#!"));
        assert.equal(span.text.endsWith("\n"), shifted || c.id !== "shebang-only-eof");
      }
      if (t.kind === "Whitespace") assert.match(span.text, /^[ \t\r\n]+$/);
      for (const q of c.queries.filter(q => q.token === t.marker)) {
        if (t.trivia && !q.position.startsWith("between-")) assert.deepEqual(q.chain, []);
      }
    }
  }
  for (const id of ["ordinary-string-comment-lookalikes", "bytes-comment-lookalikes", "char-whitespace-lookalike",
    "multiline-string-comment-lookalikes"]) {
    const c = get(id); assert.equal(c.tokens.find(t => t.marker === "leaf").trivia, false);
    assert(c.queries.find(q => q.token === "leaf").ancestors.some(a => a.kind === "Literal"));
    assert.deepEqual(oracle.document(c.source).markers.leaf, oracle.document(c.source).markers.literal);
  }
  for (const location of ["before", "after"]) {
    const damaged = get("damage-neighbor-"+location), repaired = get("repair-neighbor-"+location);
    assert.deepEqual({ ...get("redamage-neighbor-"+location), id: damaged.id }, damaged);
    assert.equal(repaired.parse, "quiet"); assert.notEqual(repaired.source, damaged.source);
    assert(damaged.queries.some(q => q.ancestors?.some(a => a.marker === "bad-param")));
  }
});

test("selection trivia keeps complete duplicate-ordered vectors, point fallbacks and actual empty vectors", () => {
  const before = (a, b) => a.line < b.line || (a.line === b.line && a.character <= b.character);
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const c of cases) {
    const doc = oracle.document(c.source, crlf, shifted);
    const positions = oracle.positions(doc, c.queries), result = oracle.expected(doc, c.queries);
    assert.equal(result.length, positions.length);
    for (const [i, row] of result.entries()) {
      if (!c.queries[i].chain.length) { assert.deepEqual(row, { range: { start: positions[i], end: positions[i] } }); continue; }
      let child;
      for (let parent = row; parent; parent = parent.parent) {
        const r = parent.range; assert(before(r.start, positions[i]) && before(positions[i], r.end));
        assert.notDeepEqual(r.start, r.end);
        if (child) { assert(before(r.start, child.start) && before(child.end, r.end)); assert.notDeepEqual(r, child); }
        child = r;
      }
      const q = c.queries[i], duplicate = c.queries.findIndex((p, j) => j < i && p.position === q.position);
      if (duplicate >= 0) assert.deepEqual(row, result[duplicate]);
    }
    assert.deepEqual(oracle.positions(doc, []), []); assert.deepEqual(oracle.expected(doc, []), []);
  }
});
