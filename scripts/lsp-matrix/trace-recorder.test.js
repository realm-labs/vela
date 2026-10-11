"use strict";
const fs=require("node:fs"),os=require("node:os"),path=require("node:path"),assert=require("node:assert/strict"),test=require("node:test");
const {traceRecorder}=require("../../editors/vscode/test/input/trace-recorder");
const temporary=run=>{const root=fs.mkdtempSync(path.join(os.tmpdir(),"vela-native-trace-"));try{run(root);}finally{fs.rmSync(root,{recursive:true,force:true});}};
test("ordinary native trace still snapshots every unchanged receipt immediately",()=>temporary(root=>{
  const r=traceRecorder(root,"default");r.record("input","key",{proof:"owned",device:"keyboard",key:"Enter"});
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(root,"trace.json"))),r.trace);
  assert.equal(r.trace[0].proof,"owned");assert.equal(r.trace[0].kind,"input");assert(Number.isFinite(Date.parse(r.trace[0].at)));
}));
test("large native trace journals all Unicode receipts and flushes complete ordered snapshots",()=>temporary(root=>{
  const r=traceRecorder(root,"default");r.record("precondition","setup",{});r.startJournal("owned.jsonl");
  for(let i=0;i<100;i++)r.record(i%2?"assertion":"input","event-"+i,{proof:"select",text:"中😀\r\n"});
  const rows=fs.readFileSync(path.join(root,"owned.jsonl"),"utf8").trimEnd().split("\n").map(s=>JSON.parse(s));
  assert.deepEqual(rows,r.trace.slice(1));assert.equal(JSON.parse(fs.readFileSync(path.join(root,"trace.json"))).length,1);
  r.flush();assert.deepEqual(JSON.parse(fs.readFileSync(path.join(root,"trace.json"))),r.trace);
  r.endJournal();r.record("assertion","clean-exit",{});assert.deepEqual(JSON.parse(fs.readFileSync(path.join(root,"trace.json"))),r.trace);
}));
test("native journal retains failure receipts and rejects nested or escaping destinations",()=>temporary(root=>{
  const r=traceRecorder(root,"default");assert.throws(()=>r.startJournal("../outside.jsonl"));
  r.startJournal("owned.jsonl");assert.throws(()=>r.startJournal("nested.jsonl"));r.record("failure","stage",{error:"bounded failure"});r.endJournal();
  assert.equal(JSON.parse(fs.readFileSync(path.join(root,"trace.json")))[0].kind,"failure");assert.throws(()=>r.endJournal());
}));
