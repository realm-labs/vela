"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const spec=require("../../tests/lsp_matrix/fixtures/folding-members.json"),{parseMarkers}=require("./fixtures"),{cases}=spec.oracle;
test("folding members pin thirty-nine complete constructor method and field sets with eight genuine empties",()=>{
 assert.equal(spec.id,"folding-members");assert.equal(cases.length,39);assert.equal(new Set(cases.map(c=>c.id)).size,39);
 assert.deepEqual(cases.map(c=>c.ranges.length),[3,4,4,4,2,3,3,2,2,3,2,3,4,5,4,4,4,4,4,4,5,2,3,1,2,3,3,4,1,1,1,0,0,0,0,0,0,0,0]);
 assert.equal(cases.reduce((n,c)=>n+c.ranges.length,0),94);
 assert.deepEqual(Object.keys(spec.files),["scripts/main.vela","scripts/helper.vela"]);assert.equal(spec.files["scripts/main.vela"],cases[0].source);
 for(const c of cases){const doc=parseMarkers(c.source);assert.deepEqual(Object.keys(doc.markers).sort(),c.ranges.map(r=>r.range).sort());assert(c.ranges.every(r=>r.kind==="region"));}
});
test("folding constructor item body and record ranges pin literal Unicode byte UTF16 and CRLF geometry",()=>{
 for(const crlf of[false,true])for(const shifted of[false,true]){
  let source=(shifted?"// shifted 中😀\n/* second 😀 */\n":"")+cases[0].source;if(crlf)source=source.replaceAll("\n","\r\n");
  const doc=parseMarkers(source),bytes=Buffer.from(doc.text),shift=shifted?2:0;
  for(const[name,line,column,byteColumn,endLine]of[["item",2,8,12,8],["body",2,35,39,8],["record",3,14,14,6]]){
   const m=doc.markers[name];assert.deepEqual([m.start.line,m.start.character,m.end.line,m.end.character],[line+shift,column,endLine+shift,1]);
   assert.equal(m.start.byte-(bytes.lastIndexOf(10,m.start.byte-1)+1),byteColumn);assert.equal(m.end.byte-(bytes.lastIndexOf(10,m.end.byte-1)+1),1);
  }
 }
});
test("folding members separate constructor values and method defaults from enclosing field metadata",()=>{
 const names=id=>cases.find(c=>c.id===id).ranges.map(r=>r.range);
 assert.deepEqual(names("record-shorthand-constructor"),["item","body","value","record"]);
 assert.deepEqual(names("nested-record-constructor"),["item","body","outer","inner"]);
 assert.deepEqual(names("chained-record-call-index"),["item","body","record","argument","index"]);
 assert.deepEqual(names("inherent-static-and-instance-bodies"),["item","static-body","method-body"]);
 for(const id of["inherent-method-default-array","trait-required-method-default-array"])assert.deepEqual(names(id),["item","default"]);
 assert.deepEqual(names("trait-impl-method-default-and-body"),["item","default","body"]);
 assert.deepEqual(names("async-method-named-callback"),["item","body","lambda","lambda-body"]);
 for(const id of["inherent-method-header-only","struct-field-initializers-stay-enclosed","enum-payload-defaults-stay-enclosed","trait-multiple-required-methods"])assert.deepEqual(names(id),["item"]);
 for(const id of["tuple-variant-multiline-scalars","unit-variant-parenthesized","static-constructor-multiline-scalars","instance-method-multiline-scalars"])assert.deepEqual(names(id),["item","body"]);
 for(const c of cases.slice(31))assert.deepEqual(c.ranges,[]);
});
