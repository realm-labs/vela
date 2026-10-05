"use strict";
const { parseMarkers } = require("./fixtures");

function document(source, crlf = false, shifted = false) {
  if (shifted) source = source.replace("[[file:start]]", "[[file:start]]// shifted 中😀\n/* second 😀 */\n");
  return parseMarkers(crlf ? source.replaceAll("\n", "\r\n") : source);
}
function point(doc, value, protocol = true) {
  const bytes = Buffer.from(doc.text);
  return { line: value.line, character: protocol ? value.character : value.byte === 0 ? 0 :
    value.byte - (bytes.lastIndexOf(10, value.byte - 1) + 1) };
}
function positions(doc, queries, protocol = true) {
  return queries.map(q => point(doc, doc.markers[q.position].start, protocol));
}
function expected(doc, queries, protocol = true) {
  return queries.map(q => {
    if (!q.chain.length) {
      const p = point(doc, doc.markers[q.position].start, protocol);
      return { range: { start: p, end: p } };
    }
    return q.chain.reduceRight((parent, name) => {
      const marker = doc.markers[name];
      const range = { start: point(doc, marker.start, protocol), end: point(doc, marker.end, protocol) };
      return parent ? { range, parent } : { range };
    }, null);
  });
}
module.exports = { document, point, positions, expected };
