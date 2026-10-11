"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const oracle=require("./selection-editor-oracle");
const {selectionInputModel,transitions,offsetAt,rangeText,paintedRows,selectionInputContracts}=require("./selection-input-contracts");
const requirements=require("./inventory").loadInventory(require("node:path").resolve(__dirname,"../..")).executionRequirements;
test("native selection retains all105 authored cases and480 positions in four forms and original helper families",()=>{
  const m=selectionInputModel(),views=m.cases.flatMap(c=>c.views),queries=views.flatMap(v=>v.queries);
  assert.equal(m.cases.length,40);assert.equal(new Set(m.cases.map(c=>c.fixture)).size,10);
  assert.equal(views.length,420);assert.equal(queries.length,1920);assert.equal(queries.reduce((n,q)=>n+q.states.length,0),29404);
  assert.equal(m.cases.reduce((n,c)=>n+Object.keys(c.files).length,0),460);
  assert.equal(views.filter(v=>!v.queries.length).length,4);assert.equal(new Set(views.map(v=>v.ref)).size,105);
  assert(m.cases.every(c=>c.windowBudgetMs===480000));
  for(const c of m.cases)for(const v of c.views){assert.equal(c.files[v.file],v.text);for(const q of v.queries)assert(q.point.character<=v.text.split(/\r?\n/)[q.point.line].length);}
});
test("native smart-select walks every literal public parent then reverses exactly to its original caret",()=>{
  const {item}=oracle.resolve({fixture:"selection-trivia",case:"baseline-field-trivia"});
  const doc=oracle.document(item.source),q=item.queries.find(q=>q.chain.length),t=transitions(doc,q);
  const flat=r=>[r.start.line,r.start.character,r.end.line,r.end.character];
  const spans=[[1,13,1,18],[1,9,1,18],[1,2,1,21],[1,0,1,21],[0,20,2,3],[0,8,2,3],[0,0,2,3],[0,0,3,0]];
  assert.deepEqual(t.states.slice(0,8).map(s=>flat(s.range)),spans);assert.equal(t.states.length,18);
  assert.deepEqual(flat(t.states[8].range),spans.at(-1));
  assert.deepEqual(t.states.slice(9,16).map(s=>flat(s.range)),spans.slice(0,-1).toReversed());
  assert.deepEqual(t.states.slice(-2).map(s=>flat(s.range)),[[1,14,1,14],[1,14,1,14]]);
  const empty=oracle.resolve({fixture:"selection-trivia",case:"empty"}).item;
  assert.deepEqual(transitions(oracle.document(empty.source),empty.queries[0]).states.map(s=>s.id),["expand-cap","shrink-cap"]);
});
test("native controller skips equal adjacent public whitespace ranges in both directions",()=>{
  const {item}=oracle.resolve({fixture:"selection-trivia",case:"mixed-whitespace-only"});
  const t=transitions(oracle.document(item.source),item.queries[0]);
  const flat=r=>[r.start.line,r.start.character,r.end.line,r.end.character];
  assert.deepEqual(t.publicRanges.map(flat),[[0,1,0,1],[0,0,0,3],[0,0,0,3],[0,0,2,3]]);
  assert.deepEqual(t.states.map(s=>[s.id,...flat(s.range)]),[
    ["expand-0",0,0,0,3],["expand-1",0,0,2,3],["expand-cap",0,0,2,3],
    ["shrink-0",0,0,0,3],["shrink-1",0,1,0,1],["shrink-cap",0,1,0,1]
  ]);
  assert.deepEqual(t.wire,[{range:{start:{line:0,character:1},end:{line:0,character:1}}}]);
});
test("native empty-source caret uses a fresh private-window controller before other family documents",()=>{
  const m=selectionInputModel(),trivia=m.cases.filter(c=>c.fixture==="selection-trivia");
  assert.equal(trivia.length,4);
  for(const c of trivia){
    assert.equal(c.views[0].ref,"selection-trivia/empty");
    assert.equal(c.views.length,16);assert.equal(c.views.filter(v=>v.ref==="selection-trivia/empty").length,1);
    assert(c.views[0].queries.every(q=>q.initial.start.line===q.point.line));
    if(c.form.endsWith("original"))assert.equal(c.views[0].text,"");
  }
});
test("native painted rows and textbox normalization retain tabs, astral UTF16 and newline boundaries",()=>{
  const text="中😀\r\n\tword\r\n",r={start:{line:0,character:1},end:{line:1,character:2}};
  assert.equal(rangeText(text,r),"😀\r\n\tw");assert.deepEqual(paintedRows(r),[0,1]);
  assert.equal(offsetAt(text,r.end),7);assert.equal(offsetAt("中😀\n\tword\n",r.end),6);
  assert.deepEqual(paintedRows({start:{line:0,character:3},end:{line:1,character:0}}),[0]);
  assert.deepEqual(paintedRows({start:{line:1,character:0},end:{line:1,character:0}}),[]);
});
test("native selection contracts retain complete physical routes, all states, finite budgets and exact scope",()=>{
  for(const platform of ["win32","darwin"]){
    const c=selectionInputContracts(requirements,platform)[0];assert.equal(c.actions.length,40924);assert.equal(c.checks.length,67448);
    assert.equal(c.artifacts.filter(f=>f.endsWith(".png")).length,6178);assert.equal(c.deadlineMs,19200000);
    assert(c.actions.every(a=>a.device==="keyboard"));assert.equal(c.checks.filter(c=>c.id.endsWith("-wire")).length,1920);
    assert.equal(c.checks.filter(c=>c.id.endsWith("-expand-cap-render")).length,1920);assert.equal(c.checks.filter(c=>c.id.endsWith("-shrink-cap-render")).length,1920);
    const targets=c.actions.filter(a=>a.id.endsWith("-target")&&!a.id.endsWith("-select-target"));
    assert.equal(targets.length,1920);assert(targets.every(a=>/^::\d+$/.test(a.text)));
    assert.equal(targets[0].text,"::47");
    assert(c.actions.filter(a=>a.id.endsWith("-expand-cap")).every(a=>a.key===(platform==="win32"?"Alt+Shift+ArrowRight":"Control+Meta+Shift+ArrowRight")));
    assert.deepEqual(c.requirements.map(r=>r.id),["vscode/UX12/selection-expand-shrink/input/local","vscode/UX12/selection-expand-shrink/render/local"]);
    for(const rows of [c.actions,c.checks,c.artifacts])assert.equal(new Set(rows.map(r=>r.id??r)).size,rows.length);
  }
});
