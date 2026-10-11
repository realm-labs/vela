"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const {keyboardFoldModel,sourceState,keyboardFoldContracts}=require("./keyboard-fold-contracts");
const requirements=require("./inventory").loadInventory(require("node:path").resolve(__dirname,"../..")).executionRequirements;

test("keyboard corpus retains every complete raw/public folding form and empty views",()=>{
  const m=keyboardFoldModel();assert.deepEqual(m.cases.map(c=>c.id),["keyboard-lf-original","keyboard-lf-shifted","keyboard-crlf-original","keyboard-crlf-shifted"]);
  const views=m.cases.flatMap(c=>c.views);assert.equal(views.length,40);assert.equal(views.filter(v=>!v.ranges.length).length,12);
  assert.equal(views.reduce((n,v)=>n+v.publicRanges.length,0),60);assert.equal(views.reduce((n,v)=>n+v.ranges.length,0),56);
  const branch=views.find(v=>v.id==="if-statement-branches");assert.equal(branch.wire.length,4);
  assert.deepEqual(branch.publicRanges,[{start:0,end:6,kind:"region"},{start:1,end:3,kind:"region"},{start:3,end:5,kind:"region"}]);
  assert.deepEqual(branch.ranges,branch.publicRanges.slice(0,2));
  for(const c of m.cases)for(const v of c.views)assert.equal(c.files[v.file],v.text);
});
test("keyboard source observation pins focus, exact caret and inclusive UTF16 visibility",()=>{
  const v={file:"scripts/sample.vela",text:"中😀\r\na\r\nb\r\n}\r\n"};
  assert.deepEqual(sourceState(v,0,{start:0,end:3}),{file:v.file,text:v.text,dirty:false,focused:true,
    selections:[{anchor:{line:0,character:0},active:{line:0,character:0}}],visibleRanges:[{start:{line:0,character:0},end:{line:0,character:3}}]});
  assert.deepEqual(sourceState(v,2).visibleRanges,[{start:{line:0,character:0},end:{line:4,character:0}}]);
  assert.deepEqual(sourceState(v,2).selections,[{anchor:{line:2,character:0},active:{line:2,character:0}}]);
});
test("keyboard acceptance records physical caret positioning and pinned platform shortcuts",()=>{
  for(const platform of ["win32","darwin"]){
    const c=keyboardFoldContracts(requirements,platform)[0],prefix="keyboard-lf-original-nested-array-literal-2";
    assert.equal(c.actions.length,416);assert.equal(c.checks.length,584);assert(c.actions.every(a=>a.device==="keyboard"));
    assert.deepEqual(c.actions.filter(a=>a.id.startsWith(prefix+"-")).map(a=>a.key),platform==="win32"
      ?["Control+Home","ArrowDown","ArrowDown","Control+Shift+[","Control+Shift+]","Control+Shift+]"]
      :["Meta+Home","ArrowDown","ArrowDown","Alt+Meta+[","Alt+Meta+]","Alt+Meta+]"]);
    assert.equal(c.checks.find(c=>c.id===prefix+"-position").expected.selections[0].active.line,2);
    assert.deepEqual(c.requirements.map(r=>r.id),["vscode/UX12/keyboard-fold/input/local","vscode/UX12/keyboard-fold/render/local"]);
    assert.equal(c.deadlineMs,480000);assert.equal(c.artifacts.filter(a=>a.endsWith(".png")).length,232);
    for(const entries of [c.actions,c.checks,c.artifacts])assert.equal(new Set(entries.map(r=>r.id??r)).size,entries.length);
  }
});
test("all empty and repeated-expand states retain whole source, caret and painted controls",()=>{
  const c=keyboardFoldContracts(requirements,"win32")[0];
  assert.equal(c.checks.filter(c=>c.id.endsWith("-wire")).length,40);
  assert.equal(c.checks.filter(c=>c.id.endsWith("-expanded-noop-render")).length,56);
  assert.equal(c.checks.filter(c=>c.id.endsWith("-empty-collapse-render")).length,12);
  for(const item of c.checks.filter(c=>c.id.endsWith("-expanded-noop-source")))assert.deepEqual(item.expected,c.checks.find(c=>c.id===item.id.replace("-expanded-noop-source","-expand-source")).expected);
  for(const item of c.checks.filter(c=>c.id.endsWith("-empty-collapse-source")))assert.deepEqual(item.expected,c.checks.find(c=>c.id===item.id.replace("-empty-collapse-source","-source")).expected);
  assert.deepEqual(c.checks.find(c=>c.id==="keyboard-lf-original-no-fold-line-comments-empty-collapse-render").expected,
    {visibleLines:[0,1,2,3],controls:[]});
});
