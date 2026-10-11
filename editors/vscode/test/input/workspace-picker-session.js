"use strict";
const fs=require("node:fs"),path=require("node:path"),assert=require("node:assert/strict");
const {spawn}=require("node:child_process"),{chromium}=require("playwright-core");
const evidence=require("../../../../scripts/lsp-matrix/local-evidence");
const {safeFile}=require("../../../../scripts/lsp-matrix/fixtures");
const {readSession}=require("./session"),{windowsDesktop}=require("./native-menu");
const {relativeFile}=require("./paths");

// Separate sequential normal windows keep the complete original module names,
// source/schema collisions and genuinely unopened source targets. No providers
// or accepted actions run through the observer bridge.
async function pickerWindow({root,profile,executable,extensions,installedServer,until,record,c,runCase}){
  const owned=path.join(root,"picker-"+c.id),workspace=path.join(owned,"中文 % picker workspace");
  fs.mkdirSync(workspace,{recursive:true});
  for(const [f,t]of Object.entries(c.files)){
    const target=path.join(workspace,safeFile(f));assert(!fs.existsSync(target));
    fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,t);
  }
  const userData=path.join(owned,"user-data");require("./profile-settings").writeProfileSettings(workspace,userData,profile);
  const settingsFile=path.join(workspace,".vscode/settings.json"),settings=JSON.parse(fs.readFileSync(settingsFile,"utf8"));
  settings["vela.workspace.roots"]=[path.join(workspace,"scripts")];settings["vela.host.schema"]=c.schema ? path.join(workspace,c.schema) : "";
  fs.writeFileSync(settingsFile,JSON.stringify(settings,null,2));assert(!fs.existsSync(path.join(workspace,"vela.toml")));
  const args=[workspace,"--extensions-dir",extensions,"--user-data-dir",userData,"--shared-data-dir",path.join(owned,"shared-data"),
    "--remote-debugging-port=0","--remote-debugging-address=127.0.0.1","--locale=en","--skip-welcome","--skip-release-notes",
    "--disable-workspace-trust","--disable-updates","--disable-gpu","--no-sandbox"];
  fs.writeFileSync(path.join(owned,"setup.json"),JSON.stringify({args,settings,profile,serverSha256:evidence.fileHash(path.join(root,installedServer))},null,2));
  const env={...process.env,VELA_TEST_RESULT_DIR:owned,VELA_TEST_EXTENSIONS_DIR:extensions,VELA_TEST_INPUT_DRIVER:"1",
    VELA_TEST_INPUT_BRIDGE:path.join(__dirname,"bridge.js"),VELA_TEST_WORKSPACE_BASE:owned};delete env.ELECTRON_RUN_AS_NODE;
  const log=fs.createWriteStream(path.join(owned,"workbench.log")),child=spawn(executable,args,{env,stdio:["ignore","pipe","pipe"]});
  let cdp,exit,closed=false,childError,browser,page,keyboardState,failure;
  for(const stream of [child.stdout,child.stderr]){
    let pending="";stream.on("data",chunk=>{log.write(chunk);pending=(pending+chunk).slice(-16384);
      const match=pending.match(/DevTools listening on (ws:\/\/127\.0\.0\.1:\d+\/[^\r\n]+)[\r\n]/);if(match)cdp=match[1];});
  }
  child.on("error",e=>{childError=e;});const exited=new Promise(resolve=>child.once("exit",(code,signal)=>{exit={code,signal};resolve(exit);}));
  child.once("close",()=>{closed=true;});
  const alive=()=>{if(childError)throw childError;if(exit)throw Error("picker workbench exited "+JSON.stringify(exit));};
  const windowBudgetMs=c.windowBudgetMs??120000;
  assert([120000,480000].includes(windowBudgetMs),"registered finite private-window budget");
  const timer=setTimeout(()=>child.kill("SIGTERM"),windowBudgetMs);
  try{
    await until("picker debug endpoint",()=>{alive();return cdp;},30000);
    browser=await chromium.connectOverCDP(cdp,{timeout:15000});
    page=await until("picker normal window",()=>{alive();return browser.contexts()[0].pages().find(p=>p.url().includes("workbench"));});
    page.setDefaultTimeout(15000);await page.bringToFront();
    await until("picker installed observer",()=>{alive();return fs.existsSync(path.join(owned,"bridge.json"));});
    const bridge=async(op,params={})=>{
      alive();const session=readSession(owned),response=await fetch(`http://127.0.0.1:${session.port}`,{method:"POST",
        headers:{authorization:`Bearer ${session.token}`},body:JSON.stringify({op,...params}),signal:AbortSignal.timeout(10000)});
      const body=await response.json();if(!response.ok)throw Error(body.error);return body.value;
    };
    if(profile.platform==="win32")keyboardState=windowsDesktop("select",child.pid,profile.keyboardLayout);
    const before=await bridge("inspect");
    assert.equal(before.extensionActive,false);assert.deepEqual(before.settings,profile.settings);assert.equal(before.vscodeVersion,profile.vscodeVersion);
    assert.equal(before.locale,profile.locale);assert.equal(before.platform,profile.platform);assert.equal(before.arch,profile.arch);assert.equal(before.serverPath,"");
    assert.equal(path.resolve(before.extensionPath),path.resolve(path.dirname(path.dirname(path.join(root,installedServer)))));
    assert.equal(evidence.fileHash(path.join(before.extensionPath,"server",profile.platform==="win32"?"vela_lsp_server.exe":"vela_lsp_server")),evidence.fileHash(path.join(root,installedServer)));
    const display=await page.evaluate(()=>({width:innerWidth,height:innerHeight,deviceScaleFactor:devicePixelRatio,screenWidth:screen.width,screenHeight:screen.height}));
    assert.deepEqual(display,profile.display);
    const roots=await bridge("workspace-roots-inspect");assert.deepEqual(roots.folders,[path.basename(workspace)]);
    const profileObservation={platform:profile.platform,case:c.id,version:before.vscodeVersion,locale:before.locale,settings:true,display:true,
      installedServer:true,normalWindow:true,roots:roots.settingsRoots.map(f=>relativeFile(workspace,f))};
    record("observation","picker-profile-"+c.id,{observed:profileObservation,display,settings:before.settings,extensionPath:before.extensionPath});
    await runCase({page,bridge,workspace,owned,profileObservation});
    if(keyboardState){windowsDesktop("restore",child.pid,keyboardState.previous,keyboardState.window);keyboardState=undefined;}
    await bridge("finish");const result=await until("picker clean exit",()=>{require("./observer-health").assertObserverHealthy(owned);return exit;},15000);
    require("./observer-health").assertObserverHealthy(owned);assert.deepEqual(result,{code:0,signal:null});
    record("assertion","picker-clean-exit-"+c.id,{observed:result});
  }catch(error){
    failure=error;if(page){await page.screenshot({path:path.join(owned,"failure.png")}).catch(()=>{});
      fs.writeFileSync(path.join(owned,"failure.aria.txt"),await page.locator("body").ariaSnapshot().catch(()=>"unavailable"));}
  }finally{
    clearTimeout(timer);
    if(keyboardState){try{windowsDesktop("restore",child.pid,keyboardState.previous,keyboardState.window);}catch(e){failure??=e;}}
    if(!exit){child.kill("SIGTERM");await Promise.race([exited,new Promise(resolve=>setTimeout(resolve,3000))]);if(!exit)child.kill("SIGKILL");}
    if(browser)await browser.close().catch(()=>{});
    try{await until("picker streams closed",()=>closed,5000);}catch(e){failure??=e;child.stdout.destroy();child.stderr.destroy();}
    await new Promise(resolve=>log.end(resolve));
    if(fs.existsSync(path.join(owned,"bridge.json"))){try{require("./logs").retainLogs(owned,workspace);}catch(e){failure??=e;}}
  }
  if(failure)throw failure;
  return exit;
}
module.exports={pickerWindow};
