"use strict";
const { loadFixture } = require("./fixture-sources");
const assert=require("node:assert/strict"),oracle=require("./selection-editor-oracle");
const spec=require("../../tests/lsp_matrix/fixtures/input-selection.json");
const corpus=require("../../tests/lsp_matrix/fixtures/selection-editor.json");
const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
function offsetAt(text,p){
  const eol=text.includes("\r\n")?"\r\n":"\n",lines=text.split(eol);
  return lines.slice(0,p.line).reduce((n,s)=>n+s.length+eol.length,0)+p.character;
}
function rangeText(text,r){return text.slice(offsetAt(text,r.start),offsetAt(text,r.end));}
function paintedRows(r){
  if(same(r.start,r.end))return[];
  const last=r.end.line-(r.end.character===0?1:0);
  return Array.from({length:last-r.start.line+1},(_,i)=>r.start.line+i);
}
function transitions(doc,q){
  const point=oracle.positions(doc,[q])[0],initial={start:point,end:point};
  const publicRanges=oracle.flatten(oracle.publicExpected(doc,[q])[0]);
  assert(publicRanges.length<=spec.oracle.maxChainDepth,"bounded authored chain");
  const ranges=publicRanges.filter(r=>!same(r,initial)).filter((r,i,rows)=>!i||!same(r,rows[i-1])),states=[];
  for(const [i,r]of ranges.entries())states.push({id:`expand-${i}`,key:"expand",range:r});
  states.push({id:"expand-cap",key:"expand",range:ranges.at(-1)??initial});
  for(const [i,r]of ranges.slice(0,-1).toReversed().concat(ranges.length?[initial]:[]).entries())states.push({id:`shrink-${i}`,key:"shrink",range:r});
  states.push({id:"shrink-cap",key:"shrink",range:initial});
  return{point,initial,wire:oracle.expected(doc,[q]),publicRanges,states,shots:[...new Set([states[0].id,"expand-cap","shrink-cap"])]};
}
function selectionInputModel(){
  const families=[...new Set(corpus.oracle.cases.map(c=>c.fixture))],cases=[];
  for(const form of spec.forms)for(const fixture of families){
    const crlf=form.startsWith("crlf"),shifted=form.endsWith("shifted"),refs=corpus.oracle.cases.filter(c=>c.fixture===fixture).toSorted((a,b)=>Number(oracle.document(oracle.resolve(b).item.source).text==="")-Number(oracle.document(oracle.resolve(a).item.source).text==="")),files={},views=[];
    const source=loadFixture(require.resolve(`../../tests/lsp_matrix/fixtures/${fixture}.json`));
    files["scripts/helper.vela"]=oracle.document(source.files["scripts/helper.vela"],crlf,shifted).text;
    for(const [i,ref]of refs.entries()){
      const {item}=oracle.resolve(ref),doc=oracle.document(item.source,crlf,shifted),file=`scripts/case-${i}.vela`;
      const queries=item.queries.map((q,i)=>({id:`q${i}`,...transitions(doc,q)}));files[file]=doc.text;
      views.push({id:`v${i}`,ref:ref.id,file,text:doc.text,queries});
    }
    cases.push({id:`select-${form}-${fixture.slice("selection-".length)}`,form,fixture,files,views,schema:null,windowBudgetMs:spec.oracle.windowBudgetMs});
  }
  return{spec,cases};
}
function sourceState(v,r){
  return{file:v.file,text:v.text,dirty:false,focused:true,selections:[{anchor:r.start,active:r.end}],selectedTexts:[rangeText(v.text,r)]};
}
function renderState(v,r){
  return{visibleLines:v.text.split(/\r?\n/).map((_,i)=>i),selectedText:rangeText(v.text,r).replaceAll("\r\n","\n"),paintedRows:paintedRows(r)};
}
function selectionInputContracts(requirements,platform="darwin"){
  assert(["darwin","win32"].includes(platform));if(!requirements.some(r=>r.id.startsWith("vscode/UX12/")))return[];
  const m=selectionInputModel(),id="ux12-selection-expand-shrink",keys=spec.oracle.keys[platform],actions=[],checks=[],artifacts=["trace.json",id+"-trace.jsonl",id+"-observations.jsonl"];
  const check=(id,level,expected)=>checks.push({id,level,expected}),shot=p=>artifacts.push(id+"-"+p+".png",id+"-"+p+".aria.txt");
  const key=(id,key)=>actions.push({id,device:"keyboard",key});
  for(const c of m.cases){
    for(const file of ["workbench.log","lsp-trace.log","extension-host.log","server-trace.jsonl","setup.json","bridge.json"])artifacts.push(`picker-${c.id}/${file}`);
    check(c.id+"-profile","Input",{platform,case:c.id,version:"1.137.0",locale:"en",settings:true,display:true,installedServer:true,normalWindow:true,roots:["scripts"]});
    for(const v of c.views){
      const p=c.id+"-"+v.id,zero={start:{line:0,character:0},end:{line:0,character:0}};
      check(p+"-baseline-source","Input",sourceState(v,zero));check(p+"-baseline-render","Render",renderState(v,zero));shot(p+"-baseline");
      for(const q of v.queries){
        const prefix=p+"-"+q.id;
        key(prefix+"-home",keys.home);key(prefix+"-end",keys.end);key(prefix+"-goto",keys.goto);
        check(prefix+"-goto-mode","Input",{prefix:":",focused:true});key(prefix+"-select-target",keys.selectAll);
        const target=`::${offsetAt(v.text,q.point)+1}`;
        actions.push({id:prefix+"-target",device:"keyboard",text:target});check(prefix+"-target-value","Input",{value:target,focused:true});key(prefix+"-accept","Enter");
        check(prefix+"-position","Input",sourceState(v,q.initial));check(prefix+"-wire","Input",{result:q.wire});
        for(const s of q.states){
          key(prefix+"-"+s.id,keys[s.key]);check(prefix+"-"+s.id+"-source","Input",sourceState(v,s.range));
          check(prefix+"-"+s.id+"-render","Render",renderState(v,s.range));
          if(q.shots.includes(s.id))shot(prefix+"-"+s.id);
        }
      }
    }
    check(c.id+"-disk","Input",c.files);check(c.id+"-clean-exit","Input",{code:0,signal:null});
  }
  return[{id,fixture:spec.id,deadlineMs:spec.oracle.aggregateBudgetMs,actions,checks,artifacts,requirements:["input","render"].map(level=>{
    const id=`vscode/UX12/selection-expand-shrink/${level}/local`,r=requirements.find(r=>r.id===id);assert(r,id);return{id,contractHash:r.contractHash};
  })}];
}
module.exports={offsetAt,rangeText,paintedRows,transitions,selectionInputModel,sourceState,renderState,selectionInputContracts};
