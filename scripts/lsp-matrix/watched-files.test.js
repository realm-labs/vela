"use strict";
const test=require('node:test'),assert=require('node:assert/strict');
const {parseMarkers}=require('./fixtures');
const spec=require('../../tests/lsp_matrix/fixtures/watched-files.json');
test('watch fixtures pin independent UTF-16 target and config-byte goldens under LF and CRLF',()=>{
  for(const crlf of [false,true]){
    const document=text=>parseMarkers(text.replaceAll('\n',crlf?'\r\n':'\n'));
    const range=(text,name)=>{const m=document(text).markers[name];return [m.start.line,m.start.character,m.end.line,m.end.character];};
    assert.deepEqual(range(spec.files['scripts/defs.vela'],'decl'),[0,17,0,23]);
    assert.deepEqual(range(spec.files['scripts/main.vela'],'call'),[1,22,1,28]);
    assert.deepEqual(range(spec.oracle.shifted,'decl'),[2,17,2,23]);
    assert.deepEqual(range(spec.oracle.overlay,'decl'),[1,17,1,23]);
    assert.deepEqual(range(spec.files['other/defs.vela'],'decl'),[2,7,2,13]);
    for(const [name,start,end] of [['value',16,21],['gone',28,32],['rank',39,43]])
      assert.deepEqual(range(spec.files['scripts/schema_caller.vela'],name),[1,start,1,end]);
    const invalid=document(spec.oracle.configInvalid),bad=invalid.markers.bad;
    assert.deepEqual([bad.start.byte,bad.end.byte],crlf?[93,102]:[88,97]);
    assert.equal(Buffer.from(invalid.text).subarray(bad.start.byte,bad.end.byte).toString(),'"scripts"');
  }
});
