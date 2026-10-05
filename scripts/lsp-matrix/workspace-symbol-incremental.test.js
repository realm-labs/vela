"use strict";
const test=require("node:test"), assert=require("node:assert/strict");
const spec=require("../../tests/lsp_matrix/fixtures/workspace-symbol-incremental.json");
const {parseMarkers}=require("./fixtures");

test("workspace incremental phases pin authored fingerprints, reverse dependencies and whole ownership",()=>{
  assert.equal(spec.id,"workspace-symbol-incremental");
  assert.deepEqual(Object.keys(spec.files),["scripts/api.vela","scripts/other.vela","scripts/main.vela","scripts/relay.vela","scripts/unrelated.vela"]);
  const phases=spec.oracle.phases;
  assert.deepEqual(phases.map(p=>[p.id,p.file??null,p.declarationChanged??null,p.importChanged??null,p.invalidated??null]),[
    ["initial",null,null,null,null],
    ["body_changes_receiver","scripts/main.vela",false,false,["main"]],
    ["body_restores_receiver","scripts/main.vela",false,false,["main"]],
    ["import_switches_owner","scripts/main.vela",false,true,["main","relay"]],
    ["declaration_changes_return","scripts/other.vela",true,false,["main","other","relay"]],
    ["missing_import_erases_members","scripts/main.vela",false,true,["main","relay"]],
    ["import_restores_owner","scripts/main.vela",false,true,["main","relay"]],
    ["removed_declaration_erases_members","scripts/api.vela",true,false,["api","main","relay"]],
    ["declaration_restores_owner","scripts/api.vela",true,false,["api","main","relay"]],
  ]);
  const ids=["api-module","api-file","api-first","api-second","api-make","main-module","main-file","main-main","other-module","other-file","other-fourth","other-third","other-make","relay-module","relay-file","relay-relay","unrelated-module","unrelated-file","unrelated-anchor"];
  for(const [index,phase]of phases.entries()) {
    const {symbols,queries}=phase.workspace;
    assert.equal(symbols.length,19);assert.equal(queries.length,17);
    assert.deepEqual(symbols.map(r=>r.id),ids);
    assert(symbols.every(r=>r.ownership==="Source"));
    assert.deepEqual(queries.slice(0,2).map(q=>q.symbols),[ids,ids]);
    assert.deepEqual(queries.slice(2,8).map(q=>q.symbols),[["api-module","api-file","api-first","api-second","api-make"],["main-module","main-file","main-main"],["api-make"],["api-make","other-make"],["api-first"],["other-fourth"]]);
    assert.deepEqual(queries[8].symbols,index===7?["api-make"]:[]);
    assert.deepEqual(queries.slice(9).map(q=>[q.query,q.symbols]),["choose","zz_","missing","value","Any","Function()","中😀","not_a_symbol_473"].map(q=>[q,[]]));
    const api=symbols.find(r=>r.id==="api-make"), other=symbols.find(r=>r.id==="other-make");
    assert.equal(api.name,index===7?"api::make_deleted":"api::make");
    assert.equal(api.identity,api.name);assert.equal(api.detail,"() -> First");
    assert.equal(other.detail,index>=4?"() -> Fourth":"() -> Third");
    assert.equal(symbols.some(r=>r.name.includes("choose")||r.name.includes("zz_")),false);
  }
});

test("workspace incremental markers keep complete current sources and Unicode columns across every edit",()=>{
  for(const crlf of [false,true]) for(const shifted of [false,true]) {
    const transform=source=>{
      if(shifted)source=source.replace("[[file:start]]","[[file:start]]// shifted 中😀\n/* extra 😀 */\n");
      return crlf?source.replaceAll("\n","\r\n"):source;
    };
    const docs=Object.fromEntries(Object.entries(spec.files).map(([f,s])=>[f,parseMarkers(transform(s))]));
    const unrelated=structuredClone(docs["scripts/unrelated.vela"]);
    for(const phase of spec.oracle.phases) {
      if(phase.file) docs[phase.file]=parseMarkers(transform(phase.source));
      const main=docs["scripts/main.vela"], name=main.markers["main-name"], bytes=Buffer.from(main.text);
      assert.deepEqual([name.start.line,name.start.character,name.end.character],[1+(shifted?2:0),15,19]);
      assert.equal(name.start.byte-bytes.lastIndexOf(10,name.start.byte-1)-1,19);
      assert.equal(bytes.subarray(name.start.byte,name.end.byte).toString(),"main");
      for(const row of phase.workspace.symbols) {
        const doc=docs[row.file], marker=doc.markers[row.range];assert(marker);
        const source=Buffer.from(doc.text).subarray(marker.start.byte,marker.end.byte).toString();
        if(row.kind==="File"||row.kind==="Module")assert.equal(source,doc.text);
        else {assert(source.startsWith("pub "));assert(source.includes(row.name.split("::").at(-1)));}
      }
      assert.deepEqual(docs["scripts/unrelated.vela"],unrelated);
    }
  }
});
