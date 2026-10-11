"use strict";
const fs=require("node:fs"),path=require("node:path"),assert=require("node:assert/strict");
const {safeFile}=require("../../../../scripts/lsp-matrix/fixtures");
function traceRecorder(root,defaultProof){
  const trace=[];let journal;
  const flush=()=>fs.writeFileSync(path.join(root,"trace.json"),JSON.stringify(trace,null,2));
  const record=(kind,id,details)=>{
    const row={kind,id,proof:defaultProof,at:new Date().toISOString(),...details};trace.push(row);
    if(journal)fs.appendFileSync(journal,JSON.stringify(row)+"\n");else flush();
  };
  const startJournal=file=>{assert(!journal,"one active trace journal");journal=path.join(root,safeFile(file));assert(!fs.existsSync(journal));fs.writeFileSync(journal,"");};
  const endJournal=()=>{assert(journal,"active trace journal required");try{flush();}finally{journal=undefined;}};
  return{trace,record,flush,startJournal,endJournal};
}
module.exports={traceRecorder};
