"use strict";
const test=require("node:test"),assert=require("node:assert/strict");
const spec=require("../../tests/lsp_matrix/fixtures/workspace-symbol-sources.json");
const {parseMarkers}=require("./fixtures");

test("workspace source phases pin configured roots, literal complete ownership and genuine rename",()=>{
  const {workspace,scratch}=spec.oracle.sequences;
  assert.deepEqual(workspace.map(p=>p.id),["initial","open-main","dirty-main","hidden-disk","close-new-disk","delete-api","recreate-api","renamed-owner","add-shared-root","remove-scripts-root","open-missing-scratch","dirty-missing-scratch","close-missing-scratch","restore-roots"]);
  assert.deepEqual(workspace.map(p=>p.workspace.symbols.length),[6,6,6,6,6,3,6,6,9,3,6,6,3,9]);
  assert.deepEqual(scratch.map(p=>p.id),["cold","open","dirty","close","reopen"]);
  assert.deepEqual(scratch.map(p=>p.workspace.symbols.length),[0,3,3,0,3]);
  assert.deepEqual(workspace[7].actions,[{op:"rename",file:"scripts/api.vela",to:"scripts/renamed.vela"}]);
  assert.deepEqual(workspace[8].roots,["scripts","shared"]);
  assert.deepEqual(workspace[9].roots,["shared"]);
  assert.deepEqual(workspace[13].roots,["scripts","shared"]);
  assert.deepEqual(workspace[3].open,{"scripts/main.vela":"dirty"});
  assert.equal(workspace[3].disk["scripts/main.vela"],"disk");
  assert.deepEqual(workspace[4].open,{});
  for(const phases of [workspace,scratch])for(const phase of phases) {
    const {symbols,queries}=phase.workspace;
    assert.equal(queries.length,14);
    assert.deepEqual(queries.slice(0,2).map(q=>q.symbols),[symbols.map(r=>r.id),symbols.map(r=>r.id)]);
    assert(symbols.every(r=>r.ownership==="Source"&&r.file!=="outside/secret.vela"));
    assert.equal(new Set(symbols.map(r=>r.id)).size,symbols.length);
    assert.deepEqual(symbols.map(r=>r.name),symbols.map(r=>r.name).toSorted());
    assert.deepEqual(queries.slice(-3).map(q=>[q.query,q.symbols]),[["choose",[]],["secret",[]],["not_a_symbol_911",[]]]);
    assert.equal(Object.hasOwn(phase.disk,"outside/scratch.vela"),false);
    const local=symbols.find(r=>r.id==="outside-scratch-decl");
    if(local){assert.equal(local.container,phases===scratch?"main":"scratch_0");assert.equal(local.identity,local.name);}
    const scratchModule=symbols.find(r=>r.id==="outside-scratch-module");
    if(scratchModule)assert.equal(scratchModule.identity,"dev.vela.scratch::"+(phases===scratch?"main":"scratch_0"));
    if(phase.id==="renamed-owner") {
      assert.equal(symbols.some(r=>r.file==="scripts/api.vela"),false);
      assert.deepEqual(symbols.filter(r=>r.file==="scripts/renamed.vela").map(r=>r.name),["renamed","renamed.vela","renamed::alpha"]);
      assert.deepEqual(queries.find(q=>q.query==="api").symbols,[]);
    }
  }
});

test("workspace source variants keep exact Unicode markers, full file extents and separate overlay bytes",()=>{
  for(const crlf of[false,true])for(const shifted of[false,true]) {
    const transform=source=>{
      if(shifted)source=source.replace("[[file:start]]","[[file:start]]// shifted 中😀\n/* extra 😀 */\n");
      return crlf?source.replaceAll("\n","\r\n"):source;
    };
    const variants=Object.fromEntries(Object.entries(spec.oracle.variants).map(([id,s])=>[id,parseMarkers(transform(s))]));
    for(const[id,doc]of Object.entries(variants)) {
      const name=doc.markers["decl-name"],bytes=Buffer.from(doc.text),constant=id==="helper",main=["main","dirty","disk"].includes(id);
      assert.deepEqual([name.start.line,name.start.character],[Number(main)+(shifted?2:0),constant?18:15]);
      assert.equal(name.start.byte-bytes.lastIndexOf(10,name.start.byte-1)-1,constant?22:19);
      assert.deepEqual([doc.markers.file.start.byte,doc.markers.file.end.byte],[0,bytes.length]);
    }
    for(const phases of Object.values(spec.oracle.sequences))for(const phase of phases) {
      const docs=Object.fromEntries(Object.entries({...phase.disk,...phase.open}).map(([file,id])=>[file,variants[id]]));
      for(const row of phase.workspace.symbols) {
        const doc=docs[row.file],range=doc.markers[row.range];assert(range);
        const text=Buffer.from(doc.text).subarray(range.start.byte,range.end.byte).toString();
        if(row.kind==="Module"||row.kind==="File")assert.equal(text,doc.text);
        else{assert(text.startsWith("pub "));assert(text.includes(row.name.split("::").at(-1)));}
      }
      if(phase.id==="hidden-disk")assert.notEqual(variants[phase.disk["scripts/main.vela"]].text,variants[phase.open["scripts/main.vela"]].text);
    }
  }
});
