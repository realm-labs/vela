"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const spec=require("../../tests/lsp_matrix/fixtures/folding-bodies.json"),{parseMarkers}=require("./fixtures"),{cases}=spec.oracle;
test("folding bodies pin thirty-four complete function method and control-flow sets with six genuine empty cases",()=>{
 assert.equal(spec.id,"folding-bodies");assert.equal(cases.length,34);assert.equal(new Set(cases.map(c=>c.id)).size,34);
 assert.deepEqual(cases.map(c=>c.ranges.length),[2,3,4,5,4,4,3,5,3,3,3,5,4,3,6,4,4,4,4,3,3,3,3,3,3,4,4,4,0,0,0,0,0,0]);
 assert.equal(cases.reduce((n,c)=>n+c.ranges.length,0),103);
 assert.deepEqual(Object.keys(spec.files),["scripts/main.vela","scripts/helper.vela"]);assert.equal(spec.files["scripts/main.vela"],cases[0].source);
 for(const c of cases){const doc=parseMarkers(c.source);assert.deepEqual(Object.keys(doc.markers).sort(),c.ranges.map(r=>r.range).sort());assert(c.ranges.every(r=>r.kind==="region"));}
});
test("folding body item and body ranges independently pin Unicode byte UTF16 and complete endpoint geometry",()=>{
 for(const crlf of[false,true])for(const shifted of[false,true]){
  let source=(shifted?"// shifted 中😀\n/* second 😀 */\n":"")+cases[0].source;if(crlf)source=source.replaceAll("\n","\r\n");
  const doc=parseMarkers(source),bytes=Buffer.from(doc.text),shift=shifted?2:0;
  for(const[name,column,byteColumn]of[["item",8,12],["body",35,39]]){
   const m=doc.markers[name];assert.deepEqual([m.start.line,m.start.character,m.end.line,m.end.character],[shift,column,shift+4,1]);
   assert.equal(m.start.byte-(bytes.lastIndexOf(10,m.start.byte-1)+1),byteColumn);assert.equal(m.end.byte-(bytes.lastIndexOf(10,m.end.byte-1)+1),1);
  }
 }
});
test("folding control-flow distinguishes branch loop pattern lambda and match-arm expression ownership",()=>{
 const get=id=>cases.find(c=>c.id===id),names=id=>get(id).ranges.map(r=>r.range);
 assert.deepEqual(names("if-statement-branches"),["item","body","then","else"]);
 assert.deepEqual(names("else-if-chain"),["item","body","then","second","else"]);
 assert.deepEqual(names("for-multiline-iterable-and-conditional"),["item","body","iter","loop","then"]);
 assert.deepEqual(names("match-expression-arm-if"),["item","body","match","arm","then","else"]);
 assert.deepEqual(names("match-record-pattern-alias-shorthand-and-multiline-fields"),["item","body","match"]);
 for(const id of["lambda-block-body","closure-captures-local","named-callback-argument","async-await-callback","trait-default-closure-body"])assert.deepEqual(names(id),["item","body","lambda","lambda-body"]);
 assert.deepEqual(names("inherent-method-nested-loop"),["item","body","loop","then"]);
 assert(get("named-callback-argument").source.includes("consume(callback = "));assert(!get("named-callback-argument").source.includes("callback:"));
 assert(get("for-two-binding-pattern").source.includes("for index, item in"));assert(get("for-tuple-pattern").source.includes("for (index, item) in"));
 assert(get("match-record-pattern-alias-shorthand-and-multiline-fields").source.includes("Choice::Named { count }"));
 for(const c of cases.slice(28))assert.deepEqual(c.ranges,[]);
});
