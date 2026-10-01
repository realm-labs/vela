"use strict";
const fs = require("node:fs"), path = require("node:path"), assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
function parseMacProcesses(text) {
  return text.split(/\r?\n/).filter(Boolean).map(line => {
    const m = /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.{24})\s+(.+)$/.exec(line);
    if (!m) throw Error("unrecognized macOS process record");
    return { pid: Number(m[1]), ppid: Number(m[2]), state: m[3], created: m[4], executable: m[5] };
  });
}
function ownedServers(records, binary, rootPid, platform) {
  const equal = (a,b) => typeof a === "string" && (platform === "win32" ? a.toLowerCase() === b.toLowerCase() : a === b);
  const parents = new Map(records.map(r => [r.pid,r.ppid]));
  const descends = pid => { const seen = new Set(); for (let d=0;d<64;d++) {
    if (pid === rootPid) return true;
    if (!pid || seen.has(pid)) return false;
    seen.add(pid); pid=parents.get(pid);
  } return false; };
  const matches = records.filter(r => equal(r.executable,binary));
  assert(matches.every(r=>descends(r.pid)), "server binary is running outside the test-owned workbench");
  assert(matches.every(r=>Number.isSafeInteger(r.pid)&&r.pid>0&&r.created), "invalid server identity");
  return matches;
}
class TestServers {
  constructor(root, binary, rootPid, platform) {
    this.binary=fs.realpathSync(binary); this.rootPid=rootPid; this.platform=platform; this.paused=[];
    assert(this.binary.startsWith(fs.realpathSync(root)+path.sep), "server must be installed under test artifacts");
    assert(Number.isSafeInteger(rootPid)&&rootPid>0);
  }
  windows(operation, record) {
    return JSON.parse(execFileSync("powershell.exe", ["-NoProfile","-NonInteractive","-File",path.join(__dirname,"server-process.ps1"),
      "-Operation",operation,"-ServerPath",this.binary,...(record?["-TargetProcess",String(record.pid),"-Created",record.created]:[])],
      {encoding:"utf8",timeout:10000,windowsHide:true,maxBuffer:2*1024*1024}));
  }
  records() {
    if(this.platform==='win32') return this.windows('list');
    assert.equal(this.platform,'darwin');
    return parseMacProcesses(execFileSync('ps',['-axo','pid=,ppid=,stat=,lstart=,comm='],{encoding:'utf8',timeout:5000,maxBuffer:2*1024*1024,
      env:{...process.env,LC_ALL:'en_US.UTF-8'}}));
  }
  inventory() { return ownedServers(this.records(),this.binary,this.rootPid,this.platform); }
  current(record, owned=true) {
    const list=owned?this.inventory():this.records().filter(r=>r.executable===this.binary || (this.platform==='win32'&&r.executable?.toLowerCase()===this.binary.toLowerCase()));
    const same=list.find(r=>r.pid===record.pid);
    if(!same) return null;
    assert.equal(same.created,record.created,"captured PID was reused");
    return same;
  }
  async suspend(record, until) {
    assert(this.current(record),"test-owned server exited before suspension"); this.paused.push(record);
    if(this.platform==='win32') { const result=this.windows('pause',record); assert(result.suspended&&result.threads>0); }
    else { process.kill(record.pid,'SIGSTOP'); await until('owned server suspended',()=>this.current(record)?.state.includes('T')); }
  }
  stop(record) {
    assert(this.current(record),"captured server must still be test-owned");
    if(this.platform==='win32') this.windows('stop',record); else process.kill(record.pid,'SIGKILL');
    this.paused=this.paused.filter(r=>r.pid!==record.pid);
  }
  cleanup() {
    for(const record of this.paused) if(this.current(record,false)) {
      if(this.platform==='win32') this.windows('stop',record); else process.kill(record.pid,'SIGKILL');
    }
    this.paused=[];
  }
}
module.exports={ TestServers, ownedServers, parseMacProcesses };
