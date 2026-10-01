"use strict";
const test=require('node:test'),assert=require('node:assert/strict');
const {ownedServers,parseMacProcesses}=require('../../editors/vscode/test/input/server-process');
test('process injection rejects unrelated parents, ancestry cycles and incomplete captured identities',()=>{
  const binary='/owned/中文 % profile/server/vela_lsp_server', root={pid:10,ppid:1,executable:'/VS Code',created:'a'};
  const host={pid:11,ppid:10,executable:'/extension host',created:'b'}, server={pid:12,ppid:11,executable:binary,created:'c'};
  assert.deepEqual(ownedServers([root,host,server],binary,10,'darwin'),[server]);
  for(const r of [{...server,ppid:99},{...server,ppid:12},{...server,created:''}])assert.throws(()=>ownedServers([root,host,r],binary,10,'darwin'));
  assert.deepEqual(ownedServers([root,host,{...server,executable:binary.toUpperCase()}],binary,10,'win32').map(r=>r.pid),[12]);
  assert.equal(ownedServers([root,host,server],binary+'-other',10,'darwin').length,0);
});
test('macOS process records preserve executable spaces Unicode percent and stable creation fields',()=>{
  const line='  12  11 T Thu Oct  1 14:10:00 2026 /owned/中文 % profile/server/vela_lsp_server';
  assert.deepEqual(parseMacProcesses(line),[{pid:12,ppid:11,state:'T',created:'Thu Oct  1 14:10:00 2026',executable:'/owned/中文 % profile/server/vela_lsp_server'}]);
  assert.throws(()=>parseMacProcesses('12 11 vela_lsp_server'));
});
