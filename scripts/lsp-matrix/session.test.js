"use strict";
const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {sessionLog,readSession}=require('../../editors/vscode/test/input/session');
test('reload observations select the marked host output directory and reject escaping or ambiguous logs',()=>{
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'vela-session-'));
  try{
    const parent=path.join(root,'user-data/logs/session/exthost'),old=path.join(parent,'old-output'),fresh=path.join(parent,'new-output');
    fs.mkdirSync(old,{recursive:true});fs.mkdirSync(fresh);
    fs.writeFileSync(path.join(old,'2-Vela LSP Trace.log'),'old');fs.writeFileSync(path.join(fresh,'2-Vela LSP Trace.log'),'new');fs.writeFileSync(path.join(parent,'exthost.log'),'host');
    const s={pid:11,port:3456,token:'a'.repeat(64),logDirectory:fresh,hostLogDirectory:parent,mode:1};
    fs.writeFileSync(path.join(root,'bridge.json'),JSON.stringify(s));assert.deepEqual(readSession(root),s);
    assert.equal(fs.readFileSync(sessionLog(s,n=>n.endsWith('-Vela LSP Trace.log')),'utf8'),'new');
    assert.equal(fs.readFileSync(sessionLog(s,n=>n==='exthost.log'),'utf8'),'host');
    assert.equal(sessionLog(s,n=>n.endsWith('-Absent.log'),true),null);
    fs.writeFileSync(path.join(fresh,'3-Vela LSP Trace.log'),'ambiguous');assert.throws(()=>sessionLog(s,n=>n.endsWith('-Vela LSP Trace.log')));
    fs.writeFileSync(path.join(root,'bridge.json'),JSON.stringify({...s,mode:2}));assert.throws(()=>readSession(root));
    fs.writeFileSync(path.join(root,'bridge.json'),JSON.stringify({...s,logDirectory:os.tmpdir()}));assert.throws(()=>readSession(root));
  }finally{fs.rmSync(root,{recursive:true,force:true});}
});
