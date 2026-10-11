"use strict";
const assert=require("node:assert/strict");
// Backend's declared ordering is name then kind_name. Equal name/kind entries
// inherit opaque document-ID spelling; native Windows opens can change it.
// Canonicalize ONLY contiguous equal-key ties, keeping full rows, multiplicity
// and every non-tie boundary. Raw pairs and their original order are retained.
function orderedWorkspaceTies(rows){
  assert(Array.isArray(rows),"explicit workspace response array");const groups=[];
  for(const row of rows){
    const previous=groups.at(-1);
    if(previous&&previous[0].name===row.name&&previous[0].kind===row.kind)previous.push(row);else groups.push([row]);
  }
  const key=r=>JSON.stringify([r.location.uri,r.location.range??null,r.containerName??null,r.data??null]);
  return groups.flatMap(g=>g.toSorted((a,b)=>{const x=key(a),y=key(b);return x<y?-1:x>y?1:0;}));
}
module.exports={orderedWorkspaceTies};
