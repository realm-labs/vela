"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const spec=require("../../tests/lsp_matrix/fixtures/folding-trivia.json"),{parseMarkers}=require("./fixtures"),{cases}=spec.oracle;
test("folding trivia pins thirty-eight complete sets with standalone trivia empties and lexical recovery",()=>{
 assert.equal(spec.id,"folding-trivia");assert.equal(cases.length,38);assert.equal(new Set(cases.map(c=>c.id)).size,38);
 assert.deepEqual(cases.map(c=>c.ranges.length),[2,2,2,2,2,2,2,2,3,4,3,3,4,3,3,2,2,2,2,2,2,2,1,1,1,1,1,0,0,0,0,0,0,0,0,0,2,0]);
 assert.equal(cases.reduce((n,c)=>n+c.ranges.length,0),60);assert.equal(cases.filter(c=>!c.ranges.length).length,10);
 assert.deepEqual(Object.keys(spec.files),["scripts/main.vela","scripts/helper.vela"]);assert.equal(spec.files["scripts/main.vela"],cases[0].source);
 for(const c of cases){const doc=parseMarkers(c.source);assert.deepEqual(Object.keys(doc.markers).sort(),[...c.ranges.map(r=>r.range),...(c.parseRange?[c.parseRange]:[])].sort());}
 assert.deepEqual(cases.filter(c=>c.parseCodes).map(c=>[c.id,c.parseCodes,c.parseMessages]),[
  ["valid-function-before-unclosed-comment",["E_LEX_BLOCK_COMMENT"],["unterminated block comment"]],
  ["no-fold-unclosed-comment",["E_LEX_BLOCK_COMMENT"],["unterminated block comment"]],
 ]);
});
test("folding trivia independently pins Unicode item body bounds and preserves file-start shebang through CRLF shifts",()=>{
 for(const crlf of[false,true])for(const shifted of[false,true]){
  const doc=parseMarkers(transform(cases[0].source,crlf,shifted)),bytes=Buffer.from(doc.text),shift=shifted?2:0;
  for(const[name,column,byteColumn]of[["item",8,12],["body",35,39]]){
   const m=doc.markers[name];assert.deepEqual([m.start.line,m.start.character,m.end.line,m.end.character],[shift,column,shift+4,1]);
   assert.equal(m.start.byte-(bytes.lastIndexOf(10,m.start.byte-1)+1),byteColumn);assert.equal(m.end.byte-(bytes.lastIndexOf(10,m.end.byte-1)+1),1);
  }
  for(const [index,start,end] of [[36,3,5],[37,0,2]]){const range=parseMarkers(transform(cases[index].source,crlf,shifted)).markers.error;assert.deepEqual([range.start.line,range.start.character,range.end.line,range.end.character],[start+shift,0,end+shift,0]);}
  for(const c of cases.filter(c=>c.source.startsWith("#!"))){
   const original=parseMarkers(c.source),shiftedDoc=parseMarkers(transform(c.source,crlf,shifted));
   assert(shiftedDoc.text.startsWith("#!"));assert.equal(shiftedDoc.text.split(/\r?\n/)[0],original.text.split("\n")[0]);
   for(const name of Object.keys(original.markers)){
    const before=original.markers[name],after=shiftedDoc.markers[name];
    assert.deepEqual([after.start.line,after.start.character,after.end.line,after.end.character],[before.start.line+shift,before.start.character,before.end.line+shift,before.end.character]);
   }
  }
 }
 assert.equal(transform("#!/usr/bin/env vela\nfn main() {}\n",true,true),"#!/usr/bin/env vela\r\n// shifted 中😀\r\n/* second 😀 */\r\nfn main() {}\r\n");
 assert.equal(transform("#!EOF",false,true),"#!EOF\n// shifted 中😀\n/* second 😀 */\n");
});
test("folding trivia retains owned item block literal and import extents while comments cannot fabricate regions",()=>{
 const get=id=>cases.find(c=>c.id===id),names=id=>get(id).ranges.map(r=>r.range);
 assert.deepEqual(names("if-else-comment-boundaries"),["item","body","then","else"]);
 assert.deepEqual(names("lambda-comment-trivia"),["item","body","lambda","lambda-body"]);
 for(const id of["struct-member-comment-spans","enum-member-comment-spans","trait-signature-comment-spans"])assert.deepEqual(names(id),["item"]);
 for(const id of["import-comment-blank-group","shebang-import-comment-group"])assert.deepEqual(get(id).ranges,[{kind:"imports",range:"imports"}]);
 assert.deepEqual(names("valid-function-before-unclosed-comment"),["item","body"]);
 for(const c of cases.filter(c=>c.id.startsWith("no-fold-")))assert.deepEqual(c.ranges,[]);
});

function transform(source, crlf, shifted) {
  if (shifted) {
    const prefix = "// shifted 中😀\n/* second 😀 */\n";
    if (source.startsWith("#!")) {
      const end = source.indexOf("\n");
      source = end < 0 ? source + "\n" + prefix : source.slice(0, end + 1) + prefix + source.slice(end + 1);
    } else source = prefix + source;
  }
  return crlf ? source.replaceAll("\n", "\r\n") : source;
}
