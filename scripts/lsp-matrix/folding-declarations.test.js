"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const spec = require("../../tests/lsp_matrix/fixtures/folding-declarations.json");
const { parseMarkers } = require("./fixtures");
const { cases } = spec.oracle;
test("folding declarations pin twenty-two complete whole sets across every top-level owner and exact negatives", () => {
  assert.equal(spec.id, "folding-declarations"); assert.equal(cases.length, 22);
  assert.deepEqual(cases.map(c => c.ranges.length), [2,2,2,1,2,1,1,1,1,1,2,2,2,2,2,1,0,0,0,0,0,0]);
  assert.equal(new Set(cases.map(c => c.id)).size,22);
  assert.deepEqual(Object.keys(spec.files), ["scripts/main.vela","scripts/helper.vela"]);
  assert.equal(spec.files["scripts/main.vela"],cases[0].source);
  for (const c of cases) {
    const doc = parseMarkers(c.source);
    assert.deepEqual(Object.keys(doc.markers).sort(),c.ranges.map(r=>r.range).sort());
    assert(c.ranges.every(r=>r.kind==="region"));
  }
});
test("folding declaration outer and body extents independently pin byte versus UTF16 geometry under all four profiles", () => {
  for (const crlf of [false,true]) for (const shifted of [false,true]) {
    let source=(shifted?"// shifted 中😀\n/* second 😀 */\n":"")+cases[0].source;
    if(crlf)source=source.replaceAll("\n","\r\n");
    const doc=parseMarkers(source), bytes=Buffer.from(doc.text), shift=shifted?2:0;
    for(const [name,column,byteColumn]of[["item",8,12],["body",45,49]]){
      const m=doc.markers[name];
      assert.deepEqual([m.start.line,m.start.character,m.end.line,m.end.character],[shift,column,shift+2,1]);
      assert.equal(m.start.byte-(bytes.lastIndexOf(10,m.start.byte-1)+1),byteColumn);
      assert.equal(m.end.byte-(bytes.lastIndexOf(10,m.end.byte-1)+1),1);
    }
  }
});
test("folding declaration sets distinguish item and body/default ranges from member and state non-ranges", () => {
  const get=id=>cases.find(c=>c.id===id), names=id=>get(id).ranges.map(r=>r.range);
  for(const id of ["public-defaulted-function","private-untyped-function","async-function","public-trait-required-default-async","inherent-impl-private-public-async","trait-impl"])assert.deepEqual(names(id),["item","body"]);
  for(const id of ["multiline-default-array","trait-parameter-default","impl-parameter-default"])assert.deepEqual(names(id),["item","default"]);
  for(const id of ["public-struct-default-members","private-struct-multiline-default","public-enum-all-forms","attribute-with-single-body"])assert.deepEqual(names(id),["item"]);
  assert.deepEqual(names("const-multiline-array"),["array"]);
  assert(get("public-enum-all-forms").source.includes("Named {\n"));
  assert(get("private-struct-multiline-default").source.includes("values: Array<i64> = [\n"));
  assert(get("single-line-all-declarations").source.includes("impl Readable for Row"));
  for(const c of cases.slice(16))assert.deepEqual(c.ranges,[]);
});
