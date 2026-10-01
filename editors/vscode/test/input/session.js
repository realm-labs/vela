"use strict";
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {logFiles}=require('./logs');
function readSession(root){
  const s=JSON.parse(fs.readFileSync(path.join(root,'bridge.json'),'utf8'));
  assert(Number.isSafeInteger(s.pid)&&s.pid>0&&Number.isInteger(s.port)&&s.port>0&&s.port<=65535&&/^[0-9a-f]{64}$/.test(s.token),'invalid bridge session');
  assert.equal(s.mode,1,'native session must use a normal installed extension host');
  for(const directory of [s.logDirectory,s.hostLogDirectory]){
    const relative=path.relative(fs.realpathSync(path.join(root,'user-data/logs')),fs.realpathSync(directory));
    assert(relative&&!relative.startsWith('..')&&!path.isAbsolute(relative),'session log directory escapes test profile');
  }
  return s;
}
function sessionLog(s,predicate,optional=false){
  const directory=predicate('exthost.log')?s.hostLogDirectory:s.logDirectory;
  const matches=logFiles(directory,predicate);
  if(optional&&matches.length===0)return null;
  assert.equal(matches.length,1,'exactly one log must belong to this extension host');
  return matches[0];
}
function snapshot(root,route,ordinal,s,workspace){
  const files=[];
  for(const [label,predicate,optional] of [['trace',n=>n.endsWith('-Vela LSP Trace.log'),true],['output',n=>/-Vela\.log$/.test(n),false],['host',n=>n==='exthost.log',false]]){
    const original=sessionLog(s,predicate,optional);if(!original)continue;
    const target=`${route}-session-${ordinal}-${label}.log`;fs.copyFileSync(original,path.join(root,target));
    files.push({path:target,original:path.relative(root,original).split(path.sep).join('/')});
  }
  if(workspace){
    const original=path.join(workspace,'.vela-lsp-trace.jsonl'),target=`${route}-session-${ordinal}-server.log`;
    fs.copyFileSync(original,path.join(root,target));files.push({path:target,original:path.relative(root,original).split(path.sep).join('/')});
  }
  return {pid:s.pid,files};
}
module.exports={readSession,sessionLog,snapshot};
