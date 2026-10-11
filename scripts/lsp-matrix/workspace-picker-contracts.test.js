"use strict";
const test=require("node:test"),assert=require("node:assert/strict"),path=require("node:path");
const {workspacePickerModel,workspacePickerContracts}=require("./workspace-picker-contracts");
const {orderedWorkspaceTies}=require("./workspace-picker-wire");
const {localContracts}=require("./local-contracts");
const requirements=require("./inventory").loadInventory(path.resolve(__dirname,"../..")).executionRequirements;
test("native workspace wire retains complete fields, multiplicity and non-tie order",()=>{
  const rows=workspacePickerModel().cases[0].wire("");const changed=structuredClone(rows);
  [changed[1],changed[2]]=[changed[2],changed[1]];assert.deepEqual(orderedWorkspaceTies(changed),rows);
  for(const mutate of [r=>r.splice(1,1),r=>r.push(r[1]),r=>{r[1].data.detail="wrong";},r=>{r[1].location.range.end.line+=1;},
    r=>{r[1].unknown="must fail";},r=>{[r[0],r[1]]=[r[1],r[0]];}]){
    const actual=structuredClone(rows);mutate(actual);assert.notDeepEqual(orderedWorkspaceTies(actual),rows);
  }
  for(const value of [null,undefined,{error:"not a response"}])assert.throws(()=>orderedWorkspaceTies(value),/explicit/);
});
test("workspace picker keeps complete original source/schema names and absent source locations",()=>{
  const m=workspacePickerModel("win32");assert.deepEqual(m.cases.map(c=>[c.rows.length,c.targets.length,c.absent.length]),[[31,31,12],[60,41,27],[31,31,12],[60,41,27]]);
  assert.equal(m.cases.reduce((n,c)=>n+c.rows.length,0),182);assert.equal(m.cases.reduce((n,c)=>n+c.targets.length,0),144);
  const d=m.cases[0],limit=d.wire("").find(r=>r.name==="api::LIMIT");
  assert.deepEqual(limit,{name:"api::LIMIT",kind:14,containerName:"api",location:{uri:"scripts/api.vela",range:{start:{line:1,character:8},end:{line:1,character:33}}},data:{detail:"i64"}});
  assert.deepEqual(m.cases[2].wire(""),d.wire(""));assert(m.cases.filter(c=>c.form==="crlf").every(c=>Object.entries(c.files).filter(([f])=>f.endsWith(".vela")).every(([,t])=>!/(?<!\r)\n/.test(t))));
  const o=m.cases[1];assert.equal(o.wire("").filter(r=>r.location.uri==="vela-schema:").length,19);
  assert(o.wire("").filter(r=>r.location.uri==="vela-schema:").every(r=>!Object.hasOwn(r.location,"range")));
  assert.deepEqual(o.rows.filter(r=>r.view.name==="source::make").map(r=>r.view.description),["source • scripts\\source.vela","."]);
  assert.deepEqual(o.rows.filter(r=>r.view.name==="source::Widget").map(r=>r.view.icon),["symbol-class","symbol-struct"]);
  assert.equal(o.targets.find(r=>r.id==="source-widget").index,1);assert.equal(d.targets.find(r=>r.id==="nested-api-file").index,1);
  assert(m.cases.every(c=>c.absent.every(q=>c.wire(q.query).length===0)));
  const before=structuredClone(require("../../tests/lsp_matrix/fixtures/workspace-symbol-ownership.json"));workspacePickerModel();assert.deepEqual(require("../../tests/lsp_matrix/fixtures/workspace-symbol-ownership.json"),before);
});
test("workspace picker owns every row, source target, cold file and complete empty partition",()=>{
  const [p,n]=workspacePickerContracts(requirements,"win32");
  assert.deepEqual([p.id,n.id],["ux11-workspace-picker","ux11-no-match"]);
  assert.equal(p.checks.filter(c=>/-\d+-row$/.test(c.id)).length,182);assert.equal(p.checks.filter(c=>c.id.endsWith("-destination")).length,144);
  assert.equal(p.checks.filter(c=>c.id.endsWith("-before")&&!c.expected.everOpened&&c.expected.open===false).length,16);
  assert.equal(n.checks.filter(c=>c.id.endsWith("-wire")).length,78);assert.equal(n.checks.filter(c=>c.id.endsWith("-document-request")).length,4);
  assert.equal(n.checks.filter(c=>c.id.endsWith("-document-retained")).length,4);
  for(const c of [p,n]){assert.equal(c.deadlineMs,480000);assert.equal(c.requirements.length,2);for(const list of [c.actions,c.checks,c.artifacts])assert.equal(new Set(list.map(r=>r.id??r)).size,list.length);}
  assert.equal(p.artifacts.filter(f=>f.endsWith("-destination.png")).length,144);assert.equal(n.artifacts.filter(f=>f.endsWith(".png")).length,82);
});
test("workspace picker binds per-platform physical keys and path presentation without changing prior contracts",()=>{
  for(const [platform,modifier]of [["win32","Control"],["darwin","Meta"]]){
    const cs=localContracts(requirements,require("../../tests/lsp_matrix/fixtures/input-driver.json"),platform);assert.equal(cs.length,54);
    const p=cs.find(c=>c.id==="ux11-workspace-picker"),n=cs.find(c=>c.id==="ux11-no-match");
    assert(p.actions.filter(a=>a.id.endsWith("-open")||a.id.endsWith("-reopen")).every(a=>a.key===modifier+"+t"));
    assert(n.actions.filter(a=>a.id.endsWith("-document-open")).every(a=>a.key===modifier+"+Shift+o"));
    const m=workspacePickerModel(platform),expected=platform==="win32"?"api • scripts\\api.vela":"api • scripts/api.vela";
    assert.equal(m.cases[0].rows.find(r=>r.id==="api-limit").view.description,expected);
    assert.deepEqual([...p.requirements,...n.requirements].map(r=>r.id),["vscode/UX11/workspace-picker/input/local","vscode/UX11/workspace-picker/render/local","vscode/UX11/no-match/input/local","vscode/UX11/no-match/render/local"]);
  }
});
