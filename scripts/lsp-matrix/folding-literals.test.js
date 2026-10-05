"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const spec = require("../../tests/lsp_matrix/fixtures/folding-literals.json"), { parseMarkers } = require("./fixtures"), { cases } = spec.oracle;
test("folding literals pin fifty-nine whole sets covering every binary and compound operator with twelve genuine empties", () => {
 assert.equal(spec.id, "folding-literals"); assert.equal(cases.length, 59); assert.equal(new Set(cases.map(c => c.id)).size, 59);
 assert.deepEqual(cases.map(c => c.ranges.length), [3,4,3,4,3,5,3,4,3,3,3,4,3,2,2,3,3, ...Array(17).fill(4), 4, ...Array(5).fill(3), 3,5,4,3,3,2,2, ...Array(12).fill(0)]);
 assert.equal(cases.reduce((n,c) => n + c.ranges.length, 0), 164);
 assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela", "scripts/helper.vela"]); assert.equal(spec.files["scripts/main.vela"], cases[0].source);
 for(const c of cases) { const doc = parseMarkers(c.source); assert.deepEqual(Object.keys(doc.markers).sort(), c.ranges.map(r => r.range).sort()); assert(c.ranges.every(r => r.kind === "region")); }
});
test("folding literal item body and array endpoints pin byte UTF16 Unicode and CRLF geometry", () => {
 for(const crlf of [false,true]) for(const shifted of [false,true]) {
  let source = (shifted ? "// shifted 中😀\n/* second 😀 */\n" : "") + cases[0].source; if(crlf) source = source.replaceAll("\n", "\r\n");
  const doc = parseMarkers(source), bytes = Buffer.from(doc.text), shift = shifted ? 2 : 0;
  for(const [name,line,column,byteColumn,endLine] of [["item",0,8,12,6],["body",0,35,39,6],["array",1,14,14,4]]) {
   const m = doc.markers[name]; assert.deepEqual([m.start.line,m.start.character,m.end.line,m.end.character], [line+shift,column,endLine+shift,1]);
   assert.equal(m.start.byte-(bytes.lastIndexOf(10,m.start.byte-1)+1), byteColumn); assert.equal(m.end.byte-(bytes.lastIndexOf(10,m.end.byte-1)+1), 1);
  }
 }
});
test("folding literals preserve nested literal ownership while scalar operators parens and call wrappers cannot invent regions", () => {
 const get = id => cases.find(c => c.id === id), names = id => get(id).ranges.map(r => r.range);
 assert.deepEqual(names("nested-array-literal"), ["item","body","outer","inner"]);
 assert.deepEqual(names("map-key-and-value-arrays"), ["item","body","map","key","value"]);
 assert.deepEqual(names("record-shorthand-and-array"), ["item","body","record","array"]);
 assert.deepEqual(names("interpolation-array-expression"), ["item","body","string","array"]);
 assert.deepEqual(names("parenthesized-array"), ["item","body","array"]);
 for(const id of ["parenthesized-scalar","unit-scalar-literal-families","scalar-operators-multiline","scalar-call-index-field-multiline"]) assert.deepEqual(names(id), ["item","body"]);
 assert.deepEqual(names("call-callee-and-arguments"), ["item","body","callee","first","second"]);
 assert.deepEqual(names("index-receiver-and-index"), ["item","body","receiver","index"]);
 assert.deepEqual(cases.filter(c => c.id.startsWith("binary-")).map(c => c.id.slice(7,-12)), ["add","subtract","multiply","divide","remainder","equal","not-equal","identity","not-identity","less","less-equal","greater","greater-equal","and","or","range","range-inclusive"]);
 assert.deepEqual(cases.filter(c => c.id.startsWith("compound-")).map(c => c.id), ["compound-assignment-add-value","compound-assignment-subtract-value","compound-assignment-multiply-value","compound-assignment-divide-value","compound-assignment-remainder-value"]);
 for(const c of cases.slice(47)) assert.deepEqual(c.ranges, []);
});
