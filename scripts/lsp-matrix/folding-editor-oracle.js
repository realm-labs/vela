"use strict";
const { parseMarkers } = require("./fixtures");

function document(source, crlf = false, shifted = false) {
  const prefix = "// shifted 中😀\n/* second 😀 */\n";
  if (shifted) {
    const newline = source.indexOf("\n");
    source = source.startsWith("#!") ? source.slice(0, newline + 1) + prefix + source.slice(newline + 1) : prefix + source;
  }
  return parseMarkers(crlf ? source.replaceAll("\n", "\r\n") : source);
}

function wire(doc, ranges) {
  return ranges.map(row => {
    const { start, end } = doc.markers[row.range];
    return { startLine: start.line, startCharacter: start.character,
      endLine: end.line, endCharacter: end.character, kind: row.kind };
  });
}

// The public command computes VS Code's folding model: stable start-line
// ordering and the first entry at a shared start line. Wire ranges stay intact.
function editor(doc, ranges) {
  const seen = new Set();
  return wire(doc, ranges).sort((a, b) => a.startLine - b.startLine)
    .filter(row => {
      if (seen.has(row.startLine)) return false;
      seen.add(row.startLine); return true;
    }).map(row => ({ start: row.startLine, end: row.endLine, kind: row.kind }));
}

function publicRanges(ranges, kinds) {
  return ranges.map(row => {
    const kind = row.kind === kinds.Imports ? "imports" : row.kind === kinds.Region ? "region" : undefined;
    if (!kind) throw Error(`unexpected public folding kind ${row.kind}`);
    return { start: row.start, end: row.end, kind };
  });
}
module.exports = { document, wire, editor, publicRanges };
