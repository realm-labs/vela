"use strict";
const fs=require("node:fs"),path=require("node:path"),assert=require("node:assert/strict");
const {isDeepStrictEqual}=require("node:util");
const {workspacePickerModel}=require("../../../../scripts/lsp-matrix/workspace-picker-contracts");
const {workspaceSymbolResponses}=require("../../../../scripts/lsp-matrix/workspace-symbol-trace");
const {orderedWorkspaceTies}=require("../../../../scripts/lsp-matrix/workspace-picker-wire");
const evidence=require("../../../../scripts/lsp-matrix/local-evidence");
const {fileUri,canonicalUri,relativeFile}=require("./paths");
const {readTrace,providerWatermark,workspaceReadiness,completedWorkerProviderRequest}=require("./readiness");

async function runWorkspacePicker({root,profile,executable,extensions,installedServer,contracts,record,until,onProof}){
  const ownedContracts=contracts.filter(c=>["ux11-workspace-picker","ux11-no-match"].includes(c.id));
  if(!ownedContracts.length)return;
  assert.equal(ownedContracts.length,2,"capture workspace picker and no-match prerequisites together");
  const model=workspacePickerModel(profile.platform),started=Date.now();
  const proofs=Object.fromEntries(ownedContracts.map(contract=>[contract.id,{contract,actions:[],checks:[],observations:[]} ]));
  const observe=(p,id,value)=>{
    p.observations.push({id,value});fs.writeFileSync(path.join(root,p.contract.id+"-observations.json"),JSON.stringify(p.observations,null,2));
  };
  const check=(p,id,value)=>{
    const definition=p.contract.checks.find(c=>c.id===id);assert(definition,`registered picker assertion ${id}`);
    observe(p,id,value);assert.deepEqual(value,definition.expected,p.contract.id+"/"+id);
    p.checks.push({...definition,status:"passed",observed:value});record("assertion",id,{proof:p.contract.id,expected:definition.expected,observed:value});
  };
  for(const c of model.cases){
    const result=await require("./workspace-picker-session").pickerWindow({root,profile,executable,extensions,installedServer,until,record,c,
      runCase:async({page,bridge,workspace,owned,profileObservation})=>{
        const positive=proofs["ux11-workspace-picker"],negative=proofs["ux11-no-match"],widget=page.locator(".quick-input-widget");
        const focused=widget.locator(".quick-input-list .monaco-list-row.focused");
        for(const p of [positive,negative])check(p,c.id+"-profile",profileObservation);
        const action=async(p,id)=>{
          assert(Date.now()-started<p.contract.deadlineMs,"finite aggregate picker budget");
          const item=p.contract.actions.find(a=>a.id===id);assert(item,`registered picker input ${id}`);
          if(item.device==="pointer")await focused.locator(".label-name").click();
          else if(item.text!==undefined)await page.keyboard.type(item.text);else await page.keyboard.press(item.key);
          p.actions.push(item);record("input",id,{proof:p.contract.id,...Object.fromEntries(Object.entries(item).filter(([k])=>k!=="id"))});
        };
        const source=async(p,id)=>{
          const expected=p.contract.checks.find(c=>c.id===id).expected;
          check(p,id,await until(id,async()=>{
            const a=(await bridge("inspect")).active;if(a?.uri!==fileUri(path.join(workspace,expected.file)))return false;
            const value={file:expected.file,text:a.text,dirty:a.dirty,selections:a.selections,
              ...(Object.hasOwn(expected,"pickerVisible")?{pickerVisible:await widget.isVisible()}: {})};
            return isDeepStrictEqual(value,expected)&&value;
          }));
        };
        const row=async(p,id,count=false)=>{
          const expected=p.contract.checks.find(c=>c.id===id).expected;
          const value=await until(id,async()=>{
            const locator=count?widget.locator(".quick-input-list .monaco-list-row"):focused;
            if(await locator.count()!==1||!await locator.isVisible())return false;
            const raw=await locator.evaluate(e=>({name:(e.querySelector(".label-name")?.textContent??"").trim(),
              description:(e.querySelector(".label-description")?.textContent??"").trim(),
              icon:Array.from(e.querySelector(".codicon[class*='codicon-symbol-']")?.classList??[]).find(x=>x.startsWith("codicon-symbol-"))?.replace("codicon-","")??""}));
            const value={...raw,...(count?{count:await locator.count()}: {})};return isDeepStrictEqual(value,expected)&&value;
          });check(p,id,value);
        };
        const snapshot=async id=>{
          await page.screenshot({path:path.join(root,id+".png")});fs.writeFileSync(path.join(root,id+".aria.txt"),await page.locator("body").ariaSnapshot());
        };
        record("precondition","picker-source-setup-"+c.id,{startup:c.startup,document:c.document});
        const since=Date.now();await bridge("setup",{file:c.startup,line:0,character:0,reset:true});
        const afterSeq=providerWatermark(readTrace(workspace));
        await bridge("setup",{file:c.document,line:0,character:0,reset:true});
        await until("picker initial mutations complete",()=>workspaceReadiness(readTrace(workspace),{since,now:Date.now()}));
        const trace=await until("picker actual client log",()=>require("./logs").logFiles(owned,n=>n.endsWith("-Vela LSP Trace.log"))[0]);
        const text=()=>fs.readFileSync(trace,"utf8");
        const wire=async(p,id,boundary)=>{
          const expected=p.contract.checks.find(c=>c.id===id).expected;
          const pairs=await until(id,()=>{
            const rows=workspaceSymbolResponses(text().slice(boundary)).filter(r=>r.params.query===expected.query);
            return rows.length&&rows;
          });assert.equal(pairs.length,1,"one fresh complete request for each literal final query");
          const pair=pairs[0];observe(p,id+"-pair",pair);
          assert.deepEqual(pair.params,{query:expected.query});assert(Array.isArray(pair.result),"explicit complete workspace result including []");
          const result=pair.result.map(r=>({...r,location:{...r.location,uri:r.location.uri.startsWith("file:")?
            relativeFile(workspace,require("node:url").fileURLToPath(canonicalUri(r.location.uri))):r.location.uri}}));
          check(p,id,{query:pair.params.query,result:orderedWorkspaceTies(result)});
        };
        const query=async(p,id,wireId)=>{
          const boundary=text().length;await action(p,id+"-select");await action(p,id+"-type");
          if(wireId)await wire(p,wireId,boundary);
        };
        await action(negative,c.id+"-document-open");await widget.waitFor({state:"visible"});
        await query(negative,c.id+"-document-positive");
        const response=await until("fresh document-symbol worker response",()=>completedWorkerProviderRequest(readTrace(workspace),{
          afterSeq,method:"textDocument/documentSymbol",matchesDocument:uri=>canonicalUri(uri)===fileUri(path.join(workspace,c.document))}));
        observe(negative,c.id+"-document-response",response);check(negative,c.id+"-document-request",{method:"textDocument/documentSymbol",completed:true});
        await row(negative,c.id+"-document-positive",true);
        await query(negative,c.id+"-document-absent");await row(negative,c.id+"-document-empty",true);
        await action(negative,c.id+"-document-enter");await source(negative,c.id+"-document-retained");
        await snapshot(negative.contract.id+"-"+c.id+"-document");await action(negative,c.id+"-document-dismiss");await widget.waitFor({state:"hidden"});
        await action(negative,c.id+"-workspace-open");await widget.waitFor({state:"visible"});
        for(const q of c.absent){
          const id=c.id+"-"+q.id;await query(negative,id,id+"-wire");await row(negative,id+"-empty",true);
          await source(negative,id+"-source");await snapshot(negative.contract.id+"-"+id);
        }
        await action(negative,c.id+"-workspace-dismiss");await widget.waitFor({state:"hidden"});await source(negative,c.id+"-final-source");
        await action(positive,c.id+"-open");await widget.waitFor({state:"visible"});
        await query(positive,c.id+"-reset",c.id+"-reset-wire");await query(positive,c.id+"-all",c.id+"-all-wire");
        await action(positive,c.id+"-home");
        for(const [i]of c.rows.entries()){
          if(i)await action(positive,c.id+`-${i}-next`);await row(positive,c.id+`-${i}-row`);
        }
        await action(positive,c.id+"-end");await row(positive,c.id+"-tail");await action(positive,c.id+"-wrap");await row(positive,c.id+"-head");
        await snapshot(positive.contract.id+"-"+c.id+"-all");
        for(const t of c.targets){
          const id=c.id+"-"+t.id;
          await query(positive,id+"-reset",id+"-reset-wire");await query(positive,id+"-query",id+"-wire");
          await action(positive,id+"-home");for(let n=0;n<t.index;n++)await action(positive,id+`-next-${n}`);
          await row(positive,id+"-row");
          const expected=positive.contract.checks.find(c=>c.id===id+"-before").expected;
          const uri=fileUri(path.join(workspace,t.file));
          check(positive,id+"-before",{file:t.file,everOpened:readTrace(workspace).some(r=>r.event==="message_received"&&r.lane==="main"&&
            r.method==="textDocument/didOpen"&&canonicalUri(r.documentUri)===uri),
            ...(Object.hasOwn(expected,"open")?{open:(await bridge("inspect")).documents.some(d=>d.uri===uri)}: {})});
          await action(positive,id+"-accept");await widget.waitFor({state:"hidden"});await source(positive,id+"-destination");
          await snapshot(positive.contract.id+"-"+id+"-destination");await action(positive,id+"-reopen");await widget.waitFor({state:"visible"});
        }
        await action(positive,c.id+"-dismiss");await widget.waitFor({state:"hidden"});
        check(positive,c.id+"-disk",Object.fromEntries(Object.keys(c.files).map(f=>[f,fs.readFileSync(path.join(workspace,f),"utf8")])));
      }});
    for(const p of Object.values(proofs))check(p,c.id+"-clean-exit",result);
    console.log(`PASS native workspace picker case ${c.id}`);
  }
  const finished=Date.now();for(const p of Object.values(proofs)){
    onProof({id:p.contract.id,fixture:p.contract.fixture,contractHash:evidence.jsonHash(p.contract),status:"passed",durationMs:finished-started,
      startedAt:new Date(started).toISOString(),finishedAt:new Date(finished).toISOString(),actions:p.actions,checks:p.checks});
    console.log(`PASS ${p.contract.id}`);
  }
}
module.exports={runWorkspacePicker};
