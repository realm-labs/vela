"use strict";
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {fileURLToPath}=require('node:url');
const {lifecycleModel}=require('../../../../scripts/lsp-matrix/lifecycle-contracts');
const {navigationResponses}=require('../../../../scripts/lsp-matrix/navigation-trace');
const {offsetAt}=require('../../../../scripts/lsp-matrix/fixtures');
const evidence=require('../../../../scripts/lsp-matrix/local-evidence');
const {fileUri,canonicalUri,relativeFile}=require('./paths');
const {readSession,sessionLog,snapshot}=require('./session');
const {TestServers}=require('./server-process');
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));

async function runLifecycle({page,bridge,record,root,workspace,contracts,until,pid,platform,binary,onProof}){
  const selected=contracts.filter(c=>c.id.startsWith('ux18-'));if(!selected.length)return;
  const m=lifecycleModel(),o=m.spec.oracle,servers=new TestServers(root,binary,pid,platform);
  const textbox=()=>page.getByRole('textbox',{name:/^lifecycle\.vela/});
  const state=async()=>{
    const active=(await bridge('inspect')).active;if(!active)return null;
    return {file:relativeFile(workspace,fileURLToPath(active.uri)),text:active.text,dirty:active.dirty,languageId:active.languageId,
      disk:fs.readFileSync(path.join(workspace,o.file),'utf8'),selections:active.selections};
  };
  const trace=()=>fs.readFileSync(sessionLog(readSession(root),n=>n.endsWith('-Vela LSP Trace.log')),'utf8');
  const output=()=>fs.readFileSync(sessionLog(readSession(root),n=>/-Vela\.log$/.test(n)),'utf8');
  try{for(const contract of selected){
    const route=contract.id.slice(5),started=Date.now(),actions=[],checks=[],sessions=[];
    const receipt=(kind,id,details)=>record(kind,id,{proof:contract.id,...details});
    const check=(id,observed)=>{
      const expected=contract.checks.find(c=>c.id===id);assert(expected,`unknown lifecycle check ${id}`);
      assert.deepEqual(observed,expected.expected,`${contract.id}/${id}`);
      checks.push({...expected,observed,status:'passed'});receipt('assertion',id,{expected:expected.expected,observed});
    };
    const action=async(id,callback)=>{
      const a=contract.actions.find(a=>a.id===id);assert(a,`unknown lifecycle action ${id}`);
      let result;if(callback)result=await callback(a);else if(a.text!==undefined)await page.keyboard.type(a.text);else await page.keyboard.press(a.key);
      actions.push(a);receipt(a.device==='command'?'command':'input',id,Object.fromEntries(Object.entries(a).filter(([key])=>key!=='id')));return result;
    };
    const destination=async id=>{
      const expected=contract.checks.find(c=>c.id===id).expected;
      check(id,await until(id,async()=>{const s=await state();receipt('observation',id+'-state',{state:s});return JSON.stringify(s)===JSON.stringify(expected)&&s;}));
    };
    const palette=async prefix=>{
      await action(prefix+'-open');const picker=page.locator('.quick-input-widget');await picker.waitFor({state:'visible'});
      await action(prefix+'-name');const title=contract.actions.find(a=>a.id===prefix+'-name').text;
      const option=picker.locator('.label-name').filter({hasText:new RegExp('^'+title+'$')});
      await option.waitFor({state:'visible'});assert.equal(await option.count(),1,'one exact native palette command');
      receipt('observation',prefix+'-palette',{title:await option.innerText(),visible:true});
      await action(prefix+'-accept');
      if(prefix!=='recover'&&prefix!=='invalid')await picker.waitFor({state:'hidden'});
    };
    const go=async prefix=>{
      await action(prefix+'-goto');const picker=page.locator('.quick-input-widget');await picker.waitFor({state:'visible'});
      await action(prefix+'-position');await action(prefix+'-place');await picker.waitFor({state:'hidden'});
    };
    const saveSession=()=>{sessions.push(snapshot(root,route,sessions.length,readSession(root),workspace));fs.writeFileSync(path.join(root,route+'-sessions.json'),JSON.stringify(sessions,null,2));};
    let boundary;
    const wire=async id=>{
      const responses=await until(id,()=>{
        const r=navigationResponses(trace().slice(boundary)).filter(r=>canonicalUri(r.params.textDocument.uri)===fileUri(path.join(workspace,o.file)));
        return r.length&&r;
      });
      assert.equal(responses.length,1,'native action sends exactly one completed request');
      const r=responses[0],targets=Array.isArray(r.result)?r.result:[r.result];assert.equal(targets.length,1);assert(targets[0]);
      const target=targets[0],uri=target.targetUri??target.uri,range=target.targetSelectionRange??target.range;
      const document=(await bridge('inspect')).documents.find(d=>d.uri===canonicalUri(uri));assert(document);
      check(id,{method:r.method,request:{file:relativeFile(workspace,fileURLToPath(r.params.textDocument.uri)),position:r.params.position},
        result:{file:relativeFile(workspace,fileURLToPath(uri)),range,text:document.text.slice(offsetAt(document.text,range.start),offsetAt(document.text,range.end))}});
      receipt('observation',id+'-id',{requestId:r.id});
    };
    const query=async()=>{
      const result=await bridge('lifecycle-query');
      const docs=(await bridge('inspect')).documents;
      return result.map(r=>{const document=docs.find(d=>d.uri===canonicalUri(r.uri));assert(document);
        return {file:relativeFile(workspace,fileURLToPath(r.uri)),range:r.range,text:document.text.slice(offsetAt(document.text, r.range.start),offsetAt(document.text,r.range.end))};});
    };
    const stoppedPolicy=async()=>{
      const beginning=Date.now(),counts=[],starts=[];
      for(let index=0;index<3;index++){
        if(index)await delay(500);
        counts.push(servers.inventory().length);starts.push((output().match(/Starting native language server:/g)??[]).length);
      }
      const duration=Date.now()-beginning;receipt('observation','policy-interval',{durationMs:duration,servers:counts,starts});
      check('stopped-policy',{servers:counts,starts,atLeast1000Ms:duration>=1000});
    };
    const showFailure=async(invalid,missing)=>{
      await palette('failure');const pane=page.locator('[id="workbench.panel.output"]');await pane.waitFor({state:'visible'});
      const message=invalid?o.failedMessage:o.closedMessage;
      const rendered=await until('visible Vela failure',async()=>{const t=(await pane.innerText()).replaceAll('\u00a0',' ');return t.includes(message)&&t;});
      // This is the workbench's real output channel selector, not a bridge query.
      const selector=pane.locator('.monaco-select-box');
      const channel=await selector.evaluate(element=>element.tagName==='SELECT'?element.selectedOptions[0]?.textContent:element.getAttribute('title')??element.textContent);
      // Monaco wraps one logical output line into several visible line divs.
      // Read their actual text nodes together without inventing newline bytes.
      const renderedNodes=(await pane.locator('.view-lines').textContent()).replaceAll('\u00a0',' ');
      check('visible-failure',{visible:await pane.isVisible(),channel:channel.trim(),message,...(invalid?{missingExecutable:renderedNodes.includes(missing)}:{})});
      receipt('observation','rendered-failure',{text:rendered,textNodes:renderedNodes});
      await page.screenshot({path:path.join(root,route+'-failure.png')});fs.writeFileSync(path.join(root,route+'-failure.aria.txt'),await pane.ariaSnapshot());
    };
    const reload=async(prefix,checkId,expectedServers)=>{
      const before=readSession(root),extensionPath=(await bridge('inspect')).extensionPath;
      saveSession();await palette(prefix);
      const after=await until('new extension host after native reload',()=>{
        try{const s=readSession(root);return s.pid!==before.pid&&s.token!==before.token&&s;}catch(error){
          if(error.code==='ENOENT'||error instanceof SyntaxError)return null;throw error;
        }
      },20000);
      await until('new client lifecycle settled',()=>{
        const log=output(),current=servers.inventory();
        const settled=expectedServers===1?log.includes('Language server started.'):log.includes(o.failedMessage);
        receipt('observation',prefix+'-settlement',{servers:current,settled});
        return settled&&current.length===expectedServers&&current.every(server=>server.ppid===after.pid);
      });
      const inspected=await bridge('inspect'),current=servers.inventory();
      check(checkId,{changedHost:after.pid!==before.pid,sameInstalledExtension:inspected.extensionPath===extensionPath,servers:current.length});
      receipt('observation',prefix+'-host',{previous:before.pid,current:after.pid,servers:current});
      saveSession();
    };

    await bridge('setup',{file:o.file,line:0,character:0,reset:true});await textbox().focus();
    await action('dirty-home');await action('dirty-prefix');await go('baseline');await destination('dirty-source');
    boundary=trace().length;await action('baseline-query');await destination('baseline-destination');await wire('baseline-wire');
    await go('pending');await destination('pending-source');
    if(route==='server-stop-reload'){
      const live=servers.inventory();assert.equal(live.length,1);
      await action('suspend-owned',()=>servers.suspend(live[0],until));check('suspended',{servers:servers.inventory().length,owned:true,suspended:true});
      boundary=trace().length;let settled=false;const requestStarted=Date.now();
      const a=contract.actions.find(a=>a.id==='pending-query');
      const pending=query().then(result=>{settled=true;return result;});pending.catch(()=>{});
      actions.push(a);receipt('command',a.id,Object.fromEntries(Object.entries(a).filter(([key])=>key!=='id')));
      const sent=await until('actual queued client request',()=>{
        const blocks=trace().slice(boundary).split('\n\n\n');blocks.pop();
        return blocks.map(b=>b.match(/Sending request '(textDocument\/definition) - \((\d+)\)'\.\nParams: ([\s\S]+)$/)).find(Boolean);
      });
      assert.equal(settled,false,'real request must still be held by the suspended child');
      assert.equal(navigationResponses(trace().slice(boundary)).length,0);
      const params=JSON.parse(sent[3]);check('pending-request',{method:sent[1],request:{file:relativeFile(workspace,fileURLToPath(params.textDocument.uri)),position:params.position},received:false});
      await action('stop-owned',()=>servers.stop(live[0]));const result=await pending,duration=Date.now()-requestStarted;
      receipt('observation','pending-completion',{durationMs:duration,result});check('bounded-failure',{result,within5000Ms:duration<=5000});
      await until('closed client policy',()=>output().includes(o.closedMessage));await showFailure(false);await stoppedPolicy();
    }else{
      const configured=await action('configure-invalid',()=>bridge('lifecycle-config',{value:'missing'}));
      const missing=path.join(workspace,o.missingDirectory,platform==='win32'?'vela_lsp_server.exe':'vela_lsp_server');
      assert.equal(fileUri(configured.path),fileUri(missing));check('invalid-config',{missing:!fs.existsSync(missing),insideWorkspace:path.relative(workspace,configured.path).split(path.sep).join('/')===o.missingDirectory+'/'+path.basename(missing)});
      await reload('invalid','invalid-reloaded',0);
      await action('open-picker');const picker=page.locator('.quick-input-widget');await picker.waitFor({state:'visible'});await action('file-name');
      const candidate=picker.locator('.label-name').filter({hasText:/^lifecycle\.vela$/});await candidate.waitFor({state:'visible'});assert.equal(await candidate.count(),1);
      await action('open-file');await picker.waitFor({state:'hidden'});await showFailure(true,configured.path);
      const requestStarted=Date.now(),result=await action('failed-query',query),duration=Date.now()-requestStarted;
      receipt('observation','failed-completion',{durationMs:duration,result});check('bounded-failure',{result,within5000Ms:duration<=5000});await stoppedPolicy();
      check('default-config',await action('restore-config',()=>bridge('lifecycle-config',{value:'default'})));
    }
    await reload('recover','reloaded',1);await action('recover-editor');await textbox().focus();await destination('restored-source');
    check('recovered-command',await action('recovered-query',query));boundary=trace().length;await action('recovered-native-query');
    await destination('recovered-destination');await wire('recovered-wire');saveSession();
    await page.screenshot({path:path.join(root,route+'-recovered.png')});const finished=Date.now();
    onProof({id:contract.id,fixture:contract.fixture,contractHash:evidence.jsonHash(contract),status:'passed',startedAt:new Date(started).toISOString(),
      finishedAt:new Date(finished).toISOString(),durationMs:finished-started,actions,checks});console.log('PASS '+contract.id);
  }}finally{servers.cleanup();}
}
module.exports={runLifecycle};
