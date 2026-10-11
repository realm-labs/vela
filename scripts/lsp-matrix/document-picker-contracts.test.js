"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { documentPickerModel, documentPickerContracts } = require("./document-picker-contracts");
const { localContracts } = require("./local-contracts");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname,"../..")).executionRequirements;
test("document picker preserves whole authored trees, immediate parent and UTF-16 destinations", () => {
  const m = documentPickerModel();
  assert.equal(Object.keys(m.files).length,22); assert.equal(m.cases.length,18);
  assert.equal(m.cases.reduce((n,c)=>n+c.nodes.length,0),158);
  assert.equal(m.cases.filter(c=>!c.nodes.length).length,2);
  assert(m.cases.every(c=>c.file.startsWith("scripts/document_picker_")));
  const c = m.cases[0], named = c.nodes.filter(n=>n.parent === "Named");
  assert.deepEqual(named.map(n=>[n.name,n.icon,n.parent]), [["key","symbol-field","Named"]]);
  assert.deepEqual(c.nodes[0].selection,{start:{line:1,character:10},end:{line:1,character:15}});
  assert.deepEqual(c.nodes.filter(n=>n.name==="read").map(n=>n.parent),["Readable","impl Readable for Widget"]);
  assert(m.cases.every(c=>c.nodes.every(n=>c.text.split(/\r?\n/)[n.selection.start.line].slice(n.selection.start.character,n.selection.end.character)===n.name)));
  assert.deepEqual(m.cases.slice(9).map(c=>c.nodes),m.cases.slice(0,9).map(c=>c.nodes));
  assert(m.cases.filter(c=>c.form==="crlf").every(c=>!/(?<!\r)\n/.test(c.text)));
});
test("document picker requires every physical selection and presentation, including empty and wrapped tails", () => {
  const [c] = documentPickerContracts(requirements);
  assert.deepEqual(c.requirements.map(r=>r.id),["vscode/UX11/document-picker/input/local","vscode/UX11/document-picker/render/local"]);
  for(const suffix of ["-row","-destination","-restored-row"])assert.equal(c.checks.filter(x=>x.id.endsWith(suffix) && (suffix!=="-row"||!x.id.endsWith("-restored-row"))).length,158);
  assert.equal(c.actions.filter(x=>/-\d+-accept$/.test(x.id)).length,158);
  assert.equal(c.actions.filter(x=>x.device==="pointer").length,40);
  assert.equal(c.checks.filter(x=>x.id.endsWith("-empty")).length,2);
  assert.equal(c.checks.filter(x=>x.id.endsWith("-no-match")).length,16);
  assert.equal(c.checks.filter(x=>x.id.endsWith("-request")).length,18);
  assert.equal(c.checks.filter(x=>x.id.endsWith("-wrapped-head")).length,16);
  assert.equal(c.checks.filter(x=>x.id.endsWith("-retained")).length,18);
  assert.equal(c.checks.filter(x=>x.id.endsWith("-final-source")).length,18);
  for(const list of [c.actions,c.checks])assert.equal(new Set(list.map(x=>x.id)).size,list.length);
  assert.equal(c.deadlineMs,240000);
});
test("document picker registers both native bindings and does not own workspace/no-match cells", () => {
  for(const [platform,modifier]of [["win32","Control"],["darwin","Meta"]]){
    const cs=localContracts(requirements,require("../../tests/lsp_matrix/fixtures/input-driver.json"),platform),c=cs.at(-1);
    assert.equal(cs.length,52);assert.equal(c.id,"ux11-document-picker");
    assert(c.actions.filter(a=>a.id.endsWith("-open")||a.id.endsWith("-reopen")).every(a=>a.key===modifier+"+Shift+o"));
    assert(c.requirements.every(r=>r.id.includes("/document-picker/")));
    assert(cs.some(c=>c.id==="ux11-outline"));
  }
});
