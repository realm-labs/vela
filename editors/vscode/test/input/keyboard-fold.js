"use strict";
const fs=require("node:fs"),path=require("node:path"),assert=require("node:assert/strict");
const {isDeepStrictEqual}=require("node:util");
const {keyboardFoldModel}=require("../../../../scripts/lsp-matrix/keyboard-fold-contracts");
const {foldingResponses}=require("../../../../scripts/lsp-matrix/folding-editor-trace");
const {gutterDOM}=require("./gutter-fold"),{fileUri,canonicalUri}=require("./paths");
const evidence=require("../../../../scripts/lsp-matrix/local-evidence");

async function runKeyboardFold({root,profile,executable,extensions,installedServer,contracts,record,until,onProof}){
  const contract=contracts.find(c=>c.id==="ux12-keyboard-fold");if(!contract)return;
  const model=keyboardFoldModel(),started=Date.now(),actions=[],checks=[],observations=[];
  const observe=(id,value)=>{observations.push({id,value});fs.writeFileSync(path.join(root,contract.id+"-observations.json"),JSON.stringify(observations,null,2));};
  const check=(id,value)=>{
    const d=contract.checks.find(c=>c.id===id);assert(d,id);observe(id,value);assert.deepEqual(value,d.expected,contract.id+"/"+id);
    checks.push({...d,status:"passed",observed:value});record("assertion",id,{proof:contract.id,expected:d.expected,observed:value});
  };
  for(const c of model.cases){
    const exit=await require("./workspace-picker-session").pickerWindow({root,profile,executable,extensions,installedServer,until,record,c,
      runCase:async({page,bridge,workspace,owned,profileObservation})=>{
        check(c.id+"-profile",profileObservation);let trace,currentFile;
        const shot=async id=>{await page.screenshot({path:path.join(root,contract.id+"-"+id+".png")});fs.writeFileSync(path.join(root,contract.id+"-"+id+".aria.txt"),await page.locator("body").ariaSnapshot());};
        const rendered=async id=>{
          const expected=contract.checks.find(c=>c.id===id).expected;
          const value=await until(id,async()=>{
            const dom=await gutterDOM(page,currentFile),value={visibleLines:dom.visibleLines,controls:dom.controls.map(({line,icon})=>({line,icon}))};
            return isDeepStrictEqual(value,expected)&&value;
          });check(id,value);
        };
        const source=async id=>{
          const expected=contract.checks.find(c=>c.id===id).expected;
          const value=await until(id,async()=>{
            const raw=await bridge("gutter-observe"),inspection=await bridge("inspect");
            if(raw.uri!==fileUri(path.join(workspace,expected.file))||inspection.active?.uri!==raw.uri)return false;
            const name=new RegExp("^"+path.posix.basename(currentFile).replace(/[.*+?^${}()|[\]\\]/g,"\\$&"));
            const focused=await page.locator(".monaco-editor").filter({has:page.getByRole("textbox",{name})}).evaluate(e=>e.contains(document.activeElement));
            const value={file:expected.file,text:raw.text,dirty:raw.dirty,focused,selections:inspection.active.selections,visibleRanges:raw.visibleRanges};
            return isDeepStrictEqual(value,expected)&&value;
          });check(id,value);
        };
        const key=async id=>{
          assert(Date.now()-started<contract.deadlineMs,"finite keyboard folding aggregate budget");
          const a=contract.actions.find(a=>a.id===id);assert(a,id);await page.keyboard.press(a.key);
          actions.push(a);record("input",id,{proof:contract.id,device:a.device,key:a.key});
        };
        const position=async(prefix,line)=>{
          await key(prefix+"-home");for(let i=0;i<line;i++)await key(prefix+"-down-"+i);
          await source(prefix+"-position");
        };
        for(const v of c.views){
          currentFile=v.file;const prefix=c.id+"-"+v.id,boundary=trace?fs.readFileSync(trace,"utf8").length:0;
          record("precondition",prefix+"-setup",{file:v.file,line:0,character:0});await bridge("setup",{file:v.file,line:0,character:0,reset:true});
          trace??=await until("keyboard folding actual client trace",()=>require("./logs").logFiles(owned,n=>n.endsWith("-Vela LSP Trace.log"))[0]);
          const pairs=await until(prefix+"-wire",()=>{
            const rows=foldingResponses(fs.readFileSync(trace,"utf8").slice(boundary)).filter(r=>canonicalUri(r.params.textDocument.uri)===fileUri(path.join(workspace,v.file)));
            return rows.length&&rows;
          });observe(prefix+"-pairs",pairs);
          for(const p of pairs){assert.deepEqual(p.params,{textDocument:{uri:p.params.textDocument.uri}});assert.deepEqual(p.result,v.wire,"complete fresh ordered folding response including []");}
          check(prefix+"-wire",{result:pairs[0].result});await rendered(prefix+"-expanded");await source(prefix+"-source");await shot(prefix+"-expanded");
          for(const [i,r]of v.ranges.entries()){
            const p=prefix+"-"+i;await position(p,r.start);
            for(const stage of ["collapse","expand","expanded-noop"]){
              const id=p+"-"+stage;await key(id);await rendered(id+"-render");await source(id+"-source");await shot(id);
            }
          }
          if(!v.ranges.length){
            const p=prefix+"-empty";await position(p,0);
            for(const stage of ["collapse","expand"]){const id=p+"-"+stage;await key(id);await rendered(id+"-render");await source(id+"-source");await shot(id);}
          }
        }
        check(c.id+"-disk",Object.fromEntries(Object.keys(c.files).map(f=>[f,fs.readFileSync(path.join(workspace,f),"utf8")])));
      }});
    check(c.id+"-clean-exit",exit);console.log("PASS native keyboard folding case "+c.id);
  }
  const finished=Date.now();onProof({id:contract.id,fixture:contract.fixture,contractHash:evidence.jsonHash(contract),status:"passed",durationMs:finished-started,
    startedAt:new Date(started).toISOString(),finishedAt:new Date(finished).toISOString(),actions,checks});console.log("PASS "+contract.id);
}
module.exports={runKeyboardFold};
