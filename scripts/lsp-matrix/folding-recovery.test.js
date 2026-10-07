"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const spec=require("../../tests/lsp_matrix/fixtures/folding-recovery.json"),{parseMarkers}=require("./fixtures"),{cases}=spec.oracle;
test("folding recovery pins fifty-four complete authored sets and all diagnosed quiet and empty partitions",()=>{
 assert.equal(spec.id,"folding-recovery");assert.equal(cases.length,54);assert.equal(new Set(cases.map(c=>c.id)).size,54);
 assert.deepEqual(cases.map(c=>c.ranges.length),[6,4,4,4,4,4,4,4,4,4,6,6,6,6,5,5,5,5,5,5,5,5,5,6,6,5,6,6,6,6,6,6,6,6,7,7,7,7,7,8,5,6,5,5,5,5,6,5,4,0,0,0,0,0]);
 assert.equal(cases.reduce((n,c)=>n+c.ranges.length,0),265);assert.equal(cases.filter(c=>!c.ranges.length).length,5);assert.equal(cases.filter(c=>c.parseError).length,43);
 assert.deepEqual(Object.keys(spec.files),["scripts/main.vela","scripts/helper.vela"]);assert.equal(spec.files["scripts/main.vela"],cases[0].source);
 for(const c of cases){const doc=parseMarkers(c.source),names=[...c.ranges,...(c.syntaxRegions??[])].map(r=>r.range),unique=[...new Set(names)];assert.equal(names.length-unique.length,c.id==="unclosed-tuple"?1:0);assert.deepEqual(Object.keys(doc.markers).sort(),unique.sort());assert.equal(typeof c.parseError,"boolean");}
});
test("folding recovery independently pins Unicode neighbor geometry and complete malformed expression CST extents",()=>{
 for(const crlf of[false,true])for(const shifted of[false,true]){
  const source=(shifted?"// shifted 中😀\n/* second 😀 */\n":"")+cases[0].source;
  const doc=parseMarkers(crlf?source.replaceAll("\n","\r\n"):source),bytes=Buffer.from(doc.text),shift=shifted?2:0;
  for(const[name,column,byteColumn]of[["before",8,12],["before-body",20,24]]){
   const m=doc.markers[name];assert.deepEqual([m.start.line,m.start.character,m.end.line,m.end.character],[shift,column,shift+2,1]);
   assert.equal(m.start.byte-(bytes.lastIndexOf(10,m.start.byte-1)+1),byteColumn);assert.equal(m.end.byte-(bytes.lastIndexOf(10,m.end.byte-1)+1),1);
  }
 }
 const get=id=>cases.find(c=>c.id===id);
 for(const id of["unclosed-triple-string"]){
  const c=get(id),doc=parseMarkers(c.source),range=doc.markers.value;
  assert.deepEqual(c.syntaxRegions,[{range:"value",kind:"PathExpr"}]);assert(!c.ranges.some(r=>r.range==="value"));
  assert.equal(range.end.byte,Buffer.byteLength(doc.text));assert.equal(c.parseError,true);
 }
 const tuple=get("unclosed-tuple");
 assert.deepEqual(tuple.syntaxRegions,[{range:"value",kind:"TupleExpr"},{range:"first",kind:"Literal"},{range:"last",kind:"Literal"}]);
 assert.deepEqual(tuple.ranges.map(r=>r.range),["before","before-body","after","after-body","broken","body","value"]);
 assert.equal(tuple.parseError,true);
 for(const crlf of[false,true])for(const shifted of[false,true]){
  const source=(shifted?"// shifted 中😀\n/* second 😀 */\n":"")+tuple.source;
  const doc=parseMarkers(crlf?source.replaceAll("\n","\r\n"):source),shift=shifted?2:0;
  for(const[name,startLine,startColumn,endLine,endColumn,text]of[["value",7,13,9,2,"(\n 1,\n 2"],["first",8,1,8,2,"1"],["last",9,1,9,2,"2"]]){
   const m=doc.markers[name];assert.deepEqual([m.start.line,m.start.character,m.end.line,m.end.character],[startLine+shift,startColumn,endLine+shift,endColumn]);
   assert.equal(Buffer.from(doc.text).subarray(m.start.byte,m.end.byte).toString(),crlf?text.replaceAll("\n","\r\n"):text);
  }
  assert.equal(doc.markers.value.end.byte,Buffer.byteLength(doc.text));
 }
 const lambda=parseMarkers(get("unclosed-lambda").source);
 for(const name of["value","lambda-body"]){const m=lambda.markers[name];assert.equal(m.end.byte,Buffer.byteLength(lambda.text));assert.equal(Buffer.from(lambda.text).subarray(m.end.byte-1,m.end.byte).toString(),";");}
});
test("folding recovery preserves damaged owners healthy neighbors quiet partials and genuinely empty sources",()=>{
 const get=id=>cases.find(c=>c.id===id),names=id=>get(id).ranges.map(r=>r.range);
 assert.deepEqual(names("function-no-body"),["before","before-body","broken","after","after-body"]);
 assert.deepEqual(names("unclosed-lambda"),["before","before-body","after","after-body","broken","body","value","lambda-body"]);
 for(const c of cases.filter(c=>c.id.startsWith("quiet-")||c.id==="unresolved-owner"||c.id==="dynamic-owner")){assert.equal(c.parseError,false);assert.deepEqual(c.ranges.map(r=>r.range),["before","before-body","broken","body","after","after-body"]);}
 assert.equal(get("unclosed-import-group").parseError,false);assert.deepEqual(get("unclosed-import-group").ranges[0],{kind:"imports",range:"imports"});
 for(const c of cases.filter(c=>c.id.startsWith("no-fold-")))assert.deepEqual(c.ranges,[]);
 for(const id of["const-no-name","state-no-value","function-no-name","enum-no-payload-default","trait-no-parameter-default","impl-no-parameter-default","unclosed-parameters","unclosed-parameter-array"])assert.equal(get(id).parseError,true);
});
