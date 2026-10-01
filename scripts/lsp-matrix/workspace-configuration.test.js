"use strict";
const test=require('node:test'),assert=require('node:assert/strict');
const {parseMarkers}=require('./fixtures');
const spec=require('../../tests/lsp_matrix/fixtures/workspace-configuration.json');
test('workspace config fixtures pin independent Unicode targets and cross-root import goldens',()=>{
  for(const crlf of [false,true]){
    const document=text=>parseMarkers(text.replaceAll('\n',crlf?'\r\n':'\n'));
    const range=(file,name)=>{const m=document(spec.files[file]).markers[name];return [m.start.line,m.start.character,m.end.line,m.end.character];};
    assert.deepEqual(range('roots/left/alpha.vela','decl'),[0,17,0,22]);
    assert.deepEqual(range('roots/right/beta.vela','decl'),[2,7,2,11]);
    assert.deepEqual(range('roots/left/caller.vela','call'),[1,27,1,32]);
    assert.deepEqual(range('roots/right/other_caller.vela','call'),[1,28,1,32]);
    assert.deepEqual(range('roots/left/cross.vela','alpha'),[2,28,2,33]);
    assert.deepEqual(range('roots/left/cross.vela','beta'),[2,37,2,41]);
    assert.deepEqual(range('roots/left/cross.vela','import-alpha'),[0,0,0,17]);
    assert.deepEqual(range('roots/left/cross.vela','import-beta'),[1,0,1,15]);
    for(const [name,start,end] of [['value',17,22],['gone',29,33],['rank',40,44]])
      assert.deepEqual(range('roots/left/schema_caller.vela',name),[1,start,1,end]);
    assert.deepEqual(JSON.parse(spec.files['schema-one.json']).facts.fields.map(f=>[f.name,f.fact.name]),[['value','i64'],['gone','bool']]);
    assert.deepEqual(JSON.parse(spec.files['schema-two.json']).facts.fields.map(f=>[f.name,f.fact.name]),[['value','string'],['rank','bool']]);
    assert.equal(document(spec.oracle.overlay).markers.decl.start.line,1);
    assert.deepEqual(spec.oracle.importNameRange,{start:{line:0,character:11},end:{line:0,character:16}});
    for(const file of ['roots/left/caller.vela','roots/left/cross.vela'])
      assert.equal(document(spec.files[file]).text.split(/\r?\n/)[0].slice(11,16),'alpha');
    for(const file of Object.values(spec.files))assert.doesNotThrow(()=>document(file));
  }
});
