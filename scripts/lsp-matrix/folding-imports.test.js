"use strict";
const assert = require("node:assert/strict"), test = require("node:test");
const spec = require("../../tests/lsp_matrix/fixtures/folding-imports.json");
const { parseMarkers } = require("./fixtures");
const { cases } = spec.oracle;
test("folding imports pin fourteen independent complete groups and singleton/comment/string exclusions", () => {
  assert.equal(spec.id, "folding-imports"); assert.equal(cases.length, 14);
  assert.deepEqual(cases.map(c => c.ranges.length), [1, 1, 1, 1, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0]);
  assert.equal(new Set(cases.map(c => c.id)).size, 14);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]);
  assert.equal(spec.files["scripts/main.vela"], cases[0].source);
  assert(cases.find(c => c.id === "resolved-and-stdlib").source.includes("std::print as output"));
  assert(cases.find(c => c.id === "pub-and-private-aliases").source.includes("pub use helper::Known as Row"));
  for (const c of cases) {
    const doc = parseMarkers(c.source);
    assert.deepEqual(Object.keys(doc.markers).sort(), c.ranges.map(r => r.range).sort());
    assert(c.ranges.every(r => r.kind === "imports"));
  }
});
test("folding import whole ranges pin literal UTF16 versus byte columns under LF CRLF and two Unicode prefix lines", () => {
  for (const crlf of [false, true]) for (const shifted of [false, true]) for (const c of cases) {
    let raw = (shifted ? "// shifted 中😀\n/* second 😀 */\n" : "") + c.source;
    if (crlf) raw = raw.replaceAll("\n", "\r\n");
    const doc = parseMarkers(raw), bytes = Buffer.from(doc.text);
    for (const r of c.ranges) {
      const m = doc.markers[r.range]; assert(m.start.line < m.end.line);
      for (const p of [m.start, m.end]) {
        const column = p.byte - (bytes.lastIndexOf(10, p.byte - 1) + 1);
        assert.equal(column - p.character, 4, "one CJK and non-BMP prefix at each authored endpoint");
      }
      if (c.id === "unresolved-pair") {
        assert.deepEqual([m.start.line, m.start.character, m.end.line, m.end.character], [shifted ? 2 : 0, 8, shifted ? 3 : 1, 37]);
        assert.equal(m.start.byte - (bytes.lastIndexOf(10, m.start.byte - 1) + 1), 12);
        assert.equal(m.end.byte - (bytes.lastIndexOf(10, m.end.byte - 1) + 1), 41);
      }
    }
  }
});
test("folding imports retain logical trivia groups and literal declaration boundaries without merging separate groups", () => {
  const trivia = cases.find(c => c.id === "blank-comment-group"), doc = parseMarkers(trivia.source);
  assert.deepEqual([doc.markers.imports.start.line, doc.markers.imports.end.line], [0, 6]);
  assert(trivia.source.includes("// use fake::{"));
  for (const id of ["constant-splits-groups", "function-splits-groups"]) {
    const c = cases.find(c => c.id === id), doc = parseMarkers(c.source);
    assert.deepEqual([doc.markers.first.start.line, doc.markers.first.end.line, doc.markers.second.start.line, doc.markers.second.end.line], [0, 1, 3, 4]);
    assert.deepEqual(c.ranges, [{ kind: "imports", range: "first" }, { kind: "imports", range: "second" }]);
  }
  for (const id of ["singletons-across-const", "singletons-across-state", "singletons-across-function"]) {
    const c = cases.find(c => c.id === id); assert.equal(c.source.split("\n").filter(line => line.includes("use ")).length >= 2, true);
    assert.deepEqual(c.ranges, []);
  }
});
