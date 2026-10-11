"use strict";
const assert=require("node:assert/strict");
const folding=require("./folding-editor-oracle");
const spec=require("../../tests/lsp_matrix/fixtures/input-gutter-fold.json");
const corpus=require("../../tests/lsp_matrix/fixtures/folding-editor.json");

function visibleRanges(text, hidden=[]){
  const lines=text.split(/\r?\n/),visible=lines.map((_,i)=>i).filter(i=>!hidden.some(r=>r.start<i&&i<=r.end));
  const groups=[];
  for(const line of visible){const g=groups.at(-1);if(g&&g.at(-1)+1===line)g.push(line);else groups.push([line]);}
  return groups.filter(g=>!hidden.length||g.length>1||lines[g[0]].length>0)
    .map(g=>({start:{line:g[0],character:0},end:{line:g.at(-1),character:lines[g.at(-1)].length}}));
}
// Pinned FoldingRegions.sanitizeAndMerge: preserve public ranges separately;
// crossing intervals at a shared end/start line cannot be workbench regions.
function workbenchRanges(ranges){
  const kept=[],parents=[];let current;
  for(const row of ranges){
    while(current&&current.end<row.start)current=parents.pop();
    if(row.end>row.start&&(!current||current.end>=row.end)){
      kept.push(row);if(current)parents.push(current);current=row;
    }
  }return kept;
}
function gutterFoldModel(){
  const cases=spec.forms.map(form=>{
    const crlf=form.startsWith("crlf"),shifted=form.endsWith("shifted"),files={},views=[];
    const sources=[{id:"disk",file:"scripts/main.vela",source:"// disk baseline 中😀\n"+corpus.files[corpus.oracle.file],ranges:corpus.oracle.cases[0].ranges},
      ...corpus.oracle.cases.map(c=>({...c,file:`scripts/${c.id}.vela`})),
      {id:"helper",file:"scripts/helper.vela",source:corpus.files["scripts/helper.vela"],ranges:[]}];
    for(const s of sources){
      const doc=folding.document(s.source,crlf,shifted);files[s.file]=doc.text;
      const publicRanges=folding.editor(doc,s.ranges);
      views.push({id:s.id,file:s.file,text:doc.text,wire:folding.wire(doc,s.ranges),publicRanges,ranges:workbenchRanges(publicRanges)});
    }
    return{id:`gutter-${form}`,form,files,views,schema:null};
  });return{spec,cases};
}
function render(text,ranges,collapsed=null){
  return{visibleLines:text.split(/\r?\n/).map((_,i)=>i).filter(i=>!collapsed||!(collapsed.start<i&&i<=collapsed.end)),
    controls:ranges.filter(r=>!collapsed||!(collapsed.start<r.start&&r.start<=collapsed.end)).map(r=>({line:r.start,icon:r===collapsed?spec.oracle.collapsedIcon:spec.oracle.expandedIcon}))};
}
function gutterFoldContracts(requirements,platform="darwin"){
  assert(["darwin","win32"].includes(platform));
  if(!requirements.some(r=>r.id.startsWith("vscode/UX12/")))return[];
  const m=gutterFoldModel(),id="ux12-gutter-fold",actions=[],checks=[],artifacts=["trace.json",id+"-observations.json"];
  const check=(id,level,expected)=>checks.push({id,level,expected});
  const shot=id=>artifacts.push(id+".png",id+".aria.txt");
  for(const c of m.cases){
    for(const file of ["workbench.log","lsp-trace.log","extension-host.log","server-trace.jsonl","setup.json","bridge.json"])artifacts.push(`picker-${c.id}/${file}`);
    check(c.id+"-profile","Input",{platform,case:c.id,version:"1.137.0",locale:"en",settings:true,display:true,installedServer:true,normalWindow:true,roots:["scripts"]});
    for(const v of c.views){
      const prefix=c.id+"-"+v.id,source={file:v.file,text:v.text,dirty:false};
      check(prefix+"-wire","Input",{result:v.wire});
      check(prefix+"-expanded","Render",render(v.text,v.ranges));shot(id+"-"+prefix+"-expanded");
      check(prefix+"-source","Input",{...source,visibleRanges:visibleRanges(v.text)});
      for(const [i,r]of v.ranges.entries()){
        const p=prefix+"-"+i;
        for(const stage of ["collapse","expand"]){
          actions.push({id:p+"-"+stage,device:"pointer",selector:`folding gutter control at source line ${r.start+1}`,clickCount:1});
          const hidden=stage==="collapse"?r:null;
          check(p+"-"+stage+"-render","Render",render(v.text,v.ranges,hidden));
          check(p+"-"+stage+"-source","Input",{...source,visibleRanges:visibleRanges(v.text,hidden?[hidden]:[])});
          shot(id+"-"+p+"-"+stage);
        }
      }
    }
    check(c.id+"-disk","Input",c.files);check(c.id+"-clean-exit","Input",{code:0,signal:null});
  }
  return[{id,fixture:spec.id,deadlineMs:spec.oracle.aggregateBudgetMs,actions,checks,artifacts,requirements:["input","render"].map(level=>{
    const id=`vscode/UX12/gutter-fold/${level}/local`,r=requirements.find(r=>r.id===id);assert(r,id);return{id,contractHash:r.contractHash};
  })}];
}
module.exports={visibleRanges,workbenchRanges,render,gutterFoldModel,gutterFoldContracts};
