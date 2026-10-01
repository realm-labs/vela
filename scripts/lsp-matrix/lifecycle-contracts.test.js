"use strict";
const test = require("node:test"), assert = require("node:assert/strict"), path = require("node:path");
const { lifecycleModel, lifecycleContracts } = require("./lifecycle-contracts");
const { parseMarkers } = require("./fixtures");
const { localContracts } = require("./local-contracts");
const requirements = require("./inventory").loadInventory(path.resolve(__dirname, "../..")).executionRequirements;
test("UX18 covers both lifecycle routes at all three levels with authored shifted recoverable edits", () => {
  const m = lifecycleModel(), cs = lifecycleContracts(requirements), o = m.spec.oracle;
  assert.deepEqual(cs.flatMap(c=>c.requirements.map(r=>r.id)).sort(), requirements.filter(r=>r.scenario==='UX18').map(r=>r.id).sort());
  assert.equal(cs.length,2);
  const marked = parseMarkers(o.dirtyPrefix + m.spec.files[o.file]);
  assert.deepEqual({line:marked.markers.call.start.line,character:marked.markers.call.start.character},o.call);
  assert.deepEqual({start:{line:marked.markers.definition.start.line,character:marked.markers.definition.start.character},
    end:{line:marked.markers.definition.end.line,character:marked.markers.definition.end.character}},o.definition);
  assert(cs.every(c=>c.checks.some(check=>check.level==='Render') && c.checks.some(check=>check.id==='recovered-wire')));
  assert(cs[0].actions.findIndex(a=>a.id==='suspend-owned') < cs[0].actions.findIndex(a=>a.id==='pending-query'));
  assert(cs[0].actions.findIndex(a=>a.id==='pending-query') < cs[0].actions.findIndex(a=>a.id==='stop-owned'));
  assert(cs.every(c=>c.checks.find(check=>check.id==='bounded-failure').expected.within5000Ms));
});
test("UX18 uses native registered reload/output palettes and independent stopped policy", () => {
  for (const [platform,modifier] of [['win32','Control'],['darwin','Meta']]) {
    const cs=localContracts(requirements,require('../../tests/lsp_matrix/fixtures/input-driver.json'),platform).filter(c=>c.id.startsWith('ux18-'));
    assert.equal(cs.length,2);
    for(const c of cs){
      assert.equal(c.actions.find(a=>a.id==='recover-open').key,modifier+'+Shift+P');
      assert.equal(c.actions.find(a=>a.id==='recover-editor').key,modifier+'+1');
      assert.deepEqual(c.checks.find(check=>check.id==='stopped-policy').expected,{servers:[0,0,0],starts:[1,1,1],atLeast1000Ms:true});
      assert.equal(c.actions.find(a=>a.id==='recover-name').text,'Developer: Reload Window');
    }
  }
});
