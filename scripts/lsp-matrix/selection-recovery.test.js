"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-recovery.json");
const oracle = require("./selection-oracle"), cases = spec.oracle.cases;
const get = id => { const c = cases.find(c => c.id === id); assert(c, id); return c; };
const flatten = row => { const a = []; for (; row; row = row.parent) {
  const r = row.range; a.push([r.start.line, r.start.character, r.end.line, r.end.character]);
} return a; };

test("selection recovery pins all authored triplets, whole CST ownership and exact negative cardinality", () => {
  assert.equal(cases.length, 250); assert.equal(new Set(cases.map(c => c.id)).size, 250);
  const queries = cases.flatMap(c => c.queries);
  assert.equal(queries.length, 820); assert.equal(queries.filter(q => q.chain.length).length, 729);
  assert.equal(queries.filter(q => !q.chain.length).length, 91);
  assert.equal(cases.filter(c => c.parse === "quiet").length, 85);
  assert.equal(cases.filter(c => c.parse === "damaged").length, 145);
  assert.equal(cases.filter(c => c.parse.startsWith("lexical/")).length, 18);
  assert.equal(cases.filter(c => c.parse.startsWith("errors/")).length, 2);
  assert.equal(cases.filter(c => !c.queries.length).length, 1);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files[spec.oracle.file], cases[0].source);
  const partitions = [...new Set(cases.filter(c => c.state === "damage" && c.queries.length).map(c => c.partition))];
  assert.equal(partitions.length, 82);
  for (const partition of partitions) {
    const first = get("damage-" + partition), repair = get("repair-" + partition), again = get("redamage-" + partition);
    assert.deepEqual({ ...again, id: first.id, state: first.state }, first);
    assert.equal(repair.parse, "quiet"); assert.notEqual(repair.source, first.source);
  }
  for (const c of cases) {
    const doc = oracle.document(c.source);
    for (const node of c.nodes) { assert(doc.markers[node.marker]); assert(node.kind); }
    for (const q of c.queries) {
      assert(doc.markers[q.position]); for (const name of q.chain) assert(doc.markers[name]);
      if (!q.token) { assert.deepEqual(q.chain, []); continue; }
      assert(doc.markers[q.token]); assert.equal(q.ancestors.at(-1).kind, "SourceFile");
      assert.deepEqual(q.ancestors.at(-1), { marker: "file", kind: "SourceFile" });
      for (const ancestor of q.ancestors) assert(c.nodes.some(n => n.marker === ancestor.marker && n.kind === ancestor.kind));
    }
  }
});

test("selection recovery independently pins member, nested type and interpolation geometry including right boundary tokens", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const protocol of [false, true]) {
    const line = 1 + (shifted ? 2 : 0), delta = protocol ? 0 : 4;
    const spans = (columns, repaired) => columns.map(([s, e]) => [line, s + delta, line, e + delta])
      .concat([[0, 0, repaired ? line + 1 : line, repaired ? 0 : columns.at(-1)[1] + delta]]);
    const check = (id, columns, repaired = false) => {
      const c = get(id), doc = oracle.document(c.source, crlf, shifted), result = oracle.expected(doc, c.queries, protocol);
      assert.deepEqual(flatten(result[0]), spans(columns, repaired), id);
      assert.deepEqual(result[0], result[2]);
      return { c, doc, result };
    };
    const damaged = check("damage-member-local", [[35,38],[35,39],[22,39],[20,39],[8,39]]);
    // File end differs from the leaf end for the damaged member.
    const wanted = spans([[38,39],[35,39],[22,39],[20,39],[8,39]], false);
    wanted.at(-1)[3] = 39 + delta;
    assert.deepEqual(flatten(damaged.result[1]), wanted);
    check("repair-member-local", [[35,38],[35,44],[22,45],[20,47],[8,47]], true);
    check("damage-nested-open-type", [[35,38],[34,38],[29,38],[28,38],[22,38],[15,38],[14,38],[8,38]]);
    check("repair-nested-open-type", [[35,38],[34,39],[29,39],[28,48],[22,48],[15,48],[14,49],[8,52]], true);
    check("damage-lexical-open-interpolation-ordinary", [[35,46],[22,46],[20,46],[8,46]]);
    const repaired = check("repair-lexical-open-interpolation-ordinary", [[43,46],[42,47],[35,48],[22,49],[20,51],[8,51]], true);
    assert.equal(repaired.doc.text.includes("\r\n"), crlf);
    assert.deepEqual(flatten(repaired.result[1]), spans([[46,47],[42,47],[35,48],[22,49],[20,51],[8,51]], true));
  }
});

test("selection recovery preserves fallback shapes and error policies without dropping same-span CST nodes", () => {
  for (const context of ["local", "return"]) {
    const unary = get("damage-unary-operand-" + context), fixed = get("repair-unary-operand-" + context);
    assert.equal(unary.nodes.find(n => n.marker === "unary").kind, "PathExpr");
    assert.equal(fixed.nodes.find(n => n.marker === "unary").kind, "UnaryExpr");
    for (const c of [unary, fixed]) assert(c.queries[0].ancestors.some(n => n.marker === "unary"));
    for (const family of ["key", "value", "close"]) get("damage-map-" + family + "-" + context);
  }
  for (const family of ["map", "set", "iterator", "option", "result"]) get("damage-builtin-open-" + family);
  assert.equal(get("damage-nested-open-type").parse, "errors/E_PARSE,syntax::type_argument_arity");
  assert.equal(get("damage-unfinished-comment").parse, "lexical/E_PARSE,E_LEX_BLOCK_COMMENT");
  for (const family of ["ordinary", "multiline"]) {
    const c = get("damage-lexical-open-interpolation-" + family);
    assert(c.parse.includes("E_LEX_STRING_INTERPOLATION"));
    assert.equal(c.nodes.find(n => n.marker === "value").kind, "PathExpr");
    assert(!c.nodes.some(n => n.kind === "Interpolation"));
    assert(get("repair-lexical-open-interpolation-" + family).nodes.some(n => n.kind === "Interpolation"));
  }
  const nested = get("damage-nested-open-type");
  assert(nested.queries[0].ancestors.some(n => n.marker === "inner-type"));
  assert(!nested.queries[0].chain.includes("inner-type"));
  for (const c of cases.filter(c => c.partition.startsWith("neighbor-"))) {
    assert.deepEqual(c.queries[0].chain, ["leaf", "field", "statement", "body", "item", "file"]);
  }
});

test("selection recovery complete vectors contain every child and preserve duplicate order, point queries and empty input", () => {
  const before = (a, b) => a.line < b.line || (a.line === b.line && a.character <= b.character);
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const c of cases) {
    const doc = oracle.document(c.source, crlf, shifted), positions = oracle.positions(doc, c.queries), results = oracle.expected(doc, c.queries);
    assert.equal(results.length, positions.length);
    for (const [index, result] of results.entries()) {
      if (!c.queries[index].chain.length) { assert.deepEqual(result, { range: { start: positions[index], end: positions[index] } }); continue; }
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
