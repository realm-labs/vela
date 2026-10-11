"use strict";
const assert=require("node:assert/strict");
const {gutterFoldModel,render,visibleRanges}=require("./gutter-fold-contracts");
const spec=require("../../tests/lsp_matrix/fixtures/input-keyboard-fold.json");

function keyboardFoldModel(){
  const model=gutterFoldModel();assert.deepEqual(spec.forms,model.spec.forms);
  return{spec,cases:model.cases.map(c=>({...c,id:`keyboard-${c.form}`}))};
}
function sourceState(v,line=0,collapsed=null){
  const point={line,character:0};
  return{file:v.file,text:v.text,dirty:false,focused:true,selections:[{anchor:point,active:point}],
    visibleRanges:visibleRanges(v.text,collapsed?[collapsed]:[])};
}
function keyboardFoldContracts(requirements,platform="darwin"){
  assert(["darwin","win32"].includes(platform));
  if(!requirements.some(r=>r.id.startsWith("vscode/UX12/")))return[];
  const model=keyboardFoldModel(),id="ux12-keyboard-fold",keys=spec.oracle.keys[platform],actions=[],checks=[],artifacts=["trace.json",id+"-observations.json"];
  const check=(id,level,expected)=>checks.push({id,level,expected});
  const shot=id=>artifacts.push(id+".png",id+".aria.txt");
  const key=(id,key)=>actions.push({id,device:"keyboard",key});
  const position=(prefix,v,line)=>{
    key(prefix+"-home",keys.home);
    for(let i=0;i<line;i++)key(prefix+"-down-"+i,"ArrowDown");
    check(prefix+"-position","Input",sourceState(v,line));
  };
  for(const c of model.cases){
    for(const file of ["workbench.log","lsp-trace.log","extension-host.log","server-trace.jsonl","setup.json","bridge.json"])artifacts.push(`picker-${c.id}/${file}`);
    check(c.id+"-profile","Input",{platform,case:c.id,version:"1.137.0",locale:"en",settings:true,display:true,installedServer:true,normalWindow:true,roots:["scripts"]});
    for(const v of c.views){
      const prefix=c.id+"-"+v.id;
      check(prefix+"-wire","Input",{result:v.wire});
      check(prefix+"-expanded","Render",render(v.text,v.ranges));shot(id+"-"+prefix+"-expanded");
      check(prefix+"-source","Input",sourceState(v));
      for(const [i,r]of v.ranges.entries()){
        const p=prefix+"-"+i;position(p,v,r.start);
        for(const stage of ["collapse","expand","expanded-noop"]){
          key(p+"-"+stage,stage==="collapse"?keys.collapse:keys.expand);
          const hidden=stage==="collapse"?r:null;
          check(p+"-"+stage+"-render","Render",render(v.text,v.ranges,hidden));
          check(p+"-"+stage+"-source","Input",sourceState(v,r.start,hidden));shot(id+"-"+p+"-"+stage);
        }
      }
      if(!v.ranges.length){
        const p=prefix+"-empty";position(p,v,0);
        for(const stage of ["collapse","expand"]){
          key(p+"-"+stage,keys[stage]);
          check(p+"-"+stage+"-render","Render",render(v.text,v.ranges));
          check(p+"-"+stage+"-source","Input",sourceState(v));shot(id+"-"+p+"-"+stage);
        }
      }
    }
    check(c.id+"-disk","Input",c.files);check(c.id+"-clean-exit","Input",{code:0,signal:null});
  }
  return[{id,fixture:spec.id,deadlineMs:spec.oracle.aggregateBudgetMs,actions,checks,artifacts,requirements:["input","render"].map(level=>{
    const id=`vscode/UX12/keyboard-fold/${level}/local`,r=requirements.find(r=>r.id===id);assert(r,id);return{id,contractHash:r.contractHash};
  })}];
}
module.exports={keyboardFoldModel,sourceState,keyboardFoldContracts};
