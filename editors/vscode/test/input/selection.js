"use strict";
const fs=require("node:fs"),path=require("node:path"),assert=require("node:assert/strict");
const {isDeepStrictEqual}=require("node:util");
const {selectionInputModel}=require("../../../../scripts/lsp-matrix/selection-input-contracts");
const {selectionTrace}=require("../../../../scripts/lsp-matrix/selection-editor-trace");
const {foldingResponses}=require("../../../../scripts/lsp-matrix/folding-editor-trace");
const {fileUri,canonicalUri}=require("./paths"),evidence=require("../../../../scripts/lsp-matrix/local-evidence");
function editor(page,file){
  const name=new RegExp("^"+path.posix.basename(file).replace(/[.*+?^${}()|[\]\\]/g,"\\$&"));
  return page.locator(".monaco-editor").filter({has:page.getByRole("textbox",{name})});
}
async function selectionDOM(page,file){
  return editor(page,file).evaluate(e=>{
    const input=e.querySelector('[role="textbox"]'),context=input?.editContext;
    const value=context?.text??input?.value,start=context?.selectionStart??input?.selectionStart,end=context?.selectionEnd??input?.selectionEnd;
    if(typeof value!=="string"||!Number.isInteger(start)||!Number.isInteger(end))throw Error("actual native textbox selection missing: "+JSON.stringify([...e.querySelectorAll('textarea,[role="textbox"]')].map(n=>({tag:n.tagName,class:n.className,editContext:!!n.editContext}))));
    const numbers=[...e.querySelectorAll(".margin-view-overlays .line-numbers")].map(n=>({line:Number(n.textContent.trim())-1,y:n.getBoundingClientRect().top+n.getBoundingClientRect().height/2}))
      .filter(n=>Number.isInteger(n.line)&&n.line>=0).sort((a,b)=>a.y-b.y);
    const rows=[...e.querySelectorAll(".view-overlays .selected-text")].flatMap(n=>{
      const b=n.getBoundingClientRect();if(b.width<=0||b.height<=0)return[];
      const y=b.top+b.height/2,nearest=numbers.toSorted((a,b)=>Math.abs(a.y-y)-Math.abs(b.y-y))[0];
      if(!nearest||Math.abs(nearest.y-y)>2)throw Error("painted selection has no source line");return[nearest.line];
    });
    return{visibleLines:numbers.map(n=>n.line),selectedText:value.slice(start,end).replaceAll("\r\n","\n"),paintedRows:[...new Set(rows)].sort((a,b)=>a-b)};
  });
}
async function runSelection({root,profile,executable,extensions,installedServer,contracts,record,until,onProof,onDocument}){
  const contract=contracts.find(c=>c.id==="ux12-selection-expand-shrink");if(!contract)return;
  const model=selectionInputModel(),started=Date.now(),actions=[],checks=[];
  const diagnosticLimit=Number(process.env.VELA_NATIVE_SELECTION_DIAGNOSTIC_DOCUMENTS??0);
  assert(Number.isSafeInteger(diagnosticLimit)&&diagnosticLimit>=0&&diagnosticLimit<=420,"bounded diagnostic document limit");let documents=0;
  const required=new Map(contract.checks.map(c=>[c.id,c])),inputs=new Map(contract.actions.map(a=>[a.id,a]));
  const observations=path.join(root,contract.id+"-observations.jsonl");assert(!fs.existsSync(observations));fs.writeFileSync(observations,"");
  const observe=(id,value)=>fs.appendFileSync(observations,JSON.stringify({id,value})+"\n");
  const check=(id,value)=>{
    const d=required.get(id);assert(d,id);observe(id,value);assert.deepEqual(value,d.expected,contract.id+"/"+id);
    checks.push({...d,status:"passed",observed:value});record("assertion",id,{proof:contract.id,expected:d.expected,observed:value});
  };
  for(const c of model.cases){
    const exit=await require("./workspace-picker-session").pickerWindow({root,profile,executable,extensions,installedServer,until,record,c,
      runCase:async({page,bridge,workspace,owned,profileObservation})=>{
        check(c.id+"-profile",profileObservation);let trace,currentFile;
        const shot=async id=>{await page.screenshot({path:path.join(root,contract.id+"-"+id+".png")});fs.writeFileSync(path.join(root,contract.id+"-"+id+".aria.txt"),await page.locator("body").ariaSnapshot());};
        const source=async id=>{
          const expected=required.get(id).expected;
          const value=await until(id,async()=>{
            const raw=await bridge("selection-observe");if(raw.uri!==fileUri(path.join(workspace,expected.file)))return false;
            const focused=await editor(page,currentFile).evaluate(e=>e.contains(document.activeElement));
            const value={file:expected.file,text:raw.text,dirty:raw.dirty,focused,selections:raw.selections,selectedTexts:raw.selectedTexts};
            return isDeepStrictEqual(value,expected)&&value;
          });check(id,value);
        };
        const rendered=async id=>{
          const expected=required.get(id).expected,value=await until(id,async()=>{
            const value=await selectionDOM(page,currentFile);return isDeepStrictEqual(value,expected)&&value;
          });check(id,value);
        };
        const key=async id=>{
          assert(Date.now()-started<contract.deadlineMs,"finite full selection aggregate budget");
          const a=inputs.get(id);assert(a,id);if(a.text!==undefined)await page.keyboard.type(a.text);else await page.keyboard.press(a.key);
          actions.push(a);record("input",id,{proof:contract.id,...Object.fromEntries(Object.entries(a).filter(([k])=>k!=="id"))});
        };
        const quickInput=()=>page.locator(".quick-input-widget input[type=text]");
        for(const v of c.views){
          currentFile=v.file;const prefix=c.id+"-"+v.id,openBoundary=trace?fs.readFileSync(trace,"utf8").length:0;
          record("precondition",prefix+"-setup",{file:v.file,line:0,character:0});await bridge("setup",{file:v.file,line:0,character:0,reset:true});
          trace??=await until("selection actual client trace",()=>require("./logs").logFiles(owned,n=>n.endsWith("-Vela LSP Trace.log"))[0]);
          const ready=await until(prefix+"-language-client-ready",()=>{
            const pairs=foldingResponses(fs.readFileSync(trace,"utf8").slice(openBoundary)).filter(row=>canonicalUri(row.params.textDocument.uri)===fileUri(path.join(workspace,v.file))&&Array.isArray(row.result));
            return pairs.length&&pairs;
          });observe(prefix+"-ready-folding-pairs",ready);
          record("precondition",prefix+"-language-client-ready",{uri:fileUri(path.join(workspace,v.file)),method:"textDocument/foldingRange",requestIds:ready.map(r=>r.id)});
          await source(prefix+"-baseline-source");await rendered(prefix+"-baseline-render");await shot(prefix+"-baseline");
          for(const q of v.queries){
            const p=prefix+"-"+q.id;
            await key(p+"-home");await key(p+"-end");await key(p+"-goto");
            const mode=await until(p+"-goto-mode",async()=>{
              const input=quickInput();if(!await input.isVisible())return false;
              const value={prefix:(await input.inputValue()).slice(0,1),focused:await input.evaluate(e=>e===document.activeElement)};
              return isDeepStrictEqual(value,required.get(p+"-goto-mode").expected)&&value;
            });check(p+"-goto-mode",mode);
            await key(p+"-select-target");await key(p+"-target");
            check(p+"-target-value",{value:await quickInput().inputValue(),focused:await quickInput().evaluate(e=>e===document.activeElement)});
            await key(p+"-accept");await source(p+"-position");
            trace??=await until("selection actual client trace",()=>require("./logs").logFiles(owned,n=>n.endsWith("-Vela LSP Trace.log"))[0]);
            const boundary=fs.readFileSync(trace,"utf8").length;
            for(const [i,s]of q.states.entries()){
              const id=p+"-"+s.id;await key(id);await source(id+"-source");await rendered(id+"-render");
              if(i===0){
                const pairs=await until(p+"-wire",()=>{
                  const rows=selectionTrace(fs.readFileSync(trace,"utf8").slice(boundary)).responses.filter(row=>canonicalUri(row.params.textDocument.uri)===fileUri(path.join(workspace,v.file))&&isDeepStrictEqual(row.params.positions,[q.point]));
                  return rows.length&&rows;
                });observe(p+"-pairs",pairs);
                for(const pair of pairs){assert.deepEqual(pair.params,{textDocument:{uri:pair.params.textDocument.uri},positions:[q.point]});assert.deepEqual(pair.result,q.wire,"complete fresh raw parent chain");}
                check(p+"-wire",{result:pairs[0].result});
              }
              if(q.shots.includes(s.id))await shot(id);
            }
          }
          onDocument();console.log("PASS native selection document "+c.id+"/"+v.ref);
          if(diagnosticLimit&&++documents>=diagnosticLimit)throw Error("diagnostic selection capture deliberately incomplete; partial proof cannot be accepted");
        }
        check(c.id+"-disk",Object.fromEntries(Object.keys(c.files).map(f=>[f,fs.readFileSync(path.join(workspace,f),"utf8")])));
      }});
    check(c.id+"-clean-exit",exit);console.log("PASS native selection window "+c.id);
  }
  const finished=Date.now();onProof({id:contract.id,fixture:contract.fixture,contractHash:evidence.jsonHash(contract),status:"passed",durationMs:finished-started,
    startedAt:new Date(started).toISOString(),finishedAt:new Date(finished).toISOString(),actions,checks});console.log("PASS "+contract.id);
}
module.exports={editor,selectionDOM,runSelection};
