"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const {visibleRanges,workbenchRanges,render,gutterFoldModel,gutterFoldContracts}=require("./gutter-fold-contracts");
const requirements=require("./inventory").loadInventory(require("node:path").resolve(__dirname,"../..")).executionRequirements;
test("gutter corpus preserves all authored Unicode forms, duplicates on wire and empty controls",()=>{
  const m=gutterFoldModel();assert.equal(m.cases.length,4);assert(m.cases.every(c=>c.views.length===10));
  assert.equal(m.cases.flatMap(c=>c.views).reduce((n,v)=>n+v.publicRanges.length,0),60);
  assert.equal(m.cases.flatMap(c=>c.views).reduce((n,v)=>n+v.ranges.length,0),56);
  assert.equal(m.cases.flatMap(c=>c.views).filter(v=>!v.ranges.length).length,12);
  const lf=m.cases[0].views.find(v=>v.id==="if-statement-branches");
  assert.deepEqual(lf.publicRanges,[{start:0,end:6,kind:"region"},{start:1,end:3,kind:"region"},{start:3,end:5,kind:"region"}]);
  assert.deepEqual(lf.ranges,lf.publicRanges.slice(0,2));
  assert.equal(lf.wire.length,4);assert.equal(lf.wire[0].startCharacter,8);assert(lf.wire[1].startCharacter>8);
  assert.deepEqual(m.cases[2].views.find(v=>v.id===lf.id).ranges,lf.ranges);
  assert.deepEqual(m.cases[1].views.find(v=>v.id===lf.id).ranges,lf.ranges.map(r=>({...r,start:r.start+2,end:r.end+2})));
});
test("gutter visibility hides inclusive body lines and nested controls without touching UTF16 text boundaries",()=>{
  const text="中😀\r\na\r\nbb\r\n} xx\r\n",ranges=[{start:0,end:3},{start:1,end:2}];
  assert.deepEqual(visibleRanges(text),[{start:{line:0,character:0},end:{line:4,character:0}}]);
  assert.deepEqual(visibleRanges(text,[ranges[1]]),[{start:{line:0,character:0},end:{line:1,character:1}},{start:{line:3,character:0},end:{line:4,character:0}}]);
  assert.deepEqual(visibleRanges(text,[ranges[0]]),[{start:{line:0,character:0},end:{line:0,character:3}}]);
  assert.deepEqual(render(text,ranges,ranges[0]),{visibleLines:[0,4],controls:[{line:0,icon:"folding-collapsed"}]});
});
test("gutter contract visits every public range with physical collapse/expand and full read-only observations",()=>{
  const c=gutterFoldContracts(requirements,"win32")[0];assert.equal(c.actions.length,112);assert.equal(c.checks.length,356);
  assert.equal(c.checks.filter(c=>c.id.endsWith("-wire")).length,40);assert.equal(c.artifacts.filter(f=>f.endsWith(".png")).length,152);
  assert.equal(c.deadlineMs,480000);assert(c.actions.every(a=>a.device==="pointer"&&a.clickCount===1));
  for(const entries of [c.actions,c.checks,c.artifacts])assert.equal(new Set(entries.map(r=>r.id??r)).size,entries.length);
  assert.equal(c.requirements.length,2);
});
test("workbench sanitization rejects only crossing rows and retains complete nested or disjoint intervals",()=>{
  const rows=[{start:0,end:6},{start:1,end:3},{start:3,end:5},{start:6,end:8},{start:9,end:11}];
  assert.deepEqual(workbenchRanges(rows),[rows[0],rows[1],rows[4]]);
  assert.deepEqual(workbenchRanges([{start:0,end:4},{start:1,end:4},{start:2,end:3}]),[{start:0,end:4},{start:1,end:4},{start:2,end:3}]);
});
