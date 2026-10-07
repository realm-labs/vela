"use strict";
const assert = require("node:assert/strict"), base = require("./selection-oracle");

// The marker DSL cannot place an end marker directly after a literal '['.
// Author its start point and its one ASCII byte explicitly; never read CSTs.
function expected(doc, queries, protocol = true) {
  return base.expected(doc, queries, protocol).map((parent, index) => {
    const query = queries[index];
    if (!Object.hasOwn(query, "token")) return parent;
    assert.equal(query.token, "[");
    assert(query.chain.length, "bracket must have independently marked parents");
    const marker = doc.markers[query.position].start;
    assert.equal(Buffer.from(doc.text)[marker.byte], 91, "authored point must select '['");
    const start = base.point(doc, marker, protocol);
    return { range: { start, end: { line: start.line, character: start.character + 1 } }, parent };
  });
}
module.exports = { ...base, expected };
