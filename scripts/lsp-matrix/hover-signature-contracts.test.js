"use strict";
const test = require("node:test"), assert = require("node:assert/strict");
const requirements = require("./inventory").loadInventory(require("node:path").resolve(__dirname,"../..")).executionRequirements;
const { hoverSignatureModel, hoverSignatureContracts } = require("./hover-signature-contracts");
const { localContracts } = require("./local-contracts");
const driver = require("../../tests/lsp_matrix/fixtures/input-driver.json");

test("UX10 covers every hover/signature route with independent Unicode and named/defaulted argument oracles", () => {
  const m = hoverSignatureModel(), contracts = hoverSignatureContracts(requirements);
  assert.deepEqual(contracts.flatMap(c=>c.requirements.map(r=>r.id)).sort(), requirements.filter(r=>r.id.startsWith("vscode/UX10/")).map(r=>r.id).sort());
  assert.equal(contracts.length,5);
  assert(m.disk.markers.target.start.byte > m.disk.markers.target.start.character);
  assert.equal(m.typed.both.text,m.typed.first.text);
  assert.equal(m.typed.first.markers.active.start.character,12);
  assert.equal(m.disk.markers["unknown-argument"].start.character-m.disk.markers.unknown.start.character,8);
  const signature=contracts.find(c=>c.id==="ux10-signature-arguments");
  assert.deepEqual(signature.checks.filter(c=>c.id.endsWith("-signature")).map(c=>c.expected.active),
    ["left: i64","right: i64 (defaulted)","left: i64","right: i64 (defaulted)","left: i64"]);
  assert.equal(contracts.find(c=>c.id==="ux10-unknown-receiver").checks.filter(c=>c.id.endsWith("-request")).length,2);
});
test("UX10 native hover chords and parameter hints follow registered Windows/macOS bindings", () => {
  for(const [platform,modifier] of [["win32","Control"],["darwin","Meta"]]) {
    const contracts=localContracts(requirements,driver,platform);
    const hover=contracts.find(c=>c.id==="ux10-keyboard-hover");
    assert.deepEqual(hover.actions.slice(0,2).map(a=>a.key),[modifier+"+k",modifier+"+i"]);
    const signature=contracts.find(c=>c.id==="ux10-signature-arguments");
    assert.equal(signature.actions.find(a=>a.id==="show-first").key,modifier+"+Shift+Space");
    assert.equal(signature.actions.find(a=>a.id==="first-home").key,platform==="win32"?"Home":"Meta+ArrowLeft");
    assert.equal(signature.actions.find(a=>a.id==="first-home").count,2);
    assert.equal(signature.actions.find(a=>a.id==="last-end").key,platform==="win32"?"End":"Meta+ArrowRight");
  }
});
