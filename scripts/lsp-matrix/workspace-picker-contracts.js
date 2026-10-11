"use strict";
const path = require("node:path"), assert = require("node:assert/strict");
const oracle = require("./workspace-symbol-oracle");
const { orderedWorkspaceTies } = require("./workspace-picker-wire");
const spec = require("../../tests/lsp_matrix/fixtures/input-workspace-picker.json");

function workspacePickerModel(platform = "darwin") {
  assert(["win32","darwin"].includes(platform));
  const cases = [];
  for (const form of spec.forms) for (const source of spec.sources) {
    const fixture = require(path.join(__dirname,"../../tests/lsp_matrix/fixtures",source.fixture));
    const docs = Object.fromEntries(Object.entries(fixture.files).filter(([f])=>f.endsWith(".vela"))
      .map(([f,t])=>[f,oracle.document(t,form === "crlf")]));
    const files = Object.fromEntries(Object.entries(docs).map(([f,d])=>[f,d.text]));
    if (fixture.oracle.schema) files["schema.json"] = JSON.stringify(oracle.metadataArtifact(fixture.oracle.schema));
    const wire = query => orderedWorkspaceTies(oracle.expected(fixture,docs,f=>f,{ symbols: fixture.oracle.queries[0].symbols.filter(id=>
      fixture.oracle.symbols.find(r=>r.id===id).name.toLowerCase().includes(query.trim().toLowerCase())) }));
    const row = symbol => ({ name: symbol.name, icon: spec.oracle.icons[symbol.kind],
      description: (symbol.container ? symbol.container + spec.oracle.descriptionSeparator : "") +
        (symbol.file ? symbol.file.replaceAll("/",platform === "win32" ? "\\" : "/") : ".") });
    const sorted = fixture.oracle.symbols.toSorted((a,b)=> a.name.toLowerCase().localeCompare(b.name.toLowerCase(),"en") ||
      spec.oracle.icons[a.kind].localeCompare(spec.oracle.icons[b.kind],"en") ||
      (a.file ? "file:"+a.file : "vela-schema:").localeCompare(b.file ? "file:"+b.file : "vela-schema:","en"));
    const rows = sorted.map(s=>({ id:s.id, file:s.file, view:row(s) }));
    const targets = sorted.filter(s=>s.file).map(s=>({ id:s.id,file:s.file,name:s.name,view:row(s),
      query:s.name.split(/\s+/)[0],index:spec.oracle.sourceTargetIndices[s.id]??0,
      range:wire("").find(r=>r.name===s.name && r.location.uri===s.file).location.range }));
    // Complete authored empty-name partitions. Multiword GUI queries have a
    // different SDK name/description policy and cannot inherit backend [] claims.
    const absent = fixture.oracle.queries.filter(q=>!q.symbols.length && !/\s/.test(q.query));
    cases.push({ id:`${form}-${source.family}`, family:source.family, form, files, rows, targets, absent, wire,
      document:source.document, positive:source.positive, startup:"scripts/imports.vela", schema:fixture.oracle.schema ? "schema.json" : null });
  }
  return { spec,cases };
}
function workspacePickerContracts(requirements, platform = "darwin") {
  if (!requirements.some(r=>r.id.startsWith("vscode/UX11/"))) return [];
  const m=workspacePickerModel(platform), parts=[];
  for(const route of ["workspace-picker","no-match"]){
    const id=`ux11-${route}`,actions=[],checks=[],artifacts=["trace.json",`${id}-observations.json`];
    const key=(id,key)=>actions.push({id,device:"keyboard",key});
    const query=(id,text)=>{key(id+"-select","Meta+a");actions.push({id:id+"-type",device:"keyboard",text});};
    const check=(id,level,expected)=>checks.push({id,level,expected});
    const shot=id=>artifacts.push(`${id}.png`,`${id}.aria.txt`);
    for(const c of m.cases){
      const prefix=c.id, base={file:c.document,text:c.files[c.document],dirty:false,selections:[{anchor:{line:0,character:0},active:{line:0,character:0}}]};
      for(const f of ["workbench.log","lsp-trace.log","extension-host.log","server-trace.jsonl","setup.json","bridge.json"])
        artifacts.push(`picker-${prefix}/${f}`);
      check(prefix+"-profile","Input",{platform,case:prefix,version:"1.137.0",locale:"en",settings:true,display:true,installedServer:true,normalWindow:true,roots:["scripts"]});
      if(route === "no-match"){
        key(prefix+"-document-open","Meta+Shift+o");query(prefix+"-document-positive","@"+c.positive);
        check(prefix+"-document-request","Input",{method:"textDocument/documentSymbol",completed:true});
        check(prefix+"-document-positive","Render",{name:c.positive,description:"",icon:"symbol-function",count:1});
        query(prefix+"-document-absent","@"+spec.oracle.documentAbsent);
        check(prefix+"-document-empty","Render",{name:spec.oracle.documentEmpty,description:"",icon:"",count:1});
        key(prefix+"-document-enter","Enter");check(prefix+"-document-retained","Input",{...base,pickerVisible:true});shot(`${id}-${prefix}-document`);
        key(prefix+"-document-dismiss","Escape");
        key(prefix+"-workspace-open","Meta+t");
        for(const q of c.absent){
          const p=prefix+"-"+q.id;query(p,"#"+q.query);
          check(p+"-wire","Input",{query:q.query,result:[]});
          check(p+"-empty","Render",{name:spec.oracle.workspaceEmpty,description:"",icon:"",count:1});
          check(p+"-source","Input",base);shot(`${id}-${p}`);
        }
        key(prefix+"-workspace-dismiss","Escape");check(prefix+"-final-source","Input",base);
      }else{
        key(prefix+"-open","Meta+t");query(prefix+"-reset","#"+spec.oracle.reset);
        check(prefix+"-reset-wire","Input",{query:spec.oracle.reset,result:[]});
        query(prefix+"-all","#");check(prefix+"-all-wire","Input",{query:"",result:c.wire("")});
        key(prefix+"-home","Control+Home");
        for(const [i,r]of c.rows.entries()){
          if(i)key(prefix+`-${i}-next`,"ArrowDown");check(prefix+`-${i}-row`,"Render",r.view);
        }
        key(prefix+"-end","Control+End");check(prefix+"-tail","Render",c.rows.at(-1).view);
        key(prefix+"-wrap","ArrowDown");check(prefix+"-head","Render",c.rows[0].view);shot(`${id}-${prefix}-all`);
        const opened=new Set([c.startup,c.document]);
        for(const [i,t]of c.targets.entries()){
          const p=prefix+"-"+t.id;
          query(p+"-reset","#"+spec.oracle.reset);check(p+"-reset-wire","Input",{query:spec.oracle.reset,result:[]});
          query(p+"-query","#"+t.name);check(p+"-wire","Input",{query:t.query,result:c.wire(t.query)});
          key(p+"-home","Control+Home");for(let n=0;n<t.index;n++)key(p+`-next-${n}`,"ArrowDown");
          check(p+"-row","Render",t.view);check(p+"-before","Input",{file:t.file,everOpened:opened.has(t.file),...(!opened.has(t.file)?{open:false}: {})});
          if(i%5===0)actions.push({id:p+"-accept",device:"pointer",selector:"workspace symbol picker focused row",clickCount:1});else key(p+"-accept","Enter");
          check(p+"-destination","Input",{file:t.file,text:c.files[t.file],dirty:false,selections:[{anchor:t.range.start,active:t.range.start}]});
          opened.add(t.file);shot(`${id}-${p}-destination`);key(p+"-reopen","Meta+t");
        }
        key(prefix+"-dismiss","Escape");check(prefix+"-disk","Input",c.files);
      }
      check(prefix+"-clean-exit","Input",{code:0,signal:null});
    }
    parts.push({id,fixture:spec.id,deadlineMs:480000,requirements:["input","render"].map(level=>{
      const id=`vscode/UX11/${route}/${level}/local`,r=requirements.find(r=>r.id===id);if(!r)throw Error(id);return{id,contractHash:r.contractHash};
    }),actions,checks,artifacts});
  }
  return parts;
}
module.exports={workspacePickerModel,workspacePickerContracts};
