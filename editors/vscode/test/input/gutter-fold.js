"use strict";
const fs=require("node:fs"),path=require("node:path"),assert=require("node:assert/strict");
const {isDeepStrictEqual}=require("node:util");
const {gutterFoldModel}=require("../../../../scripts/lsp-matrix/gutter-fold-contracts");
const {foldingResponses}=require("../../../../scripts/lsp-matrix/folding-editor-trace");
const evidence=require("../../../../scripts/lsp-matrix/local-evidence");
const {fileUri,canonicalUri}=require("./paths");

// Read actual DOM geometry/labels only. Controls are joined to the visible
// source line numbers by their painted vertical position, never by expected rows.
async function gutterDOM(page,file){
  const name=new RegExp("^"+path.posix.basename(file).replace(/[.*+?^${}()|[\]\\]/g,"\\$&"));
  return page.locator(".monaco-editor").filter({has:page.getByRole("textbox",{name})}).evaluate(e=>{
    const nums=[...e.querySelectorAll(".margin-view-overlays .line-numbers")].map(n=>({
      line:Number(n.textContent.trim())-1,y:n.getBoundingClientRect().top+n.getBoundingClientRect().height/2
    })).filter(n=>Number.isInteger(n.line)&&n.line>=0).sort((a,b)=>a.y-b.y);
    const controls=[...e.querySelectorAll(".margin-view-overlays .codicon-folding-expanded, .margin-view-overlays .codicon-folding-collapsed")].map(n=>{
      const b=n.getBoundingClientRect(),y=b.top+b.height/2,nearest=nums.toSorted((a,b)=>Math.abs(a.y-y)-Math.abs(b.y-y))[0];
      if(!nearest||Math.abs(nearest.y-y)>2)throw Error("fold control has no painted source line");
      return{line:nearest.line,icon:n.classList.contains("codicon-folding-collapsed")?"folding-collapsed":"folding-expanded",x:b.left+b.width/2,y};
    }).sort((a,b)=>a.line-b.line);
    return{visibleLines:nums.map(n=>n.line),controls};
  });
}
async function runGutterFold({root,profile,executable,extensions,installedServer,contracts,record,until,onProof}){
  const contract=contracts.find(c=>c.id==="ux12-gutter-fold");if(!contract)return;
  const model=gutterFoldModel(),started=Date.now(),actions=[],checks=[],observations=[];
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
        const render=async id=>{
          const expected=contract.checks.find(c=>c.id===id).expected;
          const value=await until(id,async()=>{
            const dom=await gutterDOM(page,currentFile),value={visibleLines:dom.visibleLines,controls:dom.controls.map(({line,icon})=>({line,icon}))};
            return isDeepStrictEqual(value,expected)&&value;
          });check(id,value);
        };
        const source=async id=>{
          const expected=contract.checks.find(c=>c.id===id).expected;
          const value=await until(id,async()=>{
            const raw=await bridge("gutter-observe");if(raw.uri!==fileUri(path.join(workspace,expected.file)))return false;
            const value={file:expected.file,text:raw.text,dirty:raw.dirty,visibleRanges:raw.visibleRanges};return isDeepStrictEqual(value,expected)&&value;
          });check(id,value);
        };
        const click=async(id,line)=>{
          assert(Date.now()-started<contract.deadlineMs,"finite gutter aggregate budget");
          const a=contract.actions.find(a=>a.id===id);assert(a,id);
          const controls=(await gutterDOM(page,currentFile)).controls.filter(c=>c.line===line);assert.equal(controls.length,1,"exact source-line gutter target");
          const target=controls[0];observe(id+"-geometry",target);await page.mouse.click(target.x,target.y);
          actions.push(a);record("input",id,{proof:contract.id,...Object.fromEntries(Object.entries(a).filter(([k])=>k!=="id"))});
        };
        for(const v of c.views){
          currentFile=v.file;const prefix=c.id+"-"+v.id,boundary=trace?fs.readFileSync(trace,"utf8").length:0;
          record("precondition",prefix+"-setup",{file:v.file,line:0,character:0});
          await bridge("setup",{file:v.file,line:0,character:0,reset:true});
          trace??=await until("gutter actual client trace",()=>require("./logs").logFiles(owned,n=>n.endsWith("-Vela LSP Trace.log"))[0]);
          const pairs=await until(prefix+"-wire",()=>{
            const rows=foldingResponses(fs.readFileSync(trace,"utf8").slice(boundary)).filter(r=>canonicalUri(r.params.textDocument.uri)===fileUri(path.join(workspace,v.file)));
            return rows.length&&rows;
          });observe(prefix+"-pairs",pairs);
          for(const p of pairs){assert.deepEqual(p.params,{textDocument:{uri:p.params.textDocument.uri}});assert.deepEqual(p.result,v.wire,"complete fresh ordered folding response including []");}
          check(prefix+"-wire",{result:pairs[0].result});
          await render(prefix+"-expanded");await source(prefix+"-source");await shot(prefix+"-expanded");
          for(const [i,r]of v.ranges.entries())for(const stage of ["collapse","expand"]){
            const p=prefix+"-"+i+"-"+stage;await click(p,r.start);await render(p+"-render");await source(p+"-source");await shot(p);
          }
        }
        check(c.id+"-disk",Object.fromEntries(Object.keys(c.files).map(f=>[f,fs.readFileSync(path.join(workspace,f),"utf8")])));
      }});
    check(c.id+"-clean-exit",exit);console.log("PASS native gutter folding case "+c.id);
  }
  const finished=Date.now();onProof({id:contract.id,fixture:contract.fixture,contractHash:evidence.jsonHash(contract),status:"passed",durationMs:finished-started,
    startedAt:new Date(started).toISOString(),finishedAt:new Date(finished).toISOString(),actions,checks});console.log("PASS "+contract.id);
}
module.exports={gutterDOM,runGutterFold};
