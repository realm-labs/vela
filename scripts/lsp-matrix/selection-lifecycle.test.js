"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/selection-lifecycle.json");
const oracle = require("./selection-oracle"), { variants, phases } = spec.oracle;
const flatten = row => { const result = []; for (; row; row = row.parent) {
  const r = row.range; result.push([r.start.line, r.start.character, r.end.line, r.end.character]);
} return result; };

test("selection lifecycle pins complete source/dependency states and diagnostic publication ownership", () => {
  assert.equal(Object.keys(variants).length, 11); assert.equal(phases.length, 43);
  assert.equal(phases.reduce((n, p) => n + p.actions.length, 0), 46);
  assert.equal(new Set(phases.map(p => p.id)).size, 43);
  const at = id => { const p = phases.find(p => p.id === id); assert(p, id); return p; };
  const helper = "scripts/helper.vela", renamed = "scripts/renamed.vela";
  assert.equal(at("damage-disk-hidden-by-open-helper").disk[helper], "helper-damage-before");
  assert.equal(at("damage-disk-hidden-by-open-helper").views[helper], "helper-damage-after");
  assert.equal(at("repair-overlay-over-damaged-disk").views[helper], "helper-base");
  assert.equal(at("close-helper-exposes-damaged-disk").views[helper], "helper-damage-before");
  assert.equal(at("repair-closed-helper").views[helper], "helper-base");
  assert.equal(at("delete-disk-keeps-overlay").disk[helper], null);
  assert.equal(at("delete-disk-keeps-overlay").views[helper], "helper-damage-after");
  assert.equal(at("close-deleted-helper-removes-source").views[helper], null);
  assert.equal(at("rename-helper-away").views[helper], null);
  assert.equal(at("rename-helper-away").views[renamed], "helper-damage-after");
  assert.equal(at("save-damaged-helper").disk[helper], "helper-damage-after");
  assert.equal(at("close-saved-damaged-helper").views[helper], "helper-damage-after");
  assert.deepEqual(phases.at(-1).views, phases[0].views);
  assert.deepEqual(phases.at(-1).open, {});
  for (const p of phases) {
    assert.deepEqual(Object.keys(p.views), Object.keys(phases[0].views));
    for (const file of Object.keys(p.views)) assert.equal(p.views[file], p.open[file] || p.disk[file]);
    for (const a of p.actions) assert.equal(a.publication,
      a.op !== "save" && !(a.op === "write" && a.file === renamed));
  }
});

test("selection lifecycle independently fixes full member and nested type geometry across Unicode newline forms", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const protocol of [false, true]) {
    const delta = protocol ? 0 : 4, shift = shifted ? 2 : 0;
    for (const [id, line, end, columns, boundary] of [
      ["main-base", 2, 3, [[31,36],[27,36],[26,37],[21,37],[14,38],[12,40],[0,40]], [[36,37],[26,37],[21,37],[14,38],[12,40],[0,40]]],
      ["helper-base", 1, 2, [[34,37],[33,38],[28,38],[21,38],[20,39],[8,53]], [[37,38],[33,38],[28,38],[21,38],[20,39],[8,53]]],
      ["helper-damage-before", 2, 3, [[26,29],[25,30],[20,30],[13,30],[12,31],[0,45]], [[29,30],[25,30],[20,30],[13,30],[12,31],[0,45]]]
    ]) {
      const v = variants[id], d = oracle.document(v.source, crlf, shifted), rows = oracle.expected(d, v.queries, protocol);
      const add = line === 1 ? delta : 0;
      const ranges = cols => cols.map(([s,e]) => [line + shift,s + add,line + shift,e + add])
        .concat([[0,0,end + shift,0]]);
      assert.deepEqual(flatten(rows[0]), ranges(columns), id);
      assert.deepEqual(flatten(rows[1]), ranges(boundary), id);
      assert.deepEqual(rows[0], rows[2]);
      assert.equal(d.text.includes("\r\n"), crlf);
    }
  }
});

test("selection lifecycle retains all CST owners including equal-span type/literal nodes and exact diagnostic spans", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) {
    for (const v of Object.values(variants)) {
      const d = oracle.document(v.source, crlf, shifted);
      for (const node of v.nodes) { assert(d.markers[node.marker]); assert(node.kind); }
      for (const q of v.queries.filter(q => q.token)) {
        assert(d.markers[q.token]); assert.equal(q.ancestors.at(-1).kind, "SourceFile");
        for (const node of q.ancestors) assert(v.nodes.some(n => n.marker === node.marker && n.kind === node.kind));
      }
      assert.deepEqual(v.diagnostics, v.id.includes("damage") ?
        [{code:"E_PARSE",message:"expected type annotation",range:"bad-param"}] : []);
    }
    const d = oracle.document(variants["helper-damage-before"].source, crlf, shifted);
    assert.deepEqual(oracle.point(d,d.markers["bad-param"].start), {line:1 + (shifted ? 2 : 0),character:18});
    assert.deepEqual(oracle.point(d,d.markers["bad-param"].end), {line:1 + (shifted ? 2 : 0),character:24});
  }
  const q = variants["helper-base"].queries[0];
  assert(q.ancestors.some(n => n.marker === "inner-type" && n.kind === "TypeHint"));
  assert(!q.chain.includes("inner-type"));
  assert(variants["decoy-base"].queries[0].ancestors.some(n => n.kind === "Literal"));
  assert(!variants["decoy-base"].queries[0].chain.includes("value"));
});

test("selection lifecycle preserves whole duplicate vectors, right token boundaries, point and empty inputs", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const v of Object.values(variants)) {
    const d = oracle.document(v.source,crlf,shifted), p = oracle.positions(d,v.queries), rows = oracle.expected(d,v.queries);
    assert.equal(rows.length,5); assert.deepEqual(rows[0],rows[2]);
    for (const index of [3,4]) assert.deepEqual(rows[index],{range:{start:p[index],end:p[index]}});
    assert.deepEqual(oracle.expected(d,[]),[]);
    for (const row of rows.slice(0,3)) {
      const spans=flatten(row); assert.equal(new Set(spans.map(s => s.join(":"))).size,spans.length);
      assert.deepEqual(spans.at(-1),[0,0,d.markers.file.end.line,0]);
    }
  }
});
